mod common;

use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::routing::post;
use common::{TestApp, Who, call, person};
use serde_json::{Value, json};
use tazko::webhooks::worker;

const C: &str = "/api/v1/c/company-a";
const SLUG: &str = "company-a";

#[derive(Clone, Default)]
struct Receiver {
    got: Arc<Mutex<Vec<(HeaderMap, String)>>>,
    answer: Arc<AtomicU16>,
}

impl Receiver {
    async fn start() -> (Self, String) {
        let r = Receiver::default();
        r.answer.store(200, Ordering::SeqCst);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "http://127.0.0.1:{}/hook",
            listener.local_addr().unwrap().port()
        );
        let app = Router::new()
            .route("/hook", post(take))
            .with_state(r.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (r, url)
    }

    fn bodies(&self) -> Vec<Value> {
        self.got
            .lock()
            .unwrap()
            .iter()
            .map(|(_, b)| serde_json::from_str(b).unwrap())
            .collect()
    }
}

async fn take(State(r): State<Receiver>, headers: HeaderMap, body: String) -> StatusCode {
    r.got.lock().unwrap().push((headers, body));
    StatusCode::from_u16(r.answer.load(Ordering::SeqCst)).unwrap()
}

async fn req(app: &TestApp, who: &Who, method: Method, path: &str, body: Option<Value>) -> Value {
    let res = call(app, who, method.clone(), path, body).await;
    assert!(
        res.status().is_success(),
        "{method} {path} -> {}",
        res.status()
    );
    serde_json::from_str(&common::body_text(res).await).unwrap_or(Value::Null)
}

struct World {
    app: TestApp,
    boss: Who,
    dev: Who,
    project: i64,
    open: i64,
    secret: i64,
}

