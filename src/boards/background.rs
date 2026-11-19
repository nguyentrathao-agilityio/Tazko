use std::collections::HashMap;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, header};
use axum::response::{Json, Response};
use axum::routing::{get, post};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use super::Board;
use crate::admin::dir_size;
use crate::api::COMPANY_PREFIX;
use crate::error::ApiError;
use crate::events;
use crate::files::{self, kind, store};
use crate::projects::{Act, path_id};
use crate::state::AppState;

pub const MAX_BYTES: u64 = 5 * 1024 * 1024;

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}/boards/{{board}}{s}");
    Router::new()
        .route(&p("/background"), post(upload))
        .route(&p("/background/{image}"), get(picture))
}

pub fn image_id(background: &str) -> Option<i64> {
    background
        .strip_prefix("img:")
        .and_then(|n| n.parse::<i64>().ok())
        .filter(|n| *n > 0)
}

#[derive(Deserialize)]
struct UploadQuery {
    name: Option<String>,
}

fn too_large() -> ApiError {
    ApiError::TooLarge(format!(
        "background pictures can be up to {}; this one is larger",
        files::size_text(MAX_BYTES)
    ))
}

async fn upload(
    State(state): State<AppState>,
    board: Board,
    headers: HeaderMap,
    Query(q): Query<UploadQuery>,
    body: Body,
) -> Result<Json<Value>, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|n| n > MAX_BYTES) {
        return Err(too_large());
    }

    let bytes = files::read_limited(body, MAX_BYTES)
        .await
        .map_err(|e| match e {
            ApiError::TooLarge(_) => too_large(),
            other => other,
        })?;
    if bytes.is_empty() {
        return Err(ApiError::BadRequest("this file is empty".into()));
    }

    let name = kind::clean_name(q.name.as_deref().unwrap_or(""));
    let found = kind::check(&name, &bytes).map_err(ApiError::Unsupported)?;
    if !found.image {
        return Err(ApiError::Unsupported(
            "choose a PNG, JPEG, GIF or WebP picture for the background".into(),
        ));
    }

    let limit = files::storage_limit(&state, board.project.company.org_id).await?;
    let dir = files::org_dir(&state, &board.project.company.slug);
    let (id, at) = (board.id, board.at());
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
            let stored: bool = tx.query_row(files::STORED, [&hash], |r| r.get(0))?;
            if !stored && let Some(limit) = limit {
                let used = dir_size(&dir);
                if used + bytes.len() as u64 > limit {
                    return Ok(Err(ApiError::TooLarge(format!(
                        "this company has used {} of its {} storage; delete old files or ask an admin for more space",
                        files::size_text(used),
                        files::size_text(limit)
                    ))));
                }
            }

            let written = match store::put(&dir, &hash, &bytes) {
                Ok(w) => w,
                Err(e) => return Ok(Err(ApiError::Internal(e.to_string()))),
            };
            let result = (|| -> rusqlite::Result<(String, Vec<String>)> {
                let mut stmt = tx.prepare("SELECT hash FROM board_images WHERE board_id = ?1")?;
                let old: Vec<String> = stmt
                    .query_map([id], |r| r.get(0))?
                    .collect::<Result<_, _>>()?;
                drop(stmt);
                tx.execute("DELETE FROM board_images WHERE board_id = ?1", [id])?;
                tx.execute(
                    "INSERT INTO board_images (board_id, hash, mime, size, name, created_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![id, hash, mime, size, name, by],
                )?;
                let background = format!("img:{}", tx.last_insert_rowid());
                tx.execute(
                    "UPDATE boards SET background = ?2 WHERE id = ?1",
                    params![id, background],
                )?;
                events::record(
                    &tx,
                    by,
                    "board_updated",
                    at,
                    json!({ "background": background, "uploaded": name }),
                )?;
                Ok((background, old))
            })();
            match result.and_then(|v| tx.commit().map(|_| v)) {
                Ok((background, old)) => {
                    for h in old.iter().filter(|h| **h != hash) {
                        let still: bool = c.query_row(files::STORED, [h], |r| r.get(0))?;
                        if !still {
                            let _ = store::remove(&dir, h);
                        }
                    }
                    Ok(Ok(background))
                }
                Err(e) => {
                    if written {
                        let _ = store::remove(&dir, &hash);
                    }
                    Err(e.into())
                }
            }
        })
        .await??;
    Ok(Json(json!({ "background": saved })))
}

async fn picture(
    State(state): State<AppState>,
    board: Board,
    Path(params): Path<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let image = path_id(&params, "image")?;
    let id = board.id;
    let found: Option<(String, String, String)> = board
        .project
        .db()
        .read(move |c| {
            Ok(c.query_row(
                "SELECT hash, mime, name FROM board_images WHERE id = ?1 AND board_id = ?2",
                params![image, id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?)
        })
        .await?;
    let (hash, mime, name) = found.ok_or(ApiError::NotFound)?;
    let dir = files::org_dir(&state, &board.project.company.slug);
    let path = store::path_of(&dir, &hash).ok_or(ApiError::NotFound)?;
    let bytes = tokio::task::spawn_blocking(move || std::fs::read(path))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map_err(|_| ApiError::NotFound)?;
    Ok(files::file_response(bytes, &mime, &name, true))
}
