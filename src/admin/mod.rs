pub mod audit;
pub mod cli;
pub mod trash;
pub mod validate;

use std::path::Path;

use axum::Router;
use axum::extract::{FromRequestParts, Path as UrlPath, Query, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{delete, get, patch, post};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::check_csrf;
use crate::db::{Database, DbError};
use crate::error::{ApiError, bad};
use crate::session::{SessionUser, now};
use crate::state::AppState;
use crate::tenant::CurrentUser;
use validate::GB;

pub struct SuperAdmin(pub SessionUser);

impl FromRequestParts<AppState> for SuperAdmin {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let CurrentUser(user) = CurrentUser::from_request_parts(parts, state).await?;
        if user.is_super_admin && user.token.is_none() {
            Ok(SuperAdmin(user))
        } else {
            Err(ApiError::NotFound)
        }
    }
}

impl SuperAdmin {
    fn writing(&self, headers: &HeaderMap) -> Result<i64, ApiError> {
        check_csrf(&CurrentUser(self.0.clone()), headers)?;
        Ok(self.0.user_id)
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/admin/companies",
            get(list_companies).post(create_company),
        )
        .route(
            "/api/v1/admin/companies/{slug}",
            get(company_detail)
                .patch(update_company)
                .delete(delete_company),
        )
        .route(
            "/api/v1/admin/companies/{slug}/suspend",
            post(suspend_company),
        )
        .route(
            "/api/v1/admin/companies/{slug}/resume",
            post(resume_company),
        )
        .route("/api/v1/admin/companies/{slug}/invites", post(invite))
        .route(
            "/api/v1/admin/companies/{slug}/members/{user_id}",
            patch(change_role).delete(remove_member),
        )
        .route("/api/v1/admin/invites", get(list_invites))
        .route("/api/v1/admin/invites/{id}", delete(revoke_invite))
        .route("/api/v1/admin/people", get(list_people))
        .route("/api/v1/admin/people/{id}", patch(update_person))
        .route("/api/v1/admin/audit", get(list_audit))
        .route("/api/v1/admin/server", get(server))
}

pub(crate) const INVALID_EMAIL: &str = "enter a valid email address";
const NAME_RULE: &str = "enter a company name of up to 80 characters";
const STORAGE_RULE: &str = "the storage limit must be between 1 and 10000 GB";
const LAST_OWNER: &str = "a company needs at least one owner";

pub(crate) fn conflict(message: &str) -> Response {
    crate::error::conflict(message).into_response()
}

enum Outcome<T> {
    Done(T),
    Refused(ApiError),
}

fn refuse<T>(e: ApiError) -> Result<Outcome<T>, DbError> {
    Ok(Outcome::Refused(e))
}

fn finish<T>(o: Outcome<T>) -> Result<T, ApiError> {
    match o {
        Outcome::Done(t) => Ok(t),
        Outcome::Refused(e) => Err(e),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Org {
    pub(crate) id: i64,
    pub(crate) slug: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) storage_limit: Option<i64>,
    pub(crate) created_at: i64,
}

fn org_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Org> {
    Ok(Org {
        id: r.get(0)?,
        slug: r.get(1)?,
        name: r.get(2)?,
        status: r.get(3)?,
        storage_limit: r.get(4)?,
        created_at: r.get(5)?,
    })
}

pub(crate) fn org(c: &Connection, slug: &str) -> rusqlite::Result<Option<Org>> {
    c.query_row(
        "SELECT id, slug, name, status, storage_limit, created_at FROM orgs WHERE slug = ?1 AND status != 'deleted'",
        params![slug],
        org_from_row,
    )
    .optional()
}

pub(crate) fn org_storage_used(state: &AppState, slug: &str) -> u64 {
    dir_size(&state.dbs.layout().orgs.join(slug))
}

pub fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

fn table_exists(c: &Connection, name: &str) -> rusqlite::Result<bool> {
    c.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [name],
        |r| r.get(0),
    )
}

