pub mod kind;
pub mod store;

use std::collections::HashMap;
use std::path::PathBuf;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post, put};
use http_body_util::BodyExt;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::admin::dir_size;
use crate::api::COMPANY_PREFIX;
use crate::boards::Board;
use crate::cards::card_in_board;
use crate::error::{ApiError, bad};
use crate::events::{self, At};
use crate::projects::{Act, path_id};
use crate::state::AppState;
use crate::tenant::Company;

const MB: u64 = 1024 * 1024;
const LINK_MAX: usize = 2000;
pub const STORED: &str = "SELECT EXISTS (SELECT 1 FROM attachments WHERE hash = ?1) OR EXISTS (SELECT 1 FROM board_images WHERE hash = ?1)";

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}/boards/{{board}}{s}");
    Router::new()
        .route(&format!("{COMPANY_PREFIX}/storage"), get(storage))
        .route(&p("/cards/{card}/attachments"), post(upload))
        .route(&p("/cards/{card}/attachments/link"), post(add_link))
        .route(
            &p("/attachments/{attachment}"),
            get(download).delete(remove),
        )
        .route(
            &p("/attachments/{attachment}/cover"),
            put(set_cover).delete(clear_cover),
        )
}

type Params = Path<HashMap<String, String>>;

pub fn org_dir(state: &AppState, slug: &str) -> PathBuf {
    state.dbs.layout().orgs.join(slug)
}

pub fn max_bytes(state: &AppState) -> u64 {
    u64::from(state.settings.max_upload_mb) * MB
}

pub fn size_text(n: u64) -> String {
    if n >= 1024 * MB {
        format!("{:.1} GB", n as f64 / (1024 * MB) as f64)
    } else if n >= MB {
        format!("{:.1} MB", n as f64 / MB as f64)
    } else if n >= 1024 {
        format!("{} KB", n / 1024)
    } else {
        format!("{n} B")
    }
}

pub async fn storage_limit(state: &AppState, org: i64) -> Result<Option<u64>, ApiError> {
    let limit: Option<i64> = state
        .dbs
        .control()
        .read(move |c| {
            Ok(
                c.query_row("SELECT storage_limit FROM orgs WHERE id = ?1", [org], |r| {
                    r.get(0)
                })?,
            )
        })
        .await?;
    Ok(limit.map(|l| l.max(0) as u64))
}

async fn storage(State(state): State<AppState>, co: Company) -> Result<Json<Value>, ApiError> {
    let limit = storage_limit(&state, co.org_id).await?;
    let dir = org_dir(&state, &co.slug);
    let used = tokio::task::spawn_blocking(move || dir_size(&dir))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!({
        "max_file": max_bytes(&state),
        "used": used,
        "limit": limit,
    })))
}

pub fn attachment_json(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let kind: String = r.get(1)?;
    let mime: String = r.get(5)?;
    Ok(json!({
        "id": r.get::<_, i64>(0)?,
        "kind": kind,
        "name": r.get::<_, String>(2)?,
        "size": r.get::<_, i64>(3)?,
        "url": r.get::<_, Option<String>>(4)?,
        "image": kind == "file" && mime.starts_with("image/"),
        "type": mime,
        "created_at": r.get::<_, i64>(6)?,
    }))
}

const COLUMNS: &str = "id, kind, name, size, url, mime, created_at";

pub fn attachments_of(c: &Connection, card: i64) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = c.prepare(&format!(
        "SELECT {COLUMNS} FROM attachments WHERE card_id = ?1 ORDER BY id"
    ))?;
    stmt.query_map([card], attachment_json)?.collect()
}

#[derive(Deserialize)]
struct UploadQuery {
    name: Option<String>,
}

pub fn too_large(max: u64) -> ApiError {
    ApiError::TooLarge(format!(
        "files can be up to {}; this one is larger",
        size_text(max)
    ))
}

