mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, Who, call, person};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";
const WEEK: i64 = 7 * 86_400;

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
    viewer: Who,
    board: i64,
    list: i64,
}

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let boss = person(&app, "boss@a.example").await;
    let viewer = person(&app, "viewer@a.example").await;
    app.member(a, boss.id, "owner").await;
    app.member(a, viewer.id, "member").await;
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
        Some(json!({ "email": "viewer@a.example", "role": "observer" })),
    )
    .await;
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
    World {
        app,
        boss,
        viewer,
        board,
        list,
    }
}

async fn card(w: &World, title: &str) -> i64 {
    req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/boards/{}/cards", w.board),
        Some(json!({ "list_id": w.list, "title": title })),
    )
    .await["cards"][0]["id"]
        .as_i64()
        .unwrap()
}

async fn detail(w: &World, card: i64) -> Value {
    req(
        &w.app,
        &w.boss,
        Method::GET,
        &format!("{C}/boards/{}/cards/{card}", w.board),
        None,
    )
    .await
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[tokio::test]
async fn completing_a_weekly_card_moves_it_a_week_and_unticks_until_undone() {
    let w = world().await;
    let id = card(&w, "Weekly report").await;
    let path = format!("{C}/boards/{}/cards/{id}", w.board);
    let due = now() + 2 * 86_400;
    req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &path,
        Some(json!({ "due_at": due, "start_at": due - 86_400, "recurrence": "weekly" })),
    )
    .await;
    let k = req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{path}/checklists"),
        Some(json!({ "title": "Steps" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let mut items = Vec::new();
    for text in ["Collect numbers", "Send"] {
        items.push(
            req(
                &w.app,
                &w.boss,
                Method::POST,
                &format!("{C}/boards/{}/checklists/{k}/items", w.board),
                Some(json!({ "text": text })),
            )
            .await["id"]
                .as_i64()
                .unwrap(),
        );
    }
    req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &format!("{C}/boards/{}/items/{}", w.board, items[0]),
        Some(json!({ "done": true })),
    )
    .await;

    let res = req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &path,
        Some(json!({ "done": true, "tz_offset": 420 })),
    )
    .await;
    assert_eq!(res["repeated"]["from"], due);
    assert_eq!(res["repeated"]["to"], due + WEEK);
    let after = detail(&w, id).await;
    assert_eq!(after["done"], false, "the card reopens");
    assert_eq!(after["due_at"], due + WEEK);
    assert_eq!(
        after["start_at"],
        due - 86_400 + WEEK,
        "the start date moves with it"
    );
    assert_eq!(after["recurrence"], "weekly");
    assert!(
        after["checklists"][0]["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["done"] == false),
        "the checklist is unticked"
    );
    let board = req(
        &w.app,
        &w.boss,
        Method::GET,
        &format!("{C}/boards/{}", w.board),
        None,
    )
    .await;
    assert_eq!(board["lists"][0]["cards"][0]["recurrence"], "weekly");
    let kinds: Vec<String> = after["activity"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["type"].as_str().unwrap().to_string())
        .collect();
    assert!(kinds.contains(&"card_repeated".to_string()), "{kinds:?}");

    let undo = format!("{path}/repeat/undo");
    assert_eq!(
        call(&w.app, &w.viewer, Method::POST, &undo, None)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    req(&w.app, &w.boss, Method::POST, &undo, None).await;
    let back = detail(&w, id).await;
    assert_eq!(back["due_at"], due);
    assert_eq!(back["start_at"], due - 86_400);
    let ticks: Vec<bool> = back["checklists"][0]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["done"].as_bool().unwrap())
        .collect();
    assert_eq!(ticks, [true, false], "undo restores the ticked items");
    assert_eq!(
        call(&w.app, &w.boss, Method::POST, &undo, None)
            .await
            .status(),
        StatusCode::CONFLICT,
        "only the last completion can be undone"
    );
}

#[tokio::test]
async fn a_late_card_moves_past_today_and_plain_cards_just_complete() {
    let w = world().await;
    let late = card(&w, "Daily standup notes").await;
    let path = format!("{C}/boards/{}/cards/{late}", w.board);
    let due = now() - 10 * 86_400;
    req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &path,
        Some(json!({ "due_at": due, "recurrence": "daily" })),
    )
    .await;
    let res = req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &path,
        Some(json!({ "done": true })),
    )
    .await;
    let next = res["repeated"]["to"].as_i64().unwrap();
    assert!(
        next > now() && next <= now() + 86_400,
        "next due is the next day after now"
    );
    assert_eq!((next - due) % 86_400, 0, "keeps the time of day");

    let plain = card(&w, "One-off").await;
    let res = call(
        &w.app,
        &w.boss,
        Method::PATCH,
        &format!("{C}/boards/{}/cards/{plain}", w.board),
        Some(json!({ "done": true })),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert_eq!(detail(&w, plain).await["done"], true);

    let undated = card(&w, "No date").await;
    let upath = format!("{C}/boards/{}/cards/{undated}", w.board);
    req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &upath,
        Some(json!({ "recurrence": "monthly" })),
    )
    .await;
    assert_eq!(
        call(
            &w.app,
            &w.boss,
            Method::PATCH,
            &upath,
            Some(json!({ "done": true }))
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        detail(&w, undated).await["done"],
        true,
        "without a due date there is nothing to repeat"
    );

    for bad in [
        json!({ "recurrence": "yearly" }),
        json!({ "recurrence": 7 }),
        json!({ "tz_offset": 5000, "done": false }),
    ] {
        assert_eq!(
            call(&w.app, &w.boss, Method::PATCH, &upath, Some(bad.clone()))
                .await
                .status(),
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
    req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &upath,
        Some(json!({ "recurrence": null })),
    )
    .await;
    assert_eq!(detail(&w, undated).await["recurrence"], Value::Null);
}
