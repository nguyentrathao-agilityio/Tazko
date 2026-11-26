pub mod manage;
pub mod members;

use std::collections::HashMap;

use axum::Router;
use axum::extract::{FromRequestParts, Path, State};
use axum::http::HeaderMap;
use axum::http::request::Parts;
use axum::response::Json;
use axum::routing::{delete, get, patch, post};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::COMPANY_PREFIX;
use crate::auth::check_csrf;
use crate::db::Database;
use crate::error::{ApiError, bad};
use crate::events::{self, At};
use crate::session::now;
use crate::state::AppState;
use crate::tenant::Company;

pub const COLORS: &[&str] = &["c0", "c1", "c2", "c3", "c4"];

pub const BACKGROUNDS: &[&str] = &[
    "dusk", "sea", "sand", "meadow", "rose", "slate", "mist", "ink", "leaf", "coral", "ocean",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectRole {
    Owner,
    Member,
    Observer,
}

impl ProjectRole {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "owner" => Some(ProjectRole::Owner),
            "member" => Some(ProjectRole::Member),
            "observer" => Some(ProjectRole::Observer),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ProjectRole::Owner => "owner",
            ProjectRole::Member => "member",
            ProjectRole::Observer => "observer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Read,
    Edit,
    Manage,
    Invite,
    CreateBoard,
}

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub name: String,
    pub description: String,
    pub color: String,
    pub background: String,
    pub members_create_boards: bool,
    pub members_invite: bool,
    pub voting: bool,
    pub aging: bool,
    pub aging_days: i64,
}

pub struct Project {
    pub company: Company,
    pub id: i64,
    pub role: ProjectRole,
    pub info: ProjectInfo,
}

impl Project {
    pub fn can(&self, act: Act) -> bool {
        if self
            .company
            .user
            .0
            .token
            .as_ref()
            .is_some_and(|t| !t.allows(act))
        {
            return false;
        }

        match (self.role, act) {
            (_, Act::Read) => true,
            (ProjectRole::Owner, _) => true,
            (ProjectRole::Member, Act::Edit) => true,
            (ProjectRole::Member, Act::CreateBoard) => self.info.members_create_boards,
            (ProjectRole::Member, Act::Invite) => self.info.members_invite,
            _ => false,
        }
    }

    pub fn user_id(&self) -> i64 {
        self.company.user.0.user_id
    }

    pub fn db(&self) -> &Database {
        &self.company.db
    }

    pub fn writing(&self, headers: &HeaderMap, act: Act) -> Result<i64, ApiError> {
        check_csrf(&self.company.user, headers)?;
        if self.can(act) {
            Ok(self.user_id())
        } else {
            Err(ApiError::Forbidden)
        }
    }

    pub fn permissions(&self) -> Value {
        json!({
            "edit": self.can(Act::Edit),
            "manage": self.can(Act::Manage),
            "invite": self.can(Act::Invite),
            "create_board": self.can(Act::CreateBoard),
        })
    }
}

pub fn path_id(params: &HashMap<String, String>, key: &str) -> Result<i64, ApiError> {
    params
        .get(key)
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .ok_or(ApiError::NotFound)
}

pub fn project_info(c: &Connection, id: i64) -> rusqlite::Result<Option<ProjectInfo>> {
    c.query_row(
        "SELECT name, description, color, background, members_create_boards, members_invite, voting, aging, aging_days
         FROM projects WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NULL",
        [id],
        |r| {
            Ok(ProjectInfo {
                name: r.get(0)?,
                description: r.get(1)?,
                color: r.get(2)?,
                background: r.get(3)?,
                members_create_boards: r.get(4)?,
                members_invite: r.get(5)?,
                voting: r.get(6)?,
                aging: r.get(7)?,
                aging_days: r.get(8)?,
            })
        },
    )
    .optional()
}

impl Project {
    pub async fn load(company: Company, id: i64) -> Result<Project, ApiError> {
        if company
            .user
            .0
            .token
            .as_ref()
            .is_some_and(|t| !t.allows_project(&company.slug, id))
        {
            return Err(ApiError::NotFound);
        }

        let user_id = company.user.0.user_id;
        let found = company
            .db
            .read(move |c| {
                let Some(role) = role_of(c, id, user_id)? else {
                    return Ok(None);
                };
                Ok(project_info(c, id)?.map(|info| (role, info)))
            })
            .await?;
        let (role, info) = found.ok_or(ApiError::NotFound)?;
        let role = ProjectRole::parse(&role).ok_or(ApiError::NotFound)?;
        Ok(Project {
            company,
            id,
            role,
            info,
        })
    }
}

