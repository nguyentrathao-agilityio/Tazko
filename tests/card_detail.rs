mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, Who, body_text, call, person};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";

struct World {
    app: TestApp,
    owner: Who,
    member: Who,
    observer: Who,
    outsider: Who,
    board: i64,
    card: i64,
}

async fn req(
    app: &TestApp,
    who: &Who,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let res = call(app, who, method, path, body).await;
    let status = res.status();
    let text = body_text(res).await;
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
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
    let (_, v) = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "P", "owner_id": owner.id })),
    )
    .await;
    let project = v["id"].as_i64().unwrap();
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
    let (_, v) = req(
        &app,
        &owner,
        Method::POST,
        &format!("{C}/projects/{project}/boards"),
        Some(json!({ "name": "B" })),
    )
    .await;
    let board = v["id"].as_i64().unwrap();
    let (_, v) = req(
        &app,
        &owner,
        Method::GET,
        &format!("{C}/boards/{board}"),
        None,
    )
    .await;
    let list = v["lists"][0]["id"].as_i64().unwrap();
    let (_, v) = req(
        &app,
        &member,
        Method::POST,
        &format!("{C}/boards/{board}/cards"),
        Some(json!({ "list_id": list, "title": "Card" })),
    )
    .await;
    let card = v["cards"][0]["id"].as_i64().unwrap();
    World {
        app,
        owner,
        member,
        observer,
        outsider,
        board,
        card,
    }
}

impl World {
    fn card_path(&self) -> String {
        format!("{C}/boards/{}/cards/{}", self.board, self.card)
    }

