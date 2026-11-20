use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::Json;
use axum::routing::{delete, get};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::COMPANY_PREFIX;
use crate::company::CompanyOwner;
use crate::db::valid_slug;
use crate::error::{ApiError, bad};
use crate::projects::{Act, path_id};
use crate::session::{SessionUser, hash_token, now, random_token};
use crate::state::AppState;
use crate::tenant::CurrentUser;

pub const RATE_PER_MINUTE: u32 = 120;
const PERSONAL: &str = "tzk_p_";
const COMPANY: &str = "tzk_c_";
const MAX_DAYS: i64 = 366;
const NOT_YOUR_PROJECTS: &str = "choose projects you belong to";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Read,
    Write,
    Manage,
}

impl Scope {
    pub fn parse(s: &str) -> Option<Scope> {
        match s {
            "read" => Some(Scope::Read),
            "write" => Some(Scope::Write),
            "manage" => Some(Scope::Manage),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Read => "read",
            Scope::Write => "write",
            Scope::Manage => "manage",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenLimits {
    pub key: String,
    pub scope: Scope,
    pub company: Option<String>,
    pub projects: Option<Vec<(String, i64)>>,
}

impl TokenLimits {
    fn other_company(&self, slug: &str) -> bool {
        self.company.as_deref().is_some_and(|c| c != slug)
    }

    pub fn allows_project(&self, slug: &str, project: i64) -> bool {
        if self.other_company(slug) {
            return false;
        }

        match &self.projects {
            None => true,
            Some(list) => list.iter().any(|(s, p)| s == slug && *p == project),
        }
    }

    pub fn allows_company(&self, slug: &str) -> bool {
        if self.other_company(slug) {
            return false;
        }

        match &self.projects {
            None => true,
            Some(list) => list.iter().any(|(s, _)| s == slug),
        }
    }

    pub fn allows(&self, act: Act) -> bool {
        match (self.scope, act) {
            (_, Act::Read) => true,
            (Scope::Read, _) => false,
            (Scope::Write, Act::Edit) => true,
            (Scope::Write, _) => false,
            (Scope::Manage, _) => true,
        }
    }

    pub fn allows_route(&self, method: &Method, route: &str) -> bool {
        if self.scope == Scope::Read && method != Method::GET && method != Method::HEAD {
            return false;
        }

        if route.ends_with("/tokens")
            || route.ends_with("/tokens/{token}")
            || route.ends_with("/live")
        {
            return false;
        }

        if route == "/api/v1/me" {
            return method == Method::GET;
        }

        let Some(rest) = route.strip_prefix(COMPANY_PREFIX) else {
            return false;
        };
        if self.projects.is_none() {
            return true;
        }

        rest.starts_with("/projects/{project}")
            || rest.starts_with("/boards/{board}")
            || rest == "/my-projects"
    }
}

#[derive(Default)]
pub struct RateLimiter {
    windows: Mutex<HashMap<String, (i64, u32)>>,
}

impl RateLimiter {
    pub fn hit(&self, key: &str, now: i64) -> Result<(), i64> {
        let window = now / 60;
        let mut map = self.windows.lock().unwrap_or_else(|e| e.into_inner());
        if map.len() > 10_000 {
            map.retain(|_, (w, _)| *w == window);
        }

        let entry = map.entry(key.to_string()).or_insert((window, 0));
        if entry.0 != window {
            *entry = (window, 0);
        }

        if entry.1 >= RATE_PER_MINUTE {
            return Err(60 - now % 60);
        }

        entry.1 += 1;
        Ok(())
    }
}

pub fn bearer(headers: &HeaderMap) -> Option<String> {
    let v = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (kind, token) = v.split_once(' ')?;
    if !kind.eq_ignore_ascii_case("bearer") {
        return None;
    }

    let token = token.trim();
    (token.len() <= 200 && token.bytes().all(|b| b.is_ascii_graphic())).then(|| token.to_string())
}

fn projects_from(raw: Option<&str>) -> Option<Vec<(String, i64)>> {
    let v: Vec<Value> = serde_json::from_str(raw?).ok()?;
    Some(
        v.iter()
            .filter_map(|p| Some((p["company"].as_str()?.to_string(), p["project"].as_i64()?)))
            .collect(),
    )
}

pub async fn resolve(state: &AppState, token: &str) -> Result<SessionUser, ApiError> {
    let hash = hash_token(token);
    let at = now();
    if token.starts_with(PERSONAL) {
        let found = state
            .dbs
            .control()
            .read(move |c| {
                Ok(c.query_row(
                    "SELECT t.id, t.scope, t.projects, t.last_used_at, u.id, u.email, u.name
                     FROM personal_tokens t JOIN users u ON u.id = t.user_id
                     WHERE t.id_hash = ?1 AND t.revoked_at IS NULL AND (t.expires_at IS NULL OR t.expires_at > ?2)
                       AND u.disabled = 0",
                    params![hash, at],
                    |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, Option<String>>(2)?,
                            r.get::<_, Option<i64>>(3)?,
                            r.get::<_, i64>(4)?,
                            r.get::<_, String>(5)?,
                            r.get::<_, String>(6)?,
                        ))
                    },
                )
                .optional()?)
            })
            .await?;
        let (id, scope, projects, last, user_id, email, name) = found.ok_or(ApiError::SignIn)?;
        if last.is_none_or(|l| at - l > 60) {
            let control = state.dbs.control().clone();
            tokio::spawn(async move {
                let _ = control
                    .write(move |c| {
                        c.execute(
                            "UPDATE personal_tokens SET last_used_at = ?2 WHERE id = ?1",
                            params![id, at],
                        )?;
                        Ok(())
                    })
                    .await;
            });
        }
        return Ok(SessionUser {
            user_id,
            email,
            name,
            is_super_admin: false,
            csrf_token: String::new(),
            token: Some(Arc::new(TokenLimits {
                key: format!("p{id}"),
                scope: Scope::parse(&scope).unwrap_or(Scope::Read),
                company: None,
                projects: projects_from(projects.as_deref()),
            })),
        });
    }