pub async fn read_limited(body: Body, max: u64) -> Result<Vec<u8>, ApiError> {
    let mut body = body;
    let mut out = Vec::new();
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| bad("the upload was interrupted; try again"))?;
        if let Ok(data) = frame.into_data() {
            if (out.len() + data.len()) as u64 > max {
                return Err(too_large(max));
            }

            out.extend_from_slice(&data);
        }
    }
    Ok(out)
}

async fn upload(
    State(state): State<AppState>,
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Query(q): Query<UploadQuery>,
    body: Body,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let card = path_id(&params, "card")?;
    let max = max_bytes(&state);
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|n| n > max) {
        return Err(too_large(max));
    }

    let bytes = read_limited(body, max).await?;
    if bytes.is_empty() {
        return Err(bad("this file is empty"));
    }

    let name = kind::clean_name(q.name.as_deref().unwrap_or(""));
    let found = kind::check(&name, &bytes).map_err(ApiError::Unsupported)?;
    let limit = storage_limit(&state, board.project.company.org_id).await?;
    let dir = org_dir(&state, &board.project.company.slug);
    let (id, pid) = (board.id, board.project.id);
    let size = bytes.len() as i64;
    let mime = found.mime.to_string();
    let (hash, bytes) = tokio::task::spawn_blocking(move || (store::hash(&bytes), bytes))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let saved = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if card_in_board(&tx, id, card, false)?.is_none() {
                return Ok(Err(ApiError::NotFound));
            }

            let stored: bool = tx.query_row(
                STORED,
                [&hash],
                |r| r.get(0),
            )?;
            if !stored && let Some(limit) = limit {
                let used = dir_size(&dir);
                if used + bytes.len() as u64 > limit {
                    return Ok(Err(ApiError::TooLarge(format!(
                        "this company has used {} of its {} storage; delete old files or ask an admin for more space",
                        size_text(used),
                        size_text(limit)
                    ))));
                }
            }

            let written = match store::put(&dir, &hash, &bytes) {
                Ok(w) => w,
                Err(e) => return Ok(Err(ApiError::Internal(e.to_string()))),
            };
            let result = (|| -> rusqlite::Result<Value> {
                tx.execute(
                    "INSERT INTO attachments (card_id, kind, name, hash, size, mime, created_by)
                     VALUES (?1, 'file', ?2, ?3, ?4, ?5, ?6)",
                    params![card, name, hash, size, mime, by],
                )?;
                let attachment = tx.last_insert_rowid();
                events::record(
                    &tx,
                    by,
                    "attachment_added",
                    At::card(pid, id, card),
                    json!({ "attachment": attachment, "name": name }),
                )?;
                let v = tx.query_row(
                    &format!("SELECT {COLUMNS} FROM attachments WHERE id = ?1"),
                    [attachment],
                    attachment_json,
                )?;
                Ok(v)
            })();
            let done = result.and_then(|v| tx.commit().map(|_| v));
            match done {
                Ok(v) => Ok(Ok(v)),
                Err(e) => {
                    if written {
                        let _ = store::remove(&dir, &hash);
                    }
                    Err(e.into())
                }
            }
        })
        .await??;
    Ok((StatusCode::CREATED, Json(saved)))
}

#[derive(Deserialize)]
struct LinkBody {
    url: String,
    name: Option<String>,
}

fn link_url(raw: &str) -> Result<String, ApiError> {
    let u = raw.trim();
    let lower = u.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"));
    match rest {
        Some(r)
            if !r.is_empty()
                && u.len() <= LINK_MAX
                && !u.chars().any(|c| c.is_whitespace() || c.is_control()) =>
        {
            Ok(u.to_string())
        }
        _ => Err(bad("enter a link that starts with https://")),
    }
}

