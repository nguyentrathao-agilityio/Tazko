mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::{TestApp, Who, body_text, call, person};
use serde_json::{Value, json};
use tazko::notifications::email;

const C: &str = "/api/v1/c/company-a";
const SLUG: &str = "company-a";

async fn req(
    app: &TestApp,
    who: &Who,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let res = call(app, who, method, path, body).await;
    let status = res.status();
    (
        status,
        serde_json::from_str(&body_text(res).await).unwrap_or(Value::Null),
    )
}

struct World {
    app: TestApp,
    lead: Who,
    dev: Who,
    board: i64,
    card: i64,
}

async fn world(env: &str) -> World {
    let app = TestApp::with_env(env);
    let a = app.company(SLUG, "active").await;
    let boss = person(&app, "boss@a.example").await;
    let lead = person(&app, "lead@a.example").await;
    let dev = person(&app, "dev@a.example").await;
    app.member(a, boss.id, "owner").await;
    app.member(a, lead.id, "member").await;
    app.member(a, dev.id, "member").await;
    let project = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "P", "owner_id": lead.id })),
    )
    .await
    .1["id"]
        .as_i64()
        .unwrap();
    req(
        &app,
        &lead,
        Method::POST,
        &format!("{C}/projects/{project}/invites"),
        Some(json!({ "email": "dev@a.example", "role": "member" })),
    )
    .await;
    let board = req(
        &app,
        &lead,
        Method::POST,
        &format!("{C}/projects/{project}/boards"),
        Some(json!({ "name": "Sprint" })),
    )
    .await
    .1["id"]
        .as_i64()
        .unwrap();
    let list = req(
        &app,
        &lead,
        Method::GET,
        &format!("{C}/boards/{board}"),
        None,
    )
    .await
    .1["lists"][0]["id"]
        .as_i64()
        .unwrap();
    let card = req(
        &app,
        &lead,
        Method::POST,
        &format!("{C}/boards/{board}/cards"),
        Some(json!({ "list_id": list, "title": "Hero" })),
    )
    .await
    .1["cards"][0]["id"]
        .as_i64()
        .unwrap();
    World {
        app,
        lead,
        dev,
        board,
        card,
    }
}

const SMTP: &str = "TAZKO_SMTP_HOST=127.0.0.1\nTAZKO_SMTP_PORT=2525\nTAZKO_MAIL_FROM=Tazko <no-reply@example.com>\n";

impl World {
    async fn mention_dev(&self, text: &str) {
        req(
            &self.app,
            &self.lead,
            Method::POST,
            &format!("{C}/boards/{}/cards/{}/comments", self.board, self.card),
            Some(json!({ "body": text })),
        )
        .await;
    }

    async fn kinds(&self, who: &Who) -> Vec<String> {
        req(
            &self.app,
            who,
            Method::GET,
            &format!("{C}/notifications"),
            None,
        )
        .await
        .1["notifications"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["kind"].as_str().unwrap().to_string())
            .collect()
    }

    async fn set(&self, who: &Who, body: Value) {
        assert_eq!(
            req(
                &self.app,
                who,
                Method::PATCH,
                "/api/v1/me/settings",
                Some(body)
            )
            .await
            .0,
            StatusCode::OK
        );
    }

    async fn emails(&self) -> Vec<(String, String, String, Option<String>)> {
        self.app
            .sql(|c| {
                c.prepare("SELECT to_addr, subject, body_text, unsubscribe_url FROM email_queue WHERE kind = 'notify' ORDER BY id")?
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                    .collect()
            })
            .await
    }

    async fn run(&self, at: i64) -> usize {
        let db = self.app.state.dbs.tenant(SLUG).unwrap();
        email::run(&self.app.state, SLUG, &db, at).await.unwrap()
    }
}

#[tokio::test]
async fn mention_emails_follow_the_choice_and_mentions_stay_in_the_app() {
    let w = world(SMTP).await;
    w.mention_dev("@dev please look").await;
    assert_eq!(w.run(tazko::session::now()).await, 1);
    let mails = w.emails().await;
    assert_eq!(mails[0].0, "dev@a.example");
    assert_eq!(mails[0].1, "lead@a.example mentioned you on Hero");
    assert!(
        mails[0].2.contains("please look")
            && mails[0]
                .2
                .contains(&format!("/c/company-a/b/{}/c/{}", w.board, w.card))
    );
    assert!(
        mails[0]
            .3
            .as_deref()
            .unwrap()
            .contains("/api/v1/email/unsubscribe?t=")
    );

    w.set(&w.dev, json!({ "notify_mention_email": false }))
        .await;
    w.mention_dev("@dev again").await;
    assert_eq!(w.run(tazko::session::now() + 600).await, 0);
    assert_eq!(
        w.emails().await.len(),
        1,
        "no email once mention emails are off"
    );
    assert_eq!(
        w.kinds(&w.dev).await,
        ["mention", "mention"],
        "mentions still show in the app"
    );

    w.set(&w.dev, json!({ "notify_mention_app": false })).await;
    assert!(
        w.kinds(&w.dev).await.is_empty(),
        "turning the app switch off hides them"
    );
}

