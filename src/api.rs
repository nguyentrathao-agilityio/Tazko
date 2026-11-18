use axum::Router;
use axum::extract::State;
use axum::response::Json;
use axum::routing::get;
use rusqlite::params;
use serde_json::{Value, json};

use crate::db::schema_version;
use crate::error::ApiError;
use crate::state::AppState;
use crate::tenant::{Company, CurrentUser};

pub const COMPANY_PREFIX: &str = "/api/v1/c/{company}";

pub const COMPANY_GET_PATHS: &[&str] = &["", "/settings", "/projects"];

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/me", get(me))
        .route(COMPANY_PREFIX, get(company_home))
}

async fn me(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Value>, ApiError> {
    let user_id = user.user_id;
    let email = user.email.clone();
    let pending: bool = state
        .dbs
        .control()
        .read({
            let email = email.clone();
            move |c| {
                Ok(c.query_row(
                    "SELECT EXISTS (SELECT 1 FROM invites i JOIN orgs o ON o.id = i.org_id
                     WHERE i.email = ?1 AND i.accepted_at IS NULL AND i.revoked_at IS NULL AND o.status = 'active')",
                    [email],
                    |r| r.get(0),
                )?)
            }
        })
        .await?;
    if pending {
        state
            .dbs
            .control()
            .write(move |c| {
                let tx = c.transaction()?;
                crate::auth::accounts::accept_invites(&tx, user_id, &email)?;
                tx.commit()?;
                Ok(())
            })
            .await?;
    }

    let companies = state
        .dbs
        .control()
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT o.slug, o.name, m.role FROM org_members m JOIN orgs o ON o.id = m.org_id
                 WHERE m.user_id = ?1 AND o.status = 'active' ORDER BY o.name COLLATE NOCASE",
            )?;
            let rows = stmt
                .query_map(params![user_id], |r| {
                    Ok(json!({ "slug": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "role": r.get::<_, String>(2)? }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;

    Ok(Json(json!({
        "id": user.user_id,
        "email": user.email,
        "name": user.name,
        "is_super_admin": user.is_super_admin,
        "csrf_token": user.csrf_token,
        "companies": companies,
    })))
}

async fn company_home(company: Company) -> Result<Json<Value>, ApiError> {
    let version = company.db.read(schema_version).await?;
    Ok(Json(json!({
        "slug": company.slug,
        "name": company.name,
        "role": company.role.as_str(),
        "schema_version": version,
    })))
}
