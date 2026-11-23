use std::collections::HashMap;
use std::time::Duration;

use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Json, Response};
use axum::routing::{get, post};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use super::CAN_SEE;
use super::prefs::{Frequency, Prefs};
use crate::auth::{check_csrf, cookies};
use crate::db::{Database, DbResult};
use crate::error::ApiError;
use crate::mail::message::{display, name, safe};
use crate::mail::{self, Outgoing};
use crate::sanitize::escape_html;
use crate::session::now;
use crate::state::AppState;
use crate::tenant::CurrentUser;

const TICK: Duration = Duration::from_secs(60);
const PER_USER: i64 = 200;
const STALE: i64 = 3 * 86_400;
const INSTANT_GAP: i64 = 300;
const HOURLY_GAP: i64 = 3600;
const DIGEST_LIMIT: usize = 30;
const UNSUBSCRIBE: &str = "unsubscribe";
const UNSUBSCRIBE_TTL: i64 = 365 * 86_400;
const TESTS_PER_HOUR: i64 = 3;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/email/unsubscribe",
            get(unsubscribe_page).post(unsubscribe),
        )
        .route("/api/v1/me/email/previews", get(previews))
        .route("/api/v1/me/email/test", post(send_test))
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: i64,
    pub user: i64,
    pub kind: String,
    pub actor: Option<String>,
    pub card_id: i64,
    pub card: String,
    pub board_id: i64,
    pub board: String,
    pub comment: Option<String>,
    pub item: Option<String>,
    pub due_at: Option<i64>,
    pub visible: bool,
}

pub struct Place<'a> {
    pub public_url: &'a str,
    pub slug: &'a str,
    pub company: &'a str,
}

impl Place<'_> {
    fn base(&self) -> String {
        format!("{}/c/{}", self.public_url, self.slug)
    }

    fn card(&self, i: &Item) -> String {
        format!("{}/b/{}/c/{}", self.base(), i.board_id, i.card_id)
    }

    fn settings(&self) -> String {
        format!("{}/me/notifications", self.base())
    }
}

fn due_phrase(i: &Item, now: i64) -> String {
    i.due_at.map_or("soon".into(), |d| when(d, now))
}

fn when(due: i64, now: i64) -> String {
    match due - now {
        l if l < -60 => "overdue".into(),
        l if l <= 60 => "now".into(),
        l if l < 2 * 3600 => "within 2 hours".into(),
        l if l < 86_400 => format!("in {} hours", l / 3600),
        l if l < 2 * 86_400 => "tomorrow".into(),
        l => format!("in {} days", l / 86_400),
    }
}

fn who(i: &Item) -> String {
    i.actor
        .clone()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Someone".into())
}

pub fn summary(i: &Item, now: i64) -> String {
    let card = safe(&i.card, 120);
    match i.kind.as_str() {
        "mention" => format!("{} mentioned you on {card}", who(i)),
        "assigned" if i.item.is_some() => {
            format!("{} assigned you a checklist item on {card}", who(i))
        }
        "assigned" => format!("{} added you to {card}", who(i)),
        "card_added" => format!("New card on {}: {card}", name(&i.board)),
        "due" if due_phrase(i, now) == "overdue" => format!("{card} is overdue"),
        "due" => format!("{card} is due {}", due_phrase(i, now)),
        _ => card,
    }
}

fn detail(i: &Item, now: i64) -> Vec<String> {
    let mut lines = vec![format!("{}.", summary(i, now))];
    match (i.kind.as_str(), &i.comment, &i.item) {
        ("mention", Some(c), _) => lines.push(format!("\u{201c}{}\u{201d}", safe(c, 300))),
        ("assigned", _, Some(item)) => lines.push(format!("\u{201c}{}\u{201d}", safe(item, 200))),
        _ => {}
    }
    lines.push(format!("Board: {}", name(&i.board)));
    lines
}

fn unsubscribe_link(public_url: &str, token: &str) -> String {
    format!("{public_url}/api/v1/email/unsubscribe?t={token}")
}

