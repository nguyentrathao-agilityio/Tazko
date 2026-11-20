pub mod message;
pub mod smtp;
pub mod stub;

use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::post;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::admin::SuperAdmin;
use crate::config::Smtp;
use crate::error::{ApiError, bad};
use crate::session::now;
use crate::state::AppState;
use message::{Mailbox, Message, clean};

const TICK: Duration = Duration::from_secs(5);
const SEND_BATCH: i64 = 10;
const BACKOFF: [i64; 5] = [60, 300, 1800, 7200, 21600];
const KEEP: i64 = 30 * 24 * 3600;
const INVITES_PER_ADDRESS_PER_DAY: i64 = 3;
const INVITES_PER_COMPANY_PER_HOUR: i64 = 50;
const TESTS_PER_HOUR: i64 = 5;

pub struct Outgoing {
    pub kind: &'static str,
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: String,
    pub org: Option<i64>,
    pub by: Option<i64>,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/v1/admin/email/test", post(send_test))
}

pub fn enabled(state: &AppState) -> bool {
    state.settings.smtp.is_some()
}

pub fn enqueue(c: &Connection, mail: &Outgoing) -> rusqlite::Result<i64> {
    c.execute(
        "INSERT INTO email_queue (kind, to_addr, subject, body_text, body_html, org_id, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            mail.kind,
            mail.to,
            mail.subject,
            mail.text,
            mail.html,
            mail.org,
            mail.by
        ],
    )?;
    Ok(c.last_insert_rowid())
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn page(lines: &[String], link: &str, button: &str) -> String {
    let body: String = lines
        .iter()
        .map(|l| format!("<p style=\"margin:0 0 14px\">{}</p>", escape(l)))
        .collect();
    format!(
        "<!doctype html><html><body style=\"margin:0;padding:24px;background:#f6f6fb;font:15px/1.5 -apple-system,Segoe UI,Roboto,sans-serif;color:#1d1d2b\">\
<div style=\"max-width:520px;margin:0 auto;background:#fff;border-radius:12px;padding:28px\">\
<p style=\"margin:0 0 18px;font-weight:700;font-size:18px\">Tazko</p>{body}\
<p style=\"margin:22px 0\"><a href=\"{}\" style=\"background:#4f46e5;color:#fff;text-decoration:none;padding:10px 18px;border-radius:8px;display:inline-block\">{}</a></p>\
<p style=\"margin:0;color:#6b6b80;font-size:13px\">{}</p></div></body></html>",
        escape(link),
        escape(button),
        escape(link)
    )
}

pub struct Invitation<'a> {
    pub to: &'a str,
    pub inviter: &'a str,
    pub company: &'a str,
    pub project: Option<&'a str>,
    pub public_url: &'a str,
    pub org: i64,
    pub by: i64,
}

impl Invitation<'_> {
    pub fn mail(&self) -> Outgoing {
        let inviter = clean(self.inviter);
        let company = clean(self.company);
        let place = match self.project {
            Some(p) => format!("the {} project in {company}", clean(p)),
            None => company.clone(),
        };
        let link = format!("{}/", self.public_url.trim_end_matches('/'));
        let lines = vec![
            format!("{inviter} invited you to {place} on Tazko."),
            format!(
                "Sign in with Google using this address ({}). You join automatically.",
                self.to
            ),
        ];
        let text = format!(
            "{}\n\n{}\n\nSign in: {link}\n\nIf you did not expect this, you can ignore this email.\n",
            lines[0], lines[1]
        );
        let mut html_lines = lines.clone();
        html_lines.push("If you did not expect this, you can ignore this email.".into());
        Outgoing {
            kind: "invite",
            to: self.to.to_string(),
            subject: format!("{inviter} invited you to {company} on Tazko"),
            text,
            html: page(&html_lines, &link, "Sign in with Google"),
            org: Some(self.org),
            by: Some(self.by),
        }
    }
}

