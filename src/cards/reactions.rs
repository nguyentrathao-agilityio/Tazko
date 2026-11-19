use std::collections::HashMap;

use axum::Router;
use axum::extract::Path;
use axum::http::HeaderMap;
use axum::response::Json;
use axum::routing::put;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::COMPANY_PREFIX;
use crate::boards::Board;
use crate::error::{ApiError, bad};
use crate::events::{self, At};
use crate::projects::{Act, path_id};
use crate::state::AppState;

pub const EMOJI: &[&str] = &["👍", "❤️", "🎉", "👀", "🚀", "✅"];

pub fn routes() -> Router<AppState> {
    Router::new().route(
        &format!("{COMPANY_PREFIX}/boards/{{board}}/comments/{{comment}}/reactions"),
        put(react),
    )
}

pub fn of_comment(c: &Connection, comment: i64, me: i64) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = c.prepare(
        "SELECT r.emoji, r.user_id, coalesce(nullif(p.name, ''), p.email, '')
         FROM comment_reactions r LEFT JOIN people p ON p.id = r.user_id
         WHERE r.comment_id = ?1 ORDER BY r.created_at, r.user_id",
    )?;
    let rows: Vec<(String, i64, String)> = stmt
        .query_map([comment], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let mut out: Vec<(String, Vec<String>, bool)> = Vec::new();
    for (emoji, user, name) in rows {
        let pos = match out.iter().position(|(e, _, _)| *e == emoji) {
            Some(i) => i,
            None => {
                out.push((emoji, Vec::new(), false));
                out.len() - 1
            }
        };
        out[pos].1.push(name);
        out[pos].2 |= user == me;
    }
    Ok(out
        .into_iter()
        .map(|(emoji, names, mine)| json!({ "emoji": emoji, "count": names.len(), "mine": mine, "people": names }))
        .collect())
}

#[derive(Deserialize)]
struct Reaction {
    emoji: String,
    on: bool,
}

async fn react(
    board: Board,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
    Json(body): Json<Reaction>,
) -> Result<Json<Value>, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let comment = path_id(&params, "comment")?;
    if !EMOJI.contains(&body.emoji.as_str()) {
        return Err(bad("choose one of the reactions"));
    }

    let (id, pid) = (board.id, board.project.id);
    let saved = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let card: Option<i64> = tx
                .query_row(
                    "SELECT k.id FROM comments m JOIN cards k ON k.id = m.card_id
                     WHERE m.id = ?1 AND k.board_id = ?2 AND k.archived_at IS NULL",
                    params![comment, id],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(card) = card else {
                return Ok(Err(ApiError::NotFound));
            };
            let n = if body.on {
                tx.execute(
                    "INSERT INTO comment_reactions (comment_id, user_id, emoji) VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING",
                    params![comment, by, body.emoji],
                )?
            } else {
                tx.execute(
                    "DELETE FROM comment_reactions WHERE comment_id = ?1 AND user_id = ?2 AND emoji = ?3",
                    params![comment, by, body.emoji],
                )?
            };
            if n > 0 {
                events::record(
                    &tx,
                    by,
                    "comment_reacted",
                    At::card(pid, id, card),
                    json!({ "comment": comment, "emoji": body.emoji, "on": body.on }),
                )?;
            }
            let list = of_comment(&tx, comment, by)?;
            tx.commit()?;
            Ok(Ok(json!({ "reactions": list })))
        })
        .await??;
    Ok(Json(saved))
}
