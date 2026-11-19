pub mod live;
pub mod mentions;

use std::collections::HashMap;
use std::time::Duration;

use axum::Router;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::{get, post, put};
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::COMPANY_PREFIX;
use crate::auth::check_csrf;
use crate::boards::Board;
use crate::db::Database;
use crate::error::ApiError;
use crate::projects::path_id;
use crate::state::AppState;
use crate::tenant::Company;

pub const DUE_SOON: i64 = 24 * 60 * 60;
pub const LIST_LIMIT: i64 = 50;

const CAN_SEE: &str = "EXISTS (SELECT 1 FROM boards b
       JOIN projects p ON p.id = b.project_id
       JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = n.user_id
      WHERE b.id = n.board_id AND p.deleted_at IS NULL AND p.archived_at IS NULL
        AND (b.restricted = 0 OR EXISTS (SELECT 1 FROM board_access x WHERE x.board_id = b.id AND x.user_id = n.user_id)))";

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}{s}");
    Router::new()
        .route(&p("/notifications"), get(list))
        .route(&p("/notifications/read"), post(read_all))
        .route(&p("/notifications/{notification}/read"), post(read_one))
        .route(&p("/notifications/live"), get(live::live))
        .route(&p("/boards/{board}/watch"), put(watch))
}

pub enum Kind {
    Mention(i64),
    Assigned,
    CardAdded,
}

pub fn notify(
    c: &Connection,
    user: i64,
    actor: i64,
    board: i64,
    card: i64,
    kind: Kind,
) -> rusqlite::Result<bool> {
    if user == actor || !crate::realtime::allowed(c, board, user)? {
        return Ok(false);
    }

    let (name, comment) = match kind {
        Kind::Mention(comment) => ("mention", Some(comment)),
        Kind::Assigned => ("assigned", None),
        Kind::CardAdded => ("card_added", None),
    };
    c.execute(
        "INSERT INTO notifications (user_id, kind, actor_id, board_id, card_id, comment_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![user, name, actor, board, card, comment],
    )?;
    Ok(true)
}

pub fn mentioned_in_comment(
    c: &Connection,
    board: i64,
    card: i64,
    comment: i64,
    actor: i64,
    text: &str,
    before: Option<&str>,
) -> rusqlite::Result<usize> {
    let people = mentions::people(c)?;
    let old = before
        .map(|b| mentions::find(&people, b))
        .unwrap_or_default();
    let mut sent = 0;
    for user in mentions::find(&people, text) {
        if !old.contains(&user) && notify(c, user, actor, board, card, Kind::Mention(comment))? {
            sent += 1;
        }
    }
    Ok(sent)
}