fn count(c: &Connection, sql: &str, p: impl rusqlite::Params) -> rusqlite::Result<i64> {
    c.query_row(sql, p, |r| r.get(0))
}

pub fn invitation_allowed(c: &Connection, org: i64, to: &str) -> rusqlite::Result<bool> {
    let at = now();
    let to_address = count(
        c,
        "SELECT count(*) FROM email_queue WHERE kind = 'invite' AND to_addr = ?1 AND created_at > ?2",
        params![to, at - 86_400],
    )?;
    let from_company = count(
        c,
        "SELECT count(*) FROM email_queue WHERE kind = 'invite' AND org_id = ?1 AND created_at > ?2",
        params![org, at - 3600],
    )?;
    Ok(to_address < INVITES_PER_ADDRESS_PER_DAY && from_company < INVITES_PER_COMPANY_PER_HOUR)
}

#[derive(Default)]
pub struct Invited {
    pub emailed: bool,
    pub limited: bool,
}

impl Invited {
    pub fn json(&self, result: &str, public_url: &str) -> Value {
        json!({ "result": result, "emailed": self.emailed, "email_limited": self.limited, "share_url": public_url })
    }
}

pub fn inviter_name(user: &crate::session::SessionUser) -> String {
    if user.name.trim().is_empty() {
        user.email.clone()
    } else {
        user.name.clone()
    }
}

pub fn invite_email(
    c: &Connection,
    wanted: bool,
    invitation: &Invitation<'_>,
) -> rusqlite::Result<Invited> {
    if !wanted {
        return Ok(Invited {
            emailed: false,
            limited: false,
        });
    }

    if !invitation_allowed(c, invitation.org, invitation.to)? {
        return Ok(Invited {
            emailed: false,
            limited: true,
        });
    }

    enqueue(c, &invitation.mail())?;
    Ok(Invited {
        emailed: true,
        limited: false,
    })
}

pub fn status(c: &Connection) -> rusqlite::Result<Value> {
    let day = now() - 86_400;
    let n = |status: &str, since: i64| {
        count(
            c,
            "SELECT count(*) FROM email_queue WHERE status = ?1 AND coalesce(sent_at, created_at) > ?2",
            params![status, since],
        )
    };
    let last_error: Option<(String, i64)> = c
        .query_row(
            "SELECT error, created_at FROM email_queue WHERE error IS NOT NULL ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(json!({
        "pending": count(c, "SELECT count(*) FROM email_queue WHERE status = 'pending'", [])?,
        "sent_24h": n("sent", day)?,
        "failed_24h": n("failed", day)?,
        "last_error": last_error.map(|(e, at)| json!({ "error": e, "at": at })),
    }))
}

pub fn describe(smtp: &Smtp) -> Value {
    json!({ "host": smtp.host, "port": smtp.port, "from": smtp.from, "signs_in": smtp.user.is_some() })
}

async fn send_test(
    State(state): State<AppState>,
    admin: SuperAdmin,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = admin.writing(&headers)?;
    if !enabled(&state) {
        return Err(bad("email is not set up on this server"));
    }

    let to = admin.0.email.clone();
    let link = format!("{}/admin", state.settings.public_url.trim_end_matches('/'));
    let queued = state
        .dbs
        .control()
        .write(move |c| {
            let recent = count(
                c,
                "SELECT count(*) FROM email_queue WHERE kind = 'test' AND created_by = ?1 AND created_at > ?2",
                params![by, now() - 3600],
            )?;
            if recent >= TESTS_PER_HOUR {
                return Ok(None);
            }

            let lines = vec!["This is a test email from your Tazko server. Email is working.".to_string()];
            enqueue(
                c,
                &Outgoing {
                    kind: "test",
                    to: to.clone(),
                    subject: "Tazko test email".into(),
                    text: format!("{}\n\n{link}\n", lines[0]),
                    html: page(&lines, &link, "Open the admin console"),
                    org: None,
                    by: Some(by),
                },
            )?;
            Ok(Some(to))
        })
        .await?;
    match queued {
        Some(to) => Ok((StatusCode::ACCEPTED, Json(json!({ "to": to })))),
        None => Err(ApiError::TooMany(3600)),
    }
}

