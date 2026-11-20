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
use crate::sanitize::escape_html;
use crate::session::{SessionUser, now};
use crate::state::AppState;
use message::{Message, name};

const TICK: Duration = Duration::from_secs(5);
const SEND_BATCH: i64 = 10;
const BACKOFF: [i64; 5] = [60, 300, 1800, 7200, 21600];
const PURGE_EVERY: i64 = 3600;
const KEEP: i64 = 30 * 24 * 3600;
const INVITES_PER_ADDRESS_PER_DAY: i64 = 3;
const INVITES_PER_COMPANY_PER_HOUR: i64 = 50;
const TESTS_PER_HOUR: i64 = 5;

#[derive(Default)]
pub struct Outgoing {
    pub kind: &'static str,
    pub to: String,
    pub subject: String,
    pub lines: Vec<String>,
    pub items: Vec<(String, String)>,
    pub link: String,
    pub button: &'static str,
    pub footer: Vec<(String, String)>,
    pub unsubscribe: Option<String>,
    pub org: Option<i64>,
    pub by: Option<i64>,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/v1/admin/email/test", post(send_test))
}

pub fn enabled(state: &AppState) -> bool {
    state.settings.smtp.is_some()
}

fn count(c: &Connection, sql: &str, p: impl rusqlite::Params) -> rusqlite::Result<i64> {
    c.query_row(sql, p, |r| r.get(0))
}

const P: &str = "<p style=\"margin:0 0 14px\">";

