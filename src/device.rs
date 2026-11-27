use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::tokens::{Scope, insert_personal};
use crate::auth::check_csrf;
use crate::error::{ApiError, bad};
use crate::session::{hash_token, now, random_token};
use crate::state::AppState;
use crate::tenant::CurrentUser;

const LIFETIME: i64 = 600;
const INTERVAL: i64 = 5;
const TOKEN_DAYS: i64 = 90;
const ALPHABET: &[u8] = b"BCDFGHJKLMNPQRSTVWXZ";
const MAX_WAITING: i64 = 1000;
const STARTS_PER_MINUTE: u32 = 10;

pub fn client_address(headers: &HeaderMap, header: &str) -> String {
    headers
        .get_all(header)
        .iter()
        .next_back()
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .and_then(|v| v.trim().parse::<std::net::IpAddr>().ok())
        .map_or_else(|| "direct".to_string(), |ip| ip.to_string())
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/cli/device", post(start))
        .route("/api/v1/cli/device/{code}", get(describe))
        .route("/api/v1/cli/device/{code}/approve", post(approve))
        .route("/api/v1/cli/device/{code}/deny", post(deny))
        .route("/api/v1/cli/token", post(token))
        .route("/cli/install.sh", get(install_script))
        .route("/cli/download", get(download))
}

fn user_code() -> String {
    let fair = 256 - 256 % ALPHABET.len();
    let mut letters = String::with_capacity(8);
    while letters.len() < 8 {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).expect("system random source");
        letters.extend(
            bytes
                .iter()
                .map(|b| *b as usize)
                .filter(|b| *b < fair)
                .map(|b| ALPHABET[b % ALPHABET.len()] as char)
                .take(8 - letters.len()),
        );
    }
    format!("{}-{}", &letters[..4], &letters[4..])
}

fn normalise(code: &str) -> String {
    let letters: String = code
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .take(8)
        .collect();
    if letters.len() == 8 {
        format!("{}-{}", &letters[..4], &letters[4..])
    } else {
        String::new()
    }
}

pub fn client_name(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .take(60)
        .collect::<String>()
        .trim()
        .to_string()
}

#[derive(Deserialize)]
struct Start {
    #[serde(default)]
    client: String,
}

async fn start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Start>,
) -> Result<Json<Value>, ApiError> {
    let at = now();
    let from = client_address(&headers, &state.settings.client_ip_header);
    state
        .rate
        .hit_up_to(&format!("device-sign-in:{from}"), at, STARTS_PER_MINUTE)
        .and_then(|()| state.rate.hit("device-sign-in", at))
        .map_err(ApiError::TooMany)?;
    let device = random_token();
    let hash = hash_token(&device);
    let client = client_name(&body.client);
    let code = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            tx.execute("DELETE FROM device_codes WHERE expires_at < ?1", [at - 3600])?;
            let waiting: i64 = tx.query_row(
                "SELECT count(*) FROM device_codes WHERE expires_at > ?1 AND approved_at IS NULL AND denied_at IS NULL AND used_at IS NULL",
                [at],
                |r| r.get(0),
            )?;
            if waiting >= MAX_WAITING {
                return Ok(None);
            }

            let mut code = user_code();
            while tx.query_row("SELECT EXISTS (SELECT 1 FROM device_codes WHERE user_code = ?1)", [&code], |r| r.get::<_, bool>(0))? {
                code = user_code();
            }
            tx.execute(
                "INSERT INTO device_codes (code_hash, user_code, client, expires_at) VALUES (?1, ?2, ?3, ?4)",
                params![hash, code, client, at + LIFETIME],
            )?;
            tx.commit()?;
            Ok(Some(code))
        })
        .await?
        .ok_or(ApiError::TooMany(60))?;
    Ok(Json(json!({
        "device_code": device,
        "user_code": code,
        "verification_uri": format!("{}/cli/authorize", state.settings.public_url),
        "expires_in": LIFETIME,
        "interval": INTERVAL,
    })))
}

fn session_only(user: &CurrentUser) -> Result<(), ApiError> {
    if user.0.token.is_some() {
        Err(ApiError::Forbidden)
    } else {
        Ok(())
    }
}

