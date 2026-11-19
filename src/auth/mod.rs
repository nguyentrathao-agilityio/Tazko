pub mod accounts;
pub mod cookies;
pub mod google;

use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Json, Redirect, Response};
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::error::ApiError;
use crate::session::{self, random_token};
use crate::state::AppState;
use crate::tenant::CurrentUser;
use accounts::Refusal;

pub const OAUTH_COOKIE: &str = "tazko_oauth";
pub const REFUSAL_COOKIE: &str = "tazko_refusal";
pub const CSRF_HEADER: &str = "x-csrf-token";
const OAUTH_PATH: &str = "/auth/google";
const OAUTH_TTL: i64 = 10 * 60;
const REFUSAL_TTL: i64 = 10 * 60;

#[derive(Serialize, Deserialize)]
struct Pending {
    state: String,
    nonce: String,
    verifier: String,
    next: String,
}

#[derive(Serialize, Deserialize)]
struct RefusalNote {
    email: String,
    reason: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/google/start", get(start))
        .route("/auth/google/callback", get(callback))
        .route("/auth/refusal", get(refusal))
        .route("/auth/signout", post(sign_out))
}

pub fn redirect_uri(state: &AppState) -> String {
    format!("{}/auth/google/callback", state.settings.public_url)
}

fn dot_segment(seg: &str) -> Option<usize> {
    match seg.to_ascii_lowercase().as_str() {
        "." | "%2e" => Some(1),
        ".." | ".%2e" | "%2e." | "%2e%2e" => Some(2),
        _ => None,
    }
}

pub fn safe_next(next: Option<&str>) -> String {
    let Some(n) = next else {
        return "/".to_string();
    };
    let plain = n
        .chars()
        .all(|c| !c.is_ascii_control() && !c.is_whitespace() && c != '\\');
    if !plain || !n.starts_with('/') || n[1..].starts_with('/') {
        return "/".to_string();
    }

    let n = n.split('#').next().unwrap_or("/");
    let (path, query) = match n.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (n, None),
    };
    let segments: Vec<&str> = path.split('/').skip(1).collect();
    let mut kept: Vec<&str> = Vec::new();
    for seg in &segments {
        match dot_segment(seg) {
            Some(1) => {}
            Some(_) => {
                kept.pop();
            }
            None => kept.push(seg),
        }
    }

    if segments.last().is_some_and(|s| dot_segment(s).is_some()) {
        kept.push("");
    }

    let resolved = format!("/{}", kept.join("/"));
    if resolved[1..].starts_with('/') || resolved.starts_with("/auth/") || resolved == "/auth" {
        return "/".to_string();
    }

    match query {
        Some(q) => format!("{resolved}?{q}"),
        None => resolved,
    }
}

fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[derive(Deserialize)]
struct StartQuery {
    next: Option<String>,
}

async fn start(State(state): State<AppState>, Query(q): Query<StartQuery>) -> Response {
    let pending = Pending {
        state: random_token(),
        nonce: random_token(),
        verifier: random_token(),
        next: safe_next(q.next.as_deref()),
    };
    let query = form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", &state.settings.google_client_id)
        .append_pair("redirect_uri", &redirect_uri(&state))
        .append_pair("response_type", "code")
        .append_pair("scope", "openid email profile")
        .append_pair("state", &pending.state)
        .append_pair("nonce", &pending.nonce)
        .append_pair("code_challenge", &pkce_challenge(&pending.verifier))
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "select_account")
        .finish();
    let auth_url = &state.google.endpoints.auth_url;
    let sep = if auth_url.contains('?') { '&' } else { '?' };
    let url = format!("{auth_url}{sep}{query}");
    let sealed = cookies::seal(state.secret.expose(), "oauth", OAUTH_TTL, &pending);
    (
        [(
            header::SET_COOKIE,
            cookies::set(OAUTH_COOKIE, &sealed, OAUTH_PATH, OAUTH_TTL),
        )],
        Redirect::to(&url),
    )
        .into_response()
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

fn back_to_sign_in(error: &str) -> Response {
    (
        [(header::SET_COOKIE, cookies::clear(OAUTH_COOKIE, OAUTH_PATH))],
        Redirect::to(&format!("/signin?error={error}")),
    )
        .into_response()
}

