use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::{delete, get, patch, post};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};

use crate::auth::check_csrf;
use crate::error::{ApiError, bad};
use crate::session::{self, now};
use crate::state::AppState;
use crate::tenant::CurrentUser;

#[derive(Clone, Copy)]
enum Kind {
    Bool,
    Choice(&'static [&'static str]),
    Zoom,
    Text(usize),
    TimeZone,
    Hour,
}

const KEYS: &[(&str, Kind)] = &[
    ("job_title", Kind::Text(80)),
    ("avatar", Kind::Choice(&["google", "initials"])),
    ("language", Kind::Choice(&["en"])),
    ("theme", Kind::Choice(&["system", "light", "dark"])),
    ("zoom", Kind::Zoom),
    ("density", Kind::Choice(&["comfortable", "compact"])),
    ("label_style", Kind::Choice(&["names", "bars"])),
    ("reduce_motion", Kind::Bool),
    ("zen", Kind::Bool),
    (
        "zen_density",
        Kind::Choice(&["comfortable", "compact", "minimal"]),
    ),
    ("zen_quiet", Kind::Bool),
    ("time_zone", Kind::TimeZone),
    ("date_format", Kind::Choice(&["MDY", "DMY", "ISO"])),
    ("time_format", Kind::Choice(&["12", "24"])),
    ("week_start", Kind::Choice(&["mon", "sun", "sat"])),
    ("notify_mention_app", Kind::Bool),
    ("notify_mention_email", Kind::Bool),
    ("notify_assigned_app", Kind::Bool),
    ("notify_assigned_email", Kind::Bool),
    ("notify_card_added_app", Kind::Bool),
    ("notify_card_added_email", Kind::Bool),
    ("notify_due_email", Kind::Bool),
    (
        "email_frequency",
        Kind::Choice(&["instant", "hourly", "daily", "never"]),
    ),
    ("due_reminder", Kind::Choice(&["at", "1h", "1d", "2d"])),
    ("daily_hour_utc", Kind::Hour),
];

pub fn defaults() -> Map<String, Value> {
    [
        ("job_title", json!("")),
        ("avatar", json!("google")),
        ("language", json!("en")),
        ("theme", json!("system")),
        ("zoom", json!(100)),
        ("density", json!("comfortable")),
        ("label_style", json!("names")),
        ("reduce_motion", json!(false)),
        ("zen", json!(false)),
        ("zen_density", json!("comfortable")),
        ("zen_quiet", json!(true)),
        ("time_zone", json!("UTC")),
        ("date_format", json!("MDY")),
        ("time_format", json!("12")),
        ("week_start", json!("mon")),
        ("notify_mention_app", json!(true)),
        ("notify_mention_email", json!(true)),
        ("notify_assigned_app", json!(true)),
        ("notify_assigned_email", json!(true)),
        ("notify_card_added_app", json!(true)),
        ("notify_card_added_email", json!(false)),
        ("notify_due_email", json!(true)),
        ("email_frequency", json!("instant")),
        ("due_reminder", json!("1d")),
        ("daily_hour_utc", json!(1)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

pub fn valid_time_zone(tz: &str) -> bool {
    !tz.is_empty()
        && tz.len() <= 64
        && tz
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/_+-".contains(c))
        && !tz.starts_with('/')
        && !tz.contains("//")
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
        Kind::Zoom => value
            .as_i64()
            .is_some_and(|z| (80..=150).contains(&z) && z % 10 == 0),
        Kind::Text(max) => value
            .as_str()
            .is_some_and(|t| t.chars().count() <= max && !t.chars().any(char::is_control)),
        Kind::TimeZone => value.as_str().is_some_and(valid_time_zone),
        Kind::Hour => value.as_i64().is_some_and(|h| (0..24).contains(&h)),
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

pub fn set(c: &Connection, user: i64, key: &str, value: Value) -> Result<(), ApiError> {
    check(key, &value)?;
    let mut current = load(c, user)?;
    current.insert(key.to_string(), value);
    c.execute(
        "INSERT INTO user_settings (user_id, settings, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (user_id) DO UPDATE SET settings = excluded.settings, updated_at = excluded.updated_at",
        params![user, Value::Object(current).to_string(), now()],
    )?;
    Ok(())
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/me/settings", get(read).patch(update))
        .route("/api/v1/me/profile", patch(profile))
        .route("/api/v1/me/sessions", get(sessions))
        .route("/api/v1/me/sessions/{id}", delete(end_session))
        .route("/api/v1/me/sessions/others", post(end_others))
}

#[derive(serde::Deserialize)]
struct Profile {
    name: String,
}

async fn profile(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Json(body): Json<Profile>,
) -> Result<StatusCode, ApiError> {
    check_csrf(&user, &headers)?;
    let name = crate::admin::validate::name(&body.name).ok_or_else(|| bad("enter your name"))?;
    let id = user.0.user_id;
    state
        .dbs
        .control()
        .write(move |c| {
            c.execute(
                "UPDATE users SET name = ?2 WHERE id = ?1",
                params![id, name],
            )?;
            Ok(())
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn current_hash(headers: &HeaderMap) -> String {
    session::cookie_token(headers)
        .map(|t| session::hash_token(&t))
        .unwrap_or_default()
}

async fn sessions(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let id = user.user_id;
    let mine = current_hash(&headers);
    let list = state
        .dbs
        .control()
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT id_hash, user_agent, created_at, last_used_at FROM sessions
                 WHERE user_id = ?1 AND expires_at > ?2 ORDER BY last_used_at DESC",
            )?;
            let rows = stmt
                .query_map(params![id, now()], |r| {
                    let hash: String = r.get(0)?;
                    Ok(json!({
                        "id": &hash[..16],
                        "user_agent": r.get::<_, Option<String>>(1)?,
                        "created_at": r.get::<_, i64>(2)?,
                        "last_used_at": r.get::<_, i64>(3)?,
                        "current": hash == mine,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(Json(json!({ "sessions": list })))
}

async fn end_session(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(handle): Path<String>,
) -> Result<StatusCode, ApiError> {
    check_csrf(&user, &headers)?;
    if handle.len() != 16 || !handle.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ApiError::NotFound);
    }

    let mine = current_hash(&headers);
    if mine.starts_with(&handle) {
        return Err(bad("use Sign out to end this session"));
    }

    let id = user.0.user_id;
    let n = state
        .dbs
        .control()
        .write(move |c| {
            Ok(c.execute(
                "DELETE FROM sessions WHERE user_id = ?1 AND substr(id_hash, 1, 16) = ?2",
                params![id, handle],
            )?)
        })
        .await?;
    if n == 0 {
        Err(ApiError::NotFound)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

async fn end_others(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    check_csrf(&user, &headers)?;
    let mine = current_hash(&headers);
    let id = user.0.user_id;
    let n = state
        .dbs
        .control()
        .write(move |c| {
            Ok(c.execute(
                "DELETE FROM sessions WHERE user_id = ?1 AND id_hash != ?2",
                params![id, mine],
            )?)
        })
        .await?;
    Ok(Json(json!({ "ended": n })))
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