fn unsubscribe_url(state: &AppState, user: i64) -> String {
    let token = cookies::seal(state.secret.expose(), UNSUBSCRIBE, UNSUBSCRIBE_TTL, user);
    unsubscribe_link(&state.settings.public_url, &token)
}

pub fn compose(
    items: &[Item],
    to: &str,
    place: &Place<'_>,
    unsubscribe: String,
    now: i64,
) -> Outgoing {
    let footer = vec![
        (
            format!("Notification settings for {}", name(place.company)),
            place.settings(),
        ),
        (
            "Unsubscribe from these emails".to_string(),
            unsubscribe.clone(),
        ),
    ];
    let base = Outgoing {
        kind: "notify",
        to: to.to_string(),
        footer,
        unsubscribe: Some(unsubscribe),
        ..Default::default()
    };
    if let [one] = items {
        let button = match one.kind.as_str() {
            "mention" => "View comment",
            _ => "Open card",
        };
        let subject = match one.kind.as_str() {
            "due" if due_phrase(one, now) == "overdue" => {
                format!("Overdue: {}", safe(&one.card, 120))
            }
            "due" => format!("Due {}: {}", due_phrase(one, now), safe(&one.card, 120)),
            _ => summary(one, now),
        };
        return Outgoing {
            subject,
            lines: detail(one, now),
            link: place.card(one),
            button,
            ..base
        };
    }

    let mut list: Vec<(String, String)> = items
        .iter()
        .take(DIGEST_LIMIT)
        .map(|i| (summary(i, now), place.card(i)))
        .collect();
    if items.len() > DIGEST_LIMIT {
        list.push((
            format!("and {} more", items.len() - DIGEST_LIMIT),
            format!("{}/projects", place.base()),
        ));
    }
    Outgoing {
        subject: format!(
            "{} updates in {} on Tazko",
            items.len(),
            name(place.company)
        ),
        lines: vec![format!(
            "Here is what happened in {} that you asked to hear about.",
            name(place.company)
        )],
        items: list,
        link: format!("{}/my-cards", place.base()),
        button: "Open Tazko",
        ..base
    }
}

#[derive(Debug)]
struct Person {
    email: String,
    active: bool,
    prefs: Prefs,
    last_sent: i64,
}

fn time_to_send(p: &Person, now: i64) -> bool {
    match p.prefs.frequency {
        Frequency::Never => false,
        Frequency::Instant => p.last_sent <= now - INSTANT_GAP,
        Frequency::Hourly => p.last_sent <= now - HOURLY_GAP,
        Frequency::Daily => {
            let today = now - now.rem_euclid(86_400) + p.prefs.daily_hour * 3600;
            let slot = if now >= today { today } else { today - 86_400 };
            p.last_sent < slot
        }
    }
}

fn waiting_users(c: &Connection) -> rusqlite::Result<Vec<i64>> {
    c.prepare("SELECT DISTINCT user_id FROM notifications WHERE emailed_at IS NULL")?
        .query_map([], |r| r.get(0))?
        .collect()
}

fn pending(c: &Connection, user: i64, now: i64) -> rusqlite::Result<Vec<Item>> {
    let mut stmt = c.prepare(&format!(
        "SELECT n.id, n.user_id, n.kind, CASE WHEN a.name = '' OR a.name = a.email THEN NULL ELSE a.name END, a.email,
                n.card_id, k.title, n.board_id, b.name, cm.body, n.item_text, n.due_at,
                {CAN_SEE} AND k.archived_at IS NULL AND n.read_at IS NULL AND n.created_at > ?3
                  AND (n.kind != 'due' OR (k.done = 0 AND k.due_at = n.due_at))
         FROM notifications n
         JOIN cards k ON k.id = n.card_id
         JOIN boards b ON b.id = n.board_id
         LEFT JOIN people a ON a.id = n.actor_id
         LEFT JOIN comments cm ON cm.id = n.comment_id
         WHERE n.emailed_at IS NULL AND n.user_id = ?1 ORDER BY n.id LIMIT ?2"
    ))?;
    stmt.query_map(params![user, PER_USER, now - STALE], |r| {
        let (shown, email): (Option<String>, Option<String>) = (r.get(3)?, r.get(4)?);
        Ok(Item {
            id: r.get(0)?,
            user: r.get(1)?,
            kind: r.get(2)?,
            actor: email.map(|e| display(shown.as_deref(), &e)),
            card_id: r.get(5)?,
            card: r.get(6)?,
            board_id: r.get(7)?,
            board: r.get(8)?,
            comment: r.get(9)?,
            item: r.get(10)?,
            due_at: r.get(11)?,
            visible: r.get(12)?,
        })
    })?
    .collect()
}

