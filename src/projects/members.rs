use std::collections::HashMap;

use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Act, Project, ProjectRole, path_id, upsert_person};
use crate::admin::{audit, validate};
use crate::error::{ApiError, bad, conflict};
use crate::events::{self, At};
use crate::session::now;
use crate::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    project: Project,
) -> Result<Json<Value>, ApiError> {
    let id = project.id;
    let me = project.user_id();
    let members = project
        .db()
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT pe.id, pe.name, pe.email, m.role, m.last_active
                 FROM project_members m JOIN people pe ON pe.id = m.user_id
                 WHERE m.project_id = ?1
                 ORDER BY m.role = 'owner' DESC, pe.name = '', pe.name COLLATE NOCASE, pe.email",
            )?;
            let rows = stmt
                .query_map([id], |r| {
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "name": r.get::<_, String>(1)?,
                        "email": r.get::<_, String>(2)?,
                        "role": r.get::<_, String>(3)?,
                        "last_active": r.get::<_, Option<i64>>(4)?,
                        "you": r.get::<_, i64>(0)? == me,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    let org_id = project.company.org_id;
    let invites = state
        .dbs
        .control()
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT i.id, i.email, i.project_role, coalesce(nullif(u.name, ''), u.email), i.created_at
                 FROM invites i LEFT JOIN users u ON u.id = i.created_by
                 WHERE i.org_id = ?1 AND i.project_id = ?2 AND i.accepted_at IS NULL AND i.revoked_at IS NULL
                 ORDER BY i.created_at DESC, i.id DESC",
            )?;
            let rows = stmt
                .query_map(params![org_id, id], |r| {
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "email": r.get::<_, String>(1)?,
                        "role": r.get::<_, String>(2)?,
                        "invited_by": r.get::<_, Option<String>>(3)?,
                        "created_at": r.get::<_, i64>(4)?,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(Json(json!({
        "members": members,
        "invites": invites,
        "can": project.permissions(),
    })))
}

#[derive(Deserialize)]
pub struct NewInvite {
    email: String,
    role: String,
}

enum Target {
    Member(i64, String, String),
    Invite,
}

pub async fn invite(
    State(state): State<AppState>,
    project: Project,
    headers: HeaderMap,
    Json(body): Json<NewInvite>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = project.writing(&headers, Act::Invite)?;
    let email = validate::email(&body.email).ok_or_else(|| bad("enter a valid email address"))?;
    let role = ProjectRole::parse(&body.role).ok_or_else(|| bad("choose a role"))?;
    if role == ProjectRole::Owner && !project.can(Act::Manage) {
        return Err(ApiError::Forbidden);
    }

    let org_id = project.company.org_id;
    let id = project.id;
    let lookup = email.clone();
    let target = state
        .dbs
        .control()
        .read(move |c| {
            let found: Option<(i64, String, String)> = c
                .query_row(
                    "SELECT u.id, u.name, u.email FROM org_members m JOIN users u ON u.id = m.user_id
                     WHERE m.org_id = ?1 AND u.email = ?2 AND u.disabled = 0",
                    params![org_id, lookup],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            Ok(match found {
                Some((uid, name, email)) => Target::Member(uid, name, email),
                None => Target::Invite,
            })
        })
        .await?;

    match target {
        Target::Member(uid, name, email) => {
            project
                .db()
                .write(move |c| {
                    let tx = c.transaction()?;
                    let exists: bool = tx.query_row(
                        "SELECT EXISTS (SELECT 1 FROM project_members WHERE project_id = ?1 AND user_id = ?2)",
                        params![id, uid],
                        |r| r.get(0),
                    )?;
                    if exists {
                        return Ok(Err(conflict("this person is already in the project")));
                    }

                    upsert_person(&tx, uid, &name, &email)?;
                    tx.execute(
                        "INSERT INTO project_members (project_id, user_id, role) VALUES (?1, ?2, ?3)",
                        params![id, uid, role.as_str()],
                    )?;
                    events::record(
                        &tx,
                        by,
                        "member_added",
                        At::project(id),
                        json!({ "user": uid, "role": role.as_str() }),
                    )?;
                    tx.commit()?;
                    Ok(Ok(()))
                })
                .await??;
            Ok((StatusCode::CREATED, Json(json!({ "result": "added" }))))
        }
        Target::Invite => {
            let project_name = project.info.name.clone();
            state
                .dbs
                .control()
                .write(move |c| {
                    let tx = c.transaction()?;
                    let pending: bool = tx.query_row(
                        "SELECT EXISTS (SELECT 1 FROM invites WHERE org_id = ?1 AND email = ?2 AND project_id = ?3
                         AND accepted_at IS NULL AND revoked_at IS NULL)",
                        params![org_id, email, id],
                        |r| r.get(0),
                    )?;
                    if pending {
                        return Ok(Err(conflict(
                            "this email address already has a pending invite",
                        )));
                    }

                    tx.execute(
                        "INSERT INTO invites (org_id, email, company_role, project_id, project_role, created_by)
                         VALUES (?1, ?2, 'member', ?3, ?4, ?5)",
                        params![org_id, email, id, role.as_str(), by],
                    )?;
                    let invite = tx.last_insert_rowid();
                    audit::record(
                        &tx,
                        Some(by),
                        "invite_created",
                        Some(org_id),
                        &email,
                        json!({ "role": "member", "project": id, "project_name": project_name, "project_role": role.as_str(), "invite": invite }),
                    )?;
                    tx.commit()?;
                    Ok(Ok(()))
                })
                .await??;
            Ok((StatusCode::CREATED, Json(json!({ "result": "invited" }))))
        }
    }
}

pub async fn revoke(
    State(state): State<AppState>,
    project: Project,
    headers: HeaderMap,
    UrlPath(params): UrlPath<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    let by = project.writing(&headers, Act::Invite)?;
    let invite = path_id(&params, "invite")?;
    let org_id = project.company.org_id;
    let id = project.id;
    let manager = project.can(Act::Manage);
    let done = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let found: Option<(String, String, Option<i64>)> = tx
                .query_row(
                    "SELECT email, project_role, created_by FROM invites WHERE id = ?1 AND org_id = ?2 AND project_id = ?3
                     AND accepted_at IS NULL AND revoked_at IS NULL",
                    params![invite, org_id, id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let Some((email, role, created_by)) = found else {
                return Ok(Err(ApiError::NotFound));
            };
            if !manager && (role == "owner" || created_by != Some(by)) {
                return Ok(Err(ApiError::Forbidden));
            }

            tx.execute(
                "UPDATE invites SET revoked_at = ?2 WHERE id = ?1",
                params![invite, now()],
            )?;
            audit::record(
                &tx,
                Some(by),
                "invite_revoked",
                Some(org_id),
                &email,
                json!({ "invite": invite, "project": id }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await?;
    done?;
    Ok(StatusCode::NO_CONTENT)
}

fn owners(c: &Connection, project: i64) -> rusqlite::Result<i64> {
    c.query_row(
        "SELECT count(*) FROM project_members WHERE project_id = ?1 AND role = 'owner'",
        [project],
        |r| r.get(0),
    )
}

fn role_of(c: &Connection, project: i64, user: i64) -> rusqlite::Result<Option<String>> {
    c.query_row(
        "SELECT role FROM project_members WHERE project_id = ?1 AND user_id = ?2",
        params![project, user],
        |r| r.get(0),
    )
    .optional()
}

#[derive(Deserialize)]
pub struct RoleChange {
    role: String,
}

pub async fn change(
    project: Project,
    headers: HeaderMap,
    UrlPath(params): UrlPath<HashMap<String, String>>,
    Json(body): Json<RoleChange>,
) -> Result<StatusCode, ApiError> {
    let by = project.writing(&headers, Act::Manage)?;
    let user = path_id(&params, "user")?;
    let role = ProjectRole::parse(&body.role).ok_or_else(|| bad("choose a role"))?;
    let id = project.id;
    project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(old) = role_of(&tx, id, user)? else {
                return Ok(Err(ApiError::NotFound));
            };
            if old == role.as_str() {
                return Ok(Ok(()));
            }

            if old == "owner" && owners(&tx, id)? <= 1 {
                return Ok(Err(bad("a project needs at least one owner")));
            }

            tx.execute(
                "UPDATE project_members SET role = ?3 WHERE project_id = ?1 AND user_id = ?2",
                params![id, user, role.as_str()],
            )?;
            events::record(
                &tx,
                by,
                "member_role_changed",
                At::project(id),
                json!({ "user": user, "from": old, "to": role.as_str() }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove(
    project: Project,
    headers: HeaderMap,
    UrlPath(params): UrlPath<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    let by = project.writing(&headers, Act::Manage)?;
    let user = path_id(&params, "user")?;
    let id = project.id;
    project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(old) = role_of(&tx, id, user)? else {
                return Ok(Err(ApiError::NotFound));
            };
            if old == "owner" && owners(&tx, id)? <= 1 {
                return Ok(Err(bad("a project needs at least one owner")));
            }

            tx.execute(
                "DELETE FROM project_members WHERE project_id = ?1 AND user_id = ?2",
                params![id, user],
            )?;
            tx.execute(
                "DELETE FROM board_access WHERE user_id = ?1 AND board_id IN (SELECT id FROM boards WHERE project_id = ?2)",
                params![user, id],
            )?;
            events::record(
                &tx,
                by,
                "member_removed",
                At::project(id),
                json!({ "user": user, "role": old }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}
