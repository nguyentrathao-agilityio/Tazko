use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use super::{guard, signature, signing_secret};
use crate::db::{Database, DbResult};
use crate::session::now;
use crate::state::AppState;

const TICK: Duration = Duration::from_secs(3);
const BATCH: i64 = 200;
const SEND_BATCH: i64 = 20;
const DRAIN_ROUNDS: usize = 10;
const TIMEOUT: Duration = Duration::from_secs(10);
const BACKOFF: [i64; 6] = [60, 300, 1800, 7200, 21600, 43200];
const ERROR_LIMIT: usize = 300;
const KEEP_LOG: i64 = 30 * 24 * 3600;
const NO_LONGER_VISIBLE: &str =
    "the webhook's maker is no longer an owner or can no longer see this board";

struct Hook {
    id: i64,
    events: Vec<String>,
    projects: Option<Vec<i64>>,
    owner: i64,
}

struct Event {
    kind: String,
    actor: Option<i64>,
    project: Option<i64>,
    board: Option<i64>,
    card: Option<i64>,
    payload: Value,
    at: i64,
}

pub async fn job(state: AppState) {
    let busy: Arc<Mutex<HashSet<String>>> = Arc::default();
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        for (slug, db) in crate::tenant::active_tenants(&state).await {
            if !busy.lock().unwrap().insert(slug.clone()) {
                continue;
            }

            let (state, busy) = (state.clone(), busy.clone());
            tokio::spawn(async move {
                run_company(&state, &slug, &db).await;
                busy.lock().unwrap().remove(&slug);
            });
        }
    }
}

pub async fn run_company(state: &AppState, slug: &str, db: &Database) {
    if let Err(e) = enqueue(state, slug, db).await {
        eprintln!("tazko: webhook queue failed for {slug}: {e:?}");
    }

    for _ in 0..DRAIN_ROUNDS {
        match deliver_due(state, slug, db, now()).await {
            Ok(n) if n as i64 == SEND_BATCH => continue,
            Ok(_) => break,
            Err(e) => {
                eprintln!("tazko: webhook delivery failed for {slug}: {e:?}");
                break;
            }
        }
    }
}

const SET_CURSOR: &str = "INSERT INTO webhook_cursor (id, last_event) VALUES (1, ?1)
     ON CONFLICT (id) DO UPDATE SET last_event = excluded.last_event";

pub fn restart_cursor(c: &Connection, except: i64) -> rusqlite::Result<()> {
    let others: bool = c.query_row(
        "SELECT EXISTS (SELECT 1 FROM webhooks WHERE active = 1 AND id != ?1)",
        [except],
        |r| r.get(0),
    )?;
    if !others {
        let latest: i64 =
            c.query_row("SELECT coalesce(max(id), 0) FROM events", [], |r| r.get(0))?;
        c.execute(SET_CURSOR, [latest])?;
    }

    Ok(())
}

pub(crate) async fn owners(state: &AppState, slug: &str) -> DbResult<Vec<i64>> {
    let slug = slug.to_string();
    state
        .dbs
        .control()
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT m.user_id FROM org_members m JOIN orgs o ON o.id = m.org_id JOIN users u ON u.id = m.user_id
                 WHERE o.slug = ?1 AND m.role = 'owner' AND u.disabled = 0",
            )?;
            let rows = stmt.query_map([slug], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
            Ok(rows)
        })
        .await
}

pub async fn enqueue(state: &AppState, slug: &str, db: &Database) -> DbResult<usize> {
    let waiting: bool = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT EXISTS (SELECT 1 FROM webhooks WHERE active = 1)
                    AND (SELECT coalesce(max(id), 0) FROM events) > coalesce((SELECT last_event FROM webhook_cursor WHERE id = 1), 0)",
                [],
                |r| r.get(0),
            )?)
        })
        .await?;
    if !waiting {
        return Ok(0);
    }

    let owners = owners(state, slug).await?;
    let slug = slug.to_string();
    db.write(move |c| {
        let tx = c.transaction()?;
        let made = enqueue_in(&tx, &slug, &owners)?;
        tx.commit()?;
        Ok(made)
    })
    .await
}