    let rest = token.strip_prefix(COMPANY).ok_or(ApiError::SignIn)?;
    let (slug, _) = rest.rsplit_once('_').ok_or(ApiError::SignIn)?;
    if !valid_slug(slug) {
        return Err(ApiError::SignIn);
    }

    let db = state.dbs.tenant(slug).map_err(|_| ApiError::SignIn)?;
    let found = db
        .read(move |c| {
            Ok(c.query_row(
                "SELECT id, scope, projects, created_by, last_used_at FROM api_tokens
                 WHERE id_hash = ?1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > ?2)",
                params![hash, at],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, Option<i64>>(4)?,
                    ))
                },
            )
            .optional()?)
        })
        .await?;
    let (id, scope, projects, creator, last) = found.ok_or(ApiError::SignIn)?;
    let person = state
        .dbs
        .control()
        .read(move |c| {
            Ok(c.query_row(
                "SELECT email, name FROM users WHERE id = ?1 AND disabled = 0",
                [creator],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?)
        })
        .await?;
    let (email, name) = person.ok_or(ApiError::SignIn)?;
    if last.is_none_or(|l| at - l > 60) {
        tokio::spawn(async move {
            let _ = db
                .write(move |c| {
                    c.execute(
                        "UPDATE api_tokens SET last_used_at = ?2 WHERE id = ?1",
                        params![id, at],
                    )?;
                    Ok(())
                })
                .await;
        });
    }
    Ok(SessionUser {
        user_id: creator,
        email,
        name,
        is_super_admin: false,
        csrf_token: String::new(),
        token: Some(Arc::new(TokenLimits {
            key: format!("c{slug}:{id}"),
            scope: Scope::parse(&scope).unwrap_or(Scope::Read),
            company: Some(slug.to_string()),
            projects: Some(
                serde_json::from_str::<Vec<i64>>(&projects)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| (slug.to_string(), p))
                    .collect(),
            ),
        })),
    })
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/me/tokens",
            get(list_personal).post(create_personal),
        )
        .route("/api/v1/me/tokens/{token}", delete(revoke_personal))
        .route(
            &format!("{COMPANY_PREFIX}/tokens"),
            get(list_company).post(create_company),
        )
        .route(
            &format!("{COMPANY_PREFIX}/tokens/{{token}}"),
            delete(revoke_company),
        )
}