#[derive(Default)]
struct TenantSummary {
    boards: i64,
    last_event: Option<i64>,
    projects: Vec<Value>,
}

async fn summary_of(state: &AppState, slug: &str) -> Result<TenantSummary, ApiError> {
    match state.dbs.tenant(slug) {
        Ok(db) => tenant_summary(&db).await,
        Err(_) => Ok(TenantSummary::default()),
    }
}

async fn tenant_summary(db: &Database) -> Result<TenantSummary, ApiError> {
    Ok(db
        .read(|c| {
            let boards = if table_exists(c, "boards")? {
                c.query_row("SELECT count(*) FROM boards", [], |r| r.get(0))?
            } else {
                0
            };
            let last_event = c.query_row("SELECT max(created_at) FROM events", [], |r| r.get(0))?;
            let mut projects = Vec::new();
            if table_exists(c, "projects")? && table_exists(c, "project_members")? {
                let mut stmt = c.prepare(
                    "SELECT p.id, p.name,
                            (SELECT count(*) FROM project_members m WHERE m.project_id = p.id),
                            (SELECT group_concat(coalesce(nullif(pe.name, ''), nullif(pe.email, ''), 'Unnamed'), ', ')
                               FROM project_members m JOIN people pe ON pe.id = m.user_id
                              WHERE m.project_id = p.id AND m.role = 'owner')
                     FROM projects p WHERE p.deleted_at IS NULL ORDER BY p.name COLLATE NOCASE",
                )?;
                let rows = stmt
                    .query_map([], |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, Option<String>>(3)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                for (id, name, members, owners) in rows {
                    projects.push(json!({
                        "name": name,
                        "members": members,
                        "boards": crate::projects::board_count(c, id)?,
                        "owners": owners
                            .map(|s| s.split(", ").map(str::to_string).collect::<Vec<_>>())
                            .unwrap_or_default(),
                    }));
                }
            }
            Ok(TenantSummary {
                boards,
                last_event,
                projects,
            })
        })
        .await?)
}

async fn list_companies(
    State(state): State<AppState>,
    _admin: SuperAdmin,
) -> Result<Json<Value>, ApiError> {
    let orgs = state
        .dbs
        .control()
        .read(|c| {
            let mut stmt = c.prepare(
                "SELECT o.id, o.slug, o.name, o.status, o.storage_limit, o.created_at,
                        (SELECT count(*) FROM org_members m WHERE m.org_id = o.id)
                 FROM orgs o WHERE o.status != 'deleted' ORDER BY o.name COLLATE NOCASE",
            )?;
            let rows = stmt
                .query_map([], |r| Ok((org_from_row(r)?, r.get::<_, i64>(6)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;

    let mut out = Vec::new();
    for (o, members) in orgs {
        let summary = summary_of(&state, &o.slug).await?;
        out.push(json!({
            "slug": o.slug,
            "name": o.name,
            "status": o.status,
            "members": members,
            "boards": summary.boards,
            "storage_used": org_storage_used(&state, &o.slug),
            "storage_limit": o.storage_limit,
            "last_activity": summary.last_event.unwrap_or(o.created_at),
        }));
    }

    Ok(Json(json!({ "companies": out })))
}

#[derive(Deserialize)]
struct NewCompany {
    name: String,
    slug: Option<String>,
    owner_email: String,
    storage_gb: Option<i64>,
}

pub(crate) enum Invited {
    Added,
    Invited,
}

pub(crate) fn invite_or_add(
    tx: &Connection,
    org: &Org,
    email: &str,
    role: &str,
    by: i64,
    add_existing: bool,
) -> Result<Result<Invited, &'static str>, DbError> {
    let found: Option<(i64, bool)> = tx
        .query_row(
            "SELECT id, disabled FROM users WHERE email = ?1",
            [email],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let user = found.map(|(id, _)| id);
    let addable = found
        .filter(|(_, disabled)| add_existing && !disabled)
        .map(|(id, _)| id);
    if let Some(user_id) = user {
        let member: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM org_members WHERE org_id = ?1 AND user_id = ?2)",
            params![org.id, user_id],
            |r| r.get(0),
        )?;
        if member {
            return Ok(Err("this person is already a member"));
        }
    }

    let pending: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM invites WHERE org_id = ?1 AND email = ?2 AND accepted_at IS NULL AND revoked_at IS NULL)",
        params![org.id, email],
        |r| r.get(0),
    )?;
    if pending {
        return Ok(Err("this email address already has a pending invite"));
    }

    let accepted = addable.map(|_| now());
    tx.execute(
        "INSERT INTO invites (org_id, email, company_role, created_by, accepted_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![org.id, email, role, by, accepted],
    )?;
    let invite_id = tx.last_insert_rowid();
    audit::record(
        tx,
        Some(by),
        "invite_created",
        Some(org.id),
        email,
        json!({ "role": role, "company": org.slug, "invite": invite_id }),
    )?;
    if let Some(user_id) = addable {
        tx.execute(
            "INSERT INTO org_members (org_id, user_id, role) VALUES (?1, ?2, ?3)",
            params![org.id, user_id, role],
        )?;
        return Ok(Ok(Invited::Added));
    }

    Ok(Ok(Invited::Invited))
}

async fn create_company(
    State(state): State<AppState>,
    admin: SuperAdmin,
    headers: HeaderMap,
    Json(body): Json<NewCompany>,
) -> Result<Response, ApiError> {
    let by = admin.writing(&headers)?;
    let name = validate::name(&body.name).ok_or_else(|| bad(NAME_RULE))?;
    let slug = validate::slug(body.slug.as_deref(), &name).ok_or_else(|| {
        bad("the short name may use a to z, 0 to 9 and inner hyphens, up to 40 characters")
    })?;
    let email = validate::email(&body.owner_email)
        .ok_or_else(|| bad("enter a valid email address for the first owner"))?;
    let gb = validate::storage_gb(body.storage_gb).ok_or_else(|| bad(STORAGE_RULE))?;
    if state.dbs.layout().orgs.join(&slug).exists() {
        return Ok(conflict(
            "a folder for that short name is already on disk; move it away or choose another short name",
        ));
    }

    let created = state
        .dbs
        .control()
        .write({
            let (slug, name) = (slug.clone(), name.clone());
            move |c| {
                let tx = c.transaction()?;
                let taken: bool = tx.query_row(
                    "SELECT EXISTS (SELECT 1 FROM orgs WHERE slug = ?1)",
                    [&slug],
                    |r| r.get(0),
                )?;
                if taken {
                    return Ok(None);
                }

                tx.execute(
                    "INSERT INTO orgs (slug, name, storage_limit) VALUES (?1, ?2, ?3)",
                    params![slug, name, gb * GB],
                )?;
                let o = org(&tx, &slug)?.expect("just inserted");
                audit::record(
                    &tx,
                    Some(by),
                    "company_created",
                    Some(o.id),
                    &name,
                    json!({ "slug": slug, "storage_gb": gb }),
                )?;
                let _ = invite_or_add(&tx, &o, &email, "owner", by, true)?;
                tx.commit()?;
                Ok(Some(o))
            }
        })
        .await?;
    let Some(o) = created else {
        return Ok(conflict(
            "that short name is in use; a deleted company keeps its short name for 30 days",
        ));
    };

    state.dbs.create_tenant(&o.slug)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "slug": o.slug, "name": o.name })),
    )
        .into_response())
}