fn enqueue_in(c: &Connection, slug: &str, owners: &[i64]) -> rusqlite::Result<usize> {
    let cursor: i64 = c
        .query_row(
            "SELECT last_event FROM webhook_cursor WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0);
    let mut hooks = active_hooks(c)?;
    hooks.retain(|h| owners.contains(&h.owner));

    let mut stmt = c.prepare(
        "SELECT id, type, actor_id, project_id, board_id, card_id, payload, created_at
         FROM events WHERE id > ?1 ORDER BY id LIMIT ?2",
    )?;
    let events = stmt
        .query_map(params![cursor, BATCH], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                Event {
                    kind: r.get(1)?,
                    actor: r.get(2)?,
                    project: r.get(3)?,
                    board: r.get(4)?,
                    card: r.get(5)?,
                    payload: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or(Value::Null),
                    at: r.get(7)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut made = 0;
    let mut last = cursor;
    for (id, event) in &events {
        last = *id;
        for (name, card) in outgoing(event) {
            for hook in hooks.iter().filter(|h| h.events.iter().any(|e| e == name)) {
                if !in_scope(c, hook, event)? {
                    continue;
                }

                let Some(body) = payload(c, slug, name, card, event)? else {
                    continue;
                };
                c.execute(
                    "INSERT INTO webhook_deliveries (webhook_id, event, payload) VALUES (?1, ?2, ?3)",
                    params![hook.id, name, body.to_string()],
                )?;
                made += 1;
            }
        }
    }
    c.execute(SET_CURSOR, [last])?;
    Ok(made)
}

fn active_hooks(c: &Connection) -> rusqlite::Result<Vec<Hook>> {
    let mut stmt =
        c.prepare("SELECT id, events, projects, created_by FROM webhooks WHERE active = 1")?;
    stmt.query_map([], |r| {
        Ok(Hook {
            id: r.get(0)?,
            events: super::parse_list(r.get(1)?).unwrap_or_default(),
            projects: super::parse_list(r.get(2)?),
            owner: r.get(3)?,
        })
    })?
    .collect()
}

fn outgoing(e: &Event) -> Vec<(&'static str, Option<i64>)> {
    let one = |name| vec![(name, e.card)];
    let each = |name| {
        e.payload["cards"]
            .as_array()
            .map(|ids| {
                ids.iter()
                    .filter_map(Value::as_i64)
                    .map(|id| (name, Some(id)))
                    .collect()
            })
            .unwrap_or_default()
    };
    match e.kind.as_str() {
        "card_created" | "card_copied" => one("card.created"),
        "cards_created" => each("card.created"),
        "card_done" | "card_updated" if e.payload["done"] == json!(true) => one("card.completed"),
        "card_updated" | "card_renamed" | "card_done" => one("card.updated"),
        "card_repeated" => one("card.completed"),
        "card_moved" => one("card.moved"),
        "cards_moved" => each("card.moved"),
        "card_archived" => one("card.archived"),
        "cards_archived" => each("card.archived"),
        "comment_added" => one("comment.created"),
        "member_added" | "member_joined" | "owner_added" => one("member.added"),
        "board_created" => one("board.created"),
        _ => vec![],
    }
}

fn in_scope(c: &Connection, hook: &Hook, e: &Event) -> rusqlite::Result<bool> {
    let Some(project) = e.project else {
        return Ok(false);
    };
    if hook
        .projects
        .as_ref()
        .is_some_and(|p| !p.contains(&project))
    {
        return Ok(false);
    }

    match e.board {
        Some(board) => crate::realtime::allowed(c, board, hook.owner),
        None => crate::api::tokens::member_project(c, hook.owner, project),
    }
}

fn named(c: &Connection, sql: &str, id: Option<i64>) -> rusqlite::Result<Value> {
    let Some(id) = id else {
        return Ok(Value::Null);
    };
    Ok(c.query_row(sql, [id], |r| {
        Ok(json!({ "id": id, "name": r.get::<_, String>(0)? }))
    })
    .optional()?
    .unwrap_or(Value::Null))
}

fn person(c: &Connection, id: Option<i64>) -> rusqlite::Result<Value> {
    let Some(id) = id else {
        return Ok(Value::Null);
    };
    Ok(
        c.query_row("SELECT name, email FROM people WHERE id = ?1", [id], |r| {
            Ok(json!({ "id": id, "name": r.get::<_, String>(0)?, "email": r.get::<_, String>(1)? }))
        })
        .optional()?
        .unwrap_or(Value::Null),
    )
}

fn card(c: &Connection, id: i64, board: Option<i64>) -> rusqlite::Result<Option<Value>> {
    c.query_row(
        "SELECT k.title, k.description_text, k.due_at, k.done, k.archived_at, l.id, l.title, k.created_at, k.updated_at
         FROM cards k JOIN lists l ON l.id = k.list_id
         WHERE k.id = ?1 AND k.board_id = ?2 AND k.is_template = 0",
        params![id, board],
        |r| {
            Ok(json!({
                "id": id,
                "title": r.get::<_, String>(0)?,
                "description": r.get::<_, String>(1)?,
                "due_at": r.get::<_, Option<i64>>(2)?,
                "done": r.get::<_, bool>(3)?,
                "archived": r.get::<_, Option<i64>>(4)?.is_some(),
                "list": { "id": r.get::<_, i64>(5)?, "name": r.get::<_, String>(6)? },
                "created_at": r.get::<_, i64>(7)?,
                "updated_at": r.get::<_, i64>(8)?,
            }))
        },
    )
    .optional()
}

fn payload(
    c: &Connection,
    slug: &str,
    name: &str,
    card_id: Option<i64>,
    e: &Event,
) -> rusqlite::Result<Option<Value>> {
    let mut body = json!({
        "event": name,
        "company": slug,
        "created_at": e.at,
        "actor": person(c, e.actor)?,
        "project": named(c, "SELECT name FROM projects WHERE id = ?1", e.project)?,
        "board": named(c, "SELECT name FROM boards WHERE id = ?1", e.board)?,
    });
    if let Some(id) = card_id {
        let Some(card) = card(c, id, e.board)? else {
            return Ok(None);
        };
        body["card"] = card;
    }

    match name {
        "card.updated" => {
            body["changed"] = json!(
                e.payload
                    .as_object()
                    .map(|m| m.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default()
            );
        }
        "card.moved" => {
            body["from_list"] = named(
                c,
                "SELECT title FROM lists WHERE id = ?1",
                e.payload["from_list"].as_i64(),
            )?;
        }
        "comment.created" => {
            let found = c
                .query_row(
                    "SELECT id, body, created_at FROM comments WHERE id = ?1 AND card_id = ?2",
                    params![e.payload["comment"].as_i64(), card_id],
                    |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "body": r.get::<_, String>(1)?, "created_at": r.get::<_, i64>(2)? })),
                )
                .optional()?;
            let Some(comment) = found else {
                return Ok(None);
            };
            body["comment"] = comment;
        }
        "member.added" => {
            body["member"] = person(c, e.payload["user"].as_i64())?;
            body["role"] = if e.kind == "owner_added" {
                json!("owner")
            } else {
                e.payload["role"].clone()
            };
        }
        _ => {}
    }
    Ok(Some(body))
}

struct Due {
    id: i64,
    hook: i64,
    url: String,
    salt: String,
    event: String,
    payload: String,
    attempts: i64,
}

struct Outcome {
    ok: bool,
    code: Option<u16>,
    error: Option<String>,
    ms: i64,
}

impl Outcome {
    fn failed(error: String, ms: i64) -> Self {
        Outcome {
            ok: false,
            code: None,
            error: Some(error),
            ms,
        }
    }
}

fn still_visible(
    c: &Connection,
    owners: &[i64],
    hook: &Hook,
    payload: &str,
) -> rusqlite::Result<bool> {
    if !owners.contains(&hook.owner) {
        return Ok(false);
    }

    let body: Value = serde_json::from_str(payload).unwrap_or(Value::Null);
    if body["event"] == "ping" {
        return Ok(true);
    }

    let e = Event {
        kind: String::new(),
        actor: None,
        project: body["project"]["id"].as_i64(),
        board: body["board"]["id"].as_i64(),
        card: None,
        payload: Value::Null,
        at: 0,
    };
    in_scope(c, hook, &e)
}

pub async fn deliver_due(state: &AppState, slug: &str, db: &Database, at: i64) -> DbResult<usize> {
    let owners = owners(state, slug).await?;
    let (due, refused) = db
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT d.id, w.id, w.url, w.salt, d.event, d.payload, d.attempts, w.created_by, w.projects
                 FROM webhook_deliveries d JOIN webhooks w ON w.id = d.webhook_id
                 WHERE d.status = 'pending' AND d.next_attempt_at <= ?1 AND w.active = 1
                 ORDER BY d.next_attempt_at, d.id LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![at, SEND_BATCH], |r| {
                    let hook = Hook {
                        id: r.get(1)?,
                        events: vec![],
                        projects: super::parse_list(r.get(8)?),
                        owner: r.get(7)?,
                    };
                    Ok((
                        hook,
                        Due {
                            id: r.get(0)?,
                            hook: r.get(1)?,
                            url: r.get(2)?,
                            salt: r.get(3)?,
                            event: r.get(4)?,
                            payload: r.get(5)?,
                            attempts: r.get(6)?,
                        },
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut due = vec![];
            let mut refused = vec![];
            for (hook, d) in rows {
                if still_visible(c, &owners, &hook, &d.payload)? {
                    due.push(d);
                } else {
                    refused.push(d.id);
                }
            }
            Ok((due, refused))
        })
        .await?;
    if !refused.is_empty() {
        let finished = now();
        db.write(move |c| {
            let tx = c.transaction()?;
            for id in refused {
                tx.execute(
                    "UPDATE webhook_deliveries SET status = 'failed', error = ?2, finished_at = ?3 WHERE id = ?1",
                    params![id, NO_LONGER_VISIBLE, finished],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .await?;
    }

    if due.is_empty() {
        return Ok(0);
    }

    let allow_local = state.settings.webhooks_allow_local;
    let mut sends = tokio::task::JoinSet::new();
    for d in due {
        let secret = signing_secret(state.secret.expose(), slug, d.hook, &d.salt);
        sends.spawn_blocking(move || {
            let outcome = send(&d.url, &secret, d.id, &d.event, &d.payload, allow_local);
            (d.id, d.attempts, outcome)
        });
    }
    let mut results = vec![];
    while let Some(done) = sends.join_next().await {
        match done {
            Ok(r) => results.push(r),
            Err(e) => eprintln!("tazko: webhook send stopped for {slug}: {e}"),
        }
    }
    let count = results.len();
    let finished = now();
    db.write(move |c| {
        let tx = c.transaction()?;
        for (id, tried, outcome) in results {
            let (status, next) = match (outcome.ok, BACKOFF.get(tried as usize)) {
                (true, _) => ("delivered", finished),
                (false, Some(wait)) => ("pending", finished + wait),
                (false, None) => ("failed", finished),
            };
            tx.execute(
                "UPDATE webhook_deliveries SET status = ?2, attempts = ?3, response_code = ?4, error = ?5,
                    duration_ms = ?6, next_attempt_at = ?7, finished_at = ?8 WHERE id = ?1",
                params![id, status, tried + 1, outcome.code, outcome.error, outcome.ms, next, finished],
            )?;
        }
        tx.execute(
            "DELETE FROM webhook_deliveries WHERE status != 'pending' AND finished_at < ?1",
            [finished - KEEP_LOG],
        )?;
        tx.commit()?;
        Ok(())
    })
    .await?;
    Ok(count)
}

fn agent(allow_local: bool) -> ureq::Agent {
    static AGENTS: OnceLock<[ureq::Agent; 2]> = OnceLock::new();
    let agents = AGENTS.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
        [false, true].map(|allow_local| {
            let config = ureq::Agent::config_builder()
                .tls_config(
                    ureq::tls::TlsConfig::builder()
                        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                        .build(),
                )
                .timeout_global(Some(TIMEOUT))
                .max_redirects(0)
                .proxy(None)
                .http_status_as_error(false)
                .user_agent("Tazko-Webhooks/1")
                .build();
            ureq::Agent::with_parts(
                config,
                ureq::unversioned::transport::DefaultConnector::new(),
                guard::Guarded { allow_local },
            )
        })
    });
    agents[usize::from(allow_local)].clone()
}

fn send(
    url: &str,
    secret: &str,
    delivery: i64,
    event: &str,
    body: &str,
    allow_local: bool,
) -> Outcome {
    if let Err(e) = guard::check_scheme(url, allow_local) {
        return Outcome::failed(e, 0);
    }

    let started = Instant::now();
    let ms = || started.elapsed().as_millis() as i64;
    let result = agent(allow_local)
        .post(url)
        .header("Content-Type", "application/json")
        .header("Tazko-Event", event)
        .header("Tazko-Delivery", delivery.to_string())
        .header("Tazko-Signature", signature(secret, now(), body))
        .send(body);
    match result {
        Ok(res) => {
            let code = res.status().as_u16();
            let ok = (200..300).contains(&code);
            Outcome {
                ok,
                code: Some(code),
                error: (!ok).then(|| format!("receiver answered {code}")),
                ms: ms(),
            }
        }
        Err(ureq::Error::HostNotFound) => Outcome::failed(
            "the address resolves to a private or unknown host".into(),
            ms(),
        ),
        Err(e) => Outcome::failed(e.to_string().chars().take(ERROR_LIMIT).collect(), ms()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: &str, payload: Value) -> Event {
        Event {
            kind: kind.into(),
            actor: None,
            project: Some(1),
            board: Some(1),
            card: Some(9),
            payload,
            at: 0,
        }
    }

    #[test]
    fn bulk_and_combined_changes_map_to_public_events() {
        assert_eq!(
            outgoing(&event("cards_archived", json!({ "cards": [3, 4] }))),
            [("card.archived", Some(3)), ("card.archived", Some(4))]
        );
        assert_eq!(
            outgoing(&event("cards_moved", json!({ "cards": [5] }))),
            [("card.moved", Some(5))]
        );
        assert_eq!(
            outgoing(&event(
                "card_updated",
                json!({ "title": "x", "done": true })
            )),
            [("card.completed", Some(9))]
        );
        assert_eq!(
            outgoing(&event("card_done", json!({ "done": false }))),
            [("card.updated", Some(9))]
        );
        assert_eq!(
            outgoing(&event("card_copied", json!({}))),
            [("card.created", Some(9))]
        );
        assert_eq!(
            outgoing(&event("member_joined", json!({}))),
            [("member.added", Some(9))]
        );
        assert!(outgoing(&event("card_restored", json!({}))).is_empty());
    }
}
