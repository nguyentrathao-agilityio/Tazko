use std::collections::HashMap;

use axum::Router;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::{get, post, put};
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::COMPANY_PREFIX;
use crate::boards::Board;
use crate::cards::{card_in_board, copy_card, end_of_list, open_list};
use crate::error::{ApiError, bad};
use crate::events::{self, At};
use crate::order;
use crate::projects::{Act, path_id};
use crate::state::AppState;

pub struct Builtin {
    pub key: &'static str,
    pub title: &'static str,
    pub label: Option<&'static str>,
    pub checklist: (&'static str, &'static [&'static str]),
}

pub const BUILTIN: &[Builtin] = &[
    Builtin {
        key: "bug",
        title: "Bug report",
        label: Some("Bug"),
        checklist: ("Steps", &["Reproduce", "Fix", "Verify"]),
    },
    Builtin {
        key: "feature",
        title: "Feature request",
        label: None,
        checklist: (
            "Done when",
            &["Problem described", "Design agreed", "Built", "Tested"],
        ),
    },
    Builtin {
        key: "meeting",
        title: "Meeting notes",
        label: None,
        checklist: ("Agenda", &["Updates", "Decisions", "Next steps"]),
    },
];

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}/boards/{{board}}{s}");
    Router::new()
        .route(&p("/card-templates"), get(list))
        .route(&p("/card-templates/use"), post(use_template))
        .route(&p("/cards/{card}/template"), put(mark))
}

pub fn of_board(c: &Connection, board: i64) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = c.prepare(
        "SELECT k.id, k.title,
                (SELECT count(*) FROM checklists x WHERE x.card_id = k.id),
                (SELECT count(*) FROM card_labels l WHERE l.card_id = k.id)
         FROM cards k WHERE k.board_id = ?1 AND k.is_template = 1 AND k.archived_at IS NULL ORDER BY k.title COLLATE NOCASE",
    )?;
    stmt.query_map([board], |r| {
        Ok(json!({ "id": r.get::<_, i64>(0)?, "title": r.get::<_, String>(1)?, "checklists": r.get::<_, i64>(2)?, "labels": r.get::<_, i64>(3)? }))
    })?
    .collect()
}

async fn list(board: Board) -> Result<Json<Value>, ApiError> {
    let id = board.id;
    let saved = board
        .project
        .db()
        .read(move |c| Ok(of_board(c, id)?))
        .await?;
    Ok(Json(json!({
        "saved": saved,
        "builtin": BUILTIN.iter().map(|b| json!({ "key": b.key, "title": b.title })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct On {
    on: bool,
}

async fn mark(
    board: Board,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
    Json(body): Json<On>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let card = path_id(&params, "card")?;
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if card_in_board(&tx, id, card, false)?.is_none() {
                return Ok(Err(ApiError::NotFound));
            }

            let n = tx.execute(
                "UPDATE cards SET is_template = ?2 WHERE id = ?1 AND is_template != ?2",
                params![card, body.on],
            )?;
            if n > 0 {
                events::record(
                    &tx,
                    by,
                    "card_template",
                    At::card(pid, id, card),
                    json!({ "on": body.on }),
                )?;
            }
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct UseTemplate {
    list_id: i64,
    card: Option<i64>,
    builtin: Option<String>,
    title: Option<String>,
}

async fn use_template(
    board: Board,
    headers: HeaderMap,
    Json(body): Json<UseTemplate>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let builtin = match (&body.card, body.builtin.as_deref()) {
        (Some(_), None) => None,
        (None, Some(k)) => Some(
            BUILTIN
                .iter()
                .find(|b| b.key == k)
                .ok_or_else(|| bad("choose a template"))?,
        ),
        _ => return Err(bad("choose a template")),
    };
    let custom_title = body.title.as_deref().map(crate::cards::title).transpose()?;
    let (id, pid, list) = (board.id, board.project.id, body.list_id);
    let source = body.card;
    let new = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if !open_list(&tx, id, list)? {
                return Ok(Err(bad("that list is not on this board")));
            }

            let pos = end_of_list(&tx, list)?;
            let new = if let Some(card) = source {
                let template: Option<String> = tx
                    .query_row(
                        "SELECT title FROM cards WHERE id = ?1 AND board_id = ?2 AND is_template = 1 AND archived_at IS NULL",
                        params![card, id],
                        |r| r.get(0),
                    )
                    .ok();
                let Some(title) = template else {
                    return Ok(Err(ApiError::NotFound));
                };
                let t = custom_title.clone().unwrap_or(title);
                let new = copy_card(&tx, card, list, id, &t, &pos, Some(by), None)?;
                tx.execute(
                    "UPDATE cards SET start_at = NULL, due_at = NULL, done = 0, cover = NULL, is_template = 0, recurrence = NULL WHERE id = ?1",
                    [new],
                )?;
                tx.execute("DELETE FROM card_members WHERE card_id = ?1", [new])?;
                tx.execute(
                    "UPDATE checklist_items SET done = 0, assignee_id = NULL, due_at = NULL WHERE checklist_id IN (SELECT id FROM checklists WHERE card_id = ?1)",
                    [new],
                )?;
                tx.execute(
                    "INSERT INTO custom_field_values (card_id, field_id, value) SELECT ?2, field_id, value FROM custom_field_values WHERE card_id = ?1",
                    params![card, new],
                )?;
                new
            } else {
                let b = builtin.expect("builtin chosen");
                let t = custom_title.clone().unwrap_or_else(|| b.title.to_string());
                tx.execute(
                    "INSERT INTO cards (board_id, list_id, title, position, created_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, list, t, pos, by],
                )?;
                let new = tx.last_insert_rowid();
                if let Some(label) = b.label {
                    tx.execute(
                        "INSERT INTO card_labels (card_id, label_id)
                         SELECT ?1, id FROM labels WHERE board_id = ?2 AND name = ?3 ORDER BY position LIMIT 1",
                        params![new, id, label],
                    )?;
                }
                let (title, items) = b.checklist;
                tx.execute(
                    "INSERT INTO checklists (card_id, title, position) VALUES (?1, ?2, ?3)",
                    params![new, title, order::between(None, None)],
                )?;
                let checklist = tx.last_insert_rowid();
                for (item, ipos) in items.iter().zip(order::spread(items.len())) {
                    tx.execute(
                        "INSERT INTO checklist_items (checklist_id, text, position) VALUES (?1, ?2, ?3)",
                        params![checklist, item, ipos],
                    )?;
                }
                new
            };
            events::record(
                &tx,
                by,
                "card_created",
                At::card(pid, id, new),
                json!({ "list": list, "template": source, "builtin": builtin.map(|b| b.key) }),
            )?;
            crate::notifications::card_added(&tx, id, new, by)?;
            tx.commit()?;
            Ok(Ok(new))
        })
        .await??;
    Ok((StatusCode::CREATED, Json(json!({ "id": new }))))
}