async fn company_detail(
    State(state): State<AppState>,
    _admin: SuperAdmin,
    UrlPath(slug): UrlPath<String>,
) -> Result<Json<Value>, ApiError> {
    let found = state
        .dbs
        .control()
        .read({
            let slug = slug.clone();
            move |c| {
                let Some(o) = org(c, &slug)? else {
                    return Ok(None);
                };
                let mut stmt = c.prepare(
                    "SELECT u.id, u.name, u.email, m.role, u.last_seen, u.disabled
                     FROM org_members m JOIN users u ON u.id = m.user_id
                     WHERE m.org_id = ?1 ORDER BY m.role = 'owner' DESC, u.name COLLATE NOCASE, u.email",
                )?;
                let members: Vec<Value> = stmt
                    .query_map([o.id], |r| {
                        Ok(json!({
                            "id": r.get::<_, i64>(0)?,
                            "name": r.get::<_, String>(1)?,
                            "email": r.get::<_, String>(2)?,
                            "role": r.get::<_, String>(3)?,
                            "last_seen": r.get::<_, Option<i64>>(4)?,
                            "disabled": r.get::<_, bool>(5)?,
                        }))
                    })?
                    .collect::<Result<_, _>>()?;
                let invites = pending_invites(c, Some(o.id))?;
                Ok(Some((o, members, invites)))
            }
        })
        .await?;
    let (o, members, invites) = found.ok_or(ApiError::NotFound)?;
    let summary = summary_of(&state, &o.slug).await?;

    Ok(Json(json!({
        "slug": o.slug,
        "name": o.name,
        "status": o.status,
        "storage_limit": o.storage_limit,
        "storage_used": org_storage_used(&state, &o.slug),
        "boards": summary.boards,
        "members": members,
        "invites": invites,
        "projects": summary.projects,
    })))
}