pub fn card_added(c: &Connection, board: i64, card: i64, actor: i64) -> rusqlite::Result<usize> {
    let mut stmt = c.prepare("SELECT user_id FROM board_watchers WHERE board_id = ?1")?;
    let watchers: Vec<i64> = stmt
        .query_map([board], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut sent = 0;
    for user in watchers {
        if notify(c, user, actor, board, card, Kind::CardAdded)? {
            sent += 1;
        }
    }
    Ok(sent)
}

const DUE_CANDIDATES: &str = "FROM card_members m
       JOIN cards k ON k.id = m.card_id
       JOIN boards b ON b.id = k.board_id
       JOIN projects p ON p.id = b.project_id
       JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = m.user_id
      WHERE k.done = 0 AND k.archived_at IS NULL AND k.due_at > ?1 AND k.due_at <= ?1 + ?2
        AND p.deleted_at IS NULL AND p.archived_at IS NULL
        AND (b.restricted = 0 OR EXISTS (SELECT 1 FROM board_access x WHERE x.board_id = b.id AND x.user_id = m.user_id))
        AND NOT EXISTS (SELECT 1 FROM notifications o
                         WHERE o.kind = 'due' AND o.user_id = m.user_id AND o.card_id = k.id AND o.due_at = k.due_at)";

pub async fn due_soon(db: &Database, now: i64) -> Result<usize, ApiError> {
    let waiting: i64 = db
        .read(move |c| {
            Ok(c.query_row(
                &format!("SELECT count(*) {DUE_CANDIDATES}"),
                params![now, DUE_SOON],
                |r| r.get(0),
            )?)
        })
        .await?;
    if waiting == 0 {
        return Ok(0);
    }

    Ok(db
        .write(move |c| {
            Ok(c.execute(
                &format!(
                    "INSERT OR IGNORE INTO notifications (user_id, kind, board_id, card_id, due_at)
                     SELECT m.user_id, 'due', k.board_id, k.id, k.due_at {DUE_CANDIDATES}"
                ),
                params![now, DUE_SOON],
            )?)
        })
        .await?)
}

pub const DUE_CHECK_EVERY: Duration = Duration::from_secs(5 * 60);

pub async fn due_soon_all(state: &AppState, now: i64) -> usize {
    let slugs: Vec<String> = state
        .dbs
        .control()
        .read(|c| {
            let mut stmt = c.prepare("SELECT slug FROM orgs WHERE status = 'active'")?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .unwrap_or_default();
    let mut sent = 0;
    for slug in slugs {
        let Ok(db) = state.dbs.tenant(&slug) else {
            continue;
        };
        match due_soon(&db, now).await {
            Ok(n) => sent += n,
            Err(e) => eprintln!("tazko: due-date alerts failed for {slug}: {e:?}"),
        }
    }
    sent
}

pub async fn due_soon_job(state: AppState) {
    let mut tick = tokio::time::interval(DUE_CHECK_EVERY);
    loop {
        tick.tick().await;
        due_soon_all(&state, crate::session::now()).await;
    }
}

pub fn unread(c: &Connection, user: i64) -> rusqlite::Result<i64> {
    c.query_row(
        &format!("SELECT count(*) FROM notifications n WHERE n.user_id = ?1 AND n.read_at IS NULL AND {CAN_SEE}"),
        [user],
        |r| r.get(0),
    )
}

#[derive(Deserialize)]
struct ListQuery {
    unread: Option<String>,
}

fn rows(c: &Connection, user: i64, only_unread: bool) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = c.prepare(&format!(
        "SELECT n.id, n.kind, n.actor_id, coalesce(nullif(a.name, ''), a.email, ''), a.email,
                n.card_id, k.title, n.board_id, b.name, n.created_at, n.read_at IS NOT NULL, n.due_at
         FROM notifications n
         JOIN cards k ON k.id = n.card_id
         JOIN boards b ON b.id = n.board_id
         LEFT JOIN people a ON a.id = n.actor_id
         WHERE n.user_id = ?1 AND (?2 = 0 OR n.read_at IS NULL) AND {CAN_SEE}
         ORDER BY n.id DESC LIMIT ?3"
    ))?;
    stmt.query_map(params![user, only_unread, LIST_LIMIT], |r| {
        let actor: Option<i64> = r.get(2)?;
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "kind": r.get::<_, String>(1)?,
            "actor": match actor {
                Some(id) => json!({ "id": id, "name": r.get::<_, String>(3)?, "email": r.get::<_, Option<String>>(4)? }),
                None => Value::Null,
            },
            "card": { "id": r.get::<_, i64>(5)?, "title": r.get::<_, String>(6)? },
            "board": { "id": r.get::<_, i64>(7)?, "name": r.get::<_, String>(8)? },
            "created_at": r.get::<_, i64>(9)?,
            "read": r.get::<_, bool>(10)?,
            "due_at": r.get::<_, Option<i64>>(11)?,
        }))
    })?
    .collect()
}

async fn list(co: Company, Query(q): Query<ListQuery>) -> Result<Json<Value>, ApiError> {
    let user = co.user.0.user_id;
    let only_unread = q.unread.is_some_and(|v| v != "0");
    let (items, count) = co
        .db
        .read(move |c| Ok((rows(c, user, only_unread)?, unread(c, user)?)))
        .await?;
    Ok(Json(json!({ "notifications": items, "unread": count })))
}

async fn read_all(co: Company, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    check_csrf(&co.user, &headers)?;
    let user = co.user.0.user_id;
    co.db
        .write(move |c| {
            c.execute(
                "UPDATE notifications SET read_at = unixepoch() WHERE user_id = ?1 AND read_at IS NULL",
                [user],
            )?;
            Ok(())
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn read_one(
    co: Company,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    check_csrf(&co.user, &headers)?;
    let user = co.user.0.user_id;
    let id = path_id(&params, "notification")?;
    let n = co
        .db
        .write(move |c| {
            Ok(c.execute(
                "UPDATE notifications SET read_at = coalesce(read_at, unixepoch()) WHERE id = ?1 AND user_id = ?2",
                params![id, user],
            )?)
        })
        .await?;
    if n == 0 {
        return Err(ApiError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct On {
    on: bool,
}

async fn watch(
    board: Board,
    headers: HeaderMap,
    Json(body): Json<On>,
) -> Result<Json<Value>, ApiError> {
    check_csrf(&board.project.company.user, &headers)?;
    let (id, user) = (board.id, board.user_id());
    board
        .project
        .db()
        .write(move |c| {
            if body.on {
                c.execute(
                    "INSERT INTO board_watchers (board_id, user_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                    params![id, user],
                )?;
            } else {
                c.execute(
                    "DELETE FROM board_watchers WHERE board_id = ?1 AND user_id = ?2",
                    params![id, user],
                )?;
            }
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "watching": body.on })))
}
