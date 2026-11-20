use std::collections::HashMap;

use axum::Router;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, patch, post, put};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{card_in_board, title};
use crate::api::COMPANY_PREFIX;
use crate::boards::Board;
use crate::error::{ApiError, bad, conflict};
use crate::events::{self, At};
use crate::order;
use crate::projects::{Act, path_id};
use crate::sanitize;
use crate::session::now;
use crate::state::AppState;

pub const LABEL_COLORS: i64 = 8;
pub const COVERS: &[&str] = &[
    "#4B4FD8", "#0F8F7E", "#D6456B", "#D33F3F", "#E0A020", "#5E6A85", "#3E9B4F", "#E2762B",
];

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}/boards/{{board}}{s}");
    Router::new()
        .route(&p("/cards/{card}"), get(view).patch(update))
        .route(&p("/cards/{card}/repeat/undo"), post(undo_repeat))
        .route(&p("/cards/{card}/labels/{label}"), put(set_label))
        .route(&p("/cards/{card}/members/{user}"), put(set_member))
        .route(&p("/cards/{card}/checklists"), post(add_checklist))
        .route(&p("/cards/{card}/comments"), post(add_comment))
        .route(&p("/labels"), post(add_label))
        .route(
            &p("/labels/{label}"),
            patch(edit_label).delete(delete_label),
        )
        .route(
            &p("/checklists/{checklist}"),
            patch(edit_checklist).delete(delete_checklist),
        )
        .route(&p("/checklists/{checklist}/move"), post(move_checklist))
        .route(&p("/checklists/{checklist}/items"), post(add_item))
        .route(&p("/items/{item}"), patch(edit_item).delete(delete_item))
        .route(&p("/items/{item}/move"), post(move_item))
        .route(
            &p("/comments/{comment}"),
            patch(edit_comment).delete(delete_comment),
        )
}

type Params = Path<HashMap<String, String>>;

type CommentRow = (i64, Option<i64>, String, i64, Option<i64>);

fn short(raw: &str, what: &str, max: usize) -> Result<String, ApiError> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(bad(&format!("enter {what}")));
    }

    if t.chars().count() > max || t.chars().any(|c| c.is_control()) {
        return Err(bad(&format!("{what} is too long")));
    }

    Ok(t.to_string())
}

pub fn labels_of_board(c: &Connection, board: i64) -> rusqlite::Result<Vec<Value>> {
    let mut stmt =
        c.prepare("SELECT id, name, color FROM labels WHERE board_id = ?1 ORDER BY position, id")?;
    stmt.query_map([board], |r| {
        Ok(json!({ "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "color": r.get::<_, i64>(2)? }))
    })?
    .collect()
}

pub fn insert_default_labels(c: &Connection, board: i64) -> rusqlite::Result<()> {
    for (i, name) in [
        "Frontend", "Backend", "Design", "Bug", "Research", "Blocked", "Content", "Urgent",
    ]
    .iter()
    .enumerate()
    {
        c.execute(
            "INSERT INTO labels (board_id, name, color, position) VALUES (?1, ?2, ?3, ?3)",
            params![board, name, i as i64],
        )?;
    }
    Ok(())
}

fn person(c: &Connection, id: Option<i64>) -> rusqlite::Result<Value> {
    let Some(id) = id else {
        return Ok(Value::Null);
    };
    Ok(c.query_row(
        "SELECT id, name, email FROM people WHERE id = ?1",
        [id],
        |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "email": r.get::<_, String>(2)? })),
    )
    .optional()?
    .unwrap_or(Value::Null))
}

pub fn card_activity(c: &Connection, card: i64, limit: i64) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = c.prepare(
        "SELECT e.id, e.type, e.actor_id, e.payload, e.created_at FROM events e
         WHERE e.card_id = ?1 AND e.type NOT IN ('board_starred', 'board_unstarred') ORDER BY e.id DESC LIMIT ?2",
    )?;
    let rows: Vec<(i64, String, Option<i64>, String, i64)> = stmt
        .query_map(params![card, limit], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<Result<_, _>>()?;
    rows.into_iter()
        .map(|(id, kind, actor, payload, at)| {
            Ok(json!({
                "id": id,
                "type": kind,
                "actor": person(c, actor)?,
                "payload": serde_json::from_str::<Value>(&payload).unwrap_or(Value::Null),
                "created_at": at,
            }))
        })
        .collect()
}