async fn describe(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(code): Path<String>,
) -> Result<Json<Value>, ApiError> {
    session_only(&user)?;
    let code = normalise(&code);
    let at = now();
    let row: Option<(String, i64, Option<i64>, Option<i64>)> = state
        .dbs
        .control()
        .read(move |c| {
            Ok(c.query_row(
                "SELECT client, expires_at, approved_at, denied_at FROM device_codes WHERE user_code = ?1",
                [code],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?)
        })
        .await?;
    let (client, expires, approved, denied) = row.ok_or(ApiError::NotFound)?;
    let status = match (approved, denied) {
        (Some(_), _) => "approved",
        (_, Some(_)) => "denied",
        _ if expires <= at => "expired",
        _ => "waiting",
    };
    Ok(Json(
        json!({ "client": client, "status": status, "expires_at": expires, "email": user.0.email }),
    ))
}

async fn decide(
    state: AppState,
    user: CurrentUser,
    headers: HeaderMap,
    code: String,
    approve: bool,
) -> Result<StatusCode, ApiError> {
    session_only(&user)?;
    check_csrf(&user, &headers)?;
    let code = normalise(&code);
    let (id, at) = (user.0.user_id, now());
    let changed = state
        .dbs
        .control()
        .write(move |c| {
            let sql = if approve {
                "UPDATE device_codes SET approved_at = ?3, user_id = ?2 WHERE user_code = ?1 AND approved_at IS NULL AND denied_at IS NULL AND expires_at > ?3"
            } else {
                "UPDATE device_codes SET denied_at = ?3, user_id = ?2 WHERE user_code = ?1 AND approved_at IS NULL AND denied_at IS NULL AND expires_at > ?3"
            };
            Ok(c.execute(sql, params![code, id, at])?)
        })
        .await?;
    if changed == 0 {
        return Err(bad(
            "this code has expired or was already used; start again from the terminal",
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn approve(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(code): Path<String>,
) -> Result<StatusCode, ApiError> {
    decide(state, user, headers, code, true).await
}

async fn deny(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Path(code): Path<String>,
) -> Result<StatusCode, ApiError> {
    decide(state, user, headers, code, false).await
}

#[derive(Deserialize)]
struct Poll {
    device_code: String,
}

type Device = (
    Option<i64>,
    String,
    i64,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

enum Outcome {
    Waiting,
    SlowDown,
    Denied,
    Gone,
    Token(String, String),
}

fn answer(status: StatusCode, error: &str) -> Response {
    (status, Json(json!({ "error": error }))).into_response()
}

async fn token(
    State(state): State<AppState>,
    Json(body): Json<Poll>,
) -> Result<Response, ApiError> {
    let hash = hash_token(&body.device_code);
    let at = now();
    let outcome = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let row: Option<Device> = tx
                .query_row(
                    "SELECT user_id, client, expires_at, approved_at, denied_at, used_at, last_poll_at FROM device_codes WHERE code_hash = ?1",
                    [&hash],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
                )
                .optional()?;
            let Some((user, client, expires, approved, denied, used, polled)) = row else {
                return Ok(Outcome::Gone);
            };
            if used.is_some() || expires <= at {
                return Ok(Outcome::Gone);
            }

            if denied.is_some() {
                return Ok(Outcome::Denied);
            }

            tx.execute("UPDATE device_codes SET last_poll_at = ?2 WHERE code_hash = ?1", params![hash, at])?;
            let (Some(user), Some(_)) = (user, approved) else {
                tx.commit()?;
                return Ok(if polled.is_some_and(|p| at - p < INTERVAL - 1) { Outcome::SlowDown } else { Outcome::Waiting });
            };
            let email: Option<String> = tx
                .query_row("SELECT email FROM users WHERE id = ?1 AND disabled = 0", [user], |r| r.get(0))
                .optional()?;
            let Some(email) = email else {
                return Ok(Outcome::Denied);
            };
            let name: String = if client.is_empty() { "Command line".to_string() } else { format!("Command line on {client}") }
                .chars()
                .take(60)
                .collect();
            let (_, secret) = insert_personal(&tx, user, &name, Scope::Write, None, Some(at + TOKEN_DAYS * 86_400))?;
            tx.execute("UPDATE device_codes SET used_at = ?2 WHERE code_hash = ?1", params![hash, at])?;
            tx.commit()?;
            Ok(Outcome::Token(secret, email))
        })
        .await?;
    Ok(match outcome {
        Outcome::Waiting => answer(StatusCode::PRECONDITION_REQUIRED, "authorization_pending"),
        Outcome::SlowDown => answer(StatusCode::TOO_MANY_REQUESTS, "slow_down"),
        Outcome::Denied => answer(StatusCode::FORBIDDEN, "access_denied"),
        Outcome::Gone => answer(StatusCode::GONE, "expired_token"),
        Outcome::Token(token, email) => (
            [(header::CACHE_CONTROL, "no-store")],
            Json(json!({ "token": token, "email": email, "scope": "write", "expires_in_days": TOKEN_DAYS })),
        )
            .into_response(),
    })
}

const INSTALL: &str = include_str!("install.sh");

async fn install_script(State(state): State<AppState>) -> Response {
    let script = INSTALL
        .replace("__TAZKO_HOST__", &state.settings.public_url)
        .replace("__TAZKO_TARGET__", &target());
    (
        [
            (header::CONTENT_TYPE, "text/x-shellscript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        script,
    )
        .into_response()
}

pub fn target() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

async fn download() -> Result<Response, ApiError> {
    let path = std::env::current_exe().map_err(|_| ApiError::NotFound)?;
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|_| ApiError::NotFound)?;
    let size = file.metadata().await.map_err(|_| ApiError::NotFound)?.len();
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CONTENT_LENGTH, size.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"tazko-{}\"", target()),
            ),
            (header::CACHE_CONTROL, "no-cache".to_string()),
        ],
        crate::files::file_body(file, size),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_codes_are_easy_to_read_and_compare() {
        let code = user_code();
        assert_eq!(code.len(), 9);
        assert!(
            code.chars()
                .all(|c| c == '-' || ALPHABET.contains(&(c as u8)))
        );
        assert_eq!(normalise("wdjb mjht"), "WDJB-MJHT");
        assert_eq!(normalise("short"), "");
        assert_eq!(client_name("lap\ntop"), "laptop");
    }

    #[test]
    fn the_client_address_is_the_last_hop_the_proxy_added() {
        let mut h = HeaderMap::new();
        assert_eq!(client_address(&h, "x-forwarded-for"), "direct");
        h.insert("x-forwarded-for", "6.6.6.6, 203.0.113.7".parse().unwrap());
        h.insert("cf-connecting-ip", "198.51.100.2".parse().unwrap());
        assert_eq!(client_address(&h, "x-forwarded-for"), "203.0.113.7");
        h.append("x-forwarded-for", "192.0.2.9".parse().unwrap());
        assert_eq!(client_address(&h, "x-forwarded-for"), "192.0.2.9");
        assert_eq!(client_address(&h, "cf-connecting-ip"), "198.51.100.2");
        h.insert("cf-connecting-ip", "not an address".parse().unwrap());
        assert_eq!(client_address(&h, "cf-connecting-ip"), "direct");
    }
}
