use std::collections::HashMap;

use axum::Router;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::{patch, post};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use super::Board;
use crate::admin::validate;
use crate::api::COMPANY_PREFIX;
use crate::error::{ApiError, bad};
use crate::events;
use crate::order;
use crate::projects::{Act, path_id};
use crate::session::now;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}/boards/{{board}}{s}");
    Router::new()
        .route(&p("/lists"), post(create))
        .route(&p("/lists/{list}"), patch(rename))
        .route(&p("/lists/{list}/move"), post(move_list))
        .route(&p("/lists/{list}/archive"), post(archive))
        .route(&p("/lists/{list}/restore"), post(restore))
        .route(&p("/lists/{list}/copy"), post(copy))
}

pub fn copy_cards(tx: &Connection, from: i64, to: i64) -> rusqlite::Result<()> {
    crate::cards::copy_cards(tx, from, to, None)
}

fn title(raw: &str) -> Result<String, ApiError> {
    validate::name(raw).ok_or_else(|| bad("enter a list title"))
}

pub fn position_of(c: &Connection, board: i64, list: i64) -> rusqlite::Result<Option<String>> {
    c.query_row(
        "SELECT position FROM lists WHERE id = ?1 AND board_id = ?2 AND archived_at IS NULL",
        params![list, board],
        |r| r.get(0),
    )
    .optional()
}

fn last_position(c: &Connection, board: i64) -> rusqlite::Result<Option<String>> {
    c.query_row(
        "SELECT max(position) FROM lists WHERE board_id = ?1",
        [board],
        |r| r.get(0),
    )
}

fn next_position(c: &Connection, board: i64, pos: &str) -> rusqlite::Result<Option<String>> {
    c.query_row(
        "SELECT min(position) FROM lists WHERE board_id = ?1 AND position > ?2",
        params![board, pos],
        |r| r.get(0),
    )
}

fn place_after(c: &Connection, board: i64, after: Option<i64>) -> Result<String, ApiError> {
    match after {
        None => {
            let first: Option<String> = c.query_row(
                "SELECT min(position) FROM lists WHERE board_id = ?1",
                [board],
                |r| r.get(0),
            )?;
            Ok(order::between(None, first.as_deref()))
        }
        Some(id) => {
            let pos =
                position_of(c, board, id)?.ok_or_else(|| bad("that list is not on this board"))?;
            let next = next_position(c, board, &pos)?;
            Ok(order::between(Some(&pos), next.as_deref()))
        }
    }
}

fn list_param(params: &HashMap<String, String>) -> Result<i64, ApiError> {
    path_id(params, "list")
}

fn find(c: &Connection, board: i64, list: i64, archived: bool) -> rusqlite::Result<Option<String>> {
    c.query_row(
        "SELECT title FROM lists WHERE id = ?1 AND board_id = ?2 AND (archived_at IS NOT NULL) = ?3",
        params![list, board, archived],
        |r| r.get(0),
    )
    .optional()
}

#[derive(Deserialize)]
struct NewList {
    title: String,
    #[serde(default)]
    after: Option<i64>,
    #[serde(default)]
    at_end: Option<bool>,
}

async fn create(
    board: Board,
    headers: HeaderMap,
    Json(body): Json<NewList>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let title = title(&body.title)?;
    let (id, at) = (board.id, board.at());
    let at_end = body.at_end.unwrap_or(body.after.is_none());
    let list = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let pos = if at_end {
                order::after(last_position(&tx, id)?.as_deref())
            } else {
                match place_after(&tx, id, body.after) {
                    Ok(p) => p,
                    Err(e) => return Ok(Err(e)),
                }
            };
            tx.execute(
                "INSERT INTO lists (board_id, title, position) VALUES (?1, ?2, ?3)",
                params![id, title, pos],
            )?;
            let list = tx.last_insert_rowid();
            events::record(
                &tx,
                by,
                "list_created",
                at,
                json!({ "list": list, "title": title }),
            )?;
            tx.commit()?;
            Ok(Ok(json!({ "id": list, "title": title, "position": pos })))
        })
        .await??;
    Ok((StatusCode::CREATED, Json(list)))
}

