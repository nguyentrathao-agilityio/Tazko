mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, Who, call, person};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";
const DUE: i64 = 1_900_000_000;

async fn req(app: &TestApp, who: &Who, method: Method, path: &str, body: Option<Value>) -> Value {
    let res = call(app, who, method.clone(), path, body).await;
    assert!(
        res.status().is_success(),
        "{method} {path} -> {}",
        res.status()
    );
    serde_json::from_str(&common::body_text(res).await).unwrap_or(Value::Null)
}

async fn status(app: &TestApp, who: &Who, method: Method, path: &str, body: Value) -> StatusCode {
    call(app, who, method, path, Some(body)).await.status()
}

struct World {
    app: TestApp,
    boss: Who,
    dev: Who,
    viewer: Who,
    outsider: Who,
    board: i64,
    card: i64,
    item: i64,
}

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let boss = person(&app, "boss@a.example").await;
    let dev = person(&app, "dev@a.example").await;
    let viewer = person(&app, "viewer@a.example").await;
    let outsider = person(&app, "outsider@a.example").await;
    app.member(a, boss.id, "owner").await;
    for w in [&dev, &viewer, &outsider] {
        app.member(a, w.id, "member").await;
    }
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
    for (email, role) in [
        ("dev@a.example", "member"),
        ("viewer@a.example", "observer"),
    ] {
        req(
            &app,
            &boss,
            Method::POST,
            &format!("{C}/projects/{project}/invites"),
            Some(json!({ "email": email, "role": role })),
        )
        .await;
    }
    let board = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects/{project}/boards"),
        Some(json!({ "name": "Sprint" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let list = req(
        &app,
        &boss,
        Method::GET,
        &format!("{C}/boards/{board}"),
        None,
    )
    .await["lists"][0]["id"]
        .as_i64()
        .unwrap();
    let card = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/boards/{board}/cards"),
        Some(json!({ "list_id": list, "title": "Launch" })),
    )
    .await["cards"][0]["id"]
        .as_i64()
        .unwrap();
    let k = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/boards/{board}/cards/{card}/checklists"),
        Some(json!({ "title": "Steps" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let item = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/boards/{board}/checklists/{k}/items"),
        Some(json!({ "text": "Write release notes" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    World {
        app,
        boss,
        dev,
        viewer,
        outsider,
        board,
        card,
        item,
    }
}

fn item_path(w: &World) -> String {
    format!("{C}/boards/{}/items/{}", w.board, w.item)
}

async fn shown(w: &World) -> Value {
    req(
        &w.app,
        &w.boss,
        Method::GET,
        &format!("{C}/boards/{}/cards/{}", w.board, w.card),
        None,
    )
    .await["checklists"][0]["items"][0]
        .clone()
}

#[tokio::test]
async fn an_assigned_item_shows_in_my_cards_with_its_due_date_and_notifies() {
    let w = world().await;
    let fresh = shown(&w).await;
    assert_eq!(fresh["assignee"], Value::Null);
    assert_eq!(fresh["due_at"], Value::Null);

    req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &item_path(&w),
        Some(json!({ "assignee": w.dev.id, "due_at": DUE })),
    )
    .await;
    let item = shown(&w).await;
    assert_eq!(item["assignee"], w.dev.id);
    assert_eq!(item["due_at"], DUE);

    let mine = req(
        &w.app,
        &w.dev,
        Method::GET,
        "/api/v1/me/cards?company=company-a",
        None,
    )
    .await;
    let items = mine["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["text"], "Write release notes");
    assert_eq!(items[0]["due_at"], DUE);
    assert_eq!(items[0]["card"]["title"], "Launch");
    assert_eq!(items[0]["company"]["slug"], "company-a");
    assert_eq!(items[0]["can_edit"], true);
    assert_eq!(
        mine["cards"].as_array().unwrap().len(),
        0,
        "the card itself is not assigned"
    );

    let inbox = req(
        &w.app,
        &w.dev,
        Method::GET,
        &format!("{C}/notifications"),
        None,
    )
    .await;
    let notes = inbox["notifications"].as_array().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["kind"], "assigned");
    assert_eq!(notes[0]["item"], "Write release notes");

    req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &item_path(&w),
        Some(json!({ "due_at": DUE + 86_400 })),
    )
    .await;
    let inbox = req(
        &w.app,
        &w.dev,
        Method::GET,
        &format!("{C}/notifications"),
        None,
    )
    .await;
    assert_eq!(
        inbox["notifications"].as_array().unwrap().len(),
        1,
        "changing the date does not notify again"
    );

    let activity = req(
        &w.app,
        &w.boss,
        Method::GET,
        &format!("{C}/boards/{}/cards/{}", w.board, w.card),
        None,
    )
    .await["activity"]
        .clone();
    let kinds: Vec<&str> = activity
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["type"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"item_dated"), "{kinds:?}");

    req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &item_path(&w),
        Some(json!({ "assignee": null, "due_at": null })),
    )
    .await;
    let mine = req(
        &w.app,
        &w.dev,
        Method::GET,
        "/api/v1/me/cards?company=company-a",
        None,
    )
    .await;
    assert_eq!(mine["items"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn only_people_who_can_work_on_the_board_are_assigned() {
    let w = world().await;
    let path = item_path(&w);
    for who in [w.viewer.id, w.outsider.id, 999_999] {
        assert_eq!(
            status(
                &w.app,
                &w.boss,
                Method::PATCH,
                &path,
                json!({ "assignee": who })
            )
            .await,
            StatusCode::BAD_REQUEST,
            "user {who}"
        );
    }
    assert_eq!(
        status(
            &w.app,
            &w.viewer,
            Method::PATCH,
            &path,
            json!({ "assignee": w.viewer.id })
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        status(
            &w.app,
            &w.boss,
            Method::PATCH,
            &path,
            json!({ "due_at": "tomorrow" })
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        status(
            &w.app,
            &w.boss,
            Method::PATCH,
            &path,
            json!({ "position": 1 })
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        status(&w.app, &w.boss, Method::PATCH, &path, json!({})).await,
        StatusCode::BAD_REQUEST
    );

    req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &path,
        Some(json!({ "assignee": w.boss.id })),
    )
    .await;
    let inbox = req(
        &w.app,
        &w.boss,
        Method::GET,
        &format!("{C}/notifications"),
        None,
    )
    .await;
    assert_eq!(
        inbox["notifications"].as_array().unwrap().len(),
        0,
        "assigning yourself sends nothing"
    );

    req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &path,
        Some(json!({ "assignee": w.dev.id })),
    )
    .await;
    req(
        &w.app,
        &w.boss,
        Method::PUT,
        &format!("{C}/boards/{}/access", w.board),
        Some(json!({ "restricted": true, "people": [] })),
    )
    .await;
    let mine = req(
        &w.app,
        &w.dev,
        Method::GET,
        "/api/v1/me/cards?company=company-a",
        None,
    )
    .await;
    assert_eq!(
        mine["items"].as_array().unwrap().len(),
        0,
        "a restricted board hides the item"
    );
}
