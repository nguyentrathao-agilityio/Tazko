use axum::Router;
use axum::extract::{Query, State};
use axum::response::Json;
use axum::routing::get;
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::boards::VISIBLE;
use crate::db::valid_slug;
use crate::error::{ApiError, bad};
use crate::session::now;
use crate::state::AppState;
use crate::tenant::CurrentUser;

pub const LIMIT: i64 = 500;

const OPEN: &str = "JOIN lists l ON l.id = k.list_id
         JOIN boards b ON b.id = k.board_id
         JOIN projects p ON p.id = b.project_id
         JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = ?2
         AND (?4 IS NULL OR p.id IN (SELECT value FROM json_each(?4)))";

const LIVE: &str = "k.archived_at IS NULL AND k.is_template = 0 AND l.archived_at IS NULL
           AND b.closed_at IS NULL AND p.deleted_at IS NULL AND p.archived_at IS NULL";

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/v1/me/cards", get(mine))
}

#[derive(Deserialize)]
struct Scope {
    company: Option<String>,
    due: Option<String>,
}

const DAY: i64 = 24 * 60 * 60;

#[derive(Clone, Copy)]
pub enum Due {
    Any,
    Overdue,
    Today,
    Week,
    None,
}

impl Due {
    fn parse(raw: Option<&str>) -> Result<Due, ApiError> {
        Ok(match raw {
            None | Some("") => Due::Any,
            Some("overdue") => Due::Overdue,
            Some("today") => Due::Today,
            Some("week") => Due::Week,
            Some("none") => Due::None,
            Some(_) => return Err(bad("due is one of overdue, today, week or none")),
        })
    }

    fn clause(self, t: &str) -> String {
        let open = format!("?3 IS NOT NULL AND {t}.done = 0");
        match self {
            Due::Any => "?3 IS NOT NULL".into(),
            Due::Overdue => format!("{open} AND {t}.due_at < ?3"),
            Due::Today => format!("{open} AND {t}.due_at >= ?3 AND {t}.due_at < ?3 + {DAY}"),
            Due::Week => format!(
                "{open} AND {t}.due_at >= ?3 AND {t}.due_at < ?3 + {}",
                7 * DAY
            ),
            Due::None => format!("{open} AND {t}.due_at IS NULL"),
        }
    }
}