async fn add_link(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<LinkBody>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let card = path_id(&params, "card")?;
    let url = link_url(&body.url)?;
    let name = match body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        Some(n) if n.chars().count() <= 200 && !n.chars().any(char::is_control) => n.to_string(),
        Some(_) => return Err(bad("the link name is too long")),
        None => url.clone(),
    };
    let (id, pid) = (board.id, board.project.id);
    let saved = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if card_in_board(&tx, id, card, false)?.is_none() {
                return Ok(Err(ApiError::NotFound));
            }

            tx.execute(
                "INSERT INTO attachments (card_id, kind, name, url, created_by) VALUES (?1, 'link', ?2, ?3, ?4)",
                params![card, name, url, by],
            )?;
            let attachment = tx.last_insert_rowid();
            events::record(
                &tx,
                by,
                "attachment_added",
                At::card(pid, id, card),
                json!({ "attachment": attachment, "name": name, "link": true }),
            )?;
            let v = tx.query_row(
                &format!("SELECT {COLUMNS} FROM attachments WHERE id = ?1"),
                [attachment],
                attachment_json,
            )?;
            tx.commit()?;
            Ok(Ok(v))
        })
        .await??;
    Ok((StatusCode::CREATED, Json(saved)))
}

struct Found {
    card: i64,
    kind: String,
    name: String,
    hash: Option<String>,
    mime: String,
    archived: bool,
}

fn find(c: &Connection, board: i64, attachment: i64) -> rusqlite::Result<Option<Found>> {
    c.query_row(
        "SELECT a.card_id, a.kind, a.name, a.hash, a.mime, k.archived_at IS NOT NULL
         FROM attachments a JOIN cards k ON k.id = a.card_id
         WHERE a.id = ?1 AND k.board_id = ?2",
        params![attachment, board],
        |r| {
            Ok(Found {
                card: r.get(0)?,
                kind: r.get(1)?,
                name: r.get(2)?,
                hash: r.get(3)?,
                mime: r.get(4)?,
                archived: r.get(5)?,
            })
        },
    )
    .optional()
}

#[derive(Deserialize)]
struct DownloadQuery {
    download: Option<String>,
}

pub fn disposition(inline: bool, name: &str) -> String {
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ' | '(' | ')') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded: String = name
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!(
        "{}; filename=\"{ascii}\"; filename*=UTF-8''{encoded}",
        if inline { "inline" } else { "attachment" }
    )
}

async fn download(
    State(state): State<AppState>,
    board: Board,
    Path(params): Params,
    Query(q): Query<DownloadQuery>,
) -> Result<Response, ApiError> {
    let attachment = path_id(&params, "attachment")?;
    let id = board.id;
    let found = board
        .project
        .db()
        .read(move |c| Ok(find(c, id, attachment)?))
        .await?
        .ok_or(ApiError::NotFound)?;
    let Some(hash) = found.hash.filter(|_| found.kind == "file") else {
        return Err(ApiError::NotFound);
    };
    let dir = org_dir(&state, &board.project.company.slug);
    let path = store::path_of(&dir, &hash).ok_or(ApiError::NotFound)?;
    let bytes = tokio::task::spawn_blocking(move || std::fs::read(path))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map_err(|_| ApiError::NotFound)?;
    let image = found.mime.starts_with("image/");
    let inline = image && q.download.is_none();
    Ok(file_response(bytes, &found.mime, &found.name, inline))
}

pub fn file_response(bytes: Vec<u8>, mime: &str, name: &str, inline: bool) -> Response {
    let mime =
        HeaderValue::from_str(mime).unwrap_or(HeaderValue::from_static("application/octet-stream"));
    let mut res = (StatusCode::OK, bytes).into_response();
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, mime);
    h.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition(inline, name))
            .unwrap_or(HeaderValue::from_static("attachment")),
    );
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    h.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=300"),
    );
    res
}