impl FromRequestParts<AppState> for Project {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let company = Company::from_request_parts(parts, state).await?;
        let Path(params) = Path::<HashMap<String, String>>::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::NotFound)?;
        let id = path_id(&params, "project")?;
        Project::load(company, id).await
    }
}

pub(crate) fn role_of(c: &Connection, project: i64, user: i64) -> rusqlite::Result<Option<String>> {
    c.query_row(
        "SELECT role FROM project_members WHERE project_id = ?1 AND user_id = ?2",
        params![project, user],
        |r| r.get(0),
    )
    .optional()
}

pub(crate) fn drop_member(c: &Connection, project: i64, user: i64) -> rusqlite::Result<()> {
    c.execute(
        "DELETE FROM project_members WHERE project_id = ?1 AND user_id = ?2",
        params![project, user],
    )?;
    c.execute(
        "DELETE FROM board_access WHERE user_id = ?1 AND board_id IN (SELECT id FROM boards WHERE project_id = ?2)",
        params![user, project],
    )?;
    Ok(())
}

pub fn upsert_person(c: &Connection, id: i64, name: &str, email: &str) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO people (id, name, email) VALUES (?1, ?2, ?3)
         ON CONFLICT (id) DO UPDATE SET name = excluded.name, email = excluded.email",
        params![id, name, email],
    )?;
    Ok(())
}

pub async fn company_person(
    state: &AppState,
    org_id: i64,
    user_id: i64,
) -> Result<(String, String), ApiError> {
    state
        .dbs
        .control()
        .read(move |c| {
            Ok(c.query_row(
                "SELECT u.name, u.email FROM org_members m JOIN users u ON u.id = m.user_id
                 WHERE m.org_id = ?1 AND m.user_id = ?2 AND u.disabled = 0",
                params![org_id, user_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?)
        })
        .await?
        .ok_or_else(|| bad("choose a member of this company"))
}

pub async fn sync_person(state: &AppState, company: &Company) -> Result<(), ApiError> {
    let user = company.user.0.clone();
    let org_id = company.org_id;
    let user_id = user.user_id;
    let stored = company
        .db
        .read(move |c| {
            Ok(c.query_row(
                "SELECT name, email FROM people WHERE id = ?1",
                [user_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?)
        })
        .await?;
    let fresh = stored.is_some_and(|(name, email)| name == user.name && email == user.email);
    let email = user.email.clone();
    let pending: Vec<(i64, i64, String)> = state
        .dbs
        .control()
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT id, project_id, project_role FROM invites
                 WHERE org_id = ?1 AND email = ?2 AND project_id IS NOT NULL
                   AND accepted_at IS NOT NULL AND revoked_at IS NULL AND project_applied_at IS NULL",
            )?;
            let rows = stmt
                .query_map(params![org_id, email], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    if fresh && pending.is_empty() {
        return Ok(());
    }

    let applied: Vec<i64> = pending.iter().map(|p| p.0).collect();
    company
        .db
        .write(move |c| {
            let tx = c.transaction()?;
            upsert_person(&tx, user_id, &user.name, &user.email)?;
            for (_, project_id, role) in &pending {
                if project_info(&tx, *project_id)?.is_none() {
                    continue;
                }

                let added = tx.execute(
                    "INSERT INTO project_members (project_id, user_id, role) VALUES (?1, ?2, ?3)
                     ON CONFLICT (project_id, user_id) DO NOTHING",
                    params![project_id, user_id, role],
                )?;
                if added > 0 {
                    events::record(
                        &tx,
                        user_id,
                        "member_joined",
                        At::project(*project_id),
                        json!({ "user": user_id, "role": role }),
                    )?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await?;
    if !applied.is_empty() {
        state
            .dbs
            .control()
            .write(move |c| {
                let at = now();
                for id in &applied {
                    c.execute(
                        "UPDATE invites SET project_applied_at = ?2 WHERE id = ?1",
                        params![id, at],
                    )?;
                }
                Ok(())
            })
            .await?;
    }

    Ok(())
}

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}{s}");
    Router::new()
        .route(&p("/my-projects"), get(mine))
        .route(&p("/projects"), get(manage::list).post(manage::create))
        .route(
            &p("/projects/{project}"),
            get(view).patch(update).delete(manage::delete),
        )
        .route(&p("/projects/{project}/archive"), post(manage::archive))
        .route(&p("/projects/{project}/restore"), post(manage::restore))
        .route(&p("/projects/{project}/owners"), post(manage::add_owner))
        .route(&p("/projects/{project}/members"), get(members::list))
        .route(
            &p("/projects/{project}/members/{user}"),
            patch(members::change).delete(members::remove),
        )
        .route(&p("/projects/{project}/invites"), post(members::invite))
        .route(
            &p("/projects/{project}/invites/{invite}"),
            delete(members::revoke),
        )
}

pub fn member_names(c: &Connection, project_id: i64) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = c.prepare(
        "SELECT pe.id, pe.name, pe.email, m.role FROM project_members m JOIN people pe ON pe.id = m.user_id
         WHERE m.project_id = ?1
         ORDER BY m.role = 'owner' DESC, pe.name = '', pe.name COLLATE NOCASE, pe.email",
    )?;
    stmt.query_map([project_id], |r| {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "name": r.get::<_, String>(1)?,
            "email": r.get::<_, String>(2)?,
            "role": r.get::<_, String>(3)?,
        }))
    })?
    .collect()
}

pub fn board_count(c: &Connection, project_id: i64) -> rusqlite::Result<i64> {
    let exists: bool = c.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'boards')",
        [],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(0);
    }

    c.query_row(
        "SELECT count(*) FROM boards WHERE project_id = ?1 AND closed_at IS NULL",
        [project_id],
        |r| r.get(0),
    )
}