fn by_session(user: &CurrentUser, headers: &HeaderMap) -> Result<i64, ApiError> {
    if user.0.token.is_some() {
        return Err(ApiError::Forbidden);
    }

    crate::auth::check_csrf(user, headers)?;
    Ok(user.0.user_id)
}

#[derive(Deserialize)]
struct ProjectRef {
    company: String,
    project: i64,
}

#[derive(Deserialize)]
struct NewPersonal {
    name: String,
    scope: String,
    projects: Option<Vec<ProjectRef>>,
    expires_in_days: Option<i64>,
}

fn parse_scope(raw: &str) -> Result<Scope, ApiError> {
    Scope::parse(raw).ok_or_else(|| bad("choose read, write or manage"))
}

fn token_name(raw: &str) -> Result<String, ApiError> {
    let n = raw.trim();
    if n.is_empty() || n.chars().count() > 60 || n.chars().any(char::is_control) {
        return Err(bad("give the token a short name"));
    }

    Ok(n.to_string())
}

fn expiry(days: Option<i64>) -> Result<Option<i64>, ApiError> {
    match days {
        None => Ok(None),
        Some(d) if (1..=MAX_DAYS).contains(&d) => Ok(Some(now() + d * 86_400)),
        Some(_) => Err(bad("a token can last from 1 to 366 days")),
    }
}

fn token_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, i64>(0)?,
        "name": r.get::<_, String>(1)?,
        "scope": r.get::<_, String>(2)?,
        "projects": serde_json::from_str::<Value>(&r.get::<_, Option<String>>(3)?.unwrap_or_else(|| "null".into())).unwrap_or(Value::Null),
        "created_at": r.get::<_, i64>(4)?,
        "expires_at": r.get::<_, Option<i64>>(5)?,
        "last_used_at": r.get::<_, Option<i64>>(6)?,
    }))
}

async fn list_personal(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Value>, ApiError> {
    if user.0.token.is_some() {
        return Err(ApiError::Forbidden);
    }

    let id = user.0.user_id;
    let tokens = state
        .dbs
        .control()
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT id, name, scope, projects, created_at, expires_at, last_used_at FROM personal_tokens
                 WHERE user_id = ?1 AND revoked_at IS NULL ORDER BY id DESC",
            )?;
            Ok(stmt.query_map([id], token_row)?.collect::<Result<Vec<_>, _>>()?)
        })
        .await?;
    Ok(Json(json!({ "tokens": tokens })))
}

fn member_project(c: &Connection, user: i64, project: i64) -> rusqlite::Result<bool> {
    c.query_row(
        "SELECT EXISTS (SELECT 1 FROM project_members m JOIN projects p ON p.id = m.project_id
         WHERE m.project_id = ?1 AND m.user_id = ?2 AND p.deleted_at IS NULL)",
        params![project, user],
        |r| r.get(0),
    )
}

