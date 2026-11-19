use std::time::Duration;

use axum::extract::State;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::{Value, json};

use crate::error::ApiError;
use crate::realtime::{origin_allowed, signed_in};
use crate::state::AppState;
use crate::tenant::Company;

const PING_EVERY: Duration = Duration::from_secs(25);

pub async fn live(
    State(state): State<AppState>,
    co: Company,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    if !origin_allowed(&headers, &state.settings.public_url) {
        return Err(ApiError::Forbidden);
    }

    let session = crate::session::cookie_token(&headers)
        .map(|t| crate::session::hash_token(&t))
        .ok_or(ApiError::SignIn)?;
    Ok(ws.on_upgrade(move |socket| run(state, co, session, socket)))
}

async fn send(socket: &mut WebSocket, v: Value) -> bool {
    socket
        .send(Message::Text(Utf8Bytes::from(v.to_string())))
        .await
        .is_ok()
}

async fn run(state: AppState, co: Company, session: String, mut socket: WebSocket) {
    let user = co.user.0.user_id;
    let org = co.org_id;
    let db = co.db.clone();
    let control = state.dbs.control().clone();
    let still_in = || {
        let (control, session) = (control.clone(), session.clone());
        async move {
            control
                .read(move |c| Ok(signed_in(c, &session, org, user)?))
                .await
                .unwrap_or(false)
        }
    };
    let count = || {
        let db = db.clone();
        async move { db.read(move |c| Ok(super::unread(c, user)?)).await.ok() }
    };
    let mut changes = db.subscribe();
    let mut last = count().await.unwrap_or(0);
    let mut open = send(&mut socket, json!({ "type": "unread", "count": last })).await;
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;
    while open {
        tokio::select! {
            changed = changes.changed() => {
                if changed.is_err() || !still_in().await {
                    let _ = send(&mut socket, json!({ "type": "closed", "reason": "signed out" })).await;
                    break;
                }
                if let Some(n) = count().await
                    && n != last
                {
                    last = n;
                    open = send(&mut socket, json!({ "type": "unread", "count": n })).await;
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(t))) if t.as_str() == "ping" => {
                        open = send(&mut socket, json!({ "type": "pong" })).await;
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
                open = socket.send(Message::Ping(Default::default())).await.is_ok();
            }
        }
    }
    let _ = socket.send(Message::Close(None)).await;
}
