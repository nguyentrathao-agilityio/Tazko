use std::collections::HashMap;

use axum::extract::{FromRequestParts, Path};
use axum::http::request::Parts;
use rusqlite::{OptionalExtension, params};

use crate::db::{Database, valid_slug};
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

        let token = session::cookie_token(&parts.headers).ok_or(ApiError::SignIn)?;
        let user = session::lookup(state.dbs.control(), &token)
            .await?
            .ok_or(ApiError::SignIn)?;
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
        let db = state.dbs.tenant(&slug)?;

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