async fn create_personal(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Json(body): Json<NewPersonal>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let id = by_session(&user, &headers)?;
    let name = token_name(&body.name)?;
    let scope = parse_scope(&body.scope)?;
    let expires = expiry(body.expires_in_days)?;
    let projects = match body.projects {
        None => None,
        Some(list) if list.is_empty() => {
            return Err(bad(
                "choose at least one project, or leave the list out for all",
            ));
        }
        Some(list) => {
            for p in &list {
                if !valid_slug(&p.company) {
                    return Err(bad(NOT_YOUR_PROJECTS));
                }

                let db = state
                    .dbs
                    .tenant(&p.company)
                    .map_err(|_| bad(NOT_YOUR_PROJECTS))?;
                let project = p.project;
                let ok = db
                    .read(move |c| Ok(member_project(c, id, project)?))
                    .await?;
                if !ok {
                    return Err(bad(NOT_YOUR_PROJECTS));
                }
            }
            Some(
                json!(
                    list.iter()
                        .map(|p| json!({ "company": p.company, "project": p.project }))
                        .collect::<Vec<_>>()
                )
                .to_string(),
            )
        }
    };
    let token = format!("{PERSONAL}{}", random_token());
    let hash = hash_token(&token);
    let row = state
        .dbs
        .control()
        .write(move |c| {
            c.execute(
                "INSERT INTO personal_tokens (user_id, name, id_hash, scope, projects, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, name, hash, scope.as_str(), projects, expires],
            )?;
            let new = c.last_insert_rowid();
            Ok(c.query_row(
                "SELECT id, name, scope, projects, created_at, expires_at, last_used_at FROM personal_tokens WHERE id = ?1",
                [new],
                token_row,
            )?)
        })
        .await?;
    let mut v = row;
    v["token"] = json!(token);
    Ok((StatusCode::CREATED, Json(v)))
}

