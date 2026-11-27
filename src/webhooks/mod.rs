pub mod guard;
pub mod worker;

use std::collections::HashMap;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::{get, patch, post};
use hmac::{Hmac, KeyInit, Mac};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::Sha256;

use crate::api::COMPANY_PREFIX;
use crate::api::tokens::member_project;
use crate::company::CompanyOwner;
use crate::error::{ApiError, bad};
use crate::projects::path_id;
use crate::session::{hex, now, random_token};
use crate::state::AppState;

pub const EVENTS: &[&str] = &[
    "card.created",
    "card.updated",
    "card.moved",
    "card.completed",
    "card.archived",
    "comment.created",
    "member.added",
    "board.created",
];

const LOG_LIMIT: i64 = 50;

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}{s}");
    Router::new()
        .route(&p("/webhooks"), get(list).post(create))
        .route(&p("/webhooks/{hook}"), patch(update).delete(remove))
        .route(&p("/webhooks/{hook}/test"), post(test))
        .route(&p("/webhooks/{hook}/secret"), post(rotate_secret))
        .route(&p("/webhooks/{hook}/deliveries"), get(deliveries))
        .route(
            &p("/webhooks/{hook}/deliveries/{delivery}/redeliver"),
            post(redeliver),
        )
}

fn hmac_hex(key: &str, message: &str) -> String {
    let mut mac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(key.as_bytes()).expect("any key length");
    mac.update(message.as_bytes());
    hex(&mac.finalize().into_bytes())
}

pub fn signing_secret(server: &str, slug: &str, hook: i64, salt: &str) -> String {
    format!(
        "whsec_{}",
        hmac_hex(server, &format!("tazko-webhook:{slug}:{hook}:{salt}"))
    )
}

pub fn signature(secret: &str, timestamp: i64, body: &str) -> String {
    format!(
        "t={timestamp},v1={}",
        hmac_hex(secret, &format!("{timestamp}.{body}"))
    )
}

pub(crate) fn parse_list<T: serde::de::DeserializeOwned>(raw: Option<String>) -> Option<Vec<T>> {
    raw.and_then(|p| serde_json::from_str(&p).ok())
}

const HOOK_SELECT: &str = "SELECT w.id, w.url, w.events, w.projects, w.active, w.created_at,
        coalesce(nullif(p.name, ''), p.email),
        d.status, d.response_code, coalesce(d.finished_at, d.created_at)
     FROM webhooks w
     LEFT JOIN people p ON p.id = w.created_by
     LEFT JOIN webhook_deliveries d ON d.id = (SELECT max(x.id) FROM webhook_deliveries x WHERE x.webhook_id = w.id AND x.attempts > 0)";

fn hooks(c: &Connection, only: Option<i64>) -> rusqlite::Result<Vec<Value>> {
    let names: HashMap<i64, String> = c
        .prepare("SELECT id, name FROM projects WHERE deleted_at IS NULL")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut stmt = c.prepare(&format!(
        "{HOOK_SELECT} WHERE ?1 IS NULL OR w.id = ?1 ORDER BY w.id"
    ))?;
    stmt.query_map([only], |r| {
        let projects: Option<Vec<i64>> = parse_list(r.get(3)?);
        let project_names = projects
            .as_ref()
            .map(|ids| ids.iter().filter_map(|i| names.get(i)).collect::<Vec<_>>());
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "url": r.get::<_, String>(1)?,
            "events": parse_list::<String>(r.get(2)?).unwrap_or_default(),
            "projects": projects,
            "project_names": project_names,
            "active": r.get::<_, bool>(4)?,
            "created_at": r.get::<_, i64>(5)?,
            "by": r.get::<_, String>(6)?,
            "last": {
                "status": r.get::<_, Option<String>>(7)?,
                "code": r.get::<_, Option<i64>>(8)?,
                "at": r.get::<_, Option<i64>>(9)?,
            },
        }))
    })?
    .collect()
}

fn hook_exists(c: &Connection, hook: i64) -> rusqlite::Result<bool> {
    c.query_row(
        "SELECT EXISTS (SELECT 1 FROM webhooks WHERE id = ?1)",
        [hook],
        |r| r.get(0),
    )
}

