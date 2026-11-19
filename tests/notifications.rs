mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use common::{TestApp, Who, body_text, call, person};
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

const C: &str = "/api/v1/c/company-a";

struct World {
    app: TestApp,
    boss: Who,
    lead: Who,
    dev: Who,
    viewer: Who,
    plain: Who,
    project: i64,
    board: i64,
    hidden: i64,
    card: i64,
    hidden_card: i64,
}

async fn req(app: &TestApp, who: &Who, method: Method, path: &str, body: Option<Value>) -> Value {
    let res = call(app, who, method, path, body).await;
    serde_json::from_str(&body_text(res).await).unwrap_or(Value::Null)
}

async fn new_card(app: &TestApp, who: &Who, board: i64, title: &str) -> i64 {
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

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let boss = person(&app, "boss@a.example").await;
    let lead = person(&app, "lead@a.example").await;
    let dev = person(&app, "dev@a.example").await;
    let viewer = person(&app, "viewer@a.example").await;
    let plain = person(&app, "plain@a.example").await;
    app.member(a, boss.id, "owner").await;
    for w in [&lead, &dev, &viewer, &plain] {
        app.member(a, w.id, "member").await;
    }
    let project = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "P", "owner_id": lead.id })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    for (e, role) in [
        ("dev@a.example", "member"),
        ("viewer@a.example", "observer"),
    ] {
        req(
            &app,
            &lead,
            Method::POST,
            &format!("{C}/projects/{project}/invites"),
            Some(json!({ "email": e, "role": role })),
        )
        .await;
    }
    let mut boards = Vec::new();
    for name in ["B", "Hidden"] {
        boards.push(
            req(
                &app,
                &lead,
                Method::POST,
                &format!("{C}/projects/{project}/boards"),
                Some(json!({ "name": name })),
            )
            .await["id"]
                .as_i64()
                .unwrap(),
        );
    }
    let card = new_card(&app, &lead, boards[0], "Hero").await;
    let hidden_card = new_card(&app, &lead, boards[1], "Secret plan").await;
    req(
        &app,
        &lead,
        Method::PUT,
        &format!("{C}/boards/{}/access", boards[1]),
        Some(json!({ "restricted": true, "people": [dev.id] })),
    )
    .await;
    World {
        app,
        boss,
        lead,
        dev,
        viewer,
        plain,
        project,
        board: boards[0],
        hidden: boards[1],
        card,
        hidden_card,
    }
}

impl World {
    async fn comment(&self, who: &Who, board: i64, card: i64, body: &str) -> i64 {
        req(
            &self.app,
            who,
            Method::POST,
            &format!("{C}/boards/{board}/cards/{card}/comments"),
            Some(json!({ "body": body })),
        )
        .await["id"]
            .as_i64()
            .unwrap()
    }

    async fn inbox(&self, who: &Who) -> Value {
        req(
            &self.app,
            who,
            Method::GET,
            &format!("{C}/notifications"),
            None,
        )
        .await
    }

    async fn kinds(&self, who: &Who) -> Vec<String> {
        self.inbox(who).await["notifications"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| {
                format!(
                    "{}:{}",
                    n["kind"].as_str().unwrap(),
                    n["card"]["title"].as_str().unwrap()
                )
            })
            .collect()
    }