fn checklists(c: &Connection, card: i64) -> rusqlite::Result<Vec<Value>> {
    let mut stmt =
        c.prepare("SELECT id, title FROM checklists WHERE card_id = ?1 ORDER BY position, id")?;
    let lists: Vec<(i64, String)> = stmt
        .query_map([card], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut out = Vec::new();
    for (id, t) in lists {
        let mut stmt = c.prepare(
            "SELECT id, text, done, assignee_id, due_at FROM checklist_items WHERE checklist_id = ?1 ORDER BY position, id",
        )?;
        let items = stmt
            .query_map([id], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "text": r.get::<_, String>(1)?,
                    "done": r.get::<_, bool>(2)?,
                    "assignee": r.get::<_, Option<i64>>(3)?,
                    "due_at": r.get::<_, Option<i64>>(4)?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        out.push(json!({ "id": id, "title": t, "items": items }));
    }
    Ok(out)
}

async fn view(board: Board, Path(params): Params) -> Result<Json<Value>, ApiError> {
    let card = path_id(&params, "card")?;
    let (id, me) = (board.id, board.user_id());
    let found = board
        .project
        .db()
        .read(move |c| {
            let row = c
                .query_row(
                    "SELECT k.title, k.description, k.start_at, k.due_at, k.done, k.cover, k.list_id, l.title,
                            k.archived_at IS NOT NULL, k.created_at, k.created_by, k.cover_attachment, k.is_template, k.recurrence
                     FROM cards k JOIN lists l ON l.id = k.list_id WHERE k.id = ?1 AND k.board_id = ?2",
                    params![card, id],
                    |r| {
                        Ok((
                            json!({
                                "id": card,
                                "title": r.get::<_, String>(0)?,
                                "description": r.get::<_, String>(1)?,
                                "start_at": r.get::<_, Option<i64>>(2)?,
                                "due_at": r.get::<_, Option<i64>>(3)?,
                                "done": r.get::<_, bool>(4)?,
                                "cover": r.get::<_, Option<String>>(5)?,
                                "list": { "id": r.get::<_, i64>(6)?, "title": r.get::<_, String>(7)? },
                                "archived": r.get::<_, bool>(8)?,
                                "created_at": r.get::<_, i64>(9)?,
                                "cover_image": r.get::<_, Option<i64>>(11)?,
                                "is_template": r.get::<_, bool>(12)?,
                                "recurrence": r.get::<_, Option<String>>(13)?,
                            }),
                            r.get::<_, Option<i64>>(10)?,
                        ))
                    },
                )
                .optional()?;
            let Some((mut v, creator)) = row else {
                return Ok(None);
            };
            let ids = |sql: &str| -> rusqlite::Result<Vec<i64>> {
                let mut stmt = c.prepare(sql)?;
                stmt.query_map([card], |r| r.get(0))?.collect()
            };
            v["labels"] = json!(ids("SELECT label_id FROM card_labels WHERE card_id = ?1")?);
            v["members"] = json!(ids("SELECT user_id FROM card_members WHERE card_id = ?1")?);
            v["checklists"] = json!(checklists(c, card)?);
            v["attachments"] = json!(crate::files::attachments_of(c, card)?);
            v["fields"] = Value::Object(super::fields::card_values(c, card, false)?);
            v["created_by"] = person(c, creator)?;
            let mut stmt = c.prepare(
                "SELECT id, user_id, body, created_at, edited_at FROM comments WHERE card_id = ?1 ORDER BY id DESC",
            )?;
            let rows: Vec<CommentRow> = stmt
                .query_map([card], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
                .collect::<Result<_, _>>()?;
            let mut comments = Vec::new();
            for (cid, user, body, at, edited) in rows {
                comments.push(json!({
                    "reactions": super::reactions::of_comment(c, cid, me)?,
                    "id": cid,
                    "author": person(c, user)?,
                    "body": body,
                    "created_at": at,
                    "edited_at": edited,
                }));
            }
            v["comments"] = json!(comments);
            v["activity"] = json!(card_activity(c, card, 100)?);
            Ok(Some(v))
        })
        .await?;
    let mut v = found.ok_or(ApiError::NotFound)?;
    v["can"] = json!({ "edit": board.can(Act::Edit) });
    Ok(Json(v))
}

fn date(v: &Value) -> Result<Option<i64>, ApiError> {
    match v {
        Value::Null => Ok(None),
        Value::Number(n) => n
            .as_i64()
            .filter(|t| (0..=32_503_680_000).contains(t))
            .map(Some)
            .ok_or_else(|| bad("that date is not valid")),
        _ => Err(bad("that date is not valid")),
    }
}

async fn update(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<Map<String, Value>>,
) -> Result<Response, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let card = path_id(&params, "card")?;
    let mut sets: Vec<(&'static str, rusqlite::types::Value)> = Vec::new();
    let mut changed = Map::new();
    let mut offset = 0;
    use rusqlite::types::Value as Sql;
    for (key, value) in &body {
        match key.as_str() {
            "title" => {
                let t = title(value.as_str().ok_or_else(|| bad("enter a card title"))?)?;
                changed.insert("title".into(), json!(t));
                sets.push(("title", Sql::Text(t)));
            }
            "description_html" | "description_markdown" => {
                let raw = value
                    .as_str()
                    .ok_or_else(|| bad("the description is not text"))?;
                if raw.len() > sanitize::MAX_INPUT {
                    return Err(bad("the description is too long"));
                }

                let html = if key == "description_html" {
                    sanitize::clean_html(raw)
                } else {
                    sanitize::markdown_to_html(raw)
                };
                let html = if sanitize::plain_text(&html).is_empty() {
                    String::new()
                } else {
                    html
                };
                changed.insert("description".into(), json!(true));
                sets.push(("description_text", Sql::Text(sanitize::plain_text(&html))));
                sets.push(("description", Sql::Text(html)));
            }
            "start_at" => {
                let d = date(value)?;
                changed.insert("start_at".into(), json!(d));
                sets.push(("start_at", d.map_or(Sql::Null, Sql::Integer)));
            }
            "due_at" => {
                let d = date(value)?;
                changed.insert("due_at".into(), json!(d));
                sets.push(("due_at", d.map_or(Sql::Null, Sql::Integer)));
            }
            "done" => {
                let d = value
                    .as_bool()
                    .ok_or_else(|| bad("done is true or false"))?;
                changed.insert("done".into(), json!(d));
                sets.push(("done", Sql::Integer(d as i64)));
            }
            "recurrence" => {
                let r = match value {
                    Value::Null => None,
                    Value::String(r) if super::repeat::valid(r) => Some(r.clone()),
                    _ => return Err(bad("choose how often the card repeats")),
                };
                changed.insert("recurrence".into(), json!(r));
                sets.push(("recurrence", r.map_or(Sql::Null, Sql::Text)));
            }
            "tz_offset" => {
                offset = value
                    .as_i64()
                    .filter(|m| (-840..=840).contains(m))
                    .ok_or_else(|| bad("the time zone offset is not valid"))?;
            }
            "cover" => {
                let cv = match value {
                    Value::Null => None,
                    Value::String(s) if COVERS.contains(&s.as_str()) => Some(s.clone()),
                    _ => return Err(bad("choose one of the cover colours")),
                };
                changed.insert("cover".into(), json!(cv));
                sets.push(("cover", cv.map_or(Sql::Null, Sql::Text)));
                sets.push(("cover_attachment", Sql::Null));
            }
            _ => return Err(bad(&format!("{key} cannot be changed here"))),
        }
    }
    if sets.is_empty() {
        return Err(bad("nothing to change"));
    }

    let (id, pid) = (board.id, board.project.id);
    let repeated = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if card_in_board(&tx, id, card, false)?.is_none() {
                return Ok(Err(ApiError::NotFound));
            }

            for (col, v) in &sets {
                tx.execute(
                    &format!("UPDATE cards SET {col} = ?2, updated_at = ?3 WHERE id = ?1"),
                    params![card, v, now()],
                )?;
            }
            let kind = if changed.len() == 1 && changed.contains_key("done") {
                "card_done"
            } else if changed.len() == 1 && changed.contains_key("title") {
                "card_renamed"
            } else {
                "card_updated"
            };
            let completing = changed.get("done") == Some(&json!(true));
            let repeated = if completing {
                reschedule(&tx, card, offset)?
            } else {
                None
            };
            match &repeated {
                Some(r) => {
                    events::record(&tx, by, "card_repeated", At::card(pid, id, card), r.clone())?
                }
                None => events::record(
                    &tx,
                    by,
                    kind,
                    At::card(pid, id, card),
                    Value::Object(changed),
                )?,
            };
            tx.commit()?;
            Ok(Ok(repeated))
        })
        .await??;
    Ok(match repeated {
        Some(r) => Json(json!({ "repeated": r })).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    })
}

fn reschedule(c: &Connection, card: i64, offset: i64) -> rusqlite::Result<Option<Value>> {
    let (rule, due, start): (Option<String>, Option<i64>, Option<i64>) = c.query_row(
        "SELECT recurrence, due_at, start_at FROM cards WHERE id = ?1",
        [card],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let (Some(rule), Some(due)) = (rule, due) else {
        return Ok(None);
    };
    let next = super::repeat::next_due(due, &rule, now(), offset);
    let ticked: Vec<i64> = c
        .prepare("SELECT i.id FROM checklist_items i JOIN checklists x ON x.id = i.checklist_id WHERE x.card_id = ?1 AND i.done = 1")?
        .query_map([card], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    c.execute(
        "UPDATE cards SET done = 0, due_at = ?2, start_at = start_at + (?2 - ?3), updated_at = ?4 WHERE id = ?1",
        params![card, next, due, now()],
    )?;
    c.execute(
        "UPDATE checklist_items SET done = 0 WHERE checklist_id IN (SELECT id FROM checklists WHERE card_id = ?1)",
        [card],
    )?;
    Ok(Some(
        json!({ "rule": rule, "from": due, "to": next, "start_from": start, "items": ticked }),
    ))
}

async fn undo_repeat(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
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

            let last: Option<(String, String)> = tx
                .query_row(
                    "SELECT type, payload FROM events WHERE card_id = ?1 ORDER BY id DESC LIMIT 1",
                    [card],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some(p) = last
                .filter(|(t, _)| t == "card_repeated")
                .and_then(|(_, p)| serde_json::from_str::<Value>(&p).ok())
            else {
                return Ok(Err(conflict("the card has changed since it was completed")));
            };
            let due: Option<i64> = tx.query_row("SELECT due_at FROM cards WHERE id = ?1", [card], |r| r.get(0))?;
            if due != p["to"].as_i64() {
                return Ok(Err(conflict("the card has changed since it was completed")));
            }

            tx.execute(
                "UPDATE cards SET due_at = ?2, start_at = ?3, updated_at = ?4 WHERE id = ?1",
                params![card, p["from"].as_i64(), p["start_from"].as_i64(), now()],
            )?;
            for item in p["items"].as_array().into_iter().flatten().filter_map(Value::as_i64) {
                tx.execute(
                    "UPDATE checklist_items SET done = 1 WHERE id = ?1 AND checklist_id IN (SELECT id FROM checklists WHERE card_id = ?2)",
                    params![item, card],
                )?;
            }
            events::record(&tx, by, "card_repeat_undone", At::card(pid, id, card), json!({ "due_at": p["from"] }))?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct On {
    on: bool,
}

async fn set_label(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<On>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let card = path_id(&params, "card")?;
    let label = path_id(&params, "label")?;
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let known: bool = tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM labels WHERE id = ?1 AND board_id = ?2)",
                params![label, id],
                |r| r.get(0),
            )?;
            if card_in_board(&tx, id, card, false)?.is_none() || !known {
                return Ok(Err(ApiError::NotFound));
            }

            let n = if body.on {
                tx.execute(
                    "INSERT INTO card_labels (card_id, label_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                    params![card, label],
                )?
            } else {
                tx.execute(
                    "DELETE FROM card_labels WHERE card_id = ?1 AND label_id = ?2",
                    params![card, label],
                )?
            };
            if n > 0 {
                let kind = if body.on { "label_added" } else { "label_removed" };
                events::record(&tx, by, kind, At::card(pid, id, card), json!({ "label": label }))?;
            }
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_member(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<On>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let card = path_id(&params, "card")?;
    let user = path_id(&params, "user")?;
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if card_in_board(&tx, id, card, false)?.is_none() {
                return Ok(Err(ApiError::NotFound));
            }

            let n = if body.on {
                if !can_be_assigned(&tx, id, user)? {
                    return Ok(Err(bad("choose a project owner or member who can see this board")));
                }

                tx.execute(
                    "INSERT INTO card_members (card_id, user_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                    params![card, user],
                )?
            } else {
                tx.execute(
                    "DELETE FROM card_members WHERE card_id = ?1 AND user_id = ?2",
                    params![card, user],
                )?
            };
            if n > 0 {
                let kind = if body.on { "member_assigned" } else { "member_unassigned" };
                events::record(&tx, by, kind, At::card(pid, id, card), json!({ "user": user }))?;
                if body.on {
                    crate::notifications::notify(&tx, user, by, id, card, crate::notifications::Kind::Assigned)?;
                }
            }
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct LabelBody {
    name: Option<String>,
    color: Option<i64>,
}

fn label_name(raw: Option<&str>) -> Result<String, ApiError> {
    let n = raw.unwrap_or("").trim();
    if n.chars().count() > 40 || n.chars().any(char::is_control) {
        return Err(bad("label names are at most 40 characters"));
    }

    Ok(n.to_string())
}

async fn add_label(
    board: Board,
    headers: HeaderMap,
    Json(body): Json<LabelBody>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let name = label_name(body.name.as_deref())?;
    let color = body
        .color
        .filter(|c| (0..LABEL_COLORS).contains(c))
        .ok_or_else(|| bad("choose a colour"))?;
    let at = board.at();
    let id = board.id;
    let label = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let pos: i64 = tx.query_row(
                "SELECT coalesce(max(position), 0) + 1 FROM labels WHERE board_id = ?1",
                [id],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO labels (board_id, name, color, position) VALUES (?1, ?2, ?3, ?4)",
                params![id, name, color, pos],
            )?;
            let label = tx.last_insert_rowid();
            events::record(
                &tx,
                by,
                "label_created",
                at,
                json!({ "label": label, "name": name }),
            )?;
            tx.commit()?;
            Ok(label)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": label }))))
}

async fn edit_label(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<LabelBody>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let label = path_id(&params, "label")?;
    let name = body
        .name
        .as_deref()
        .map(|n| label_name(Some(n)))
        .transpose()?;
    let color = match body.color {
        Some(c) if (0..LABEL_COLORS).contains(&c) => Some(c),
        Some(_) => return Err(bad("choose a colour")),
        None => None,
    };
    let (id, at) = (board.id, board.at());
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let n = tx.execute(
                "UPDATE labels SET name = coalesce(?3, name), color = coalesce(?4, color) WHERE id = ?1 AND board_id = ?2",
                params![label, id, name, color],
            )?;
            if n == 0 {
                return Ok(Err(ApiError::NotFound));
            }

            events::record(&tx, by, "label_changed", at, json!({ "label": label }))?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_label(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let label = path_id(&params, "label")?;
    let (id, at) = (board.id, board.at());
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let n = tx.execute(
                "DELETE FROM labels WHERE id = ?1 AND board_id = ?2",
                params![label, id],
            )?;
            if n == 0 {
                return Ok(Err(ApiError::NotFound));
            }

            events::record(&tx, by, "label_deleted", at, json!({ "label": label }))?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct TitleBody {
    title: String,
}

async fn add_checklist(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<TitleBody>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let card = path_id(&params, "card")?;
    let t = short(&body.title, "a checklist title", 200)?;
    let (id, pid) = (board.id, board.project.id);
    let new = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if card_in_board(&tx, id, card, false)?.is_none() {
                return Ok(Err(ApiError::NotFound));
            }

            let last: Option<String> = tx.query_row(
                "SELECT max(position) FROM checklists WHERE card_id = ?1",
                [card],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO checklists (card_id, title, position) VALUES (?1, ?2, ?3)",
                params![card, t, order::after(last.as_deref())],
            )?;
            let new = tx.last_insert_rowid();
            events::record(
                &tx,
                by,
                "checklist_added",
                At::card(pid, id, card),
                json!({ "checklist": new, "title": t }),
            )?;
            tx.commit()?;
            Ok(Ok(new))
        })
        .await??;
    Ok((StatusCode::CREATED, Json(json!({ "id": new }))))
}

fn checklist_card(c: &Connection, board: i64, checklist: i64) -> rusqlite::Result<Option<i64>> {
    c.query_row(
        "SELECT k.id FROM checklists x JOIN cards k ON k.id = x.card_id
         WHERE x.id = ?1 AND k.board_id = ?2 AND k.archived_at IS NULL",
        params![checklist, board],
        |r| r.get(0),
    )
    .optional()
}

fn item_card(c: &Connection, board: i64, item: i64) -> rusqlite::Result<Option<(i64, i64)>> {
    c.query_row(
        "SELECT k.id, x.id FROM checklist_items i JOIN checklists x ON x.id = i.checklist_id
         JOIN cards k ON k.id = x.card_id
         WHERE i.id = ?1 AND k.board_id = ?2 AND k.archived_at IS NULL",
        params![item, board],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

async fn edit_checklist(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<TitleBody>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let checklist = path_id(&params, "checklist")?;
    let t = short(&body.title, "a checklist title", 200)?;
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(card) = checklist_card(&tx, id, checklist)? else {
                return Ok(Err(ApiError::NotFound));
            };
            tx.execute(
                "UPDATE checklists SET title = ?2 WHERE id = ?1",
                params![checklist, t],
            )?;
            events::record(
                &tx,
                by,
                "checklist_renamed",
                At::card(pid, id, card),
                json!({ "checklist": checklist, "title": t }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_checklist(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let checklist = path_id(&params, "checklist")?;
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(card) = checklist_card(&tx, id, checklist)? else {
                return Ok(Err(ApiError::NotFound));
            };
            tx.execute("DELETE FROM checklists WHERE id = ?1", [checklist])?;
            events::record(
                &tx,
                by,
                "checklist_deleted",
                At::card(pid, id, card),
                json!({ "checklist": checklist }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct After {
    after: Option<i64>,
}

fn place(
    c: &Connection,
    table: &str,
    parent_col: &str,
    parent: i64,
    after: Option<i64>,
    me: i64,
) -> Result<String, ApiError> {
    match after {
        None => {
            let first: Option<String> = c.query_row(
                &format!("SELECT min(position) FROM {table} WHERE {parent_col} = ?1 AND id != ?2"),
                params![parent, me],
                |r| r.get(0),
            )?;
            Ok(order::between(None, first.as_deref()))
        }
        Some(a) => {
            let pos: Option<String> = c
                .query_row(
                    &format!("SELECT position FROM {table} WHERE id = ?1 AND {parent_col} = ?2"),
                    params![a, parent],
                    |r| r.get(0),
                )
                .optional()?;
            let pos = pos.ok_or_else(|| bad("that is not on this card"))?;
            let next: Option<String> = c.query_row(
                &format!("SELECT min(position) FROM {table} WHERE {parent_col} = ?1 AND position > ?2 AND id != ?3"),
                params![parent, pos, me],
                |r| r.get(0),
            )?;
            Ok(order::between(Some(&pos), next.as_deref()))
        }
    }
}

async fn move_checklist(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<After>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let checklist = path_id(&params, "checklist")?;
    if body.after == Some(checklist) {
        return Err(bad("a checklist cannot move after itself"));
    }

    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(card) = checklist_card(&tx, id, checklist)? else {
                return Ok(Err(ApiError::NotFound));
            };
            let pos = match place(&tx, "checklists", "card_id", card, body.after, checklist) {
                Ok(p) => p,
                Err(e) => return Ok(Err(e)),
            };
            tx.execute(
                "UPDATE checklists SET position = ?2 WHERE id = ?1",
                params![checklist, pos],
            )?;
            events::record(
                &tx,
                by,
                "checklist_moved",
                At::card(pid, id, card),
                json!({ "checklist": checklist, "after": body.after }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct ItemBody {
    text: String,
}

async fn add_item(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<ItemBody>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let checklist = path_id(&params, "checklist")?;
    let text = short(&body.text, "an item", 500)?;
    let (id, pid) = (board.id, board.project.id);
    let item = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(card) = checklist_card(&tx, id, checklist)? else {
                return Ok(Err(ApiError::NotFound));
            };
            let last: Option<String> = tx.query_row(
                "SELECT max(position) FROM checklist_items WHERE checklist_id = ?1",
                [checklist],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO checklist_items (checklist_id, text, position) VALUES (?1, ?2, ?3)",
                params![checklist, text, order::after(last.as_deref())],
            )?;
            let item = tx.last_insert_rowid();
            events::record(
                &tx,
                by,
                "item_added",
                At::card(pid, id, card),
                json!({ "item": item, "text": text }),
            )?;
            tx.commit()?;
            Ok(Ok(item))
        })
        .await??;
    Ok((StatusCode::CREATED, Json(json!({ "id": item }))))
}

fn can_be_assigned(c: &Connection, board: i64, user: i64) -> rusqlite::Result<bool> {
    c.query_row(
        &format!(
            "SELECT EXISTS (SELECT 1 FROM project_members m JOIN boards b ON b.project_id = m.project_id
             WHERE b.id = ?1 AND m.user_id = ?2 AND m.role != 'observer' AND {})",
            crate::boards::VISIBLE
        ),
        params![board, user],
        |r| r.get(0),
    )
}

async fn edit_item(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<Map<String, Value>>,
) -> Result<StatusCode, ApiError> {
    use rusqlite::types::Value as Sql;
    let by = board.writing(&headers, Act::Edit)?;
    let item = path_id(&params, "item")?;
    let mut sets: Vec<(&'static str, Sql)> = Vec::new();
    let mut kinds = Vec::new();
    let mut payload = json!({ "item": item });
    let mut assignee = None;
    for (key, value) in &body {
        if value.is_null() && matches!(key.as_str(), "text" | "done") {
            continue;
        }

        match key.as_str() {
            "text" => {
                let t = short(
                    value.as_str().ok_or_else(|| bad("enter the item text"))?,
                    "an item",
                    500,
                )?;
                sets.push(("text", Sql::Text(t)));
                kinds.push("item_changed");
            }
            "done" => {
                let d = value
                    .as_bool()
                    .ok_or_else(|| bad("done is true or false"))?;
                sets.push(("done", Sql::Integer(d as i64)));
                kinds.push(if d { "item_checked" } else { "item_unchecked" });
            }
            "assignee" => {
                let a = match value {
                    Value::Null => None,
                    Value::Number(n) => Some(n.as_i64().ok_or_else(|| bad("choose a person"))?),
                    _ => return Err(bad("choose a person")),
                };
                assignee = Some(a);
                payload["user"] = json!(a);
                sets.push(("assignee_id", a.map_or(Sql::Null, Sql::Integer)));
                kinds.push(if a.is_some() {
                    "item_assigned"
                } else {
                    "item_unassigned"
                });
            }
            "due_at" => {
                let d = date(value)?;
                payload["due_at"] = json!(d);
                sets.push(("due_at", d.map_or(Sql::Null, Sql::Integer)));
                kinds.push("item_dated");
            }
            _ => return Err(bad(&format!("{key} cannot be changed here"))),
        }
    }
    if sets.is_empty() {
        return Err(bad("nothing to change"));
    }

    let kind = if kinds.len() == 1 {
        kinds[0]
    } else {
        "item_changed"
    };
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some((card, _)) = item_card(&tx, id, item)? else {
                return Ok(Err(ApiError::NotFound));
            };
            let (before, text): (Option<i64>, String) = tx.query_row(
                "SELECT assignee_id, text FROM checklist_items WHERE id = ?1",
                [item],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let newly = assignee.flatten().filter(|u| Some(*u) != before);
            if let Some(user) = newly
                && !can_be_assigned(&tx, id, user)?
            {
                return Ok(Err(bad(
                    "choose a project owner or member who can see this board",
                )));
            }

            let columns = sets
                .iter()
                .enumerate()
                .map(|(i, (col, _))| format!("{col} = ?{}", i + 2))
                .collect::<Vec<_>>()
                .join(", ");
            let mut values: Vec<Sql> = vec![Sql::Integer(item)];
            values.extend(sets.into_iter().map(|(_, v)| v));
            tx.execute(
                &format!("UPDATE checklist_items SET {columns} WHERE id = ?1"),
                rusqlite::params_from_iter(values),
            )?;
            events::record(&tx, by, kind, At::card(pid, id, card), payload)?;
            if let Some(user) = newly {
                let text = body
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map_or(text, str::to_string);
                crate::notifications::notify(
                    &tx,
                    user,
                    by,
                    id,
                    card,
                    crate::notifications::Kind::ItemAssigned(text),
                )?;
            }
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_item(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let item = path_id(&params, "item")?;
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some((card, _)) = item_card(&tx, id, item)? else {
                return Ok(Err(ApiError::NotFound));
            };
            tx.execute("DELETE FROM checklist_items WHERE id = ?1", [item])?;
            events::record(
                &tx,
                by,
                "item_deleted",
                At::card(pid, id, card),
                json!({ "item": item }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

async fn move_item(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<After>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let item = path_id(&params, "item")?;
    if body.after == Some(item) {
        return Err(bad("an item cannot move after itself"));
    }

    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some((card, checklist)) = item_card(&tx, id, item)? else {
                return Ok(Err(ApiError::NotFound));
            };
            let pos = match place(
                &tx,
                "checklist_items",
                "checklist_id",
                checklist,
                body.after,
                item,
            ) {
                Ok(p) => p,
                Err(e) => return Ok(Err(e)),
            };
            tx.execute(
                "UPDATE checklist_items SET position = ?2 WHERE id = ?1",
                params![item, pos],
            )?;
            events::record(
                &tx,
                by,
                "item_moved",
                At::card(pid, id, card),
                json!({ "item": item, "after": body.after }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct CommentBody {
    body: String,
}

fn comment_text(raw: &str) -> Result<String, ApiError> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(bad("write a comment first"));
    }

    if t.chars().count() > 10_000 {
        return Err(bad("the comment is too long"));
    }

    Ok(t.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect())
}

async fn add_comment(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<CommentBody>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let card = path_id(&params, "card")?;
    let text = comment_text(&body.body)?;
    let (id, pid) = (board.id, board.project.id);
    let comment = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if card_in_board(&tx, id, card, false)?.is_none() {
                return Ok(Err(ApiError::NotFound));
            }

            tx.execute(
                "INSERT INTO comments (card_id, user_id, body) VALUES (?1, ?2, ?3)",
                params![card, by, text],
            )?;
            let comment = tx.last_insert_rowid();
            events::record(
                &tx,
                by,
                "comment_added",
                At::card(pid, id, card),
                json!({ "comment": comment }),
            )?;
            crate::notifications::mentioned_in_comment(&tx, id, card, comment, by, &text, None)?;
            tx.commit()?;
            Ok(Ok(comment))
        })
        .await??;
    Ok((StatusCode::CREATED, Json(json!({ "id": comment }))))
}

fn own_comment(c: &Connection, board: i64, comment: i64, user: i64) -> Result<i64, ApiError> {
    let found: Option<(i64, Option<i64>)> = c
        .query_row(
            "SELECT k.id, m.user_id FROM comments m JOIN cards k ON k.id = m.card_id
             WHERE m.id = ?1 AND k.board_id = ?2",
            params![comment, board],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match found {
        None => Err(ApiError::NotFound),
        Some((_, author)) if author != Some(user) => Err(ApiError::Forbidden),
        Some((card, _)) => Ok(card),
    }
}

async fn edit_comment(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<CommentBody>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let comment = path_id(&params, "comment")?;
    let text = comment_text(&body.body)?;
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let card = match own_comment(&tx, id, comment, by) {
                Ok(card) => card,
                Err(e) => return Ok(Err(e)),
            };
            let before: String =
                tx.query_row("SELECT body FROM comments WHERE id = ?1", [comment], |r| {
                    r.get(0)
                })?;
            tx.execute(
                "UPDATE comments SET body = ?2, edited_at = ?3 WHERE id = ?1",
                params![comment, text, now()],
            )?;
            crate::notifications::mentioned_in_comment(
                &tx,
                id,
                card,
                comment,
                by,
                &text,
                Some(&before),
            )?;
            events::record(
                &tx,
                by,
                "comment_edited",
                At::card(pid, id, card),
                json!({ "comment": comment }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_comment(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let comment = path_id(&params, "comment")?;
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let card = match own_comment(&tx, id, comment, by) {
                Ok(card) => card,
                Err(e) => return Ok(Err(e)),
            };
            tx.execute("DELETE FROM comments WHERE id = ?1", [comment])?;
            events::record(
                &tx,
                by,
                "comment_deleted",
                At::card(pid, id, card),
                json!({ "comment": comment }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}
