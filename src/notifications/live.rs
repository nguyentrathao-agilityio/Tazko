use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::json;

use crate::error::ApiError;
use crate::realtime::{PING_EVERY, say_signed_out, send, still_signed_in, upgrade_session};
use crate::state::AppState;
use crate::tenant::Company;

pub async fn live(
    State(state): State<AppState>,
    co: Company,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let session = upgrade_session(&headers, &state.settings.public_url)?;
    Ok(ws.on_upgrade(move |socket| run(state, co, session, socket)))
}

async fn run(state: AppState, co: Company, session: String, mut socket: WebSocket) {
    let user = co.user.0.user_id;
    let org = co.org_id;
    let db = co.db.clone();
    let control = state.dbs.control().clone();
    let still_in = || still_signed_in(&control, &session, org, user);
    let count = || {
        let (db, state) = (db.clone(), state.clone());
        async move {
            let hidden = super::hidden_for(&state, user).await.ok()?;
            db.read(move |c| Ok(super::unread(c, user, &hidden)?))
                .await
                .ok()
        }
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
                    say_signed_out(&mut socket).await;
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
                    say_signed_out(&mut socket).await;
                    break;
                }
                open = socket.send(Message::Ping(Default::default())).await.is_ok();
            }
        }
    }
    let _ = socket.send(Message::Close(None)).await;
}
