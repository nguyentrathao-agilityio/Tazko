mod common;

use std::time::{Duration, Instant};

use axum::http::Method;
use common::{TestApp, Who, body_text, call, person};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

const C: &str = "/api/v1/c/company-a";

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct World {
    app: TestApp,
    addr: String,
    owner: Who,
    member: Who,
    observer: Who,
    outsider: Who,
    project: i64,
    board: i64,
    list: i64,
}

async fn req(app: &TestApp, who: &Who, method: Method, path: &str, body: Option<Value>) -> Value {
    let res = call(app, who, method, path, body).await;
    serde_json::from_str(&body_text(res).await).unwrap_or(Value::Null)
}

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let boss = person(&app, "boss@a.example").await;
    let owner = person(&app, "lead@a.example").await;
    let member = person(&app, "dev@a.example").await;
    let observer = person(&app, "viewer@a.example").await;
    let outsider = person(&app, "plain@a.example").await;
    app.member(a, boss.id, "owner").await;
    for w in [&owner, &member, &observer, &outsider] {
        app.member(a, w.id, "member").await;
    }
    let project = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "P", "owner_id": owner.id })),
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
            &owner,
            Method::POST,
            &format!("{C}/projects/{project}/invites"),
            Some(json!({ "email": e, "role": role })),
        )
        .await;
    }
    let board = req(
        &app,
        &owner,
        Method::POST,
        &format!("{C}/projects/{project}/boards"),
        Some(json!({ "name": "B" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let list = req(
        &app,
        &owner,
        Method::GET,
        &format!("{C}/boards/{board}"),
        None,
    )
    .await["lists"][0]["id"]
        .as_i64()
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let router = app.router();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    World {
        app,
        addr,
        owner,
        member,
        observer,
        outsider,
        project,
        board,
        list,
    }
}

impl World {
    async fn connect(&self, who: &Who, after: Option<i64>, origin: &str) -> Result<Ws, u16> {
        let q = after.map(|a| format!("?after={a}")).unwrap_or_default();
        let url = format!("ws://{}{C}/boards/{}/live{q}", self.addr, self.board);
        let mut request = url.into_client_request().unwrap();
        request
            .headers_mut()
            .insert("cookie", who.cookie.parse().unwrap());
        request
            .headers_mut()
            .insert("origin", origin.parse().unwrap());
        match tokio_tungstenite::connect_async(request).await {
            Ok((ws, _)) => Ok(ws),
            Err(tokio_tungstenite::tungstenite::Error::Http(res)) => Err(res.status().as_u16()),
            Err(e) => panic!("{e}"),
        }
    }

    fn origin(&self) -> String {
        format!("http://{}", self.addr)
    }

    async fn add_card(&self, who: &Who, title: &str) -> i64 {
        req(
            &self.app,
            who,
            Method::POST,
            &format!("{C}/boards/{}/cards", self.board),
            Some(json!({ "list_id": self.list, "title": title })),
        )
        .await["cards"][0]["id"]
            .as_i64()
            .unwrap()
    }
}

async fn next(ws: &mut Ws, kind: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let msg = tokio::time::timeout(left, ws.next())
            .await
            .unwrap_or_else(|_| panic!("no {kind} message in time"));
        match msg {
            Some(Ok(Message::Text(t))) => {
                let v: Value = serde_json::from_str(t.as_str()).unwrap();
                if v["type"] == kind {
                    return v;
                }
            }
            Some(Ok(Message::Close(_))) | None => return json!({ "type": "socket-closed" }),
            Some(Ok(_)) => {}
            Some(Err(e)) => panic!("{e}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_change_reaches_everyone_on_the_board_within_a_second() {
    let w = world().await;
    let mut observer = w.connect(&w.observer, None, &w.origin()).await.unwrap();
    let hello = next(&mut observer, "hello").await;
    assert!(hello["last"].as_i64().unwrap() > 0);
    let mut member = w.connect(&w.member, None, &w.origin()).await.unwrap();
    next(&mut member, "hello").await;
    let mut names: Vec<String> = Vec::new();
    for _ in 0..3 {
        let presence = next(&mut observer, "presence").await;
        names = presence["presence"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["email"].as_str().unwrap().to_string())
            .collect();
        if names.len() == 2 {
            break;
        }
    }
    assert_eq!(names, vec!["dev@a.example", "viewer@a.example"]);

    let started = Instant::now();
    let card = w.add_card(&w.member, "Hello").await;
    let got = next(&mut observer, "events").await;
    let took = started.elapsed();
    assert!(took < Duration::from_secs(1), "took {took:?}");
    assert_eq!(got["events"][0]["type"], "card_created");
    assert_eq!(got["events"][0]["card"], card);

    req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{C}/boards/{}/cards/{card}/move", w.board),
        Some(json!({ "list_id": w.list, "after": null })),
    )
    .await;
    let moved = next(&mut observer, "events").await;
    assert_eq!(moved["events"][0]["type"], "card_moved");

    observer
        .send(Message::Text("{\"type\":\"card_created\"}".into()))
        .await
        .unwrap();
    observer.send(Message::Text("ping".into())).await.unwrap();
    next(&mut observer, "pong").await;
    let count: i64 = w
        .app
        .state
        .dbs
        .tenant("company-a")
        .unwrap()
        .read(|c| Ok(c.query_row("SELECT count(*) FROM cards", [], |r| r.get(0))?))
        .await
        .unwrap();
    assert_eq!(count, 1, "messages sent over the socket change nothing");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reconnect_with_the_last_seen_event_gets_what_was_missed() {
    let w = world().await;
    let mut ws = w.connect(&w.member, None, &w.origin()).await.unwrap();
    let last = next(&mut ws, "hello").await["last"].as_i64().unwrap();
    drop(ws);
    w.add_card(&w.owner, "Missed one").await;
    w.add_card(&w.owner, "Missed two").await;

    let mut again = w.connect(&w.member, Some(last), &w.origin()).await.unwrap();
    next(&mut again, "hello").await;
    let caught = next(&mut again, "events").await;
    assert_eq!(caught["catch_up"], true);
    assert_eq!(caught["events"].as_array().unwrap().len(), 2);

    for _ in 0..(tazko::realtime::CATCH_UP_LIMIT + 1) {
        w.app
            .state
            .dbs
            .tenant("company-a")
            .unwrap()
            .write({
                let board = w.board;
                let project = w.project;
                move |c| {
                    c.execute(
                        "INSERT INTO events (type, actor_id, project_id, board_id) VALUES ('board_updated', 1, ?1, ?2)",
                        rusqlite::params![project, board],
                    )?;
                    Ok(())
                }
            })
            .await
            .unwrap();
    }
    let mut far = w.connect(&w.member, Some(last), &w.origin()).await.unwrap();
    next(&mut far, "hello").await;
    assert_eq!(next(&mut far, "reset").await["type"], "reset");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_live_connection_follows_the_same_access_rules() {
    let w = world().await;
    assert_eq!(
        w.connect(&w.outsider, None, &w.origin()).await.err(),
        Some(404)
    );
    assert_eq!(
        w.connect(&w.member, None, "https://evil.example")
            .await
            .err(),
        Some(403)
    );

    req(
        &w.app,
        &w.owner,
        Method::PUT,
        &format!("{C}/boards/{}/access", w.board),
        Some(json!({ "restricted": true, "people": [] })),
    )
    .await;
    assert_eq!(
        w.connect(&w.member, None, &w.origin()).await.err(),
        Some(404)
    );
    req(
        &w.app,
        &w.owner,
        Method::PUT,
        &format!("{C}/boards/{}/access", w.board),
        Some(json!({ "restricted": false })),
    )
    .await;

    let mut ws = w.connect(&w.member, None, &w.origin()).await.unwrap();
    next(&mut ws, "hello").await;
    req(
        &w.app,
        &w.owner,
        Method::DELETE,
        &format!("{C}/projects/{}/members/{}", w.project, w.member.id),
        None,
    )
    .await;
    let closed = next(&mut ws, "closed").await;
    assert_eq!(closed["reason"], "no access");
    assert_eq!(next(&mut ws, "never").await["type"], "socket-closed");
}