    async fn stored(&self) -> i64 {
        self.app
            .state
            .dbs
            .tenant("company-a")
            .unwrap()
            .read(|c| Ok(c.query_row("SELECT count(*) FROM notifications", [], |r| r.get(0))?))
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn mentioning_someone_without_access_creates_no_notification() {
    let w = world().await;
    w.comment(
        &w.dev,
        w.board,
        w.card,
        "@plain @boss @dev can you look? cc @lead @viewer",
    )
    .await;
    assert_eq!(w.kinds(&w.lead).await, ["mention:Hero"]);
    assert_eq!(w.kinds(&w.viewer).await, ["mention:Hero"]);
    assert!(
        w.kinds(&w.plain).await.is_empty(),
        "company member outside the project"
    );
    assert!(
        w.kinds(&w.boss).await.is_empty(),
        "company owner who has not joined the project"
    );
    assert!(
        w.kinds(&w.dev).await.is_empty(),
        "no notification for mentioning yourself"
    );
    assert_eq!(w.stored().await, 2);

    w.comment(
        &w.dev,
        w.hidden,
        w.hidden_card,
        "@viewer @lead @plain secret",
    )
    .await;
    assert_eq!(
        w.kinds(&w.viewer).await,
        ["mention:Hero"],
        "restricted board the observer is not on"
    );
    assert_eq!(
        w.kinds(&w.lead).await,
        ["mention:Secret plan", "mention:Hero"],
        "the owner who restricted the board stays on it"
    );
    assert_eq!(w.stored().await, 3);

    let first = w.inbox(&w.viewer).await["notifications"][0].clone();
    assert_eq!(first["actor"]["id"], w.dev.id);
    assert_eq!(first["board"]["id"], w.board);
    assert_eq!(w.inbox(&w.viewer).await["unread"], 1);
    req(
        &w.app,
        &w.lead,
        Method::DELETE,
        &format!("{C}/projects/{}/members/{}", w.project, w.viewer.id),
        None,
    )
    .await;
    let after = w.inbox(&w.viewer).await;
    assert_eq!(
        after["notifications"].as_array().unwrap().len(),
        0,
        "removed members stop seeing the card title"
    );
    assert_eq!(after["unread"], 0);
}

#[tokio::test]
async fn editing_a_comment_notifies_only_newly_mentioned_people() {
    let w = world().await;
    let id = w.comment(&w.dev, w.board, w.card, "hello @lead").await;
    let path = format!("{C}/boards/{}/comments/{id}", w.board);
    req(
        &w.app,
        &w.dev,
        Method::PATCH,
        &path,
        Some(json!({ "body": "hello @lead, again" })),
    )
    .await;
    assert_eq!(w.kinds(&w.lead).await.len(), 1);
    req(
        &w.app,
        &w.dev,
        Method::PATCH,
        &path,
        Some(json!({ "body": "hello @lead and @viewer" })),
    )
    .await;
    assert_eq!(w.kinds(&w.lead).await.len(), 1);
    assert_eq!(w.kinds(&w.viewer).await, ["mention:Hero"]);
}

#[tokio::test]
async fn assignment_and_watched_boards_notify_other_people() {
    let w = world().await;
    let member = |user: i64| format!("{C}/boards/{}/cards/{}/members/{user}", w.board, w.card);
    req(
        &w.app,
        &w.lead,
        Method::PUT,
        &member(w.dev.id),
        Some(json!({ "on": true })),
    )
    .await;
    req(
        &w.app,
        &w.lead,
        Method::PUT,
        &member(w.lead.id),
        Some(json!({ "on": true })),
    )
    .await;
    assert_eq!(w.kinds(&w.dev).await, ["assigned:Hero"]);
    assert!(w.kinds(&w.lead).await.is_empty());

    let watch = format!("{C}/boards/{}/watch", w.board);
    assert_eq!(
        req(
            &w.app,
            &w.viewer,
            Method::PUT,
            &watch,
            Some(json!({ "on": true }))
        )
        .await["watching"],
        true
    );
    assert_eq!(
        req(
            &w.app,
            &w.viewer,
            Method::GET,
            &format!("{C}/boards/{}", w.board),
            None
        )
        .await["watching"],
        true
    );
    req(
        &w.app,
        &w.dev,
        Method::PUT,
        &watch,
        Some(json!({ "on": true })),
    )
    .await;
    new_card(&w.app, &w.dev, w.board, "Pricing page").await;
    assert_eq!(w.kinds(&w.viewer).await, ["card_added:Pricing page"]);
    assert_eq!(
        w.kinds(&w.dev).await,
        ["assigned:Hero"],
        "not for your own card"
    );
    assert_eq!(
        call(
            &w.app,
            &w.plain,
            Method::PUT,
            &watch,
            Some(json!({ "on": true }))
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    req(
        &w.app,
        &w.viewer,
        Method::PUT,
        &watch,
        Some(json!({ "on": false })),
    )
    .await;
    new_card(&w.app, &w.dev, w.board, "Footer").await;
    assert_eq!(w.kinds(&w.viewer).await.len(), 1);
}

#[tokio::test]
async fn due_soon_alerts_are_sent_once_to_card_members_who_can_see_the_card() {
    let w = world().await;
    let now = tazko::session::now();
    let card = |id: i64, board: i64| format!("{C}/boards/{board}/cards/{id}");
    for (who, id, board) in [
        (&w.lead, w.card, w.board),
        (&w.dev, w.hidden_card, w.hidden),
    ] {
        req(
            &w.app,
            who,
            Method::PATCH,
            &card(id, board),
            Some(json!({ "due_at": now + 3600 })),
        )
        .await;
        req(
            &w.app,
            who,
            Method::PUT,
            &format!("{}/members/{}", card(id, board), w.dev.id),
            Some(json!({ "on": true })),
        )
        .await;
    }
    req(
        &w.app,
        &w.lead,
        Method::PUT,
        &format!("{}/members/{}", card(w.card, w.board), w.lead.id),
        Some(json!({ "on": true })),
    )
    .await;
    let later = new_card(&w.app, &w.lead, w.board, "Later").await;
    req(
        &w.app,
        &w.lead,
        Method::PATCH,
        &card(later, w.board),
        Some(json!({ "due_at": now + 3 * 86_400 })),
    )
    .await;
    req(
        &w.app,
        &w.lead,
        Method::PUT,
        &format!("{}/members/{}", card(later, w.board), w.dev.id),
        Some(json!({ "on": true })),
    )
    .await;

    assert_eq!(
        tazko::notifications::due_soon_all(&w.app.state, now).await,
        3
    );
    assert_eq!(
        tazko::notifications::due_soon_all(&w.app.state, now + 60).await,
        0,
        "a second run (or a restart) sends nothing new"
    );
    let dev = w.kinds(&w.dev).await;
    assert_eq!(dev.iter().filter(|k| k.starts_with("due:")).count(), 2);
    assert!(dev.contains(&"due:Secret plan".to_string()));
    assert_eq!(w.kinds(&w.lead).await, ["due:Hero"]);

    let (hidden, hidden_card) = (w.hidden, w.hidden_card);
    w.app
        .state
        .dbs
        .tenant("company-a")
        .unwrap()
        .write(move |c| {
            c.execute("DELETE FROM board_access WHERE board_id = ?1", [hidden])?;
            c.execute(
                "UPDATE cards SET due_at = ?2 WHERE id = ?1",
                [hidden_card, now + 7200],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(
        tazko::notifications::due_soon_all(&w.app.state, now + 120).await,
        0,
        "no alert once the board is hidden from them"
    );
    assert!(
        !w.kinds(&w.dev)
            .await
            .contains(&"due:Secret plan".to_string())
    );

    req(
        &w.app,
        &w.lead,
        Method::PATCH,
        &card(w.card, w.board),
        Some(json!({ "due_at": now + 5400 })),
    )
    .await;
    assert_eq!(
        tazko::notifications::due_soon_all(&w.app.state, now + 180).await,
        2,
        "a new due date alerts again"
    );
    req(
        &w.app,
        &w.lead,
        Method::PATCH,
        &card(w.card, w.board),
        Some(json!({ "due_at": now + 6000, "done": true })),
    )
    .await;
    assert_eq!(
        tazko::notifications::due_soon_all(&w.app.state, now + 240).await,
        0,
        "not for finished cards"
    );
}

#[tokio::test]
async fn people_mark_only_their_own_notifications_read() {
    let w = world().await;
    w.comment(&w.dev, w.board, w.card, "@lead one").await;
    w.comment(&w.dev, w.board, w.card, "@lead two").await;
    let inbox = w.inbox(&w.lead).await;
    assert_eq!(inbox["unread"], 2);
    let id = inbox["notifications"][0]["id"].as_i64().unwrap();
    let one = format!("{C}/notifications/{id}/read");
    assert_eq!(
        call(&w.app, &w.dev, Method::POST, &one, None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&w.app, &w.lead, Method::POST, &one, None)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    let inbox = w.inbox(&w.lead).await;
    assert_eq!(
        (
            inbox["unread"].as_i64(),
            inbox["notifications"][0]["read"].as_bool()
        ),
        (Some(1), Some(true))
    );
    let unread = req(
        &w.app,
        &w.lead,
        Method::GET,
        &format!("{C}/notifications?unread=1"),
        None,
    )
    .await;
    assert_eq!(unread["notifications"].as_array().unwrap().len(), 1);
    call(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{C}/notifications/read"),
        None,
    )
    .await;
    assert_eq!(w.inbox(&w.lead).await["unread"], 0);
    assert_eq!(
        w.app
            .get("/api/v1/c/company-b/notifications", Some(&w.lead.cookie))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_bell_count_arrives_over_the_live_connection() {
    let w = world().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let router = w.app.router();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let connect = |origin: &str| {
        let mut r = format!("ws://{addr}{C}/notifications/live")
            .into_client_request()
            .unwrap();
        r.headers_mut()
            .insert("cookie", w.lead.cookie.parse().unwrap());
        r.headers_mut().insert("origin", origin.parse().unwrap());
        tokio_tungstenite::connect_async(r)
    };
    assert!(connect("https://evil.example").await.is_err());
    let (mut ws, _) = connect(&format!("http://{addr}")).await.unwrap();
    let mut next = async || -> Value {
        loop {
            let m = tokio::time::timeout(Duration::from_secs(3), ws.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            if let Message::Text(t) = m {
                let v: Value = serde_json::from_str(t.as_str()).unwrap();
                if v["type"] == "unread" {
                    return v;
                }
            }
        }
    };
    assert_eq!(next().await["count"], 0);
    w.comment(&w.dev, w.board, w.card, "@lead look").await;
    assert_eq!(next().await["count"], 1);
    call(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{C}/notifications/read"),
        None,
    )
    .await;
    assert_eq!(next().await["count"], 0);
}
