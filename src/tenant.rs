use std::collections::HashMap;

use axum::extract::{FromRequestParts, Path};
use axum::http::request::Parts;
use rusqlite::{OptionalExtension, params};

use crate::db::{Database, DbError, valid_slug};
use crate::error::ApiError;
use crate::session::{self, SessionUser};
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct CurrentUser(pub SessionUser);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(user) = parts.extensions.get::<CurrentUser>() {
            return Ok(user.clone());
        }

        let user = if let Some(bearer) = crate::api::tokens::bearer(&parts.headers) {
            let user = crate::api::tokens::resolve(state, &bearer).await?;
            let limits = user.token.clone().ok_or(ApiError::SignIn)?;
            state
                .rate
                .hit(&limits.key, session::now())
                .map_err(ApiError::TooMany)?;
            let route = parts
                .extensions
                .get::<axum::extract::MatchedPath>()
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            if !limits.allows_route(&parts.method, &route) {
                return Err(ApiError::Forbidden);
            }

            user
        } else {
            let token = session::cookie_token(&parts.headers).ok_or(ApiError::SignIn)?;
            session::lookup(state.dbs.control(), &token)
                .await?
                .ok_or(ApiError::SignIn)?
        };
        let user = CurrentUser(user);
        parts.extensions.insert(user.clone());
        Ok(user)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanyRole {
    Owner,
    Member,
}

impl CompanyRole {
    pub fn as_str(self) -> &'static str {
        match self {
            CompanyRole::Owner => "owner",
            CompanyRole::Member => "member",
        }
    }
}

pub struct Company {
    pub org_id: i64,
    pub slug: String,
    pub name: String,
    pub role: CompanyRole,
    pub user: CurrentUser,
    pub db: Database,
}

impl FromRequestParts<AppState> for Company {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        let Path(params) = Path::<HashMap<String, String>>::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::NotFound)?;
        let slug = params.get("company").cloned().ok_or(ApiError::NotFound)?;
        if !valid_slug(&slug) {
            return Err(ApiError::NotFound);
        }

        if user
            .0
            .token
            .as_ref()
            .is_some_and(|t| !t.allows_company(&slug))
        {
            return Err(ApiError::NotFound);
        }

        let user_id = user.0.user_id;
        let found = state
            .dbs
            .control()
            .read({
                let slug = slug.clone();
                move |c| {
                    Ok(c.query_row(
                        "SELECT o.id, o.name, m.role FROM orgs o
                         JOIN org_members m ON m.org_id = o.id AND m.user_id = ?2
                         WHERE o.slug = ?1 AND o.status = 'active'",
                        params![slug, user_id],
                        |r| {
                            Ok((
                                r.get::<_, i64>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, String>(2)?,
                            ))
                        },
                    )
                    .optional()?)
                }
            })
            .await?;
        let (org_id, name, role) = found.ok_or(ApiError::NotFound)?;
        let role = if role == "owner" {
            CompanyRole::Owner
        } else {
            CompanyRole::Member
        };
        if role != CompanyRole::Owner && user.0.token.as_ref().is_some_and(|t| t.company.is_some())
        {
            return Err(ApiError::NotFound);
        }
        let db = match state.dbs.tenant(&slug) {
            Ok(db) => db,
            Err(DbError::Missing(_)) => return Err(ApiError::NotFound),
            Err(e) => return Err(e.into()),
        };

        if let Some(limits) = user.0.token.as_ref().filter(|t| t.projects.is_some()) {
            let id = |k: &str| params.get(k).and_then(|v| v.parse::<i64>().ok());
            let project = match (id("project"), id("board")) {
                (Some(p), _) => Some(p),
                (None, Some(b)) => {
                    db.read(move |c| {
                        Ok(
                            c.query_row("SELECT project_id FROM boards WHERE id = ?1", [b], |r| {
                                r.get(0)
                            })
                            .optional()?,
                        )
                    })
                    .await?
                }
                (None, None) => None,
            };
            if let Some(p) = project
                && !limits.allows_project(&slug, p)
            {
                return Err(ApiError::NotFound);
            }

            if project.is_none() && (id("project").is_some() || id("board").is_some()) {
                return Err(ApiError::NotFound);
            }
        }

        Ok(Company {
            org_id,
            slug,
            name,
            role,
            user,
            db,
        })
    }
}

pub async fn active_tenants(state: &AppState) -> Vec<(String, Database)> {
    let slugs: Vec<String> = state
        .dbs
        .control()
        .read(|c| {
            let mut stmt = c.prepare("SELECT slug FROM orgs WHERE status = 'active'")?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .unwrap_or_default();
    slugs
        .into_iter()
        .filter_map(|slug| state.dbs.tenant(&slug).ok().map(|db| (slug, db)))
        .collect()
}