    async fn card(&self) -> Value {
        req(
            &self.app,
            &self.member,
            Method::GET,
            &self.card_path(),
            None,
        )
        .await
        .1
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rich_text_is_cleaned_on_the_server() {
    let w = world().await;
    let attacks = [
        "<p>Plan</p><script>alert(1)</script>",
        "<img src=x onerror=alert(1)><p onclick=\"steal()\">hi</p>",
        "<a href=\"javascript:alert(1)\">x</a><a href=\"https://ok.example\" onmouseover=\"x()\">ok</a>",
        "<svg onload=alert(1)></svg><iframe src=https://x.example></iframe><style>*{}</style>",
    ];
    for html in attacks {
        let (s, _) = req(
            &w.app,
            &w.member,
            Method::PATCH,
            &w.card_path(),
            Some(json!({ "description_html": html })),
        )
        .await;
        assert_eq!(s, StatusCode::NO_CONTENT);
        let d = w.card().await["description"]
            .as_str()
            .unwrap()
            .to_ascii_lowercase();
        for bad in [
            "<script",
            "onerror",
            "onclick",
            "onload",
            "onmouseover",
            "javascript:",
            "<img",
            "<svg",
            "<iframe",
            "<style",
        ] {
            assert!(!d.contains(bad), "{html} kept {bad}: {d}");
        }
    }
    req(&w.app, &w.member, Method::PATCH, &w.card_path(), Some(json!({ "description_markdown": "**Goal** [site](https://example.com) <script>x</script>" }))).await;
    let d = w.card().await["description"].as_str().unwrap().to_string();
    assert!(d.contains("<strong>Goal</strong>"), "{d}");
    assert!(d.contains("href=\"https://example.com\""), "{d}");
    assert!(!d.contains("<script"), "{d}");
    req(
        &w.app,
        &w.member,
        Method::PATCH,
        &w.card_path(),
        Some(json!({ "description_html": "<p> </p>" })),
    )
    .await;
    assert_eq!(w.card().await["description"], "");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn labels_members_dates_done_and_cover() {
    let w = world().await;
    let (_, b) = req(
        &w.app,
        &w.member,
        Method::GET,
        &format!("{C}/boards/{}", w.board),
        None,
    )
    .await;
    assert_eq!(b["labels"].as_array().unwrap().len(), 8);
    let label = b["labels"][3]["id"].as_i64().unwrap();
    let path = w.card_path();
    assert_eq!(
        req(
            &w.app,
            &w.member,
            Method::PUT,
            &format!("{path}/labels/{label}"),
            Some(json!({ "on": true }))
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        req(
            &w.app,
            &w.member,
            Method::PUT,
            &format!("{path}/members/{}", w.member.id),
            Some(json!({ "on": true }))
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (s, _) = req(
        &w.app,
        &w.member,
        Method::PUT,
        &format!("{path}/members/{}", w.observer.id),
        Some(json!({ "on": true })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "observers cannot be assigned");
    let (s, _) = req(
        &w.app,
        &w.member,
        Method::PUT,
        &format!("{path}/members/{}", w.outsider.id),
        Some(json!({ "on": true })),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::BAD_REQUEST,
        "only people who can see the board"
    );
    let (s, _) = req(&w.app, &w.member, Method::PATCH, &path, Some(json!({ "due_at": 1_900_000_000, "start_at": 1_899_000_000, "done": true, "cover": "#4B4FD8" }))).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = req(
        &w.app,
        &w.member,
        Method::PATCH,
        &path,
        Some(json!({ "cover": "red; background:url(x)" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = req(
        &w.app,
        &w.member,
        Method::PATCH,
        &path,
        Some(json!({ "board_id": 9 })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    let card = w.card().await;
    assert_eq!(card["labels"], json!([label]));
    assert_eq!(card["members"], json!([w.member.id]));
    assert_eq!(card["done"], true);
    let (_, b) = req(
        &w.app,
        &w.member,
        Method::GET,
        &format!("{C}/boards/{}", w.board),
        None,
    )
    .await;
    let tile = &b["lists"][0]["cards"][0];
    assert_eq!(tile["labels"], json!([label]));
    assert_eq!(tile["due_at"], 1_900_000_000);
    assert_eq!(tile["cover"], "#4B4FD8");

    req(
        &w.app,
        &w.member,
        Method::PATCH,
        &path,
        Some(json!({ "due_at": null })),
    )
    .await;
    assert_eq!(w.card().await["due_at"], Value::Null);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn checklists_and_items_keep_their_order_and_counts() {
    let w = world().await;
    let path = w.card_path();
    let base = format!("{C}/boards/{}", w.board);
    let (_, v) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{path}/checklists"),
        Some(json!({ "title": "Launch" })),
    )
    .await;
    let a = v["id"].as_i64().unwrap();
    let (_, v) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{path}/checklists"),
        Some(json!({ "title": "QA" })),
    )
    .await;
    let b = v["id"].as_i64().unwrap();
    req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{base}/checklists/{b}/move"),
        Some(json!({ "after": null })),
    )
    .await;
    let mut items = Vec::new();
    for t in ["one", "two", "three"] {
        let (_, v) = req(
            &w.app,
            &w.member,
            Method::POST,
            &format!("{base}/checklists/{a}/items"),
            Some(json!({ "text": t })),
        )
        .await;
        items.push(v["id"].as_i64().unwrap());
    }
    req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{base}/items/{}/move", items[2]),
        Some(json!({ "after": null })),
    )
    .await;
    req(
        &w.app,
        &w.member,
        Method::PATCH,
        &format!("{base}/items/{}", items[0]),
        Some(json!({ "done": true })),
    )
    .await;
    req(
        &w.app,
        &w.member,
        Method::PATCH,
        &format!("{base}/checklists/{a}"),
        Some(json!({ "title": "Launch day" })),
    )
    .await;

    let card = w.card().await;
    let titles: Vec<&str> = card["checklists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["QA", "Launch day"]);
    let texts: Vec<&str> = card["checklists"][1]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, vec!["three", "one", "two"]);
    let (_, board) = req(&w.app, &w.member, Method::GET, &base, None).await;
    assert_eq!(board["lists"][0]["cards"][0]["items"], 3);
    assert_eq!(board["lists"][0]["cards"][0]["items_done"], 1);

    req(
        &w.app,
        &w.member,
        Method::DELETE,
        &format!("{base}/items/{}", items[1]),
        None,
    )
    .await;
    req(
        &w.app,
        &w.member,
        Method::DELETE,
        &format!("{base}/checklists/{b}"),
        None,
    )
    .await;
    let card = w.card().await;
    assert_eq!(card["checklists"].as_array().unwrap().len(), 1);
    assert_eq!(card["checklists"][0]["items"].as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn observers_cannot_edit_or_comment_and_people_edit_only_their_own_comments() {
    let w = world().await;
    let path = w.card_path();
    let base = format!("{C}/boards/{}", w.board);
    let (s, v) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{path}/comments"),
        Some(json!({ "body": "Looks good" })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let comment = v["id"].as_i64().unwrap();
    let (_, v) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{path}/checklists"),
        Some(json!({ "title": "x" })),
    )
    .await;
    let checklist = v["id"].as_i64().unwrap();

    let writes: Vec<(Method, String, Option<Value>)> = vec![
        (
            Method::POST,
            format!("{path}/comments"),
            Some(json!({ "body": "me too" })),
        ),
        (
            Method::PATCH,
            path.clone(),
            Some(json!({ "description_html": "<p>x</p>" })),
        ),
        (Method::PATCH, path.clone(), Some(json!({ "done": true }))),
        (
            Method::PUT,
            format!("{path}/labels/1"),
            Some(json!({ "on": true })),
        ),
        (
            Method::PUT,
            format!("{path}/members/{}", w.observer.id),
            Some(json!({ "on": true })),
        ),
        (
            Method::POST,
            format!("{path}/checklists"),
            Some(json!({ "title": "x" })),
        ),
        (
            Method::POST,
            format!("{base}/checklists/{checklist}/items"),
            Some(json!({ "text": "x" })),
        ),
        (
            Method::POST,
            format!("{base}/labels"),
            Some(json!({ "name": "x", "color": 1 })),
        ),
        (
            Method::PATCH,
            format!("{base}/comments/{comment}"),
            Some(json!({ "body": "x" })),
        ),
    ];
    for (m, p, body) in writes {
        let (s, _) = req(&w.app, &w.observer, m.clone(), &p, body).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{m} {p}");
    }

    let (s, _) = req(
        &w.app,
        &w.owner,
        Method::PATCH,
        &format!("{base}/comments/{comment}"),
        Some(json!({ "body": "rewritten" })),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = req(
        &w.app,
        &w.owner,
        Method::DELETE,
        &format!("{base}/comments/{comment}"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = req(
        &w.app,
        &w.member,
        Method::PATCH,
        &format!("{base}/comments/{comment}"),
        Some(json!({ "body": "Looks great" })),
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let card = w.card().await;
    assert_eq!(card["comments"][0]["body"], "Looks great");
    assert!(card["comments"][0]["edited_at"].is_i64());
    assert_eq!(card["comments"][0]["author"]["id"], w.member.id);
    let (s, _) = req(
        &w.app,
        &w.member,
        Method::DELETE,
        &format!("{base}/comments/{comment}"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn labels_and_checklists_from_another_board_are_not_found() {
    let w = world().await;
    let (_, v) = req(
        &w.app,
        &w.owner,
        Method::POST,
        &format!("{C}/projects/1/boards"),
        Some(json!({ "name": "Other" })),
    )
    .await;
    let other = v["id"].as_i64().unwrap();
    let (_, b) = req(
        &w.app,
        &w.owner,
        Method::GET,
        &format!("{C}/boards/{other}"),
        None,
    )
    .await;
    let foreign = b["labels"][0]["id"].as_i64().unwrap();
    let (s, _) = req(
        &w.app,
        &w.member,
        Method::PUT,
        &format!("{}/labels/{foreign}", w.card_path()),
        Some(json!({ "on": true })),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (_, v) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{}/checklists", w.card_path()),
        Some(json!({ "title": "Mine" })),
    )
    .await;
    let checklist = v["id"].as_i64().unwrap();
    let (s, _) = req(
        &w.app,
        &w.member,
        Method::PATCH,
        &format!("{C}/boards/{other}/checklists/{checklist}"),
        Some(json!({ "title": "x" })),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn copying_a_board_copies_labels_checklists_and_members() {
    let w = world().await;
    let path = w.card_path();
    let (_, b) = req(
        &w.app,
        &w.member,
        Method::GET,
        &format!("{C}/boards/{}", w.board),
        None,
    )
    .await;
    let label = b["labels"][0]["id"].as_i64().unwrap();
    req(
        &w.app,
        &w.member,
        Method::PUT,
        &format!("{path}/labels/{label}"),
        Some(json!({ "on": true })),
    )
    .await;
    req(
        &w.app,
        &w.member,
        Method::PUT,
        &format!("{path}/members/{}", w.member.id),
        Some(json!({ "on": true })),
    )
    .await;
    let (_, v) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{path}/checklists"),
        Some(json!({ "title": "Steps" })),
    )
    .await;
    let checklist = v["id"].as_i64().unwrap();
    req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{C}/boards/{}/checklists/{checklist}/items", w.board),
        Some(json!({ "text": "a" })),
    )
    .await;
    let (_, v) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{C}/boards/{}/copy", w.board),
        Some(json!({ "name": "B2" })),
    )
    .await;
    let copy = v["id"].as_i64().unwrap();
    let (_, b2) = req(
        &w.app,
        &w.member,
        Method::GET,
        &format!("{C}/boards/{copy}"),
        None,
    )
    .await;
    let tile = &b2["lists"][0]["cards"][0];
    let new_label = b2["labels"][0]["id"].as_i64().unwrap();
    assert_ne!(new_label, label);
    assert_eq!(tile["labels"], json!([new_label]));
    assert_eq!(tile["members"], json!([w.member.id]));
    assert_eq!(tile["items"], 1);
}
