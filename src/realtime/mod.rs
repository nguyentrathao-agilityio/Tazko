pub mod hub;

use std::time::Duration;

use axum::Router;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, header};
use axum::response::Response;
use axum::routing::get;
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::COMPANY_PREFIX;
use crate::boards::{Board, VISIBLE};
use crate::db::Database;
use crate::error::ApiError;
use crate::state::AppState;
use hub::Viewer;

pub const CATCH_UP_LIMIT: i64 = 500;
const PING_EVERY: Duration = Duration::from_secs(25);

pub fn routes() -> Router<AppState> {
    Router::new().route(
        &format!("{COMPANY_PREFIX}/boards/{{board}}/live"),
        get(live),
    )
}

#[derive(Deserialize)]
struct After {
    after: Option<i64>,
}

pub fn origin_allowed(headers: &HeaderMap, public_url: &str) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let origin = origin.trim_end_matches('/');
    if origin.eq_ignore_ascii_case(public_url.trim_end_matches('/')) {
        return true;
    }

    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let bare = origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"))
        .unwrap_or("");
    !host.is_empty() && bare.eq_ignore_ascii_case(host)
}

async fn live(
    State(state): State<AppState>,
    board: Board,
    headers: HeaderMap,
    Query(q): Query<After>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    if !origin_allowed(&headers, &state.settings.public_url) {
        return Err(ApiError::Forbidden);
    }

    let session = crate::session::cookie_token(&headers)
        .map(|t| crate::session::hash_token(&t))
        .ok_or(ApiError::SignIn)?;
    Ok(ws.on_upgrade(move |socket| run(state, board, session, q.after, socket)))
}

pub fn allowed(c: &Connection, board: i64, user: i64) -> rusqlite::Result<bool> {
    c.query_row(
        &format!(
            "SELECT EXISTS (SELECT 1 FROM boards b
               JOIN projects p ON p.id = b.project_id
               JOIN project_members m ON m.project_id = p.id AND m.user_id = ?2
              WHERE b.id = ?1 AND p.deleted_at IS NULL AND p.archived_at IS NULL AND {VISIBLE})"
        ),
        params![board, user],
        |r| r.get(0),
    )
}

pub fn signed_in(c: &Connection, session: &str, org: i64, user: i64) -> rusqlite::Result<bool> {
    c.query_row(
        "SELECT EXISTS (SELECT 1 FROM sessions s
           JOIN users u ON u.id = s.user_id
           JOIN org_members m ON m.user_id = u.id AND m.org_id = ?2
           JOIN orgs o ON o.id = m.org_id
          WHERE s.id_hash = ?1 AND s.user_id = ?3 AND s.expires_at > ?4 AND u.disabled = 0 AND o.status = 'active')",
        params![session, org, user, crate::session::now()],
        |r| r.get(0),
    )
}

pub fn events_after(
    c: &Connection,
    board: i64,
    after: i64,
    limit: i64,
) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = c.prepare(
        "SELECT e.id, e.type, e.actor_id, coalesce(nullif(p.name, ''), p.email, ''), e.card_id, e.created_at
         FROM events e LEFT JOIN people p ON p.id = e.actor_id
         WHERE e.board_id = ?1 AND e.id > ?2 AND e.type NOT IN ('board_starred', 'board_unstarred')
         ORDER BY e.id LIMIT ?3",
    )?;
    stmt.query_map(params![board, after, limit], |r| {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "type": r.get::<_, String>(1)?,
            "actor": { "id": r.get::<_, Option<i64>>(2)?, "name": r.get::<_, String>(3)? },
            "card": r.get::<_, Option<i64>>(4)?,
            "created_at": r.get::<_, i64>(5)?,
        }))
    })?
    .collect()
}

pub fn last_event(c: &Connection) -> rusqlite::Result<i64> {
    c.query_row("SELECT coalesce(max(id), 0) FROM events", [], |r| r.get(0))
}

async fn send(socket: &mut WebSocket, v: Value) -> bool {
    socket
        .send(Message::Text(Utf8Bytes::from(v.to_string())))
        .await
        .is_ok()
}

enum Step {
    Events(Vec<Value>, i64),
    Reset(i64),
    Gone,
}

async fn fetch(db: &Database, board: i64, user: i64, after: i64) -> Option<Step> {
    db.read(move |c| {
        if !allowed(c, board, user)? {
            return Ok(Step::Gone);
        }

        let events = events_after(c, board, after, CATCH_UP_LIMIT + 1)?;
        if events.len() as i64 > CATCH_UP_LIMIT {
            return Ok(Step::Reset(last_event(c)?));
        }

        let last = events
            .last()
            .and_then(|e| e["id"].as_i64())
            .unwrap_or(after.max(0));
        Ok(Step::Events(events, last))
    })
    .await
    .ok()
}

