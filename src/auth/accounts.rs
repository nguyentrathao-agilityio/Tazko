use rusqlite::{Connection, OptionalExtension, params};
use serde_json::json;

use super::google::Claims;
use crate::db::DbResult;
use crate::session::now;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    EmailNotVerified,
    NotInvited,
    Disabled,
    AccountMismatch,
    InviteExpired,
}

impl Refusal {
    pub fn code(self) -> &'static str {
        match self {
            Refusal::EmailNotVerified => "email_not_verified",
            Refusal::NotInvited => "not_invited",
            Refusal::Disabled => "disabled",
            Refusal::AccountMismatch => "account_mismatch",
            Refusal::InviteExpired => "invite_expired",
        }
    }
}

pub fn sign_in(c: &mut Connection, claims: &Claims) -> DbResult<Result<i64, Refusal>> {
    let email = claims
        .email
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let tx = c.transaction()?;
    let outcome = decide(&tx, claims, &email)?;
    match outcome {
        Ok(user_id) => {
            accept_invites(&tx, user_id, &email)?;
            tx.execute(
                "UPDATE users SET name = coalesce(?2, name), avatar = ?3, hd = ?4, last_seen = ?5 WHERE id = ?1",
                params![user_id, claims.name, claims.picture, claims.hd, now()],
            )?;
        }
        Err(reason) => {
            tx.execute(
                "INSERT INTO audit_log (action, target, detail) VALUES ('sign_in_refused', ?1, ?2)",
                params![
                    email,
                    json!({ "reason": reason.code(), "hd": claims.hd }).to_string()
                ],
            )?;
        }
    }

    tx.commit()?;
    Ok(outcome)
}

fn decide(tx: &Connection, claims: &Claims, email: &str) -> DbResult<Result<i64, Refusal>> {
    if !claims.email_verified || email.is_empty() {
        return Ok(Err(Refusal::EmailNotVerified));
    }

    let by_sub: Option<(i64, bool)> = tx
        .query_row(
            "SELECT id, disabled FROM users WHERE google_sub = ?1",
            params![claims.sub],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((id, disabled)) = by_sub {
        if disabled {
            return Ok(Err(Refusal::Disabled));
        }

        let taken: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM users WHERE email = ?1 AND id != ?2)",
            params![email, id],
            |r| r.get(0),
        )?;
        if !taken {
            tx.execute(
                "UPDATE users SET email = ?2 WHERE id = ?1",
                params![id, email],
            )?;
        }

        return Ok(Ok(id));
    }

    let by_email: Option<(i64, bool, Option<String>)> = tx
        .query_row(
            "SELECT id, disabled, google_sub FROM users WHERE email = ?1",
            params![email],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    if let Some((id, disabled, sub)) = by_email {
        if sub.is_some() {
            return Ok(Err(Refusal::AccountMismatch));
        }

        if disabled {
            return Ok(Err(Refusal::Disabled));
        }

        tx.execute(
            "UPDATE users SET google_sub = ?2 WHERE id = ?1",
            params![id, claims.sub],
        )?;
        return Ok(Ok(id));
    }

    let invited: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM invites i JOIN orgs o ON o.id = i.org_id
         WHERE i.email = ?1 AND i.accepted_at IS NULL AND i.revoked_at IS NULL AND i.expires_at > unixepoch()
           AND o.status = 'active')",
        params![email],
        |r| r.get(0),
    )?;
    if !invited {
        let expired: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM invites i JOIN orgs o ON o.id = i.org_id
             WHERE i.email = ?1 AND i.accepted_at IS NULL AND i.revoked_at IS NULL AND o.status = 'active')",
            params![email],
            |r| r.get(0),
        )?;
        return Ok(Err(if expired {
            Refusal::InviteExpired
        } else {
            Refusal::NotInvited
        }));
    }

    tx.execute(
        "INSERT INTO users (email, google_sub, name) VALUES (?1, ?2, coalesce(?3, ''))",
        params![email, claims.sub, claims.name],
    )?;
    Ok(Ok(tx.last_insert_rowid()))
}

pub fn accept_invites(tx: &Connection, user_id: i64, email: &str) -> DbResult<()> {
    let at = now();
    tx.execute(
        "INSERT INTO org_members (org_id, user_id, role)
         SELECT i.org_id, ?1, CASE WHEN max(i.company_role = 'owner') THEN 'owner' ELSE 'member' END
         FROM invites i JOIN orgs o ON o.id = i.org_id
         WHERE i.email = ?2 AND i.accepted_at IS NULL AND i.revoked_at IS NULL AND i.expires_at > unixepoch()
           AND o.status = 'active'
         GROUP BY i.org_id
         ON CONFLICT (org_id, user_id) DO NOTHING",
        params![user_id, email],
    )?;
    tx.execute(
        "UPDATE invites SET accepted_at = ?2
         WHERE email = ?1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > unixepoch()
           AND org_id IN (SELECT id FROM orgs WHERE status = 'active')",
        params![email, at],
    )?;
    Ok(())
}