fn person(c: &Connection, user: i64, org: i64) -> rusqlite::Result<Option<Person>> {
    let found: Option<(String, bool)> = c
        .query_row(
            "SELECT u.email, u.disabled = 0 AND EXISTS (SELECT 1 FROM org_members m WHERE m.org_id = ?2 AND m.user_id = u.id)
             FROM users u WHERE u.id = ?1",
            params![user, org],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((email, active)) = found else {
        return Ok(None);
    };
    let last_sent: i64 = c
        .query_row(
            "SELECT last_sent_at FROM email_digests WHERE user_id = ?1 AND org_id = ?2",
            params![user, org],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0);
    Ok(Some(Person {
        email,
        active,
        prefs: Prefs::load(c, user)?,
        last_sent,
    }))
}

pub async fn job(state: AppState) {
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        for (slug, db) in crate::tenant::active_tenants(&state).await {
            if let Err(e) = run(&state, &slug, &db, now()).await {
                eprintln!("tazko: notification emails failed for {slug}: {e:?}");
            }
        }
    }
}

async fn mark(db: &Database, ids: Vec<i64>, now: i64) -> DbResult<()> {
    if ids.is_empty() {
        return Ok(());
    }

    db.write(move |c| {
        let tx = c.transaction()?;
        {
            let mut stmt =
                tx.prepare_cached("UPDATE notifications SET emailed_at = ?2 WHERE id = ?1")?;
            for id in ids {
                stmt.execute(params![id, now])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
    .await
}

pub async fn run(state: &AppState, slug: &str, db: &Database, now: i64) -> DbResult<usize> {
    let users = db.read(|c| Ok(waiting_users(c)?)).await?;
    if users.is_empty() {
        return Ok(0);
    }

    if !mail::enabled(state) {
        db.write(move |c| {
            Ok(c.execute(
                "UPDATE notifications SET emailed_at = ?1 WHERE emailed_at IS NULL",
                [now],
            )?)
        })
        .await?;
        return Ok(0);
    }

    let slug_owned = slug.to_string();
    let (org, company, people) = state
        .dbs
        .control()
        .read(move |c| {
            let (org, company): (i64, String) = c.query_row(
                "SELECT id, name FROM orgs WHERE slug = ?1",
                [&slug_owned],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let mut people = HashMap::new();
            for u in users {
                people.insert(u, person(c, u, org)?);
            }
            Ok((org, company, people))
        })
        .await?;

    let due: Vec<(i64, Option<Person>)> = people
        .into_iter()
        .filter(|(_, p)| {
            p.as_ref().is_none_or(|p| {
                !p.active || p.prefs.frequency == Frequency::Never || time_to_send(p, now)
            })
        })
        .collect();
    if due.is_empty() {
        return Ok(0);
    }

    let ids: Vec<i64> = due.iter().map(|(u, _)| *u).collect();
    let items: Vec<Vec<Item>> = db
        .read(move |c| {
            let mut all = vec![];
            for user in ids {
                all.push(pending(c, user, now)?);
            }
            Ok(all)
        })
        .await?;

    let place = Place {
        public_url: &state.settings.public_url,
        slug,
        company: &company,
    };
    let mut handled: Vec<i64> = vec![];
    let mut outgoing: Vec<(i64, Outgoing)> = vec![];
    for ((user, p), list) in due.into_iter().zip(items) {
        handled.extend(list.iter().map(|i| i.id));
        let Some(p) = p.filter(|p| p.active) else {
            continue;
        };
        let wanted: Vec<Item> = list
            .into_iter()
            .filter(|i| i.visible && p.prefs.by_email(&i.kind))
            .collect();
        if !wanted.is_empty() {
            let mut mail = compose(&wanted, &p.email, &place, unsubscribe_url(state, user), now);
            mail.org = Some(org);
            outgoing.push((user, mail));
        }
    }

    mark(db, handled, now).await?;
    let sent = outgoing.len();
    if sent > 0 {
        state
            .dbs
            .control()
            .write(move |c| {
                let tx = c.transaction()?;
                for (user, mail) in outgoing {
                    mail::enqueue(&tx, &mail)?;
                    tx.execute(
                        "INSERT INTO email_digests (user_id, org_id, last_sent_at) VALUES (?1, ?2, ?3)
                         ON CONFLICT (user_id, org_id) DO UPDATE SET last_sent_at = excluded.last_sent_at",
                        params![user, org, now],
                    )?;
                }
                tx.commit()?;
                Ok(())
            })
            .await?;
    }
    Ok(sent)
}

#[derive(Deserialize)]
struct Token {
    t: String,
}

fn page(title: &str, body: &str) -> Response {
    crate::api::docs::with_headers(
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>{title}</title></head>\
<body style=\"font:16px/1.5 -apple-system,Segoe UI,Roboto,sans-serif;max-width:480px;margin:48px auto;padding:0 16px;color:#1d1d2b\"><h1 style=\"font-size:22px\">{title}</h1>{body}</body></html>"
        ),
        "text/html; charset=utf-8",
    )
}

async fn unsubscribe_page(Query(q): Query<Token>) -> Response {
    let t = escape_html(&q.t);
    page(
        "Stop notification emails",
        &format!(
            "<p>Tazko will stop emailing you about mentions, assignments, new cards and due dates. You still see them in the app.</p>\
<form method=\"post\" action=\"/api/v1/email/unsubscribe?t={t}\"><button type=\"submit\" style=\"font:inherit;padding:8px 16px\">Unsubscribe</button></form>"
        ),
    )
}

async fn unsubscribe(
    State(state): State<AppState>,
    Query(q): Query<Token>,
) -> Result<Response, ApiError> {
    let Some(user) = cookies::open::<i64>(state.secret.expose(), UNSUBSCRIBE, &q.t) else {
        let mut res = page(
            "This link has expired",
            "<p>This unsubscribe link is not valid any more. You can turn notification emails off under Settings, Notifications.</p>",
        );
        *res.status_mut() = StatusCode::BAD_REQUEST;
        return Ok(res);
    };
    state
        .dbs
        .control()
        .write(move |c| {
            let change = [("email_frequency".to_string(), json!("never"))]
                .into_iter()
                .collect();
            Ok(crate::settings::store(c, user, change)?)
        })
        .await?;
    Ok(page(
        "You are unsubscribed",
        "<p>Tazko will not email you notifications any more. You still see them in the app, and you can turn emails back on under Settings, Notifications.</p>",
    ))
}

fn samples(now: i64) -> Vec<Item> {
    let base = Item {
        id: 0,
        user: 0,
        kind: String::new(),
        actor: Some("Mai Tran".into()),
        card_id: 42,
        card: "Redesign homepage hero".into(),
        board_id: 7,
        board: "Sprint 14".into(),
        comment: None,
        item: None,
        due_at: None,
        visible: true,
    };
    vec![
        Item {
            kind: "mention".into(),
            comment: Some("Option B reads best on mobile. @you can you check the contrast?".into()),
            ..base.clone()
        },
        Item {
            kind: "assigned".into(),
            ..base.clone()
        },
        Item {
            kind: "card_added".into(),
            card: "Pricing table component".into(),
            ..base.clone()
        },
        Item {
            kind: "due".into(),
            actor: None,
            card: "Pricing table component".into(),
            due_at: Some(now + 86_400),
            ..base
        },
    ]
}

async fn previews(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Value>, ApiError> {
    let at = now();
    let to = user.email.clone();
    let place = Place {
        public_url: &state.settings.public_url,
        slug: "your-company",
        company: "Your company",
    };
    let unsub = unsubscribe_link(&state.settings.public_url, "preview");
    let invite = mail::InviteMail::new(&state, &user, true).mail(
        &to,
        "Your company",
        Some("Website relaunch"),
        0,
        0,
    );
    let items = samples(at);
    let mut list: Vec<(&str, Outgoing)> = vec![("invite", invite)];
    for i in &items {
        list.push((
            i.kind.as_str(),
            compose(std::slice::from_ref(i), &to, &place, unsub.clone(), at),
        ));
    }
    list.push(("digest", compose(&items, &to, &place, unsub.clone(), at)));
    let from = state.settings.smtp.as_ref().map(|s| s.from.header());
    let out: Vec<Value> = list
        .into_iter()
        .map(|(kind, mail)| {
            let (text, html) = mail::bodies(&mail);
            json!({ "kind": kind, "subject": mail.subject, "to": mail.to, "text": text, "html": html })
        })
        .collect();
    Ok(Json(json!({ "from": from, "previews": out })))
}

async fn send_test(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    check_csrf(&user, &headers)?;
    if user.0.token.is_some() {
        return Err(ApiError::Forbidden);
    }

    let by = user.0.user_id;
    let mail = Outgoing {
        kind: "test",
        to: user.0.email.clone(),
        subject: "Tazko test email".into(),
        lines: vec![
            "This is a test email from Tazko. Your notification emails will look like this.".into(),
        ],
        link: format!("{}/", state.settings.public_url),
        button: "Open Tazko",
        unsubscribe: Some(unsubscribe_url(&state, by)),
        by: Some(by),
        ..Default::default()
    };
    mail::queue_test(&state, mail, TESTS_PER_HOUR).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_and_digest_emails_link_back_and_offer_unsubscribe() {
        let now = 1_000_000;
        let place = Place {
            public_url: "https://t.example",
            slug: "acme",
            company: "Acme",
        };
        let items = samples(now);
        let one = compose(
            &items[..1],
            "a@b.co",
            &place,
            "https://t.example/u".into(),
            now,
        );
        assert_eq!(
            one.subject,
            "Mai Tran mentioned you on Redesign homepage hero"
        );
        assert_eq!(one.link, "https://t.example/c/acme/b/7/c/42");
        assert_eq!(one.unsubscribe.as_deref(), Some("https://t.example/u"));
        let due = compose(&items[3..], "a@b.co", &place, "u".into(), now);
        assert_eq!(due.subject, "Due tomorrow: Pricing table component");
        let digest = compose(&items, "a@b.co", &place, "u".into(), now);
        assert_eq!(digest.subject, "4 updates in Acme on Tazko");
        assert_eq!(digest.items.len(), 4);
        let (text, html) = mail::bodies(&digest);
        assert!(
            text.contains("Unsubscribe from these emails: u")
                && html.contains("Notification settings for Acme")
        );
    }

    #[test]
    fn delivery_timing_follows_the_chosen_frequency() {
        let mut p = Person {
            email: "a@b.co".into(),
            active: true,
            prefs: Prefs::from_settings(&crate::settings::defaults()),
            last_sent: 0,
        };
        let now = 10 * 86_400 + 9 * 3600;
        assert!(time_to_send(&p, now));
        p.last_sent = now - 60;
        assert!(
            !time_to_send(&p, now),
            "instant emails are at least 5 minutes apart"
        );
        p.prefs.frequency = Frequency::Daily;
        p.prefs.daily_hour = 9;
        p.last_sent = now - 86_400;
        assert!(time_to_send(&p, now));
        p.prefs.daily_hour = 8;
        assert!(
            time_to_send(&p, now),
            "a missed daily hour is caught up later the same day"
        );
        p.last_sent = now - 60;
        assert!(!time_to_send(&p, now));
        p.prefs.daily_hour = 10;
        p.last_sent = now - 20 * 3600;
        assert!(
            !time_to_send(&p, now),
            "already sent after yesterday's 10:00"
        );
    }
}