#[derive(Deserialize)]
struct Rename {
    title: String,
}

async fn rename(
    board: Board,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
    Json(body): Json<Rename>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let list = list_param(&params)?;
    let title = title(&body.title)?;
    let (id, at) = (board.id, board.at());
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(old) = find(&tx, id, list, false)? else {
                return Ok(Err(ApiError::NotFound));
            };
            tx.execute(
                "UPDATE lists SET title = ?2 WHERE id = ?1",
                params![list, title],
            )?;
            events::record(
                &tx,
                by,
                "list_renamed",
                at,
                json!({ "list": list, "from": old, "to": title }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct Move {
    after: Option<i64>,
}

async fn move_list(
    board: Board,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
    Json(body): Json<Move>,
) -> Result<Json<Value>, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let list = list_param(&params)?;
    if body.after == Some(list) {
        return Err(bad("a list cannot move after itself"));
    }

    let (id, at) = (board.id, board.at());
    let pos = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if find(&tx, id, list, false)?.is_none() {
                return Ok(Err(ApiError::NotFound));
            }

            let pos = match place_after(&tx, id, body.after) {
                Ok(p) => p,
                Err(e) => return Ok(Err(e)),
            };
            tx.execute(
                "UPDATE lists SET position = ?2 WHERE id = ?1",
                params![list, pos],
            )?;
            events::record(
                &tx,
                by,
                "list_moved",
                at,
                json!({ "list": list, "after": body.after }),
            )?;
            tx.commit()?;
            Ok(Ok(pos))
        })
        .await??;
    Ok(Json(json!({ "position": pos })))
}

async fn set_archived(
    board: Board,
    headers: HeaderMap,
    params: HashMap<String, String>,
    archive: bool,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let list = list_param(&params)?;
    let (id, at) = (board.id, board.at());
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(title) = find(&tx, id, list, !archive)? else {
                return Ok(Err(ApiError::NotFound));
            };
            if archive {
                tx.execute(
                    "UPDATE lists SET archived_at = ?2 WHERE id = ?1",
                    params![list, now()],
                )?;
            } else {
                let pos = order::after(last_position(&tx, id)?.as_deref());
                tx.execute(
                    "UPDATE lists SET archived_at = NULL, position = ?2 WHERE id = ?1",
                    params![list, pos],
                )?;
            }
            let kind = if archive {
                "list_archived"
            } else {
                "list_restored"
            };
            events::record(&tx, by, kind, at, json!({ "list": list, "title": title }))?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

async fn archive(
    board: Board,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    set_archived(board, headers, params, true).await
}

async fn restore(
    board: Board,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    set_archived(board, headers, params, false).await
}

async fn copy(
    board: Board,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
    Json(body): Json<Rename>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let list = list_param(&params)?;
    let title = title(&body.title)?;
    let (id, at) = (board.id, board.at());
    let copy = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if find(&tx, id, list, false)?.is_none() {
                return Ok(Err(ApiError::NotFound));
            }

            let pos = match place_after(&tx, id, Some(list)) {
                Ok(p) => p,
                Err(e) => return Ok(Err(e)),
            };
            tx.execute(
                "INSERT INTO lists (board_id, title, position) VALUES (?1, ?2, ?3)",
                params![id, title, pos],
            )?;
            let new = tx.last_insert_rowid();
            copy_cards(&tx, list, new)?;
            events::record(
                &tx,
                by,
                "list_copied",
                at,
                json!({ "list": new, "from": list, "title": title }),
            )?;
            tx.commit()?;
            Ok(Ok(new))
        })
        .await??;
    Ok((StatusCode::CREATED, Json(json!({ "id": copy }))))
}