fn queued(
    c: &Connection,
    changed: usize,
    hook: i64,
) -> rusqlite::Result<Result<StatusCode, ApiError>> {
    Ok(if changed > 0 {
        Ok(StatusCode::ACCEPTED)
    } else if hook_exists(c, hook)? {
        Err(bad("resume the webhook first"))
    } else {
        Err(ApiError::NotFound)
    })
}

async fn list(CompanyOwner(co): CompanyOwner) -> Result<Json<Value>, ApiError> {
    let hooks = co.db.read(|c| Ok(hooks(c, None)?)).await?;
    Ok(Json(json!({ "webhooks": hooks, "events": EVENTS })))
}

#[derive(Deserialize)]
struct NewHook {
    url: String,
    events: Vec<String>,
    projects: Option<Vec<i64>>,
}

async fn create(
    State(state): State<AppState>,
    owner: CompanyOwner,
    headers: HeaderMap,
    Json(body): Json<NewHook>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = owner.writing(&headers)?;
    let co = owner.0;
    let (name, email) = (co.user.0.name.clone(), co.user.0.email.clone());
    let allow_local = state.settings.webhooks_allow_local;
    let url = body.url.trim().to_string();
    let check = url.clone();
    tokio::task::spawn_blocking(move || guard::check_url(&check, allow_local))
        .await
        .map_err(|_| bad("that address could not be checked"))?
        .map_err(|e| bad(&e))?;
    let mut events = body.events;
    events.sort();
    events.dedup();
    if events.is_empty() {
        return Err(bad("choose at least one event"));
    }

    if let Some(e) = events.iter().find(|e| !EVENTS.contains(&e.as_str())) {
        return Err(bad(&format!("{e} is not a webhook event")));
    }

    if body.projects.as_ref().is_some_and(Vec::is_empty) {
        return Err(bad(
            "choose at least one project, or leave the list out for all",
        ));
    }

    let projects = body.projects.map(|mut list| {
        list.sort_unstable();
        list.dedup();
        list
    });
    let salt = random_token();
    let events_json = json!(events).to_string();
    let projects_json = projects.as_ref().map(|p| json!(p).to_string());
    let stored = salt.clone();
    let (hook, id) = co
        .db
        .write(move |c| {
            let tx = c.transaction()?;
            crate::projects::upsert_person(&tx, by, &name, &email)?;
            for p in projects.iter().flatten() {
                if !member_project(&tx, by, *p)? {
                    return Ok(Err(bad("choose projects you belong to")));
                }
            }
            tx.execute(
                "INSERT INTO webhooks (url, events, projects, salt, created_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![url, events_json, projects_json, stored, by],
            )?;
            let id = tx.last_insert_rowid();
            worker::restart_cursor(&tx, id)?;
            let hook = hooks(&tx, Some(id))?.remove(0);
            tx.commit()?;
            Ok(Ok((hook, id)))
        })
        .await??;
    let mut hook = hook;
    hook["secret"] = json!(signing_secret(state.secret.expose(), &co.slug, id, &salt));
    Ok((StatusCode::CREATED, Json(hook)))
}

#[derive(Deserialize)]
struct HookChange {
    active: bool,
}

