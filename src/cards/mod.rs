pub mod detail;
pub mod fields;
pub mod links;
pub mod reactions;
pub mod repeat;
pub mod votes;

use std::collections::HashMap;

use axum::Router;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::post;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::COMPANY_PREFIX;
use crate::boards::Board;
use crate::error::{ApiError, bad};
use crate::events::{self, At};
use crate::order;
use crate::projects::{Act, path_id};
use crate::session::now;
use crate::state::AppState;

pub const MAX_TITLE: usize = 500;
pub const MAX_PASTE: usize = 100;

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}/boards/{{board}}{s}");
    Router::new()
        .route(&p("/cards"), post(create))
        .route(&p("/cards/{card}/move"), post(move_card))
        .route(&p("/cards/{card}/archive"), post(archive))
        .route(&p("/cards/{card}/restore"), post(restore))
        .route(&p("/cards/{card}/copy"), post(copy))
        .route(&p("/lists/{list}/sort"), post(sort))
        .route(&p("/lists/{list}/move-cards"), post(move_all))
        .route(&p("/lists/{list}/archive-cards"), post(archive_all))
        .merge(detail::routes())
        .merge(fields::routes())
        .merge(links::routes())
        .merge(votes::routes())
        .merge(reactions::routes())
}

pub fn title(raw: &str) -> Result<String, ApiError> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(bad("enter a card title"));
    }

    if t.chars().count() > MAX_TITLE || t.chars().any(|c| c.is_control() && c != '\n') {
        return Err(bad("the card title is too long"));
    }

    Ok(t.replace('\n', " "))
}

pub fn open_list(c: &Connection, board: i64, list: i64) -> rusqlite::Result<bool> {
    c.query_row(
        "SELECT EXISTS (SELECT 1 FROM lists WHERE id = ?1 AND board_id = ?2 AND archived_at IS NULL)",
        params![list, board],
        |r| r.get(0),
    )
}

pub struct CardRow {
    pub list: i64,
    pub title: String,
    pub position: String,
}

pub fn card_in_board(
    c: &Connection,
    board: i64,
    card: i64,
    archived: bool,
) -> rusqlite::Result<Option<CardRow>> {
    c.query_row(
        "SELECT list_id, title, position FROM cards
         WHERE id = ?1 AND board_id = ?2 AND (archived_at IS NOT NULL) = ?3",
        params![card, board, archived],
        |r| {
            Ok(CardRow {
                list: r.get(0)?,
                title: r.get(1)?,
                position: r.get(2)?,
            })
        },
    )
    .optional()
}

pub fn end_of_list(c: &Connection, list: i64) -> rusqlite::Result<String> {
    let last: Option<String> = c.query_row(
        "SELECT max(position) FROM cards WHERE list_id = ?1",
        [list],
        |r| r.get(0),
    )?;
    Ok(order::after(last.as_deref()))
}

pub fn top_of_list(c: &Connection, list: i64, except: Option<i64>) -> rusqlite::Result<String> {
    let first: Option<String> = c.query_row(
        "SELECT min(position) FROM cards WHERE list_id = ?1 AND archived_at IS NULL AND id IS NOT ?2",
        params![list, except],
        |r| r.get(0),
    )?;
    Ok(order::between(None, first.as_deref()))
}

pub fn after_card(
    c: &Connection,
    list: i64,
    after: i64,
    except: Option<i64>,
) -> Result<String, ApiError> {
    let pos: Option<String> = c
        .query_row(
            "SELECT position FROM cards WHERE id = ?1 AND list_id = ?2 AND archived_at IS NULL",
            params![after, list],
            |r| r.get(0),
        )
        .optional()?;
    let pos = pos.ok_or_else(|| bad("that card is not in this list"))?;
    let next: Option<String> = c.query_row(
        "SELECT min(position) FROM cards WHERE list_id = ?1 AND position > ?2 AND id IS NOT ?3",
        params![list, pos, except],
        |r| r.get(0),
    )?;
    Ok(order::between(Some(&pos), next.as_deref()))
}