pub(crate) fn pending_invites(c: &Connection, org_id: Option<i64>) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = c.prepare(
        "SELECT i.id, i.email, i.company_role, coalesce(nullif(u.name, ''), u.email), i.created_at, o.slug, o.name
         FROM invites i JOIN orgs o ON o.id = i.org_id LEFT JOIN users u ON u.id = i.created_by
         WHERE i.accepted_at IS NULL AND i.revoked_at IS NULL AND o.status != 'deleted'
           AND (?1 IS NULL OR i.org_id = ?1)
         ORDER BY i.created_at DESC, i.id DESC",
    )?;
    stmt.query_map([org_id], |r| {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "email": r.get::<_, String>(1)?,
            "role": r.get::<_, String>(2)?,
            "invited_by": r.get::<_, Option<String>>(3)?,
            "created_at": r.get::<_, i64>(4)?,
            "company_slug": r.get::<_, String>(5)?,
            "company_name": r.get::<_, String>(6)?,
        }))
    })?
    .collect()
}

#[derive(Deserialize)]
struct CompanyChange {
    name: Option<String>,
    storage_gb: Option<i64>,
}

async fn update_company(
    State(state): State<AppState>,
    admin: SuperAdmin,
    headers: HeaderMap,
    UrlPath(slug): UrlPath<String>,
    Json(body): Json<CompanyChange>,
) -> Result<StatusCode, ApiError> {
    let by = admin.writing(&headers)?;
    let name = body
        .name
        .as_deref()
        .map(|n| validate::name(n).ok_or_else(|| bad(NAME_RULE)))
        .transpose()?;
    let gb = body
        .storage_gb
        .map(|g| validate::storage_gb(Some(g)).ok_or_else(|| bad(STORAGE_RULE)))
        .transpose()?;
    let outcome = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(o) = org(&tx, &slug)? else {
                return refuse(ApiError::NotFound);
            };
            let new_name = name.unwrap_or_else(|| o.name.clone());
            let new_limit = gb.map(|g| g * GB).or(o.storage_limit);
            tx.execute(
                "UPDATE orgs SET name = ?2, storage_limit = ?3 WHERE id = ?1",
                params![o.id, new_name, new_limit],
            )?;
            audit::record(
                &tx,
                Some(by),
                "company_updated",
                Some(o.id),
                &new_name,
                json!({ "from_name": o.name, "storage_gb": new_limit.map(|l| l / GB) }),
            )?;
            tx.commit()?;
            Ok(Outcome::Done(()))
        })
        .await?;
    finish(outcome)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_status(
    state: &AppState,
    by: i64,
    slug: String,
    to: &'static str,
) -> Result<StatusCode, ApiError> {
    let action = if to == "suspended" {
        "company_suspended"
    } else {
        "company_resumed"
    };
    let outcome = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(o) = org(&tx, &slug)? else {
                return refuse(ApiError::NotFound);
            };
            if o.status == to {
                return refuse(bad(&format!("the company is already {to}")));
            }

            tx.execute(
                "UPDATE orgs SET status = ?2 WHERE id = ?1",
                params![o.id, to],
            )?;
            audit::record(
                &tx,
                Some(by),
                action,
                Some(o.id),
                &o.name,
                json!({ "slug": o.slug }),
            )?;
            tx.commit()?;
            Ok(Outcome::Done(()))
        })
        .await?;
    finish(outcome)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn suspend_company(
    State(state): State<AppState>,
    admin: SuperAdmin,
    headers: HeaderMap,
    UrlPath(slug): UrlPath<String>,
) -> Result<StatusCode, ApiError> {
    let by = admin.writing(&headers)?;
    set_status(&state, by, slug, "suspended").await
}