struct Due {
    id: i64,
    to: String,
    subject: String,
    text: String,
    html: String,
    attempts: i64,
}

pub async fn job(state: AppState) {
    if !enabled(&state) {
        return;
    }

    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        if let Err(e) = deliver_due(&state, now()).await {
            eprintln!("tazko: email delivery failed: {e:?}");
        }
    }
}

pub async fn deliver_due(state: &AppState, at: i64) -> crate::db::DbResult<usize> {
    let Some(smtp) = state.settings.smtp.clone() else {
        return Ok(0);
    };
    let due = state
        .dbs
        .control()
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT id, to_addr, subject, body_text, body_html, attempts FROM email_queue
                 WHERE status = 'pending' AND next_attempt_at <= ?1 ORDER BY next_attempt_at, id LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![at, SEND_BATCH], |r| {
                    Ok(Due { id: r.get(0)?, to: r.get(1)?, subject: r.get(2)?, text: r.get(3)?, html: r.get(4)?, attempts: r.get(5)? })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    if due.is_empty() {
        return Ok(0);
    }

    let count = due.len();
    let results = tokio::task::spawn_blocking(move || {
        let from = Mailbox::parse(&smtp.from);
        due.into_iter()
            .map(|d| {
                let result = match &from {
                    None => Err(smtp::Failure::Permanent(
                        "TAZKO_MAIL_FROM is not a valid address".into(),
                    )),
                    Some(from) => Message {
                        from,
                        to: &d.to,
                        subject: &d.subject,
                        text: &d.text,
                        html: &d.html,
                        date: now(),
                    }
                    .render()
                    .ok_or_else(|| {
                        smtp::Failure::Permanent("the recipient address is not valid".into())
                    })
                    .and_then(|data| smtp::send(&smtp, &from.addr, &d.to, &data)),
                };
                (d.id, d.attempts, result)
            })
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();
    let finished = now();
    state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            for (id, tried, result) in results {
                let (status, next, error) = match &result {
                    Ok(()) => ("sent", finished, None),
                    Err(smtp::Failure::Temporary(e)) => match BACKOFF.get(tried as usize) {
                        Some(wait) => ("pending", finished + wait, Some(e.clone())),
                        None => ("failed", finished, Some(e.clone())),
                    },
                    Err(smtp::Failure::Permanent(e)) => ("failed", finished, Some(e.clone())),
                };
                tx.execute(
                    "UPDATE email_queue SET status = ?2, attempts = ?3, error = ?4, next_attempt_at = ?5,
                        sent_at = CASE WHEN ?2 = 'sent' THEN ?6 ELSE sent_at END WHERE id = ?1",
                    params![id, status, tried + 1, error, next, finished],
                )?;
            }
            tx.execute(
                "DELETE FROM email_queue WHERE status != 'pending' AND created_at < ?1",
                [finished - KEEP],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitation_text_is_escaped_and_single_line() {
        let mail = Invitation {
            to: "mai@example.com",
            inviter: "Lan <script>\r\nBcc: x@y.z",
            company: "Acme & Co",
            project: Some("Web"),
            public_url: "https://tazko.example.com",
            org: 1,
            by: 2,
        }
        .mail();
        assert_eq!(
            mail.subject,
            "Lan <script> Bcc: x@y.z invited you to Acme & Co on Tazko"
        );
        assert!(mail.html.contains(
            "Lan &lt;script&gt; Bcc: x@y.z invited you to the Web project in Acme &amp; Co"
        ));
        assert!(!mail.html.contains("<script>"));
        assert!(mail.text.contains("Sign in: https://tazko.example.com/"));
    }
}