pub fn assigned(
    c: &Connection,
    user: i64,
    due: Due,
    now: i64,
    projects: Option<&str>,
) -> rusqlite::Result<Vec<Value>> {
    let due = due.clause("k");
    let mut stmt = c.prepare(&format!(
        "SELECT k.id, k.title, k.done, k.due_at, l.title, b.id, b.name, p.id, p.name,
                (SELECT count(*) FROM checklist_items i JOIN checklists x ON x.id = i.checklist_id WHERE x.card_id = k.id),
                (SELECT count(*) FROM checklist_items i JOIN checklists x ON x.id = i.checklist_id WHERE x.card_id = k.id AND i.done = 1),
                pm.role != 'observer', k.recurrence
         FROM card_members cm
         JOIN cards k ON k.id = cm.card_id
         {OPEN}
         WHERE cm.user_id = ?2 AND {LIVE} AND {VISIBLE} AND {due}
         ORDER BY k.done, k.due_at IS NULL, k.due_at, k.id LIMIT ?1"
    ))?;
    let rows: Vec<(Value, i64)> = stmt
        .query_map(params![LIMIT, user, now, projects], |r| {
            let card: i64 = r.get(0)?;
            Ok((
                json!({
                    "id": card,
                    "title": r.get::<_, String>(1)?,
                    "done": r.get::<_, bool>(2)?,
                    "due_at": r.get::<_, Option<i64>>(3)?,
                    "list": r.get::<_, String>(4)?,
                    "board": { "id": r.get::<_, i64>(5)?, "name": r.get::<_, String>(6)? },
                    "project": { "id": r.get::<_, i64>(7)?, "name": r.get::<_, String>(8)? },
                    "items": r.get::<_, i64>(9)?,
                    "items_done": r.get::<_, i64>(10)?,
                    "can_edit": r.get::<_, bool>(11)?,
                    "recurrence": r.get::<_, Option<String>>(12)?,
                }),
                card,
            ))
        })?
        .collect::<Result<_, _>>()?;
    let mut labels = c.prepare(
        "SELECT lb.id, lb.name, lb.color FROM card_labels cl JOIN labels lb ON lb.id = cl.label_id
         WHERE cl.card_id = ?1 ORDER BY lb.position, lb.id LIMIT 2",
    )?;
    let mut out = Vec::with_capacity(rows.len());
    for (mut v, card) in rows {
        let ls: Vec<Value> = labels
            .query_map([card], |r| {
                Ok(json!({ "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "color": r.get::<_, i64>(2)? }))
            })?
            .collect::<Result<_, _>>()?;
        v["labels"] = json!(ls);
        out.push(v);
    }
    Ok(out)
}

pub fn assigned_items(
    c: &Connection,
    user: i64,
    due: Due,
    now: i64,
    projects: Option<&str>,
) -> rusqlite::Result<Vec<Value>> {
    let due = due.clause("i");
    let mut stmt = c.prepare(&format!(
        "SELECT i.id, i.text, i.done, i.due_at, k.id, k.title, b.id, b.name, p.id, p.name, pm.role != 'observer'
         FROM checklist_items i
         JOIN checklists x ON x.id = i.checklist_id
         JOIN cards k ON k.id = x.card_id
         {OPEN}
         WHERE i.assignee_id = ?2 AND {LIVE} AND {VISIBLE} AND {due}
         ORDER BY i.done, i.due_at IS NULL, i.due_at, i.id LIMIT ?1"
    ))?;
    stmt.query_map(params![LIMIT, user, now, projects], |r| {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "text": r.get::<_, String>(1)?,
            "done": r.get::<_, bool>(2)?,
            "due_at": r.get::<_, Option<i64>>(3)?,
            "card": { "id": r.get::<_, i64>(4)?, "title": r.get::<_, String>(5)? },
            "board": { "id": r.get::<_, i64>(6)?, "name": r.get::<_, String>(7)? },
            "project": { "id": r.get::<_, i64>(8)?, "name": r.get::<_, String>(9)? },
            "can_edit": r.get::<_, bool>(10)?,
        }))
    })?
    .collect()
}

async fn mine(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(q): Query<Scope>,
) -> Result<Json<Value>, ApiError> {
    if let Some(slug) = &q.company
        && !valid_slug(slug)
    {
        return Err(ApiError::NotFound);
    }

    let due = Due::parse(q.due.as_deref())?;
    let at = now();
    let id = user.user_id;
    let only = q.company.clone();
    let companies: Vec<(String, String)> = state
        .dbs
        .control()
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT o.slug, o.name FROM org_members m JOIN orgs o ON o.id = m.org_id
                 WHERE m.user_id = ?1 AND o.status = 'active' AND (?2 IS NULL OR o.slug = ?2)
                 ORDER BY o.name COLLATE NOCASE",
            )?;
            Ok(stmt
                .query_map(params![id, only], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?)
        })
        .await?;
    let limits = user.token.clone();
    let companies: Vec<(String, String)> = companies
        .into_iter()
        .filter(|(slug, _)| limits.as_ref().is_none_or(|t| t.allows_company(slug)))
        .collect();
    if q.company.is_some() && companies.is_empty() {
        return Err(ApiError::NotFound);
    }

    let mut cards = Vec::new();
    let mut items = Vec::new();
    for (slug, name) in companies {
        let Ok(db) = state.dbs.tenant(&slug) else {
            continue;
        };
        let projects = limits
            .as_ref()
            .and_then(|t| t.projects.as_ref())
            .map(|list| {
                json!(
                    list.iter()
                        .filter(|(s, _)| *s == slug)
                        .map(|(_, p)| p)
                        .collect::<Vec<_>>()
                )
                .to_string()
            });
        let (found, found_items) = db
            .read(move |c| {
                let only = projects.as_deref();
                Ok((
                    assigned(c, id, due, at, only)?,
                    assigned_items(c, id, due, at, only)?,
                ))
            })
            .await?;
        let company = json!({ "slug": slug, "name": name });
        for (list, rows) in [(&mut cards, found), (&mut items, found_items)] {
            for mut v in rows {
                v["company"] = company.clone();
                list.push(v);
            }
        }
    }
    Ok(Json(json!({ "cards": cards, "items": items })))
}