async fn resume_company(
    State(state): State<AppState>,
    admin: SuperAdmin,
    headers: HeaderMap,
    UrlPath(slug): UrlPath<String>,
) -> Result<StatusCode, ApiError> {
    let by = admin.writing(&headers)?;
    set_status(&state, by, slug, "active").await
}

#[derive(Deserialize)]
struct Confirm {
    confirm: String,
}

async fn delete_company(
    State(state): State<AppState>,
    admin: SuperAdmin,
    headers: HeaderMap,
    UrlPath(slug): UrlPath<String>,
    Json(body): Json<Confirm>,
) -> Result<StatusCode, ApiError> {
    let by = admin.writing(&headers)?;
    let found = state
        .dbs
        .control()
        .read({
            let slug = slug.clone();
            move |c| Ok(org(c, &slug)?)
        })
        .await?;
    let o = found.ok_or(ApiError::NotFound)?;
    if body.confirm.trim() != o.slug {
        return Err(bad("type the company's short name to confirm"));
    }

    let at = now();
    state
        .dbs
        .control()
        .write({
            let o = o.clone();
            move |c| {
                let tx = c.transaction()?;
                tx.execute(
                    "UPDATE orgs SET status = 'deleted', deleted_at = ?2 WHERE id = ?1",
                    params![o.id, at],
                )?;
                tx.execute(
                    "UPDATE invites SET revoked_at = ?2 WHERE org_id = ?1 AND accepted_at IS NULL AND revoked_at IS NULL",
                    params![o.id, at],
                )?;
                audit::record(
                    &tx,
                    Some(by),
                    "company_deleted",
                    Some(o.id),
                    &o.name,
                    json!({ "slug": o.slug, "kept_until": at + trash::KEEP_SECS }),
                )?;
                tx.commit()?;
                Ok(())
            }
        })
        .await?;

    let layout = state.dbs.layout().clone();
    let moved = state
        .dbs
        .retire(&o.slug, || trash::move_to_trash(&layout, &o.slug, at));
    if let Err(e) = moved {
        eprintln!(
            "tazko: company {} is deleted but its folder could not be moved to trash: {e}",
            o.slug
        );
        return Err(ApiError::Internal(e.to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct NewInvite {
    email: String,
    role: String,
}

pub(crate) fn company_role(raw: &str) -> Result<&'static str, ApiError> {
    match raw {
        "owner" => Ok("owner"),
        "member" => Ok("member"),
        _ => Err(bad("the role must be owner or member")),
    }
}

async fn invite(
    State(state): State<AppState>,
    admin: SuperAdmin,
    headers: HeaderMap,
    UrlPath(slug): UrlPath<String>,
    Json(body): Json<NewInvite>,
) -> Result<Response, ApiError> {
    let by = admin.writing(&headers)?;
    let email = validate::email(&body.email).ok_or_else(|| bad(INVALID_EMAIL))?;
    let role = company_role(&body.role)?;
    let outcome = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(o) = org(&tx, &slug)? else {
                return refuse(ApiError::NotFound);
            };
            let invited = invite_or_add(&tx, &o, &email, role, by, true)?;
            if invited.is_ok() {
                tx.commit()?;
            }

            Ok(Outcome::Done(invited))
        })
        .await?;
    let result = match finish(outcome)? {
        Ok(Invited::Added) => "added",
        Ok(Invited::Invited) => "invited",
        Err(msg) => return Ok(conflict(msg)),
    };
    Ok((StatusCode::CREATED, Json(json!({ "result": result }))).into_response())
}

async fn revoke_invite(
    State(state): State<AppState>,
    admin: SuperAdmin,
    headers: HeaderMap,
    UrlPath(id): UrlPath<i64>,
) -> Result<StatusCode, ApiError> {
    let by = admin.writing(&headers)?;
    let revoked = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let revoked = revoke_pending_invite(&tx, id, None, by)?;
            tx.commit()?;
            Ok(revoked)
        })
        .await?;
    if revoked {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}