#[tokio::test]
async fn several_waiting_notifications_become_one_digest_and_hourly_waits() {
    let w = world(SMTP).await;
    w.set(&w.dev, json!({ "email_frequency": "hourly" })).await;
    let t0 = tazko::session::now();
    w.mention_dev("@dev one").await;
    req(
        &w.app,
        &w.lead,
        Method::PUT,
        &format!(
            "{C}/boards/{}/cards/{}/members/{}",
            w.board, w.card, w.dev.id
        ),
        Some(json!({ "on": true })),
    )
    .await;
    assert_eq!(w.run(t0).await, 1);
    let mails = w.emails().await;
    assert_eq!(mails[0].1, "2 updates in COMPANY-A on Tazko");
    assert!(mails[0].2.contains("lead@a.example added you to Hero"));
    w.mention_dev("@dev two").await;
    assert_eq!(w.run(t0 + 600).await, 0, "the next hourly digest waits");
    assert_eq!(w.run(t0 + 3700).await, 1);
}

#[tokio::test]
async fn the_reminder_lead_time_is_each_persons_choice() {
    let w = world("").await;
    req(
        &w.app,
        &w.lead,
        Method::PUT,
        &format!(
            "{C}/boards/{}/cards/{}/members/{}",
            w.board, w.card, w.dev.id
        ),
        Some(json!({ "on": true })),
    )
    .await;
    let now = tazko::session::now();
    req(
        &w.app,
        &w.lead,
        Method::PATCH,
        &format!("{C}/boards/{}/cards/{}", w.board, w.card),
        Some(json!({ "due_at": now + 5 * 3600 })),
    )
    .await;
    w.set(
        &w.dev,
        json!({ "due_reminder": "1h", "notify_due_email": false }),
    )
    .await;
    tazko::notifications::due_soon_all(&w.app.state, now).await;
    assert!(
        !w.kinds(&w.dev).await.contains(&"due".to_string()),
        "one hour before, not five"
    );
    tazko::notifications::due_soon_all(&w.app.state, now + 4 * 3600 + 60).await;
    assert!(w.kinds(&w.dev).await.contains(&"due".to_string()));
    assert_eq!(
        req(
            &w.app,
            &w.dev,
            Method::PATCH,
            "/api/v1/me/settings",
            Some(json!({ "notify_due_app": false }))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST,
        "due reminders always show in the app"
    );
}

#[tokio::test]
async fn one_click_unsubscribe_turns_notification_emails_off() {
    let w = world(SMTP).await;
    w.mention_dev("@dev hi").await;
    w.run(tazko::session::now()).await;
    let url = w.emails().await[0].3.clone().unwrap();
    let path = url.trim_start_matches("http://localhost:8791");
    let page = w.app.get(path, None).await;
    assert_eq!(page.status(), StatusCode::OK);
    assert!(body_text(page).await.contains("<form method=\"post\""));
    let post = w
        .app
        .send(
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(post.status(), StatusCode::OK);
    let settings = req(&w.app, &w.dev, Method::GET, "/api/v1/me/settings", None)
        .await
        .1;
    assert_eq!(settings["email_frequency"], "never");
    let bad = w
        .app
        .send(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/email/unsubscribe?t=forged.value")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn previews_and_a_personal_test_email() {
    let w = world(SMTP).await;
    let (status, body) = req(
        &w.app,
        &w.dev,
        Method::GET,
        "/api/v1/me/email/previews",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let kinds: Vec<&str> = body["previews"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "invite",
            "mention",
            "assigned",
            "card_added",
            "reminder",
            "digest"
        ]
    );
    assert!(
        body["previews"][1]["html"]
            .as_str()
            .unwrap()
            .contains("View comment")
    );
    for _ in 0..3 {
        assert_eq!(
            req(&w.app, &w.dev, Method::POST, "/api/v1/me/email/test", None)
                .await
                .0,
            StatusCode::ACCEPTED
        );
    }
    assert_eq!(
        req(&w.app, &w.dev, Method::POST, "/api/v1/me/email/test", None)
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    let w2 = world("").await;
    assert_eq!(
        req(
            &w2.app,
            &w2.dev,
            Method::POST,
            "/api/v1/me/email/test",
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}