async fn update(
    owner: CompanyOwner,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
    Json(body): Json<HookChange>,
) -> Result<StatusCode, ApiError> {
    owner.writing(&headers)?;
    let hook = path_id(&params, "hook")?;
    let found = owner
        .0
        .db
        .write(move |c| {
            let tx = c.transaction()?;
            let active: Option<bool> = tx
                .query_row("SELECT active FROM webhooks WHERE id = ?1", [hook], |r| {
                    r.get(0)
                })
                .optional()?;
            if active.is_some_and(|a| a != body.active) {
                tx.execute(
                    "UPDATE webhooks SET active = ?2 WHERE id = ?1",
                    params![hook, body.active],
                )?;
                if body.active {
                    worker::restart_cursor(&tx, hook)?;
                }
            }

            tx.commit()?;
            Ok(active.is_some())
        })
        .await?;
    if !found {
        return Err(ApiError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn remove(
    owner: CompanyOwner,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    owner.writing(&headers)?;
    let hook = path_id(&params, "hook")?;
    let n = owner
        .0
        .db
        .write(move |c| Ok(c.execute("DELETE FROM webhooks WHERE id = ?1", [hook])?))
        .await?;
    if n == 0 {
        return Err(ApiError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn rotate_secret(
    State(state): State<AppState>,
    owner: CompanyOwner,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let by = owner.writing(&headers)?;
    let hook = path_id(&params, "hook")?;
    let salt = random_token();
    let stored = salt.clone();
    let maker = owner
        .0
        .db
        .write(move |c| {
            let maker: Option<Option<i64>> = c
                .query_row(
                    "SELECT created_by FROM webhooks WHERE id = ?1",
                    [hook],
                    |r| r.get(0),
                )
                .optional()?;
            if maker == Some(Some(by)) {
                c.execute(
                    "UPDATE webhooks SET salt = ?2 WHERE id = ?1",
                    params![hook, stored],
                )?;
            }

            Ok(maker)
        })
        .await?;
    match maker {
        None => return Err(ApiError::NotFound),
        Some(m) if m != Some(by) => {
            return Err(ApiError::ForbiddenBecause(
                "only the owner who added this webhook can replace its secret".into(),
            ));
        }
        Some(_) => {}
    }

    let org = owner.0.org_id;
    let audited = state
        .dbs
        .control()
        .write(move |c| {
            Ok(crate::admin::audit::record(
                c,
                Some(by),
                "webhook_secret_rotated",
                Some(org),
                "webhook",
                json!({ "webhook": hook }),
            )?)
        })
        .await;
    if let Err(e) = audited {
        eprintln!("tazko: the audit entry for a webhook secret could not be written: {e:?}");
    }

    let secret = signing_secret(state.secret.expose(), &owner.0.slug, hook, &salt);
    Ok(Json(json!({ "secret": secret })))
}

async fn test(
    owner: CompanyOwner,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    owner.writing(&headers)?;
    let hook = path_id(&params, "hook")?;
    let payload = json!({ "event": "ping", "company": owner.0.slug, "created_at": now(), "message": "Test event from Tazko" }).to_string();
    owner
        .0
        .db
        .write(move |c| {
            let n = c.execute(
                "INSERT INTO webhook_deliveries (webhook_id, event, payload) SELECT id, 'ping', ?2 FROM webhooks WHERE id = ?1 AND active = 1",
                params![hook, payload],
            )?;
            Ok(queued(c, n, hook)?)
        })
        .await?
}

async fn deliveries(
    CompanyOwner(co): CompanyOwner,
    Path(params): Path<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let hook = path_id(&params, "hook")?;
    let rows = co
        .db
        .read(move |c| {
            if !hook_exists(c, hook)? {
                return Ok(None);
            }

            let mut stmt = c.prepare(
                "SELECT id, event, status, attempts, response_code, error, duration_ms, created_at, finished_at, next_attempt_at
                 FROM webhook_deliveries WHERE webhook_id = ?1 ORDER BY id DESC LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![hook, LOG_LIMIT], |r| {
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "event": r.get::<_, String>(1)?,
                        "status": r.get::<_, String>(2)?,
                        "attempts": r.get::<_, i64>(3)?,
                        "code": r.get::<_, Option<i64>>(4)?,
                        "error": r.get::<_, Option<String>>(5)?,
                        "ms": r.get::<_, Option<i64>>(6)?,
                        "created_at": r.get::<_, i64>(7)?,
                        "finished_at": r.get::<_, Option<i64>>(8)?,
                        "next_attempt_at": r.get::<_, i64>(9)?,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Some(rows))
        })
        .await?;
    rows.map(|d| Json(json!({ "deliveries": d })))
        .ok_or(ApiError::NotFound)
}

async fn redeliver(
    owner: CompanyOwner,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    owner.writing(&headers)?;
    let hook = path_id(&params, "hook")?;
    let delivery = path_id(&params, "delivery")?;
    owner
        .0
        .db
        .write(move |c| {
            let n = c.execute(
                "INSERT INTO webhook_deliveries (webhook_id, event, payload)
                 SELECT d.webhook_id, d.event, d.payload FROM webhook_deliveries d JOIN webhooks w ON w.id = d.webhook_id
                 WHERE d.id = ?2 AND d.webhook_id = ?1 AND w.active = 1",
                params![hook, delivery],
            )?;
            Ok(queued(c, n, hook)?)
        })
        .await?
}
