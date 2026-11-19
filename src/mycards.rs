use axum::Router;
use axum::extract::{Query, State};
use axum::response::Json;
use axum::routing::get;
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::boards::VISIBLE;
use crate::db::valid_slug;
use crate::error::ApiError;
use crate::state::AppState;
use crate::tenant::CurrentUser;

pub const LIMIT: i64 = 500;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/v1/me/cards", get(mine))
}

#[derive(Deserialize)]
struct Scope {
    company: Option<String>,
}

pub fn assigned(c: &Connection, user: i64) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = c.prepare(&format!(
        "SELECT k.id, k.title, k.done, k.due_at, l.title, b.id, b.name, p.id, p.name,
                (SELECT count(*) FROM checklist_items i JOIN checklists x ON x.id = i.checklist_id WHERE x.card_id = k.id),
                (SELECT count(*) FROM checklist_items i JOIN checklists x ON x.id = i.checklist_id WHERE x.card_id = k.id AND i.done = 1),
                pm.role != 'observer'
         FROM card_members cm
         JOIN cards k ON k.id = cm.card_id
         JOIN lists l ON l.id = k.list_id
         JOIN boards b ON b.id = k.board_id
         JOIN projects p ON p.id = b.project_id
         JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = ?2
         WHERE cm.user_id = ?2 AND k.archived_at IS NULL AND k.is_template = 0 AND l.archived_at IS NULL
           AND b.closed_at IS NULL AND p.deleted_at IS NULL AND p.archived_at IS NULL AND {VISIBLE}
         ORDER BY k.done, k.due_at IS NULL, k.due_at, k.id LIMIT ?1"
    ))?;
    let rows: Vec<(Value, i64)> = stmt
        .query_map(params![LIMIT, user], |r| {
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
    if q.company.is_some() && companies.is_empty() {
        return Err(ApiError::NotFound);
    }

    let mut cards = Vec::new();
    for (slug, name) in companies {
        let Ok(db) = state.dbs.tenant(&slug) else {
            continue;
        };
        let found = db.read(move |c| Ok(assigned(c, id)?)).await?;
        let company = json!({ "slug": slug, "name": name });
        for mut v in found {
            v["company"] = company.clone();
            cards.push(v);
        }
    }
    Ok(Json(json!({ "cards": cards })))
}
