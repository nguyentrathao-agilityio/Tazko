use std::collections::HashMap;

use axum::Router;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::{delete, get, post};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use super::card_in_board;
use crate::api::COMPANY_PREFIX;
use crate::boards::{Board, VISIBLE};
use crate::error::{ApiError, bad};
use crate::events::{self, At};
use crate::projects::{Act, path_id};
use crate::state::AppState;

type Params = Path<HashMap<String, String>>;

const SEARCH_LIMIT: i64 = 30;

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}/boards/{{board}}/cards/{{card}}{s}");
    Router::new()
        .route(&p("/links"), post(add))
        .route(&p("/links/{other}"), delete(remove))
        .route(&p("/link-targets"), get(targets))
}

fn stored(kind: &str) -> Option<(&'static str, bool)> {
    match kind {
        "relates" => Some(("relates", false)),
        "blocks" => Some(("blocks", false)),
        "blocked_by" => Some(("blocks", true)),
        "duplicates" => Some(("duplicates", false)),
        "duplicated_by" => Some(("duplicates", true)),
        _ => None,
    }
}

fn seen_from(kind: &str, outgoing: bool) -> &'static str {
    match (kind, outgoing) {
        ("blocks", true) => "blocks",
        ("blocks", false) => "blocked_by",
        ("duplicates", true) => "duplicates",
        ("duplicates", false) => "duplicated_by",
        _ => "relates",
    }
}

fn reachable(c: &Connection, project: i64, card: i64, user: i64) -> rusqlite::Result<Option<i64>> {
    c.query_row(
        &format!(
            "SELECT b.id FROM cards k JOIN boards b ON b.id = k.board_id
             WHERE k.id = ?1 AND b.project_id = ?3 AND k.archived_at IS NULL AND k.is_template = 0 AND b.closed_at IS NULL AND {VISIBLE}"
        ),
        params![card, user, project],
        |r| r.get(0),
    )
    .optional()
}

pub fn for_card(c: &Connection, card: i64, user: i64) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = c.prepare(&format!(
        "SELECT o.id, o.title, o.done, o.archived_at IS NOT NULL, b.id, b.name, l.title, x.kind, x.from_card = ?1
         FROM card_links x
         JOIN cards o ON o.id = CASE WHEN x.from_card = ?1 THEN x.to_card ELSE x.from_card END
         JOIN boards b ON b.id = o.board_id
         JOIN lists l ON l.id = o.list_id
         WHERE (x.from_card = ?1 OR x.to_card = ?1) AND {VISIBLE}
         ORDER BY x.id"
    ))?;
    stmt.query_map(params![card, user], |r| {
        let kind: String = r.get(7)?;
        Ok(json!({
            "card": { "id": r.get::<_, i64>(0)?, "title": r.get::<_, String>(1)?, "done": r.get::<_, bool>(2)?, "archived": r.get::<_, bool>(3)? },
            "board": { "id": r.get::<_, i64>(4)?, "name": r.get::<_, String>(5)? },
            "list": r.get::<_, String>(6)?,
            "kind": seen_from(&kind, r.get(8)?),
        }))
    })?
    .collect()
}

pub fn board_flags(
    c: &Connection,
    board: i64,
    user: i64,
) -> rusqlite::Result<HashMap<i64, (i64, bool)>> {
    let mut stmt = c.prepare(&format!(
        "SELECT k.id,
                (SELECT count(*) FROM card_links x
                 JOIN cards o ON o.id = CASE WHEN x.from_card = k.id THEN x.to_card ELSE x.from_card END
                 JOIN boards b ON b.id = o.board_id
                 WHERE (x.from_card = k.id OR x.to_card = k.id) AND {VISIBLE}),
                EXISTS (SELECT 1 FROM card_links x
                 JOIN cards o ON o.id = x.from_card
                 JOIN boards b ON b.id = o.board_id
                 WHERE x.to_card = k.id AND x.kind = 'blocks' AND o.done = 0 AND o.archived_at IS NULL AND {VISIBLE})
         FROM cards k
         WHERE k.board_id = ?1 AND k.archived_at IS NULL
           AND EXISTS (SELECT 1 FROM card_links x WHERE x.from_card = k.id OR x.to_card = k.id)"
    ))?;
    stmt.query_map(params![board, user], |r| {
        Ok((r.get(0)?, (r.get(1)?, r.get(2)?)))
    })?
    .collect()
}

#[derive(Deserialize)]
struct NewLink {
    card: i64,
    kind: String,
}

