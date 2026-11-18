use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Json;
use axum::routing::get;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};

use crate::auth::check_csrf;
use crate::error::{ApiError, bad};
use crate::session::now;
use crate::state::AppState;
use crate::tenant::CurrentUser;

#[derive(Clone, Copy)]
enum Kind {
    Bool,
    Choice(&'static [&'static str]),
}

const KEYS: &[(&str, Kind)] = &[
    ("zen", Kind::Bool),
    (
        "zen_density",
        Kind::Choice(&["comfortable", "compact", "minimal"]),
    ),
    ("zen_quiet", Kind::Bool),
];

pub fn defaults() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("zen".into(), json!(false));
    m.insert("zen_density".into(), json!("comfortable"));
    m.insert("zen_quiet".into(), json!(true));
    m
}

fn check(key: &str, value: &Value) -> Result<(), ApiError> {
    let kind = KEYS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, kind)| *kind)
        .ok_or_else(|| bad(&format!("{key} is not a setting")))?;
    let ok = match kind {
        Kind::Bool => value.is_boolean(),
        Kind::Choice(options) => value.as_str().is_some_and(|v| options.contains(&v)),
    };
    if ok {
        Ok(())
    } else {
        Err(bad(&format!("{key} has a value it cannot take")))
    }
}

pub fn load(c: &Connection, user: i64) -> rusqlite::Result<Map<String, Value>> {
    let stored: Option<String> = c
        .query_row(
            "SELECT settings FROM user_settings WHERE user_id = ?1",
            [user],
            |r| r.get(0),
        )
        .optional()?;
    let mut out = defaults();
    if let Some(Value::Object(saved)) = stored.and_then(|s| serde_json::from_str(&s).ok()) {
        for (k, v) in saved {
            if check(&k, &v).is_ok() {
                out.insert(k, v);
            }
        }
    }
    Ok(out)
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/v1/me/settings", get(read).patch(update))
}

async fn read(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Value>, ApiError> {
    let id = user.user_id;
    let settings = state.dbs.control().read(move |c| Ok(load(c, id)?)).await?;
    Ok(Json(Value::Object(settings)))
}

async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Json(body): Json<Map<String, Value>>,
) -> Result<Json<Value>, ApiError> {
    check_csrf(&user, &headers)?;
    if body.is_empty() {
        return Err(bad("nothing to change"));
    }

    for (k, v) in &body {
        check(k, v)?;
    }

    let id = user.0.user_id;
    let settings = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let mut current = load(&tx, id)?;
            for (k, v) in body {
                current.insert(k, v);
            }
            tx.execute(
                "INSERT INTO user_settings (user_id, settings, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (user_id) DO UPDATE SET settings = excluded.settings, updated_at = excluded.updated_at",
                params![id, Value::Object(current.clone()).to_string(), now()],
            )?;
            tx.commit()?;
            Ok(current)
        })
        .await?;
    Ok(Json(Value::Object(settings)))
}