async fn world(env: &str) -> World {
    let app = TestApp::with_env(env);
    let a = app.company(SLUG, "active").await;
    let boss = person(&app, "boss@a.example").await;
    let dev = person(&app, "dev@a.example").await;
    app.member(a, boss.id, "owner").await;
    app.member(a, dev.id, "member").await;
    let project = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "Web", "owner_id": boss.id })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects/{project}/invites"),
        Some(json!({ "email": "dev@a.example", "role": "member" })),
    )
    .await;
    let open = board(&app, &boss, project, "Open").await;
    let secret = board(&app, &dev, project, "Secret").await;
    let dev_id = dev.id;
    app.state
        .dbs
        .tenant(SLUG)
        .unwrap()
        .write(move |c| {
            c.execute("UPDATE boards SET restricted = 1 WHERE id = ?1", [secret])?;
            c.execute(
                "INSERT INTO board_access (board_id, user_id) VALUES (?1, ?2)",
                [secret, dev_id],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    World {
        app,
        boss,
        dev,
        project,
        open,
        secret,
    }
}

async fn board(app: &TestApp, who: &Who, project: i64, name: &str) -> i64 {
    req(
        app,
        who,
        Method::POST,
        &format!("{C}/projects/{project}/boards"),
        Some(json!({ "name": name })),
    )
    .await["id"]
        .as_i64()
        .unwrap()
}

async fn card(app: &TestApp, who: &Who, board: i64, title: &str) -> i64 {
    let list =
        req(app, who, Method::GET, &format!("{C}/boards/{board}"), None).await["lists"][0]["id"]
            .as_i64()
            .unwrap();
    req(
        app,
        who,
        Method::POST,
        &format!("{C}/boards/{board}/cards"),
        Some(json!({ "list_id": list, "title": title })),
    )
    .await["cards"][0]["id"]
        .as_i64()
        .unwrap()
}

async fn hook(w: &World, url: &str, events: &[&str]) -> Value {
    let res = call(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/webhooks"),
        Some(json!({ "url": url, "events": events })),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    common::body_json(res).await
}

async fn tick(w: &World, at: i64) -> (usize, usize) {
    let db = w.app.state.dbs.tenant(SLUG).unwrap();
    let queued = worker::enqueue(&w.app.state, SLUG, &db).await.unwrap();
    let sent = worker::deliver_due(&w.app.state, SLUG, &db, at)
        .await
        .unwrap();
    (queued, sent)
}

fn verify(secret: &str, headers: &HeaderMap, body: &str) -> bool {
    let sig = headers["tazko-signature"].to_str().unwrap();
    let t = sig[2..sig.find(',').unwrap()].parse().unwrap();
    tazko::webhooks::signature(secret, t, body) == sig
}

#[tokio::test]
async fn owners_get_signed_events_only_from_boards_they_can_see() {
    let w = world("TAZKO_WEBHOOKS_ALLOW_LOCAL=1\n").await;
    let (rx, url) = Receiver::start().await;
    assert_eq!(
        call(
            &w.app,
            &w.dev,
            Method::POST,
            &format!("{C}/webhooks"),
            Some(json!({ "url": url, "events": ["card.created"] }))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN,
        "only company owners manage webhooks"
    );
    assert_eq!(
        call(&w.app, &w.dev, Method::GET, &format!("{C}/webhooks"), None)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );

    let made = hook(&w, &url, &["card.created", "comment.created"]).await;
    let secret = made["secret"].as_str().unwrap().to_string();
    assert!(secret.starts_with("whsec_"));
    let listed = req(&w.app, &w.boss, Method::GET, &format!("{C}/webhooks"), None).await;
    assert_eq!(listed["webhooks"].as_array().unwrap().len(), 1);
    assert!(
        listed["webhooks"][0].get("secret").is_none(),
        "the secret is shown once"
    );

    assert_eq!(tick(&w, tazko::session::now()).await, (0, 0));
    let seen = card(&w.app, &w.boss, w.open, "Ship the beta").await;
    card(&w.app, &w.dev, w.secret, "Hidden plan").await;
    req(
        &w.app,
        &w.dev,
        Method::POST,
        &format!("{C}/boards/{}/cards/{seen}/comments", w.open),
        Some(json!({ "body": "on it" })),
    )
    .await;
    assert_eq!(tick(&w, tazko::session::now()).await, (2, 2));

    let got = rx.got.lock().unwrap().clone();
    for (headers, body) in &got {
        assert!(verify(&secret, headers, body), "signature verifies");
        assert!(!body.contains("Hidden plan") && !body.contains("Secret"));
    }
    let bodies = rx.bodies();
    assert_eq!(bodies[0]["event"], "card.created");
    assert_eq!(bodies[0]["card"]["title"], "Ship the beta");
    assert_eq!(bodies[0]["board"]["name"], "Open");
    assert_eq!(bodies[0]["project"]["id"], w.project);
    assert_eq!(bodies[1]["event"], "comment.created");
    assert_eq!(bodies[1]["comment"]["body"], "on it");
    assert_eq!(got[1].0["tazko-event"], "comment.created");

    let id = made["id"].as_i64().unwrap();
    assert_eq!(
        call(
            &w.app,
            &w.boss,
            Method::POST,
            &format!("{C}/webhooks/{id}/test"),
            None
        )
        .await
        .status(),
        StatusCode::ACCEPTED
    );
    assert_eq!(tick(&w, tazko::session::now()).await, (0, 1));
    assert_eq!(rx.bodies()[2]["event"], "ping");

    let a = w
        .app
        .sql(|c| {
            c.query_row("SELECT id FROM orgs WHERE slug = 'company-a'", [], |r| {
                r.get::<_, i64>(0)
            })
        })
        .await;
    let boss = w.boss.id;
    w.app
        .sql(move |c| {
            c.execute(
                "UPDATE org_members SET role = 'member' WHERE org_id = ?1 AND user_id = ?2",
                [a, boss],
            )
        })
        .await;
    card(&w.app, &w.dev, w.open, "After the owner stepped down").await;
    assert_eq!(
        tick(&w, tazko::session::now()).await,
        (0, 0),
        "a webhook stops when its maker is no longer a company owner"
    );
}

#[tokio::test]
async fn failing_receivers_are_retried_logged_and_can_be_paused() {
    let w = world("TAZKO_WEBHOOKS_ALLOW_LOCAL=1\n").await;
    let (rx, url) = Receiver::start().await;
    rx.answer.store(500, Ordering::SeqCst);
    let id = hook(&w, &url, &["card.created"]).await["id"]
        .as_i64()
        .unwrap();
    tick(&w, tazko::session::now()).await;
    card(&w.app, &w.boss, w.open, "First").await;
    let t0 = tazko::session::now();
    assert_eq!(tick(&w, t0).await, (1, 1));
    let log = req(
        &w.app,
        &w.boss,
        Method::GET,
        &format!("{C}/webhooks/{id}/deliveries"),
        None,
    )
    .await;
    let d = &log["deliveries"][0];
    assert_eq!(d["status"], "pending");
    assert_eq!(d["attempts"], 1);
    assert_eq!(d["code"], 500);
    assert!(d["next_attempt_at"].as_i64().unwrap() >= t0 + 60);
    assert_eq!(tick(&w, t0).await, (0, 0), "waits before retrying");

    rx.answer.store(204, Ordering::SeqCst);
    assert_eq!(tick(&w, t0 + 61).await, (0, 1));
    let log = req(
        &w.app,
        &w.boss,
        Method::GET,
        &format!("{C}/webhooks/{id}/deliveries"),
        None,
    )
    .await;
    assert_eq!(log["deliveries"][0]["status"], "delivered");
    assert_eq!(log["deliveries"][0]["attempts"], 2);
    let listed = req(&w.app, &w.boss, Method::GET, &format!("{C}/webhooks"), None).await;
    assert_eq!(listed["webhooks"][0]["last"]["code"], 204);

    let first = log["deliveries"][0]["id"].as_i64().unwrap();
    req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/webhooks/{id}/deliveries/{first}/redeliver"),
        None,
    )
    .await;
    assert_eq!(tick(&w, tazko::session::now()).await, (0, 1));
    assert_eq!(rx.bodies().len(), 3);

    rx.answer.store(500, Ordering::SeqCst);
    card(&w.app, &w.boss, w.open, "Second").await;
    let t1 = tazko::session::now();
    tick(&w, t1).await;
    let mut at = t1;
    for _ in 0..6 {
        at += 50_000;
        tick(&w, at).await;
    }
    let log = req(
        &w.app,
        &w.boss,
        Method::GET,
        &format!("{C}/webhooks/{id}/deliveries"),
        None,
    )
    .await;
    assert_eq!(log["deliveries"][0]["status"], "failed");
    assert_eq!(log["deliveries"][0]["attempts"], 7);

    req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &format!("{C}/webhooks/{id}"),
        Some(json!({ "active": false })),
    )
    .await;
    card(&w.app, &w.boss, w.open, "While paused").await;
    assert_eq!(tick(&w, tazko::session::now()).await, (0, 0));
    req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &format!("{C}/webhooks/{id}"),
        Some(json!({ "active": true })),
    )
    .await;
    assert_eq!(
        tick(&w, tazko::session::now()).await,
        (0, 0),
        "events from while it was paused are not sent"
    );
    rx.answer.store(200, Ordering::SeqCst);
    card(&w.app, &w.boss, w.open, "After resuming").await;
    assert_eq!(tick(&w, tazko::session::now()).await, (1, 1));

    req(
        &w.app,
        &w.boss,
        Method::DELETE,
        &format!("{C}/webhooks/{id}"),
        None,
    )
    .await;
    assert_eq!(
        call(
            &w.app,
            &w.boss,
            Method::GET,
            &format!("{C}/webhooks/{id}/deliveries"),
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn webhooks_cannot_target_private_addresses_or_other_projects() {
    let w = world("").await;
    for url in [
        "http://127.0.0.1:9/hook",
        "https://127.0.0.1/hook",
        "https://localhost/hook",
        "https://10.0.0.1/hook",
        "https://192.168.1.10/hook",
        "https://169.254.169.254/latest",
        "https://[::1]/hook",
        "https://[fd00::1]/hook",
        "https://[::ffff:127.0.0.1]/hook",
        "https://user:pw@93.184.215.14/hook",
        "ftp://93.184.215.14/hook",
        "not a url",
    ] {
        assert_eq!(
            call(
                &w.app,
                &w.boss,
                Method::POST,
                &format!("{C}/webhooks"),
                Some(json!({ "url": url, "events": ["card.created"] }))
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST,
            "{url} is refused"
        );
    }

    let other: i64 = w
        .app
        .state
        .dbs
        .tenant(SLUG)
        .unwrap()
        .write(|c| {
            c.execute("INSERT INTO projects (name) VALUES ('Not mine')", [])?;
            Ok(c.last_insert_rowid())
        })
        .await
        .unwrap();
    assert_eq!(
        call(
            &w.app,
            &w.boss,
            Method::POST,
            &format!("{C}/webhooks"),
            Some(json!({ "url": "https://93.184.215.14/hook", "events": ["card.created"], "projects": [other] }))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST,
            "only projects the owner belongs to"
    );
}