async fn mine(State(state): State<AppState>, company: Company) -> Result<Json<Value>, ApiError> {
    sync_person(&state, &company).await?;
    let user_id = company.user.0.user_id;
    let limits = company.user.0.token.clone();
    let slug = company.slug.clone();
    let projects = company
        .db
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT p.id, p.name, p.description, p.color, m.role
                 FROM project_members m JOIN projects p ON p.id = m.project_id
                 WHERE m.user_id = ?1 AND p.deleted_at IS NULL AND p.archived_at IS NULL
                 ORDER BY p.name COLLATE NOCASE",
            )?;
            let rows = stmt
                .query_map([user_id], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut out = Vec::new();
            for (id, name, description, color, role) in rows {
                if limits
                    .as_ref()
                    .is_some_and(|t| !t.allows_project(&slug, id))
                {
                    continue;
                }

                out.push(json!({
                    "id": id,
                    "name": name,
                    "description": description,
                    "color": color,
                    "role": role,
                    "members": member_names(c, id)?,
                    "boards": board_count(c, id)?,
                }));
            }
            Ok(out)
        })
        .await?;
    Ok(Json(json!({ "projects": projects })))
}

async fn view(State(state): State<AppState>, project: Project) -> Result<Json<Value>, ApiError> {
    sync_person(&state, &project.company).await?;
    let id = project.id;
    let user_id = project.user_id();
    let members = project
        .db()
        .write(move |c| {
            c.execute(
                "UPDATE project_members SET last_active = ?3 WHERE project_id = ?1 AND user_id = ?2",
                params![id, user_id, now()],
            )?;
            Ok(c.query_row(
                "SELECT count(*) FROM project_members WHERE project_id = ?1",
                [id],
                |r| r.get::<_, i64>(0),
            )?)
        })
        .await?;
    let i = &project.info;
    Ok(Json(json!({
        "id": project.id,
        "name": i.name,
        "description": i.description,
        "color": i.color,
        "background": i.background,
        "members_create_boards": i.members_create_boards,
        "members_invite": i.members_invite,
        "voting": i.voting,
        "aging": i.aging,
        "aging_days": i.aging_days,
        "role": project.role.as_str(),
        "member_count": members,
        "can": project.permissions(),
    })))
}

#[derive(Deserialize)]
pub struct ProjectChange {
    name: Option<String>,
    description: Option<String>,
    color: Option<String>,
    background: Option<String>,
    members_create_boards: Option<bool>,
    members_invite: Option<bool>,
    voting: Option<bool>,
    aging: Option<bool>,
    aging_days: Option<i64>,
}