pub fn summaries(c: &Connection, board: i64) -> rusqlite::Result<HashMap<i64, Vec<Value>>> {
    let ids = |sql: &str| -> rusqlite::Result<HashMap<i64, Vec<i64>>> {
        let mut stmt = c.prepare(sql)?;
        let mut out: HashMap<i64, Vec<i64>> = HashMap::new();
        for row in stmt.query_map([board], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
            let (card, v) = row?;
            out.entry(card).or_default().push(v);
        }
        Ok(out)
    };
    let mut labels = ids(
        "SELECT cl.card_id, cl.label_id FROM card_labels cl JOIN cards k ON k.id = cl.card_id
         JOIN labels l ON l.id = cl.label_id WHERE k.board_id = ?1 ORDER BY l.position, l.id",
    )?;
    let mut members = ids(
        "SELECT cm.card_id, cm.user_id FROM card_members cm JOIN cards k ON k.id = cm.card_id WHERE k.board_id = ?1",
    )?;
    let mut stmt = c.prepare(
        "SELECT k.id, k.list_id, k.title, k.position, k.description != '', k.start_at, k.due_at, k.done, k.cover,
                (SELECT count(*) FROM comments m WHERE m.card_id = k.id),
                (SELECT count(*) FROM checklist_items i JOIN checklists x ON x.id = i.checklist_id WHERE x.card_id = k.id),
                (SELECT count(*) FROM checklist_items i JOIN checklists x ON x.id = i.checklist_id WHERE x.card_id = k.id AND i.done = 1),
                (SELECT count(*) FROM attachments a WHERE a.card_id = k.id), k.cover_attachment, k.is_template, k.updated_at, k.recurrence
         FROM cards k WHERE k.board_id = ?1 AND k.archived_at IS NULL ORDER BY k.list_id, k.position",
    )?;
    let mut out: HashMap<i64, Vec<Value>> = HashMap::new();
    let rows = stmt.query_map([board], |r| {
        let id: i64 = r.get(0)?;
        Ok((
            r.get::<_, i64>(1)?,
            id,
            json!({
                "id": id,
                "title": r.get::<_, String>(2)?,
                "position": r.get::<_, String>(3)?,
                "has_description": r.get::<_, bool>(4)?,
                "start_at": r.get::<_, Option<i64>>(5)?,
                "due_at": r.get::<_, Option<i64>>(6)?,
                "done": r.get::<_, bool>(7)?,
                "cover": r.get::<_, Option<String>>(8)?,
                "comments": r.get::<_, i64>(9)?,
                "items": r.get::<_, i64>(10)?,
                "items_done": r.get::<_, i64>(11)?,
                "attachments": r.get::<_, i64>(12)?,
                "cover_image": r.get::<_, Option<i64>>(13)?,
                "is_template": r.get::<_, bool>(14)?,
                "updated_at": r.get::<_, i64>(15)?,
                "recurrence": r.get::<_, Option<String>>(16)?,
            }),
        ))
    })?;
    let mut front = fields::board_values(c, board)?;
    for row in rows {
        let (list, id, mut card) = row?;
        if let Some(f) = front.remove(&id) {
            card["fields"] = Value::Object(f);
        }
        card["labels"] = json!(labels.remove(&id).unwrap_or_default());
        card["members"] = json!(members.remove(&id).unwrap_or_default());
        out.entry(list).or_default().push(card);
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
pub fn copy_card(
    c: &Connection,
    card: i64,
    list: i64,
    board: i64,
    title: &str,
    position: &str,
    by: Option<i64>,
    labels: Option<&HashMap<i64, i64>>,
) -> rusqlite::Result<i64> {
    c.execute(
        "INSERT INTO cards (board_id, list_id, title, position, created_by, description, description_text, start_at, due_at, done, cover, recurrence)
         SELECT ?2, ?3, ?4, ?5, ?6, description, description_text, start_at, due_at, done, cover, recurrence FROM cards WHERE id = ?1",
        params![card, board, list, title, position, by],
    )?;
    let new = c.last_insert_rowid();
    let old_labels: Vec<i64> = {
        let mut stmt = c.prepare("SELECT label_id FROM card_labels WHERE card_id = ?1")?;
        stmt.query_map([card], |r| r.get(0))?
            .collect::<Result<_, _>>()?
    };
    for l in old_labels {
        let mapped = match labels {
            Some(m) => m.get(&l).copied(),
            None => Some(l),
        };
        if let Some(l) = mapped {
            c.execute(
                "INSERT INTO card_labels (card_id, label_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                params![new, l],
            )?;
        }
    }
    c.execute(
        "INSERT INTO card_members (card_id, user_id) SELECT ?2, user_id FROM card_members WHERE card_id = ?1",
        params![card, new],
    )?;
    let lists: Vec<(i64, String, String)> = {
        let mut stmt =
            c.prepare("SELECT id, title, position FROM checklists WHERE card_id = ?1")?;
        stmt.query_map([card], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?
    };
    for (old, t, pos) in lists {
        c.execute(
            "INSERT INTO checklists (card_id, title, position) VALUES (?1, ?2, ?3)",
            params![new, t, pos],
        )?;
        let id = c.last_insert_rowid();
        c.execute(
            "INSERT INTO checklist_items (checklist_id, text, done, position, assignee_id, due_at)
             SELECT ?2, text, done, position, assignee_id, due_at FROM checklist_items WHERE checklist_id = ?1",
            params![old, id],
        )?;
    }
    Ok(new)
}

pub fn copy_cards(
    c: &Connection,
    from: i64,
    to: i64,
    labels: Option<&HashMap<i64, i64>>,
) -> rusqlite::Result<()> {
    let board: i64 = c.query_row("SELECT board_id FROM lists WHERE id = ?1", [to], |r| {
        r.get(0)
    })?;
    let cards: Vec<(i64, String, String, Option<i64>)> = {
        let mut stmt = c.prepare(
            "SELECT id, title, position, created_by FROM cards WHERE list_id = ?1 AND archived_at IS NULL ORDER BY position",
        )?;
        stmt.query_map([from], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<_, _>>()?
    };
    for (id, title, pos, by) in cards {
        copy_card(c, id, to, board, &title, &pos, by, labels)?;
    }
    Ok(())
}

pub fn copy_labels(c: &Connection, from: i64, to: i64) -> rusqlite::Result<HashMap<i64, i64>> {
    let rows: Vec<(i64, String, i64, i64)> = {
        let mut stmt =
            c.prepare("SELECT id, name, color, position FROM labels WHERE board_id = ?1")?;
        stmt.query_map([from], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<_, _>>()?
    };
    let mut map = HashMap::new();
    for (id, name, color, pos) in rows {
        c.execute(
            "INSERT INTO labels (board_id, name, color, position) VALUES (?1, ?2, ?3, ?4)",
            params![to, name, color, pos],
        )?;
        map.insert(id, c.last_insert_rowid());
    }
    Ok(map)
}

fn card_param(params: &HashMap<String, String>) -> Result<i64, ApiError> {
    path_id(params, "card")
}

#[derive(Deserialize)]
struct NewCards {
    list_id: i64,
    title: Option<String>,
    titles: Option<Vec<String>>,
    items: Option<Vec<NewCard>>,
    #[serde(default)]
    top: bool,
}

#[derive(Deserialize)]
struct NewCard {
    title: String,
    #[serde(default)]
    labels: Vec<i64>,
    #[serde(default)]
    members: Vec<i64>,
    due_at: Option<i64>,
}

const MAX_QUICK: usize = 20;

async fn create(
    board: Board,
    headers: HeaderMap,
    Json(body): Json<NewCards>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let plain = |t: &String| NewCard {
        title: t.clone(),
        labels: vec![],
        members: vec![],
        due_at: None,
    };
    let raw: Vec<NewCard> = match (&body.title, &body.titles, body.items) {
        (Some(t), None, None) => vec![plain(t)],
        (None, Some(ts), None) => ts
            .iter()
            .filter(|t| !t.trim().is_empty())
            .map(plain)
            .collect(),
        (None, None, Some(items)) => items
            .into_iter()
            .filter(|c| !c.title.trim().is_empty())
            .collect(),
        _ => return Err(bad("send a title, a list of titles or a list of cards")),
    };
    if raw.is_empty() {
        return Err(bad("enter a card title"));
    }

    if raw.len() > MAX_PASTE {
        return Err(bad("paste at most 100 lines at once"));
    }

    let mut cards = Vec::with_capacity(raw.len());
    for c in raw {
        if c.labels.len() > MAX_QUICK || c.members.len() > MAX_QUICK {
            return Err(bad("add at most 20 labels and 20 people at once"));
        }

        if c.due_at.is_some_and(|d| !detail::valid_time(d)) {
            return Err(bad("that date is not valid"));
        }

        cards.push(NewCard {
            title: title(&c.title)?,
            ..c
        });
    }

    let (id, pid, list, top) = (board.id, board.project.id, body.list_id, body.top);
    let created = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if !open_list(&tx, id, list)? {
                return Ok(Err(bad("that list is not on this board")));
            }

            for card in &cards {
                for label in &card.labels {
                    if !detail::label_on_board(&tx, id, *label)? {
                        return Ok(Err(bad("that label is not on this board")));
                    }
                }
                for user in &card.members {
                    if !detail::can_be_assigned(&tx, id, *user)? {
                        return Ok(Err(bad("choose a project owner or member who can see this board")));
                    }
                }
            }

            let mut created = Vec::new();
            let mut prev: Option<String> = None;
            let first_existing: Option<String> = if top {
                tx.query_row(
                    "SELECT min(position) FROM cards WHERE list_id = ?1 AND archived_at IS NULL",
                    [list],
                    |r| r.get(0),
                )?
            } else {
                None
            };
            for new in &cards {
                let pos = if top {
                    order::between(prev.as_deref(), first_existing.as_deref())
                } else {
                    end_of_list(&tx, list)?
                };
                tx.execute(
                    "INSERT INTO cards (board_id, list_id, title, position, created_by, due_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![id, list, new.title, pos, by, new.due_at],
                )?;
                let card = tx.last_insert_rowid();
                let at = At::card(pid, id, card);
                for label in &new.labels {
                    if tx.execute(
                        "INSERT INTO card_labels (card_id, label_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                        params![card, label],
                    )? > 0
                    {
                        events::record(&tx, by, "label_added", at, json!({ "label": label }))?;
                    }
                }
                for user in &new.members {
                    if tx.execute(
                        "INSERT INTO card_members (card_id, user_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                        params![card, user],
                    )? > 0
                    {
                        events::record(&tx, by, "member_assigned", at, json!({ "user": user }))?;
                        crate::notifications::notify(&tx, *user, by, id, card, crate::notifications::Kind::Assigned)?;
                    }
                }
                created.push(json!({ "id": card, "title": new.title, "position": pos, "list_id": list }));
                prev = Some(pos);
            }
            let ids: Vec<&Value> = created.iter().map(|c| &c["id"]).collect();
            for c in &created {
                crate::notifications::card_added(&tx, id, c["id"].as_i64().unwrap_or_default(), by)?;
            }
            if created.len() == 1 {
                let card = created[0]["id"].as_i64().unwrap_or_default();
                events::record(&tx, by, "card_created", At::card(pid, id, card), json!({ "list": list, "title": cards[0].title }))?;
            } else {
                events::record(&tx, by, "cards_created", At::board(pid, id), json!({ "list": list, "cards": ids }))?;
            }
            tx.commit()?;
            Ok(Ok(created))
        })
        .await??;
    Ok((StatusCode::CREATED, Json(json!({ "cards": created }))))
}

#[derive(Deserialize)]
struct Move {
    list_id: i64,
    after: Option<i64>,
}

async fn move_card(
    board: Board,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
    Json(body): Json<Move>,
) -> Result<Json<Value>, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let card = card_param(&params)?;
    if body.after == Some(card) {
        return Err(bad("a card cannot move after itself"));
    }

    let (id, pid) = (board.id, board.project.id);
    let pos = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(old) = card_in_board(&tx, id, card, false)? else {
                return Ok(Err(ApiError::NotFound));
            };
            if !open_list(&tx, id, body.list_id)? {
                return Ok(Err(bad("that list is not on this board")));
            }

            let pos = match body.after {
                None => top_of_list(&tx, body.list_id, Some(card))?,
                Some(after) => match after_card(&tx, body.list_id, after, Some(card)) {
                    Ok(p) => p,
                    Err(e) => return Ok(Err(e)),
                },
            };
            tx.execute(
                "UPDATE cards SET list_id = ?2, position = ?3, updated_at = ?4 WHERE id = ?1",
                params![card, body.list_id, pos, now()],
            )?;
            events::record(
                &tx,
                by,
                "card_moved",
                At::card(pid, id, card),
                json!({ "from_list": old.list, "to_list": body.list_id, "after": body.after }),
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
    let card = card_param(&params)?;
    let (id, pid) = (board.id, board.project.id);
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(row) = card_in_board(&tx, id, card, !archive)? else {
                return Ok(Err(ApiError::NotFound));
            };
            if archive {
                tx.execute(
                    "UPDATE cards SET archived_at = ?2 WHERE id = ?1",
                    params![card, now()],
                )?;
            } else {
                let list = if open_list(&tx, id, row.list)? {
                    row.list
                } else {
                    let first: Option<i64> = tx
                        .query_row(
                            "SELECT id FROM lists WHERE board_id = ?1 AND archived_at IS NULL ORDER BY position LIMIT 1",
                            [id],
                            |r| r.get(0),
                        )
                        .optional()?;
                    match first {
                        Some(l) => l,
                        None => return Ok(Err(bad("add a list before restoring cards"))),
                    }
                };
                let pos = end_of_list(&tx, list)?;
                tx.execute(
                    "UPDATE cards SET archived_at = NULL, list_id = ?2, position = ?3 WHERE id = ?1",
                    params![card, list, pos],
                )?;
            }
            let kind = if archive { "card_archived" } else { "card_restored" };
            events::record(&tx, by, kind, At::card(pid, id, card), json!({ "title": row.title }))?;
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

#[derive(Deserialize)]
struct CopyCard {
    title: String,
    list_id: i64,
}

async fn copy(
    board: Board,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
    Json(body): Json<CopyCard>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let card = card_param(&params)?;
    let t = title(&body.title)?;
    let (id, pid, list) = (board.id, board.project.id, body.list_id);
    let new = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if card_in_board(&tx, id, card, false)?.is_none() {
                return Ok(Err(ApiError::NotFound));
            }

            if !open_list(&tx, id, list)? {
                return Ok(Err(bad("that list is not on this board")));
            }

            let pos = end_of_list(&tx, list)?;
            let new = copy_card(&tx, card, list, id, &t, &pos, Some(by), None)?;
            events::record(
                &tx,
                by,
                "card_copied",
                At::card(pid, id, new),
                json!({ "from": card, "list": list }),
            )?;
            tx.commit()?;
            Ok(Ok(new))
        })
        .await??;
    Ok((StatusCode::CREATED, Json(json!({ "id": new }))))
}

#[derive(Deserialize)]
struct Sort {
    by: String,
}

async fn sort(
    board: Board,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
    Json(body): Json<Sort>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let list = path_id(&params, "list")?;
    let order_by = match body.by.as_str() {
        "name" => "title COLLATE NOCASE, id",
        "new" => "created_at DESC, id DESC",
        "old" => "created_at, id",
        "votes" if board.project.info.voting => {
            "(SELECT count(*) FROM card_votes v WHERE v.card_id = cards.id) DESC, position, id"
        }
        _ => return Err(bad("choose how to sort")),
    };
    let (id, at) = (board.id, board.at());
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if !open_list(&tx, id, list)? {
                return Ok(Err(ApiError::NotFound));
            }

            let ids: Vec<i64> = {
                let mut stmt = tx.prepare(&format!(
                    "SELECT id FROM cards WHERE list_id = ?1 AND archived_at IS NULL ORDER BY {order_by}"
                ))?;
                stmt.query_map([list], |r| r.get(0))?.collect::<Result<_, _>>()?
            };
            for (card, pos) in ids.iter().zip(order::spread(ids.len())) {
                tx.execute("UPDATE cards SET position = ?2 WHERE id = ?1", params![card, pos])?;
            }
            events::record(&tx, by, "list_sorted", at, json!({ "list": list, "by": body.by }))?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct MoveAll {
    to: i64,
}

async fn move_all(
    board: Board,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
    Json(body): Json<MoveAll>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let list = path_id(&params, "list")?;
    if list == body.to {
        return Err(bad("choose another list"));
    }

    let (id, at) = (board.id, board.at());
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if !open_list(&tx, id, list)? || !open_list(&tx, id, body.to)? {
                return Ok(Err(ApiError::NotFound));
            }

            let ids: Vec<i64> = {
                let mut stmt = tx.prepare(
                    "SELECT id FROM cards WHERE list_id = ?1 AND archived_at IS NULL ORDER BY position",
                )?;
                stmt.query_map([list], |r| r.get(0))?.collect::<Result<_, _>>()?
            };
            for card in &ids {
                let pos = end_of_list(&tx, body.to)?;
                tx.execute(
                    "UPDATE cards SET list_id = ?2, position = ?3 WHERE id = ?1",
                    params![card, body.to, pos],
                )?;
            }
            events::record(&tx, by, "cards_moved", at, json!({ "from_list": list, "to_list": body.to, "cards": ids }))?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

async fn archive_all(
    board: Board,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let list = path_id(&params, "list")?;
    let (id, at) = (board.id, board.at());
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if !open_list(&tx, id, list)? {
                return Ok(Err(ApiError::NotFound));
            }

            let ids: Vec<i64> = {
                let mut stmt =
                    tx.prepare("SELECT id FROM cards WHERE list_id = ?1 AND archived_at IS NULL")?;
                stmt.query_map([list], |r| r.get(0))?
                    .collect::<Result<_, _>>()?
            };
            tx.execute(
                "UPDATE cards SET archived_at = ?2 WHERE list_id = ?1 AND archived_at IS NULL",
                params![list, now()],
            )?;
            events::record(
                &tx,
                by,
                "cards_archived",
                at,
                json!({ "list": list, "cards": ids }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}