async fn revoke_personal(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    let id = by_session(&user, &headers)?;
    let token = path_id(&params, "token")?;
    let n = state
        .dbs
        .control()
        .write(move |c| {
            Ok(c.execute(
                "UPDATE personal_tokens SET revoked_at = unixepoch() WHERE id = ?1 AND user_id = ?2 AND revoked_at IS NULL",
                params![token, id],
            )?)
        })
        .await?;
    if n == 0 {
        return Err(ApiError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct NewCompany {
    name: String,
    scope: String,
    projects: Vec<i64>,
    expires_in_days: Option<i64>,
}

async fn list_company(CompanyOwner(co): CompanyOwner) -> Result<Json<Value>, ApiError> {
    if co.user.0.token.is_some() {
        return Err(ApiError::Forbidden);
    }

    let tokens = co
        .db
        .read(|c| {
            let mut stmt = c.prepare(
                "SELECT t.id, t.name, t.scope, t.projects, t.created_at, t.expires_at, t.last_used_at,
                        coalesce(nullif(p.name, ''), p.email, '')
                 FROM api_tokens t LEFT JOIN people p ON p.id = t.created_by
                 WHERE t.revoked_at IS NULL ORDER BY t.id DESC",
            )?;
            Ok(stmt
                .query_map([], |r| {
                    let mut row = token_row(r)?;
                    row["by"] = json!(r.get::<_, String>(7)?);
                    Ok(row)
                })?
                .collect::<Result<Vec<_>, _>>()?)
        })
        .await?;
    Ok(Json(json!({ "tokens": tokens })))
}

async fn create_company(
    State(state): State<AppState>,
    CompanyOwner(co): CompanyOwner,
    headers: HeaderMap,
    Json(body): Json<NewCompany>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = by_session(&co.user, &headers)?;
    let name = token_name(&body.name)?;
    let scope = parse_scope(&body.scope)?;
    let expires = expiry(body.expires_in_days)?;
    let mut projects = body.projects;
    projects.sort_unstable();
    projects.dedup();
    if projects.is_empty() {
        return Err(bad("choose at least one project"));
    }

    let token = format!("{COMPANY}{}_{}", co.slug, random_token());
    let hash = hash_token(&token);
    let list = json!(projects).to_string();
    let row = co
        .db
        .write(move |c| {
            for p in &projects {
                if !member_project(c, by, *p)? {
                    return Ok(Err(bad(NOT_YOUR_PROJECTS)));
                }
            }
            c.execute(
                "INSERT INTO api_tokens (name, id_hash, scope, projects, created_by, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![name, hash, scope.as_str(), list, by, expires],
            )?;
            let new = c.last_insert_rowid();
            Ok(Ok(c.query_row(
                "SELECT id, name, scope, projects, created_at, expires_at, last_used_at FROM api_tokens WHERE id = ?1",
                [new],
                token_row,
            )?))
        })
        .await??;
    record_token_audit(
        &state,
        by,
        co.org_id,
        "api_token_created",
        json!({ "token": row["id"], "name": row["name"], "scope": row["scope"] }),
    )
    .await?;
    let mut v = row;
    v["token"] = json!(token);
    Ok((StatusCode::CREATED, Json(v)))
}

async fn revoke_company(
    State(state): State<AppState>,
    CompanyOwner(co): CompanyOwner,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    let by = by_session(&co.user, &headers)?;
    let token = path_id(&params, "token")?;
    let n = co
        .db
        .write(move |c| {
            Ok(c.execute(
                "UPDATE api_tokens SET revoked_at = unixepoch() WHERE id = ?1 AND revoked_at IS NULL",
                [token],
            )?)
        })
        .await?;
    if n == 0 {
        return Err(ApiError::NotFound);
    }

    record_token_audit(
        &state,
        by,
        co.org_id,
        "api_token_revoked",
        json!({ "token": token }),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn record_token_audit(
    state: &AppState,
    by: i64,
    org: i64,
    action: &'static str,
    detail: Value,
) -> Result<(), ApiError> {
    state
        .dbs
        .control()
        .write(move |c| {
            Ok(crate::admin::audit::record(
                c,
                Some(by),
                action,
                Some(org),
                "token",
                detail,
            )?)
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(scope: Scope, projects: Option<Vec<(String, i64)>>) -> TokenLimits {
        TokenLimits {
            key: "k".into(),
            scope,
            company: None,
            projects,
        }
    }

    #[test]
    fn scopes_cap_what_a_token_can_do() {
        let read = limits(Scope::Read, None);
        assert!(read.allows(Act::Read) && !read.allows(Act::Edit));
        assert!(!read.allows_route(&Method::POST, "/api/v1/c/{company}/boards/{board}/cards"));
        let write = limits(Scope::Write, None);
        assert!(
            write.allows(Act::Edit)
                && !write.allows(Act::Manage)
                && !write.allows(Act::CreateBoard)
        );
        assert!(limits(Scope::Manage, None).allows(Act::Manage));
    }

    #[test]
    fn project_limited_tokens_reach_only_project_routes() {
        let t = limits(Scope::Write, Some(vec![("a".into(), 1)]));
        assert!(t.allows_project("a", 1));
        assert!(!t.allows_project("a", 2));
        assert!(!t.allows_project("b", 1));
        assert!(t.allows_route(
            &Method::GET,
            "/api/v1/c/{company}/boards/{board}/cards/{card}"
        ));
        assert!(t.allows_route(&Method::GET, "/api/v1/c/{company}/my-projects"));
        for r in [
            "/api/v1/c/{company}/search",
            "/api/v1/c/{company}/notifications",
            "/api/v1/c/{company}/starred",
            "/api/v1/admin/companies",
            "/api/v1/me/tokens",
        ] {
            assert!(!t.allows_route(&Method::GET, r), "{r}");
        }
        assert!(
            !limits(Scope::Manage, None)
                .allows_route(&Method::GET, "/api/v1/c/{company}/boards/{board}/live")
        );
    }

    #[test]
    fn the_rate_limit_resets_each_minute() {
        let r = RateLimiter::default();
        for _ in 0..RATE_PER_MINUTE {
            assert!(r.hit("a", 120).is_ok());
        }
        assert_eq!(r.hit("a", 130), Err(50));
        assert!(r.hit("b", 130).is_ok());
        assert!(r.hit("a", 180).is_ok());
    }

    #[test]
    fn only_bearer_headers_carry_tokens() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "Bearer tzk_p_abc".parse().unwrap());
        assert_eq!(bearer(&h).as_deref(), Some("tzk_p_abc"));
        h.insert(header::AUTHORIZATION, "Basic abc".parse().unwrap());
        assert_eq!(bearer(&h), None);
    }
}