async fn update(
    project: Project,
    headers: HeaderMap,
    Json(body): Json<ProjectChange>,
) -> Result<axum::http::StatusCode, ApiError> {
    let by = project.writing(&headers, Act::Manage)?;
    let mut info = project.info.clone();
    let before = project.info.clone();
    if let Some(n) = &body.name {
        info.name = crate::admin::validate::name(n).ok_or_else(|| bad("enter a project name"))?;
    }

    if let Some(d) = &body.description {
        let d = d.trim();
        if d.chars().count() > 500 {
            return Err(bad("the description is too long"));
        }

        info.description = d.to_string();
    }

    if let Some(c) = &body.color {
        if !COLORS.contains(&c.as_str()) {
            return Err(bad("choose one of the colours"));
        }

        info.color = c.clone();
    }

    if let Some(b) = &body.background {
        if !BACKGROUNDS.contains(&b.as_str()) {
            return Err(bad("choose one of the backgrounds"));
        }

        info.background = b.clone();
    }

    if let Some(v) = body.members_create_boards {
        info.members_create_boards = v;
    }

    if let Some(v) = body.members_invite {
        info.members_invite = v;
    }

    if let Some(v) = body.voting {
        info.voting = v;
    }

    if let Some(v) = body.aging {
        info.aging = v;
    }

    if let Some(d) = body.aging_days {
        if ![3, 7, 14, 30].contains(&d) {
            return Err(bad("choose 3, 7, 14 or 30 days"));
        }

        info.aging_days = d;
    }

    let id = project.id;
    project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            tx.execute(
                "UPDATE projects SET name = ?2, description = ?3, color = ?4, background = ?5,
                 members_create_boards = ?6, members_invite = ?7, voting = ?8, aging = ?9, aging_days = ?10 WHERE id = ?1",
                params![
                    id,
                    info.name,
                    info.description,
                    info.color,
                    info.background,
                    info.members_create_boards,
                    info.members_invite,
                    info.voting,
                    info.aging,
                    info.aging_days
                ],
            )?;
            events::record(
                &tx,
                by,
                "project_updated",
                At::project(id),
                json!({
                    "name": info.name,
                    "members_create_boards": info.members_create_boards,
                    "members_invite": info.members_invite,
                    "voting": info.voting,
                    "aging": info.aging,
                    "aging_days": info.aging_days,
                }),
            )?;
            if (before.voting, before.aging, before.aging_days) != (info.voting, info.aging, info.aging_days) {
                let boards: Vec<i64> = tx
                    .prepare("SELECT id FROM boards WHERE project_id = ?1 AND closed_at IS NULL")?
                    .query_map([id], |r| r.get(0))?
                    .collect::<Result<_, _>>()?;
                for b in boards {
                    events::record(&tx, by, "card_settings_changed", At::board(id, b), json!({ "voting": info.voting, "aging": info.aging, "aging_days": info.aging_days }))?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

fn revoke_tokens_of(c: &Connection, user_id: i64) -> rusqlite::Result<usize> {
    c.execute(
        "UPDATE api_tokens SET revoked_at = ?2 WHERE created_by = ?1 AND revoked_at IS NULL",
        params![user_id, now()],
    )
}

fn tenant_if_any(state: &AppState, slug: &str) -> Result<Option<crate::db::Database>, ApiError> {
    match state.dbs.tenant(slug) {
        Ok(db) => Ok(Some(db)),
        Err(crate::db::DbError::Missing(_)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub async fn revoke_company_tokens(
    state: &AppState,
    slug: &str,
    user_id: i64,
) -> Result<(), ApiError> {
    if let Some(db) = tenant_if_any(state, slug)? {
        db.write(move |c| Ok(revoke_tokens_of(c, user_id)?)).await?;
    }

    Ok(())
}

pub async fn forget_member(
    state: &AppState,
    slug: &str,
    org_id: i64,
    user_id: i64,
) -> Result<(), ApiError> {
    state
        .dbs
        .control()
        .write(move |c| {
            c.execute(
                "UPDATE invites SET revoked_at = ?3
                 WHERE org_id = ?1 AND project_id IS NOT NULL AND revoked_at IS NULL AND project_applied_at IS NULL
                   AND email = (SELECT email FROM users WHERE id = ?2)",
                params![org_id, user_id, now()],
            )?;
            Ok(())
        })
        .await?;
    let Some(db) = tenant_if_any(state, slug)? else {
        return Ok(());
    };
    db.write(move |c| {
        let tx = c.transaction()?;
        let projects: Vec<(i64, String)> = {
            let mut stmt =
                tx.prepare("SELECT project_id, role FROM project_members WHERE user_id = ?1")?;
            stmt.query_map([user_id], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?
        };
        revoke_tokens_of(&tx, user_id)?;
        for (project, role) in projects {
            drop_member(&tx, project, user_id)?;
            events::record(
                &tx,
                user_id,
                "member_removed",
                At::project(project),
                json!({ "user": user_id, "role": role, "reason": "left the company" }),
            )?;
        }
        tx.commit()?;
        Ok(())
    })
    .await?;
    Ok(())
}

pub async fn purge(db: &Database, now: i64) -> Result<usize, ApiError> {
    Ok(db
        .write(move |c| {
            Ok(c.execute(
                "DELETE FROM projects WHERE deleted_at IS NOT NULL AND deleted_at <= ?1",
                [now - crate::admin::trash::KEEP_SECS],
            )?)
        })
        .await?)
}
