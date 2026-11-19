use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::{HeaderMap, header};
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::db::{Database, DbResult};

pub const COOKIE: &str = "tazko_session";
pub const LIFETIME_SECS: i64 = 30 * 24 * 60 * 60;
const TOUCH_AFTER_SECS: i64 = 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionUser {
    pub user_id: i64,
    pub email: String,
    pub name: String,
    pub is_super_admin: bool,
    pub csrf_token: String,
    pub token: Option<std::sync::Arc<crate::api::tokens::TokenLimits>>,
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("system random source");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.trim().to_string())
        .filter(|v| v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()))
}

pub async fn create(
    control: &Database,
    user_id: i64,
    user_agent: Option<String>,
) -> DbResult<(String, String)> {
    let token = random_token();
    let csrf = random_token();
    let (id_hash, csrf_stored) = (hash_token(&token), csrf.clone());
    control
        .write(move |c| {
            let at = now();
            c.execute(
                "INSERT INTO sessions (id_hash, user_id, csrf_token, user_agent, created_at, last_used_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
                params![id_hash, user_id, csrf_stored, user_agent, at, at + LIFETIME_SECS],
            )?;
            Ok(())
        })
        .await?;
    Ok((token, csrf))
}

pub async fn lookup(control: &Database, token: &str) -> DbResult<Option<SessionUser>> {
    let id_hash = hash_token(token);
    let found = control
        .read({
            let id_hash = id_hash.clone();
            move |c| {
                Ok(c.query_row(
                    "SELECT u.id, u.email, u.name, u.is_super_admin, s.csrf_token, s.last_used_at
                     FROM sessions s JOIN users u ON u.id = s.user_id
                     WHERE s.id_hash = ?1 AND s.expires_at > ?2 AND u.disabled = 0",
                    params![id_hash, now()],
                    |r| {
                        Ok((
                            SessionUser {
                                user_id: r.get(0)?,
                                email: r.get(1)?,
                                name: r.get(2)?,
                                is_super_admin: r.get(3)?,
                                csrf_token: r.get(4)?,
                                token: None,
                            },
                            r.get::<_, i64>(5)?,
                        ))
                    },
                )
                .optional()?)
            }
        })
        .await?;

    let Some((user, last_used)) = found else {
        return Ok(None);
    };
    if now() - last_used > TOUCH_AFTER_SECS {
        control
            .write(move |c| {
                let at = now();
                c.execute(
                    "UPDATE sessions SET last_used_at = ?2, expires_at = ?3 WHERE id_hash = ?1",
                    params![id_hash, at, at + LIFETIME_SECS],
                )?;
                Ok(())
            })
            .await?;
    }

    Ok(Some(user))
}