pub fn bodies(mail: &Outgoing) -> (String, String) {
    let mut text: Vec<String> = mail.lines.clone();
    if !mail.items.is_empty() {
        text.push(
            mail.items
                .iter()
                .map(|(t, l)| format!("- {t}\n  {l}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    text.push(format!("{}: {}", mail.button, mail.link));
    for (t, l) in &mail.footer {
        text.push(format!("{t}: {l}"));
    }

    let mut body: String = mail
        .lines
        .iter()
        .map(|l| format!("{P}{}</p>", escape_html(l)))
        .collect();
    if !mail.items.is_empty() {
        body.push_str("<ul style=\"margin:0 0 14px;padding-left:20px\">");
        for (t, l) in &mail.items {
            body.push_str(&format!(
                "<li style=\"margin:0 0 8px\"><a href=\"{}\" style=\"color:#4f46e5\">{}</a></li>",
                escape_html(l),
                escape_html(t)
            ));
        }
        body.push_str("</ul>");
    }
    let (link, button) = (escape_html(&mail.link), escape_html(mail.button));
    let footer: String = mail
        .footer
        .iter()
        .map(|(t, l)| {
            format!(
                "<a href=\"{}\" style=\"color:#6b6b80\">{}</a>",
                escape_html(l),
                escape_html(t)
            )
        })
        .collect::<Vec<_>>()
        .join(" · ");
    let html = format!(
        "<!doctype html><html><body style=\"margin:0;padding:24px;background:#f6f6fb;font:15px/1.5 -apple-system,Segoe UI,Roboto,sans-serif;color:#1d1d2b\">\
<div style=\"max-width:520px;margin:0 auto;background:#fff;border-radius:12px;padding:28px\">\
<p style=\"margin:0 0 18px;font-weight:700;font-size:18px\">Tazko</p>{body}\
<p style=\"margin:22px 0\"><a href=\"{link}\" style=\"background:#4f46e5;color:#fff;text-decoration:none;padding:10px 18px;border-radius:8px;display:inline-block\">{button}</a></p>\
<p style=\"margin:0;color:#6b6b80;font-size:13px\">{link}</p>\
<p style=\"margin:14px 0 0;color:#6b6b80;font-size:13px\">{footer}</p></div></body></html>"
    );
    (text.join("\n\n") + "\n", html)
}

pub fn enqueue(c: &Connection, mail: &Outgoing) -> rusqlite::Result<i64> {
    let (text, html) = bodies(mail);
    c.execute(
        "INSERT INTO email_queue (kind, to_addr, subject, body_text, body_html, org_id, created_by, unsubscribe_url)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![mail.kind, mail.to, mail.subject, text, html, mail.org, mail.by, mail.unsubscribe],
    )?;
    Ok(c.last_insert_rowid())
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Emailed {
    #[default]
    Skipped,
    Limited,
    Sent,
}

impl Emailed {
    pub fn json(self, result: &str, public_url: &str) -> Value {
        json!({ "result": result, "emailed": self == Emailed::Sent, "email_limited": self == Emailed::Limited, "share_url": public_url })
    }
}

pub struct InviteMail {
    wanted: bool,
    inviter: String,
    public_url: String,
}

impl InviteMail {
    pub fn new(state: &AppState, user: &SessionUser, send_email: bool) -> Self {
        let inviter = message::display(if user.name.trim().is_empty() {
            &user.email
        } else {
            &user.name
        });
        InviteMail {
            wanted: send_email && enabled(state),
            inviter,
            public_url: state.settings.public_url.clone(),
        }
    }

    pub fn mail(
        &self,
        to: &str,
        company: &str,
        project: Option<&str>,
        org: i64,
        by: i64,
    ) -> Outgoing {
        let company = name(company);
        let place = match project {
            Some(p) => format!("the {} project in {company}", name(p)),
            None => company.clone(),
        };
        Outgoing {
            kind: "invite",
            to: to.to_string(),
            subject: format!("You are invited to {company} on Tazko"),
            lines: vec![
                format!("{} invited you to {place} on Tazko.", self.inviter),
                format!("Sign in with Google using this address ({to}). You join automatically."),
                "If you did not expect this, you can ignore this email.".into(),
            ],
            link: format!("{}/", self.public_url),
            button: "Sign in with Google",
            org: Some(org),
            by: Some(by),
            ..Default::default()
        }
    }

    pub fn queue(
        &self,
        c: &Connection,
        to: &str,
        company: &str,
        project: Option<&str>,
        org: i64,
        by: i64,
    ) -> rusqlite::Result<Emailed> {
        let disabled: bool = c.query_row(
            "SELECT EXISTS (SELECT 1 FROM users WHERE email = ?1 AND disabled = 1)",
            [to],
            |r| r.get(0),
        )?;
        if !self.wanted || !to.is_ascii() || disabled {
            return Ok(Emailed::Skipped);
        }

        if !invitation_allowed(c, org, to)? {
            return Ok(Emailed::Limited);
        }

        enqueue(c, &self.mail(to, company, project, org, by))?;
        Ok(Emailed::Sent)
    }
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

pub fn status(c: &Connection) -> rusqlite::Result<Value> {
    let day = now() - 86_400;
    let (pending, sent, failed): (i64, i64, i64) = c.query_row(
        "SELECT coalesce(sum(status = 'pending'), 0),
                coalesce(sum(status = 'sent' AND sent_at > ?1), 0),
                coalesce(sum(status = 'failed' AND last_attempt_at > ?1), 0)
         FROM email_queue",
        [day],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let last_error: Option<(String, i64)> = c
        .query_row(
            "SELECT error, last_attempt_at FROM email_queue WHERE error IS NOT NULL AND last_attempt_at > ?1
             ORDER BY last_attempt_at DESC, id DESC LIMIT 1",
            [day],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(json!({
        "pending": pending,
        "sent_24h": sent,
        "failed_24h": failed,
        "last_error": last_error.map(|(e, at)| json!({ "error": e, "at": at })),
    }))
}

pub fn describe(smtp: &Smtp) -> Value {
    json!({ "host": smtp.host, "port": smtp.port, "from": smtp.from.header(), "signs_in": smtp.user.is_some() })
}

pub async fn queue_test(
    state: &AppState,
    mail: Outgoing,
    limit: i64,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if !enabled(state) {
        return Err(bad("email is not set up on this server"));
    }

    let to = mail.to.clone();
    let queued = state
        .dbs
        .control()
        .write(move |c| {
            let recent = count(
                c,
                "SELECT count(*) FROM email_queue WHERE kind = 'test' AND created_by = ?1 AND created_at > ?2",
                params![mail.by, now() - 3600],
            )?;
            if recent >= limit {
                return Ok(false);
            }

            enqueue(c, &mail)?;
            Ok(true)
        })
        .await?;
    if !queued {
        return Err(ApiError::TooMany(3600));
    }

    Ok((StatusCode::ACCEPTED, Json(json!({ "to": to }))))
}

async fn send_test(
    State(state): State<AppState>,
    admin: SuperAdmin,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = admin.writing(&headers)?;
    let mail = Outgoing {
        kind: "test",
        to: admin.0.email.clone(),
        subject: "Tazko test email".into(),
        lines: vec!["This is a test email from your Tazko server. Email is working.".into()],
        link: format!("{}/admin", state.settings.public_url),
        button: "Open the admin console",
        by: Some(by),
        ..Default::default()
    };
    queue_test(&state, mail, TESTS_PER_HOUR).await
}

struct Due {
    id: i64,
    to: String,
    subject: String,
    text: String,
    html: String,
    attempts: i64,
    unsubscribe: Option<String>,
}

pub async fn job(state: AppState) {
    if !enabled(&state) {
        return;
    }

    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_purge = 0;
    loop {
        tick.tick().await;
        if let Err(e) = deliver_due(&state, now()).await {
            eprintln!("tazko: email delivery failed: {e:?}");
        }

        if now() - last_purge > PURGE_EVERY {
            last_purge = now();
            if let Err(e) = purge(&state, last_purge).await {
                eprintln!("tazko: email clean-up failed: {e:?}");
            }
        }
    }
}

pub async fn purge(state: &AppState, at: i64) -> crate::db::DbResult<usize> {
    state
        .dbs
        .control()
        .write(move |c| {
            Ok(c.execute(
                "DELETE FROM email_queue WHERE status != 'pending' AND created_at < ?1",
                [at - KEEP],
            )?)
        })
        .await
}

fn send_one(smtp: &Smtp, d: &Due) -> Result<(), smtp::Failure> {
    let data = Message {
        from: &smtp.from,
        to: &d.to,
        subject: &d.subject,
        text: &d.text,
        html: &d.html,
        date: now(),
        unsubscribe: d.unsubscribe.as_deref(),
    }
    .render()
    .ok_or_else(|| smtp::Failure::Permanent("the recipient address is not valid".into()))?;
    smtp::send(smtp, &d.to, &data)
}

pub async fn deliver_due(state: &AppState, at: i64) -> crate::db::DbResult<usize> {
    if !enabled(state) {
        return Ok(0);
    }

    let due = state
        .dbs
        .control()
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT id, to_addr, subject, body_text, body_html, attempts, unsubscribe_url FROM email_queue
                 WHERE status = 'pending' AND next_attempt_at <= ?1 ORDER BY next_attempt_at, id LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![at, SEND_BATCH], |r| {
                    Ok(Due { id: r.get(0)?, to: r.get(1)?, subject: r.get(2)?, text: r.get(3)?, html: r.get(4)?, attempts: r.get(5)?, unsubscribe: r.get(6)? })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    let Some(smtp) = state.settings.smtp.clone().filter(|_| !due.is_empty()) else {
        return Ok(0);
    };
    let results = tokio::task::spawn_blocking(move || {
        let mut results = vec![];
        for d in due {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| send_one(&smtp, &d)))
                    .unwrap_or_else(|_| {
                        Err(smtp::Failure::Temporary(
                            "sending stopped unexpectedly".into(),
                        ))
                    });
            let stop = matches!(result, Err(smtp::Failure::Temporary(_)));
            results.push((d.id, d.attempts, result));
            if stop {
                break;
            }
        }
        results
    })
    .await
    .unwrap_or_default();
    let count = results.len();
    let finished = now();
    state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            for (id, tried, result) in results {
                let (status, next, error) = match result {
                    Ok(()) => ("sent", finished, None),
                    Err(smtp::Failure::Temporary(e)) => match BACKOFF.get(tried as usize) {
                        Some(wait) => ("pending", finished + wait, Some(e)),
                        None => ("failed", finished, Some(e)),
                    },
                    Err(smtp::Failure::Permanent(e)) => ("failed", finished, Some(e)),
                };
                tx.execute(
                    "UPDATE email_queue SET status = ?2, attempts = ?3, error = ?4, next_attempt_at = ?5, last_attempt_at = ?6,
                        sent_at = CASE WHEN ?2 = 'sent' THEN ?6 ELSE sent_at END WHERE id = ?1",
                    params![id, status, tried + 1, error, next, finished],
                )?;
            }
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
    fn invitation_text_is_escaped_short_and_link_free() {
        let invite = InviteMail {
            wanted: true,
            inviter: name(
                "Lan <script>\r\nBcc: x@y.z visit https://evil.example and www.evil.example",
            ),
            public_url: "https://tazko.example.com".into(),
        };
        let mail = invite.mail("mai@example.com", "Acme & Co", Some("Web"), 1, 2);
        assert_eq!(mail.subject, "You are invited to Acme & Co on Tazko");
        assert!(!invite.inviter.contains("://") && !invite.inviter.contains("www.evil"));
        assert!(invite.inviter.chars().count() <= 60);
        let page = bodies(&mail).1;
        assert!(page.contains("Lan &lt;script&gt; Bcc: x@y. z"));
        assert!(page.contains("the Web project in Acme &amp; Co"));
        assert!(!page.contains("<script>"));
        assert_eq!(mail.link, "https://tazko.example.com/");
    }
}