async fn add(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<NewLink>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let card = path_id(&params, "card")?;
    let (kind, reverse) =
        stored(&body.kind).ok_or_else(|| bad("choose how the cards are linked"))?;
    if body.card == card {
        return Err(bad("a card cannot be linked to itself"));
    }

    let (id, pid, other) = (board.id, board.project.id, body.card);
    let label = body.kind.clone();
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if card_in_board(&tx, id, card, false)?.is_none() {
                return Ok(Err(ApiError::NotFound));
            }

            let Some(other_board) = reachable(&tx, pid, other, by)? else {
                return Ok(Err(ApiError::NotFound));
            };
            let (from, to) = if reverse { (other, card) } else { (card, other) };
            tx.execute(
                "DELETE FROM card_links WHERE min(from_card, to_card) = min(?1, ?2) AND max(from_card, to_card) = max(?1, ?2)",
                params![from, to],
            )?;
            tx.execute(
                "INSERT INTO card_links (from_card, to_card, kind, created_by) VALUES (?1, ?2, ?3, ?4)",
                params![from, to, kind, by],
            )?;
            events::record(&tx, by, "link_added", At::card(pid, id, card), json!({ "card": other, "kind": label }))?;
            let back = stored(&label).map(|(k, r)| seen_from(k, r)).unwrap_or("relates");
            events::record(&tx, by, "link_added", At::card(pid, other_board, other), json!({ "card": card, "kind": back }))?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let card = path_id(&params, "card")?;
    let other = path_id(&params, "other")?;
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if card_in_board(&tx, id, card, false)?.is_none() {
                return Ok(Err(ApiError::NotFound));
            }

            let other_board: Option<i64> = tx
                .query_row(
                    &format!("SELECT b.id FROM cards k JOIN boards b ON b.id = k.board_id WHERE k.id = ?1 AND b.project_id = ?3 AND {VISIBLE}"),
                    params![other, by, pid],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(other_board) = other_board else {
                return Ok(Err(ApiError::NotFound));
            };
            let n = tx.execute(
                "DELETE FROM card_links WHERE (from_card = ?1 AND to_card = ?2) OR (from_card = ?2 AND to_card = ?1)",
                params![card, other],
            )?;
            if n == 0 {
                return Ok(Err(ApiError::NotFound));
            }

            events::record(&tx, by, "link_removed", At::card(pid, id, card), json!({ "card": other }))?;
            events::record(&tx, by, "link_removed", At::card(pid, other_board, other), json!({ "card": card }))?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct Search {
    q: Option<String>,
}

async fn targets(
    board: Board,
    Path(params): Params,
    Query(q): Query<Search>,
) -> Result<Json<Value>, ApiError> {
    let card = path_id(&params, "card")?;
    let (id, pid, user) = (board.id, board.project.id, board.user_id());
    let needle = format!(
        "%{}%",
        q.q.unwrap_or_default().trim().replace(['%', '_'], "")
    );
    let found = board
        .project
        .db()
        .read(move |c| {
            if card_in_board(c, id, card, false)?.is_none() {
                return Ok(None);
            }

            let mut stmt = c.prepare(&format!(
                "SELECT k.id, k.title, b.id, b.name, l.title
                 FROM cards k JOIN boards b ON b.id = k.board_id JOIN lists l ON l.id = k.list_id
                 WHERE b.project_id = ?3 AND k.id != ?1 AND k.archived_at IS NULL AND k.is_template = 0
                   AND l.archived_at IS NULL AND b.closed_at IS NULL AND {VISIBLE}
                   AND k.title LIKE ?4
                   AND NOT EXISTS (SELECT 1 FROM card_links x WHERE (x.from_card = ?1 AND x.to_card = k.id) OR (x.to_card = ?1 AND x.from_card = k.id))
                 ORDER BY b.id = ?5 DESC, k.updated_at DESC LIMIT ?6"
            ))?;
            let rows = stmt
                .query_map(params![card, user, pid, needle, id, SEARCH_LIMIT], |r| {
                    Ok(json!({
                        "card": { "id": r.get::<_, i64>(0)?, "title": r.get::<_, String>(1)? },
                        "board": { "id": r.get::<_, i64>(2)?, "name": r.get::<_, String>(3)? },
                        "list": r.get::<_, String>(4)?,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Some(rows))
        })
        .await?;
    found
        .map(|cards| Json(json!({ "cards": cards })))
        .ok_or(ApiError::NotFound)
}