async fn run(
    state: AppState,
    board: Board,
    session: String,
    after: Option<i64>,
    mut socket: WebSocket,
) {
    let db = board.project.db().clone();
    let user = board.user_id();
    let (id, slug) = (board.id, board.project.company.slug.clone());
    let me = &board.project.company.user.0;
    let viewer = Viewer {
        id: user,
        name: me.name.clone(),
        email: me.email.clone(),
    };
    let org = board.project.company.org_id;
    let control = state.dbs.control().clone();
    let still_in = move || {
        let (control, session) = (control.clone(), session.clone());
        async move {
            control
                .read(move |c| Ok(signed_in(c, &session, org, user)?))
                .await
                .unwrap_or(false)
        }
    };
    let mut changes = db.subscribe();
    let (conn, mut presence) = state.live.join(&slug, id, viewer);

    let mut last = match after {
        Some(a) => a.max(0),
        None => db.read(|c| Ok(last_event(c)?)).await.unwrap_or(0),
    };
    let hello = json!({ "type": "hello", "last": last, "presence": state.live.viewers(&slug, id) });
    let mut open = send(&mut socket, hello).await;

    if open && after.is_some() {
        match fetch(&db, id, user, last).await {
            Some(Step::Events(events, l)) => {
                if !events.is_empty() {
                    open = send(
                        &mut socket,
                        json!({ "type": "events", "events": events, "catch_up": true }),
                    )
                    .await;
                }
                last = l;
            }
            Some(Step::Reset(l)) => {
                open = send(&mut socket, json!({ "type": "reset", "last": l })).await;
                last = l;
            }
            Some(Step::Gone) | None => open = false,
        }
    }

    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;
    if open {
        loop {
            tokio::select! {
                changed = changes.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    if !still_in().await {
                        let _ = send(&mut socket, json!({ "type": "closed", "reason": "signed out" })).await;
                        break;
                    }
                    match fetch(&db, id, user, last).await {
                        Some(Step::Events(events, l)) => {
                            last = l;
                            if !events.is_empty() && !send(&mut socket, json!({ "type": "events", "events": events })).await {
                                break;
                            }
                        }
                        Some(Step::Reset(l)) => {
                            last = l;
                            if !send(&mut socket, json!({ "type": "reset", "last": l })).await {
                                break;
                            }
                        }
                        Some(Step::Gone) | None => {
                            let _ = send(&mut socket, json!({ "type": "closed", "reason": "no access" })).await;
                            break;
                        }
                    }
                }
                seen = presence.changed() => {
                    if seen.is_err() {
                        break;
                    }
                    let list = state.live.viewers(&slug, id);
                    if !send(&mut socket, json!({ "type": "presence", "presence": list })).await {
                        break;
                    }
                }
                incoming = socket.recv() => {
                    match incoming {
                        Some(Ok(Message::Text(t))) if t.as_str() == "ping" => {
                            if !send(&mut socket, json!({ "type": "pong" })).await {
                                break;
                            }
                        }
                        Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                        Some(Ok(_)) => {}
                    }
                }
                _ = ping.tick() => {
                if !still_in().await {
                    let _ = send(&mut socket, json!({ "type": "closed", "reason": "signed out" })).await;
                    break;
                }
                    if socket.send(Message::Ping(Default::default())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
    state.live.leave(&slug, id, conn);
    let _ = socket.send(Message::Close(None)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn h(origin: Option<&str>, host: &str) -> HeaderMap {
        let mut m = HeaderMap::new();
        if let Some(o) = origin {
            m.insert(header::ORIGIN, HeaderValue::from_str(o).unwrap());
        }
        m.insert(header::HOST, HeaderValue::from_str(host).unwrap());
        m
    }

    #[test]
    fn only_the_app_itself_may_open_a_live_connection() {
        let url = "http://localhost:8791";
        assert!(origin_allowed(
            &h(Some("http://localhost:8791"), "localhost:8791"),
            url
        ));
        assert!(origin_allowed(
            &h(Some("http://127.0.0.1:8791"), "127.0.0.1:8791"),
            url
        ));
        assert!(!origin_allowed(
            &h(Some("https://evil.example"), "localhost:8791"),
            url
        ));
        assert!(!origin_allowed(
            &h(Some("http://localhost:8791.evil.example"), "localhost:8791"),
            url
        ));
        assert!(!origin_allowed(&h(None, "localhost:8791"), url));
    }
}
