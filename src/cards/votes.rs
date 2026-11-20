use std::collections::HashMap;

use axum::Router;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::put;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use super::card_in_board;
use crate::api::COMPANY_PREFIX;
use crate::boards::Board;
use crate::error::{ApiError, bad};
use crate::events::{self, At};
use crate::projects::{Act, path_id};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        &format!("{COMPANY_PREFIX}/boards/{{board}}/cards/{{card}}/vote"),
        put(vote),
    )
}

pub fn board_votes(
    c: &Connection,
    board: i64,
    user: i64,
) -> rusqlite::Result<HashMap<i64, (i64, bool)>> {
    let mut stmt = c.prepare(
        "SELECT v.card_id, count(*), max(v.user_id = ?2)
         FROM card_votes v JOIN cards k ON k.id = v.card_id
         WHERE k.board_id = ?1 AND k.archived_at IS NULL
         GROUP BY v.card_id",
    )?;
    stmt.query_map(params![board, user], |r| {
        Ok((r.get(0)?, (r.get(1)?, r.get(2)?)))
    })?
    .collect()
}

pub fn voters(c: &Connection, card: i64) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = c.prepare(
        "SELECT p.id, p.name, p.email FROM card_votes v JOIN people p ON p.id = v.user_id
         WHERE v.card_id = ?1 ORDER BY v.created_at, p.id",
    )?;
    stmt.query_map([card], |r| {
        Ok(json!({ "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "email": r.get::<_, String>(2)? }))
    })?
    .collect()
}

#[derive(Deserialize)]
struct Vote {
    on: bool,
}

async fn vote(
    board: Board,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
    Json(body): Json<Vote>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    if !board.project.info.voting {
        return Err(bad("voting is turned off in this project"));
    }

    let card = path_id(&params, "card")?;
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let template: Option<bool> = tx
                .query_row("SELECT is_template FROM cards WHERE id = ?1", [card], |r| r.get(0))
                .optional()?;
            if card_in_board(&tx, id, card, false)?.is_none() || template != Some(false) {
                return Ok(Err(ApiError::NotFound));
            }

            let n = if body.on {
                tx.execute("INSERT INTO card_votes (card_id, user_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING", params![card, by])?
            } else {
                tx.execute("DELETE FROM card_votes WHERE card_id = ?1 AND user_id = ?2", params![card, by])?
            };
            if n > 0 {
                events::record(&tx, by, if body.on { "card_voted" } else { "card_unvoted" }, At::card(pid, id, card), json!({}))?;
            }
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}