async fn remove(
    State(state): State<AppState>,
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let attachment = path_id(&params, "attachment")?;
    let dir = org_dir(&state, &board.project.company.slug);
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(found) = find(&tx, id, attachment)? else {
                return Ok(Err(ApiError::NotFound));
            };
            if found.archived {
                return Ok(Err(bad("restore the card first")));
            }

            tx.execute("DELETE FROM attachments WHERE id = ?1", [attachment])?;
            events::record(
                &tx,
                by,
                "attachment_removed",
                At::card(pid, id, found.card),
                json!({ "attachment": attachment, "name": found.name }),
            )?;
            let orphan = match &found.hash {
                Some(h) => !tx.query_row(STORED, [h], |r| r.get::<_, bool>(0))?,
                None => false,
            };
            tx.commit()?;
            if orphan && let Some(h) = &found.hash {
                let _ = store::remove(&dir, h);
            }

            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

async fn cover(
    board: Board,
    headers: HeaderMap,
    params: Params,
    on: bool,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let attachment = path_id(&params, "attachment")?;
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(found) = find(&tx, id, attachment)? else {
                return Ok(Err(ApiError::NotFound));
            };
            if found.archived {
                return Ok(Err(bad("restore the card first")));
            }

            if on && !(found.kind == "file" && found.mime.starts_with("image/")) {
                return Ok(Err(bad("only images can be a cover")));
            }

            if on {
                tx.execute(
                    "UPDATE cards SET cover_attachment = ?2, cover = NULL, updated_at = unixepoch() WHERE id = ?1",
                    params![found.card, attachment],
                )?;
            } else {
                tx.execute(
                    "UPDATE cards SET cover_attachment = NULL, updated_at = unixepoch() WHERE id = ?1 AND cover_attachment = ?2",
                    params![found.card, attachment],
                )?;
            }
            events::record(
                &tx,
                by,
                "card_updated",
                At::card(pid, id, found.card),
                json!({ "cover": if on { json!({ "attachment": attachment }) } else { Value::Null } }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_cover(
    board: Board,
    headers: HeaderMap,
    params: Params,
) -> Result<StatusCode, ApiError> {
    cover(board, headers, params, true).await
}

async fn clear_cover(
    board: Board,
    headers: HeaderMap,
    params: Params,
) -> Result<StatusCode, ApiError> {
    cover(board, headers, params, false).await
}

pub async fn sweep(db: &crate::db::Database, org_dir: PathBuf) -> Result<usize, ApiError> {
    Ok(db
        .write(move |c| {
            let mut stmt =
                c.prepare("SELECT hash FROM attachments WHERE hash IS NOT NULL UNION SELECT hash FROM board_images")?;
            let kept: std::collections::HashSet<String> = stmt
                .query_map([], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            let mut removed = 0;
            let Ok(folders) = std::fs::read_dir(store::files_dir(&org_dir)) else {
                return Ok(0);
            };
            for folder in folders.filter_map(Result::ok) {
                let Ok(files) = std::fs::read_dir(folder.path()) else {
                    continue;
                };
                for f in files.filter_map(Result::ok) {
                    let name = f.file_name().to_string_lossy().into_owned();
                    if store::valid_hash(&name)
                        && !kept.contains(&name)
                        && store::remove(&org_dir, &name).is_ok()
                    {
                        removed += 1;
                    }
                }
            }
            Ok(removed)
        })
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_names_are_quoted_safely() {
        assert_eq!(
            disposition(false, "Bản vẽ \"x\".pdf"),
            "attachment; filename=\"B_n v_ _x_.pdf\"; filename*=UTF-8''B%E1%BA%A3n%20v%E1%BA%BD%20%22x%22.pdf"
        );
        assert!(disposition(true, "a.png").starts_with("inline;"));
    }

    #[test]
    fn sizes_read_like_people_write_them() {
        assert_eq!(size_text(10 * MB), "10.0 MB");
        assert_eq!(size_text(5 * 1024 * MB), "5.0 GB");
        assert_eq!(size_text(2048), "2 KB");
    }
}