async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<CallbackQuery>,
) -> Response {
    if q.error.is_some() {
        return back_to_sign_in("cancelled");
    }

    let pending: Option<Pending> = cookies::read(&headers, OAUTH_COOKIE)
        .and_then(|v| cookies::open(state.secret.expose(), "oauth", &v));
    let (Some(pending), Some(code), Some(got_state)) = (pending, q.code, q.state) else {
        return back_to_sign_in("expired");
    };
    if !bool::from(subtle::ConstantTimeEq::ct_eq(
        got_state.as_bytes(),
        pending.state.as_bytes(),
    )) {
        return back_to_sign_in("expired");
    }

    let claims = {
        let state = state.clone();
        let redirect = redirect_uri(&state);
        tokio::task::spawn_blocking(move || {
            let id_token = state.google.exchange_code(&google::Exchange {
                code: &code,
                client_id: &state.settings.google_client_id,
                client_secret: state.settings.google_client_secret.expose(),
                redirect_uri: &redirect,
                code_verifier: &pending.verifier,
            })?;
            state.google.verify_id_token(
                &id_token,
                &state.settings.google_client_id,
                &pending.nonce,
            )
        })
        .await
    };
    let claims = match claims {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            eprintln!("tazko: sign-in failed: {e}");
            return back_to_sign_in("failed");
        }
        Err(_) => return back_to_sign_in("failed"),
    };

    let outcome = state
        .dbs
        .control()
        .write({
            let claims = claims.clone();
            move |c| accounts::sign_in(c, &claims)
        })
        .await;
    let user_id = match outcome {
        Ok(Ok(id)) => id,
        Ok(Err(reason)) => return refused(&state, &claims, reason),
        Err(e) => {
            eprintln!("tazko: sign-in failed: {e}");
            return back_to_sign_in("failed");
        }
    };

    let agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(200).collect());
    let token = match session::create(state.dbs.control(), user_id, agent).await {
        Ok((token, _)) => token,
        Err(e) => {
            eprintln!("tazko: sign-in failed: {e}");
            return back_to_sign_in("failed");
        }
    };

    let mut res = Redirect::to(&pending.next).into_response();
    let h = res.headers_mut();
    h.append(
        header::SET_COOKIE,
        cookies::set(session::COOKIE, &token, "/", session::LIFETIME_SECS)
            .parse()
            .unwrap(),
    );
    h.append(
        header::SET_COOKIE,
        cookies::clear(OAUTH_COOKIE, OAUTH_PATH).parse().unwrap(),
    );
    res
}

fn refused(state: &AppState, claims: &google::Claims, reason: Refusal) -> Response {
    let note = RefusalNote {
        email: claims.email.clone().unwrap_or_default(),
        reason: reason.code().to_string(),
    };
    let sealed = cookies::seal(state.secret.expose(), "refusal", REFUSAL_TTL, &note);
    let mut res = Redirect::to("/signin/refused").into_response();
    let h = res.headers_mut();
    h.append(
        header::SET_COOKIE,
        cookies::set(REFUSAL_COOKIE, &sealed, "/auth/refusal", REFUSAL_TTL)
            .parse()
            .unwrap(),
    );
    h.append(
        header::SET_COOKIE,
        cookies::clear(OAUTH_COOKIE, OAUTH_PATH).parse().unwrap(),
    );
    res
}

async fn refusal(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let note: RefusalNote = cookies::read(&headers, REFUSAL_COOKIE)
        .and_then(|v| cookies::open(state.secret.expose(), "refusal", &v))
        .ok_or(ApiError::NotFound)?;
    Ok(Json(json!({ "email": note.email, "reason": note.reason })))
}

pub fn check_csrf(user: &CurrentUser, headers: &HeaderMap) -> Result<(), ApiError> {
    if user.0.token.is_some() {
        return Ok(());
    }

    let sent = headers
        .get(CSRF_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if bool::from(subtle::ConstantTimeEq::ct_eq(
        sent.as_bytes(),
        user.0.csrf_token.as_bytes(),
    )) && !sent.is_empty()
    {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

async fn sign_out(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    check_csrf(&user, &headers)?;
    if let Some(token) = session::cookie_token(&headers) {
        let id_hash = session::hash_token(&token);
        state
            .dbs
            .control()
            .write(move |c| Ok(c.execute("DELETE FROM sessions WHERE id_hash = ?1", [id_hash])?))
            .await?;
    }

    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, cookies::clear(session::COOKIE, "/"))],
    )
        .into_response())
}
