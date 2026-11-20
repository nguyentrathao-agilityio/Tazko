use axum::Router;
use axum::extract::{FromRequestParts, Path as UrlPath, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{delete, get, patch, post};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::admin::{self, validate};
use crate::api::COMPANY_PREFIX;
use crate::auth::check_csrf;
use crate::db::DbError;
use crate::error::{ApiError, bad};
use crate::state::AppState;
use crate::tenant::{Company, CompanyRole};

pub const OWNER_ROUTES: &[(Method, &str)] = &[
    (Method::GET, "/settings"),
    (Method::GET, "/projects"),
    (Method::POST, "/invites"),
    (Method::DELETE, "/invites/1"),
    (Method::PATCH, "/members/1"),
    (Method::DELETE, "/members/1"),
];

pub struct CompanyOwner(pub Company);

impl FromRequestParts<AppState> for CompanyOwner {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let company = Company::from_request_parts(parts, state).await?;
        if company
            .user
            .0
            .token
            .as_ref()
            .is_some_and(|t| t.scope != crate::api::tokens::Scope::Manage || t.projects.is_some())
        {
            return Err(ApiError::Forbidden);
        }

        if company.role == CompanyRole::Owner {
            Ok(CompanyOwner(company))
        } else {
            Err(ApiError::Forbidden)
        }
    }
}

impl CompanyOwner {
    pub(crate) fn writing(&self, headers: &HeaderMap) -> Result<i64, ApiError> {
        check_csrf(&self.0.user, headers)?;
        Ok(self.0.user.0.user_id)
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(&format!("{COMPANY_PREFIX}/settings"), get(settings))
        .route(&format!("{COMPANY_PREFIX}/invites"), post(invite))
        .route(&format!("{COMPANY_PREFIX}/invites/{{id}}"), delete(revoke))
        .route(
            &format!("{COMPANY_PREFIX}/members/{{user_id}}"),
            patch(change_role).delete(remove_member),
        )
}

fn members(c: &Connection, org_id: i64) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = c.prepare(
        "SELECT u.id, u.name, u.email, m.role, u.last_seen
         FROM org_members m JOIN users u ON u.id = m.user_id
         WHERE m.org_id = ?1 AND u.disabled = 0
         ORDER BY m.role = 'owner' DESC, u.name = '', u.name COLLATE NOCASE, u.email",
    )?;
    stmt.query_map([org_id], |r| {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "name": r.get::<_, String>(1)?,
            "email": r.get::<_, String>(2)?,
            "role": r.get::<_, String>(3)?,
            "last_seen": r.get::<_, Option<i64>>(4)?,
            "disabled": false,
        }))
    })?
    .collect()
}

async fn settings(
    State(state): State<AppState>,
    CompanyOwner(co): CompanyOwner,
) -> Result<Json<Value>, ApiError> {
    let org_id = co.org_id;
    let (members, invites, limit) = state
        .dbs
        .control()
        .read(move |c| {
            let limit: Option<i64> = c.query_row(
                "SELECT storage_limit FROM orgs WHERE id = ?1",
                [org_id],
                |r| r.get(0),
            )?;
            Ok((
                members(c, org_id)?,
                admin::pending_invites(c, Some(org_id))?,
                limit,
            ))
        })
        .await?;
    Ok(Json(json!({
        "slug": co.slug,
        "name": co.name,
        "members": members,
        "invites": invites,
        "storage_used": admin::org_storage_used(&state, &co.slug),
        "storage_limit": limit,
    })))
}

#[derive(Deserialize)]
struct NewInvite {
    email: String,
    role: String,
}

async fn invite(
    State(state): State<AppState>,
    owner: CompanyOwner,
    headers: HeaderMap,
    Json(body): Json<NewInvite>,
) -> Result<Response, ApiError> {
    let by = owner.writing(&headers)?;
    let email = validate::email(&body.email).ok_or_else(|| bad(admin::INVALID_EMAIL))?;
    let role = admin::company_role(&body.role)?;
    let slug = owner.0.slug.clone();
    let outcome = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let o = admin::org(&tx, &slug)?.ok_or(DbError::Closed)?;
            let res = admin::invite_or_add(&tx, &o, &email, role, by, false)?;
            if res.is_ok() {
                tx.commit()?;
            }

            Ok(res)
        })
        .await?;
    Ok(match outcome {
        Ok(_) => (StatusCode::CREATED, Json(json!({ "result": "invited" }))).into_response(),
        Err(msg) => admin::conflict(msg),
    })
}

async fn revoke(
    State(state): State<AppState>,
    owner: CompanyOwner,
    headers: HeaderMap,
    UrlPath((_, id)): UrlPath<(String, i64)>,
) -> Result<StatusCode, ApiError> {
    let by = owner.writing(&headers)?;
    let org_id = owner.0.org_id;
    let revoked = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let revoked = admin::revoke_pending_invite(&tx, id, Some(org_id), by)?;
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

#[derive(Deserialize)]
struct RoleChange {
    role: String,
}

async fn change_role(
    State(state): State<AppState>,
    owner: CompanyOwner,
    headers: HeaderMap,
    UrlPath((_, user_id)): UrlPath<(String, i64)>,
    Json(body): Json<RoleChange>,
) -> Result<StatusCode, ApiError> {
    let by = owner.writing(&headers)?;
    let role = admin::company_role(&body.role)?;
    let org_id = owner.0.org_id;
    let change = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let change = admin::change_member_role(&tx, org_id, user_id, role, by)?;
            tx.commit()?;
            Ok(change)
        })
        .await?;
    change.into_result()?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_member(
    State(state): State<AppState>,
    owner: CompanyOwner,
    headers: HeaderMap,
    UrlPath((_, user_id)): UrlPath<(String, i64)>,
) -> Result<StatusCode, ApiError> {
    let by = owner.writing(&headers)?;
    let org_id = owner.0.org_id;
    let change = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let change = admin::remove_org_member(&tx, org_id, user_id, by)?;
            tx.commit()?;
            Ok(change)
        })
        .await?;
    change.into_result()?;
    crate::projects::forget_member(&state, &owner.0.slug, org_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
