use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{board_count, company_person, member_names, purge, role_of, upsert_person};
use crate::admin::{audit, validate};
use crate::auth::check_csrf;
use crate::company::CompanyOwner;
use crate::error::{ApiError, bad, conflict};
use crate::events::{self, At};
use crate::session::now;
use crate::state::AppState;

fn writing(owner: &CompanyOwner, headers: &HeaderMap) -> Result<i64, ApiError> {
    check_csrf(&owner.0.user, headers)?;
    Ok(owner.0.user.0.user_id)
}

fn live_name(c: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    c.query_row(
        "SELECT name FROM projects WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        |r| r.get(0),
    )
    .optional()
}

async fn audit_project(
    state: &AppState,
    by: i64,
    action: &'static str,
    org_id: i64,
    target: String,
    detail: Value,
) -> Result<(), ApiError> {
    state
        .dbs
        .control()
        .write(move |c| {
            audit::record(c, Some(by), action, Some(org_id), &target, detail)?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn list(
    State(state): State<AppState>,
    CompanyOwner(co): CompanyOwner,
) -> Result<Json<Value>, ApiError> {
    super::sync_person(&state, &co).await?;
    purge(&co.db, now()).await?;
    let projects = co
        .db
        .read(|c| {
            let mut stmt = c.prepare(
                "SELECT id, name, archived_at FROM projects WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut out = Vec::new();
            for (id, name, archived) in rows {
                let members = member_names(c, id)?;
                let owners: Vec<&Value> = members.iter().filter(|m| m["role"] == "owner").collect();
                out.push(json!({
                    "id": id,
                    "name": name,
                    "status": if archived.is_some() { "archived" } else { "active" },
                    "owners": owners,
                    "members": members.len(),
                    "boards": board_count(c, id)?,
                }));
            }
            Ok(out)
        })
        .await?;
    let org_id = co.org_id;
    let people = state
        .dbs
        .control()
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT u.id, u.name, u.email, m.role FROM org_members m JOIN users u ON u.id = m.user_id
                 WHERE m.org_id = ?1 AND u.disabled = 0
                 ORDER BY u.name = '', u.name COLLATE NOCASE, u.email",
            )?;
            let rows = stmt
                .query_map([org_id], |r| {
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "name": r.get::<_, String>(1)?,
                        "email": r.get::<_, String>(2)?,
                        "role": r.get::<_, String>(3)?,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(Json(json!({ "projects": projects, "people": people })))
}

#[derive(Deserialize)]
pub struct NewProject {
    name: String,
    owner_id: i64,
}

pub async fn create(
    State(state): State<AppState>,
    owner: CompanyOwner,
    headers: HeaderMap,
    Json(body): Json<NewProject>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = writing(&owner, &headers)?;
    let name = validate::name(&body.name).ok_or_else(|| bad("enter a project name"))?;
    let co = &owner.0;
    let (person_name, person_email) = company_person(&state, co.org_id, body.owner_id).await?;
    let owner_id = body.owner_id;
    let stored = name.clone();
    let id = co
        .db
        .write(move |c| {
            let tx = c.transaction()?;
            upsert_person(&tx, owner_id, &person_name, &person_email)?;
            let color = super::COLORS[stored.len() % super::COLORS.len()];
            tx.execute(
                "INSERT INTO projects (name, color) VALUES (?1, ?2)",
                params![stored, color],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO project_members (project_id, user_id, role) VALUES (?1, ?2, 'owner')",
                params![id, owner_id],
            )?;
            events::record(
                &tx,
                by,
                "project_created",
                At::project(id),
                json!({ "name": stored, "owner": owner_id }),
            )?;
            tx.commit()?;
            Ok(id)
        })
        .await?;
    audit_project(
        &state,
        by,
        "project_created",
        co.org_id,
        name.clone(),
        json!({ "project": id, "owner": body.owner_id }),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

async fn set_archived(
    state: AppState,
    owner: CompanyOwner,
    headers: HeaderMap,
    id: i64,
    archive: bool,
) -> Result<StatusCode, ApiError> {
    let by = writing(&owner, &headers)?;
    let co = owner.0;
    let action = if archive {
        "project_archived"
    } else {
        "project_restored"
    };
    let name = co
        .db
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(name) = live_name(&tx, id)? else {
                return Ok(None);
            };
            let at = archive.then(now);
            tx.execute(
                "UPDATE projects SET archived_at = ?2 WHERE id = ?1",
                params![id, at],
            )?;
            events::record(&tx, by, action, At::project(id), json!({ "name": name }))?;
            tx.commit()?;
            Ok(Some(name))
        })
        .await?
        .ok_or(ApiError::NotFound)?;
    audit_project(
        &state,
        by,
        action,
        co.org_id,
        name,
        json!({ "project": id }),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn archive(
    State(state): State<AppState>,
    owner: CompanyOwner,
    headers: HeaderMap,
    UrlPath((_, id)): UrlPath<(String, i64)>,
) -> Result<StatusCode, ApiError> {
    set_archived(state, owner, headers, id, true).await
}

pub async fn restore(
    State(state): State<AppState>,
    owner: CompanyOwner,
    headers: HeaderMap,
    UrlPath((_, id)): UrlPath<(String, i64)>,
) -> Result<StatusCode, ApiError> {
    set_archived(state, owner, headers, id, false).await
}

#[derive(Deserialize)]
pub struct Confirm {
    confirm: String,
}

pub async fn delete(
    State(state): State<AppState>,
    owner: CompanyOwner,
    headers: HeaderMap,
    UrlPath((_, id)): UrlPath<(String, i64)>,
    Json(body): Json<Confirm>,
) -> Result<StatusCode, ApiError> {
    let by = writing(&owner, &headers)?;
    let co = owner.0;
    let typed = body.confirm.trim().to_string();
    let name = co
        .db
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(name) = live_name(&tx, id)? else {
                return Ok(Err(ApiError::NotFound));
            };
            if name != typed {
                return Ok(Err(bad("type the project name to confirm")));
            }

            let at = now();
            tx.execute(
                "UPDATE projects SET deleted_at = ?2 WHERE id = ?1",
                params![id, at],
            )?;
            events::record(
                &tx,
                by,
                "project_deleted",
                At::project(id),
                json!({ "name": name }),
            )?;
            tx.commit()?;
            Ok(Ok(name))
        })
        .await??;
    let org_id = co.org_id;
    state
        .dbs
        .control()
        .write(move |c| {
            c.execute(
                "UPDATE invites SET revoked_at = ?3
                 WHERE org_id = ?1 AND project_id = ?2 AND revoked_at IS NULL AND project_applied_at IS NULL",
                params![org_id, id, now()],
            )?;
            Ok(())
        })
        .await?;
    audit_project(
        &state,
        by,
        "project_deleted",
        co.org_id,
        name,
        json!({ "project": id, "kept_until": now() + crate::admin::trash::KEEP_SECS }),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct NewOwner {
    user_id: i64,
}

pub async fn add_owner(
    State(state): State<AppState>,
    owner: CompanyOwner,
    headers: HeaderMap,
    UrlPath((_, id)): UrlPath<(String, i64)>,
    Json(body): Json<NewOwner>,
) -> Result<StatusCode, ApiError> {
    let by = writing(&owner, &headers)?;
    let co = owner.0;
    let (name, email) = company_person(&state, co.org_id, body.user_id).await?;
    let user_id = body.user_id;
    let target = email.clone();
    let project = co
        .db
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(project) = live_name(&tx, id)? else {
                return Ok(Err(ApiError::NotFound));
            };
            let role = role_of(&tx, id, user_id)?;
            if role.as_deref() == Some("owner") {
                return Ok(Err(conflict("this person already owns the project")));
            }

            upsert_person(&tx, user_id, &name, &email)?;
            tx.execute(
                "INSERT INTO project_members (project_id, user_id, role) VALUES (?1, ?2, 'owner')
                 ON CONFLICT (project_id, user_id) DO UPDATE SET role = 'owner'",
                params![id, user_id],
            )?;
            events::record(
                &tx,
                by,
                "owner_added",
                At::project(id),
                json!({ "user": user_id, "from": role }),
            )?;
            tx.commit()?;
            Ok(Ok(project))
        })
        .await??;
    let action = if user_id == by {
        "project_joined"
    } else {
        "project_owner_added"
    };
    audit_project(
        &state,
        by,
        action,
        co.org_id,
        target,
        json!({ "project": id, "name": project }),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