pub(crate) fn revoke_pending_invite(
    tx: &Connection,
    id: i64,
    org_id: Option<i64>,
    by: i64,
) -> rusqlite::Result<bool> {
    let found: Option<(i64, String)> = tx
        .query_row(
            "SELECT org_id, email FROM invites WHERE id = ?1 AND (?2 IS NULL OR org_id = ?2) AND accepted_at IS NULL AND revoked_at IS NULL",
            params![id, org_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((org_id, email)) = found else {
        return Ok(false);
    };
    tx.execute(
        "UPDATE invites SET revoked_at = ?2 WHERE id = ?1",
        params![id, now()],
    )?;
    audit::record(
        tx,
        Some(by),
        "invite_revoked",
        Some(org_id),
        &email,
        json!({ "invite": id }),
    )?;
    Ok(true)
}

fn owners(c: &Connection, org_id: i64) -> rusqlite::Result<i64> {
    c.query_row(
        "SELECT count(*) FROM org_members WHERE org_id = ?1 AND role = 'owner'",
        [org_id],
        |r| r.get(0),
    )
}

fn member(c: &Connection, org_id: i64, user_id: i64) -> rusqlite::Result<Option<(String, String)>> {
    c.query_row(
        "SELECT m.role, u.email FROM org_members m JOIN users u ON u.id = m.user_id WHERE m.org_id = ?1 AND m.user_id = ?2",
        params![org_id, user_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

pub(crate) enum MemberChange {
    Done,
    NotMember,
    LastOwner,
}

impl MemberChange {
    pub(crate) fn into_result(self) -> Result<(), ApiError> {
        match self {
            MemberChange::Done => Ok(()),
            MemberChange::NotMember => Err(ApiError::NotFound),
            MemberChange::LastOwner => Err(bad(LAST_OWNER)),
        }
    }
}

pub(crate) fn change_member_role(
    tx: &Connection,
    org_id: i64,
    user_id: i64,
    role: &str,
    by: i64,
) -> rusqlite::Result<MemberChange> {
    let Some((old, email)) = member(tx, org_id, user_id)? else {
        return Ok(MemberChange::NotMember);
    };
    if old == role {
        return Ok(MemberChange::Done);
    }

    if old == "owner" && owners(tx, org_id)? <= 1 {
        return Ok(MemberChange::LastOwner);
    }

    tx.execute(
        "UPDATE org_members SET role = ?3 WHERE org_id = ?1 AND user_id = ?2",
        params![org_id, user_id, role],
    )?;
    audit::record(
        tx,
        Some(by),
        "role_changed",
        Some(org_id),
        &email,
        json!({ "from": old, "to": role }),
    )?;
    Ok(MemberChange::Done)
}

pub(crate) fn remove_org_member(
    tx: &Connection,
    org_id: i64,
    user_id: i64,
    by: i64,
) -> rusqlite::Result<MemberChange> {
    let Some((role, email)) = member(tx, org_id, user_id)? else {
        return Ok(MemberChange::NotMember);
    };
    if role == "owner" && owners(tx, org_id)? <= 1 {
        return Ok(MemberChange::LastOwner);
    }

    tx.execute(
        "DELETE FROM org_members WHERE org_id = ?1 AND user_id = ?2",
        params![org_id, user_id],
    )?;
    audit::record(
        tx,
        Some(by),
        "member_removed",
        Some(org_id),
        &email,
        json!({ "role": role }),
    )?;
    Ok(MemberChange::Done)
}

#[derive(Deserialize)]
struct RoleChange {
    role: String,
}

async fn change_role(
    State(state): State<AppState>,
    admin: SuperAdmin,
    headers: HeaderMap,
    UrlPath((slug, user_id)): UrlPath<(String, i64)>,
    Json(body): Json<RoleChange>,
) -> Result<StatusCode, ApiError> {
    let by = admin.writing(&headers)?;
    let role = company_role(&body.role)?;
    let outcome = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(o) = org(&tx, &slug)? else {
                return refuse(ApiError::NotFound);
            };
            let change = change_member_role(&tx, o.id, user_id, role, by)?;
            tx.commit()?;
            Ok(Outcome::Done(change))
        })
        .await?;
    finish(outcome)?.into_result()?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_member(
    State(state): State<AppState>,
    admin: SuperAdmin,
    headers: HeaderMap,
    UrlPath((slug, user_id)): UrlPath<(String, i64)>,
) -> Result<StatusCode, ApiError> {
    let by = admin.writing(&headers)?;
    let slug_for_projects = slug.clone();
    let outcome = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(o) = org(&tx, &slug)? else {
                return refuse(ApiError::NotFound);
            };
            let change = remove_org_member(&tx, o.id, user_id, by)?;
            tx.commit()?;
            Ok(Outcome::Done((o.id, change)))
        })
        .await?;
    let (org_id, change) = finish(outcome)?;
    change.into_result()?;
    crate::projects::forget_member(&state, &slug_for_projects, org_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_invites(
    State(state): State<AppState>,
    _admin: SuperAdmin,
) -> Result<Json<Value>, ApiError> {
    let invites = state
        .dbs
        .control()
        .read(|c| Ok(pending_invites(c, None)?))
        .await?;
    Ok(Json(json!({ "invites": invites })))
}

async fn list_people(
    State(state): State<AppState>,
    _admin: SuperAdmin,
) -> Result<Json<Value>, ApiError> {
    let people = state
        .dbs
        .control()
        .read(|c| {
            let mut stmt = c.prepare(
                "SELECT u.id, u.name, u.email, u.last_seen, u.is_super_admin, u.disabled,
                        (SELECT json_group_array(o.name) FROM org_members m JOIN orgs o ON o.id = m.org_id
                          WHERE m.user_id = u.id AND o.status != 'deleted')
                 FROM users u ORDER BY u.name = '', u.name COLLATE NOCASE, u.email",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    let companies: Value = serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or(json!([]));
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "name": r.get::<_, String>(1)?,
                        "email": r.get::<_, String>(2)?,
                        "last_seen": r.get::<_, Option<i64>>(3)?,
                        "is_super_admin": r.get::<_, bool>(4)?,
                        "disabled": r.get::<_, bool>(5)?,
                        "companies": companies,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(Json(json!({ "people": people })))
}

#[derive(Deserialize)]
struct PersonChange {
    is_super_admin: Option<bool>,
    disabled: Option<bool>,
}

async fn update_person(
    State(state): State<AppState>,
    admin: SuperAdmin,
    headers: HeaderMap,
    UrlPath(id): UrlPath<i64>,
    Json(body): Json<PersonChange>,
) -> Result<StatusCode, ApiError> {
    let by = admin.writing(&headers)?;
    if id == by && (body.is_super_admin == Some(false) || body.disabled == Some(true)) {
        return Err(bad(
            "you cannot remove your own super admin access or turn off your own account",
        ));
    }

    let outcome = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let found: Option<(String, bool, bool)> = tx
                .query_row(
                    "SELECT email, is_super_admin, disabled FROM users WHERE id = ?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let Some((email, sup, off)) = found else {
                return refuse(ApiError::NotFound);
            };
            if let Some(want) = body.is_super_admin.filter(|w| *w != sup) {
                tx.execute(
                    "UPDATE users SET is_super_admin = ?2 WHERE id = ?1",
                    params![id, want],
                )?;
                let action = if want {
                    "super_admin_granted"
                } else {
                    "super_admin_revoked"
                };
                audit::record(&tx, Some(by), action, None, &email, json!({}))?;
            }

            if let Some(want) = body.disabled.filter(|w| *w != off) {
                tx.execute(
                    "UPDATE users SET disabled = ?2 WHERE id = ?1",
                    params![id, want],
                )?;
                if want {
                    tx.execute("DELETE FROM sessions WHERE user_id = ?1", [id])?;
                }

                let action = if want {
                    "person_disabled"
                } else {
                    "person_enabled"
                };
                audit::record(&tx, Some(by), action, None, &email, json!({}))?;
            }

            tx.commit()?;
            Ok(Outcome::Done(()))
        })
        .await?;
    finish(outcome)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct AuditQuery {
    kind: Option<String>,
    before: Option<i64>,
}

async fn list_audit(
    State(state): State<AppState>,
    _admin: SuperAdmin,
    Query(q): Query<AuditQuery>,
) -> Result<Json<Value>, ApiError> {
    let actions: Option<Vec<String>> = match q.kind.as_deref() {
        None | Some("all") | Some("") => None,
        Some(k) => Some(
            audit::actions_of(k)
                .ok_or_else(|| bad("unknown filter"))?
                .iter()
                .map(|a| a.to_string())
                .collect(),
        ),
    };
    let before = q.before.unwrap_or(i64::MAX);
    let entries = state
        .dbs
        .control()
        .read(move |c| {
            let filter = actions.map(|a| serde_json::to_string(&a).unwrap());
            let mut stmt = c.prepare(
                "SELECT a.id, a.at, a.action, a.target, a.detail, u.email, coalesce(nullif(u.name, ''), u.email), o.name
                 FROM audit_log a LEFT JOIN users u ON u.id = a.actor_id LEFT JOIN orgs o ON o.id = a.org_id
                 WHERE a.id < ?1 AND (?2 IS NULL OR a.action IN (SELECT value FROM json_each(?2)))
                 ORDER BY a.id DESC LIMIT 100",
            )?;
            let rows = stmt
                .query_map(params![before, filter], |r| {
                    let action: String = r.get(2)?;
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "at": r.get::<_, i64>(1)?,
                        "action": action,
                        "kind": audit::kind(&action),
                        "target": r.get::<_, Option<String>>(3)?,
                        "detail": serde_json::from_str::<Value>(&r.get::<_, String>(4)?).unwrap_or(json!({})),
                        "actor_email": r.get::<_, Option<String>>(5)?,
                        "actor": r.get::<_, Option<String>>(6)?,
                        "company": r.get::<_, Option<String>>(7)?,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(Json(json!({ "entries": entries })))
}

async fn server(
    State(state): State<AppState>,
    _admin: SuperAdmin,
) -> Result<Json<Value>, ApiError> {
    let companies: i64 = state
        .dbs
        .control()
        .read(|c| {
            Ok(c.query_row(
                "SELECT count(*) FROM orgs WHERE status != 'deleted'",
                [],
                |r| r.get(0),
            )?)
        })
        .await?;
    Ok(Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": state.started.elapsed().as_secs(),
        "data_bytes": dir_size(&state.dbs.layout().root),
        "companies": companies,
        "public_url": state.settings.public_url,
        "email": if state.settings.smtp.is_some() { "on" } else { "off" },
        "last_backup": crate::backup::read_status(&state.dbs.layout().root, "backup"),
        "last_restore_test": crate::backup::read_status(&state.dbs.layout().root, "restore-test"),
    })))
}
