mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, Who, call, person};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";

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
    viewer: Who,
    project: i64,
}

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let boss = person(&app, "boss@a.example").await;
    let dev = person(&app, "dev@a.example").await;
    let viewer = person(&app, "viewer@a.example").await;
    app.member(a, boss.id, "owner").await;
    app.member(a, dev.id, "member").await;
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
    World {
        app,
        boss,
        dev,
        viewer,
        project,
    }
}

async fn board(w: &World, project: i64, name: &str) -> (i64, i64) {
    let b = req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/projects/{project}/boards"),
        Some(json!({ "name": name })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let l = req(
        &w.app,
        &w.boss,
        Method::GET,
        &format!("{C}/boards/{b}"),
        None,
    )
    .await["lists"][0]["id"]
        .as_i64()
        .unwrap();
    (b, l)
}

async fn card(w: &World, (b, l): (i64, i64), title: &str) -> i64 {
    req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/boards/{b}/cards"),
        Some(json!({ "list_id": l, "title": title })),
    )
    .await["cards"][0]["id"]
        .as_i64()
        .unwrap()
}

async fn links(w: &World, who: &Who, b: i64, card: i64) -> Vec<(String, String)> {
    let d = req(
        &w.app,
        who,
        Method::GET,
        &format!("{C}/boards/{b}/cards/{card}"),
        None,
    )
    .await;
    d["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["kind"].as_str().unwrap().to_string(),
                l["card"]["title"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

async fn tile(w: &World, who: &Who, b: i64, card: i64) -> Value {
    let d = req(&w.app, who, Method::GET, &format!("{C}/boards/{b}"), None).await;
    d["lists"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|l| l["cards"].as_array().unwrap().clone())
        .find(|c| c["id"] == card)
        .unwrap()
}

#[tokio::test]
async fn a_link_shows_on_both_cards_and_a_done_blocker_clears_the_badge() {
    let w = world().await;
    let one = board(&w, w.project, "Sprint").await;
    let two = board(&w, w.project, "Ops").await;
    let api = card(&w, one, "Build API").await;
    let ui = card(&w, two, "Build UI").await;
    let old = card(&w, one, "Old UI ticket").await;

    req(
        &w.app,
        &w.dev,
        Method::POST,
        &format!("{C}/boards/{}/cards/{ui}/links", two.0),
        Some(json!({ "card": api, "kind": "blocked_by" })),
    )
    .await;
    assert_eq!(
        links(&w, &w.boss, two.0, ui).await,
        [("blocked_by".to_string(), "Build API".to_string())]
    );
    assert_eq!(
        links(&w, &w.boss, one.0, api).await,
        [("blocks".to_string(), "Build UI".to_string())]
    );
    let t = tile(&w, &w.boss, two.0, ui).await;
    assert_eq!(
        (t["links"].clone(), t["blocked"].clone()),
        (json!(1), json!(true))
    );
    assert_eq!(
        tile(&w, &w.boss, one.0, api).await["blocked"],
        false,
        "the blocker itself is not blocked"
    );

    req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/boards/{}/cards/{old}/links", one.0),
        Some(json!({ "card": ui, "kind": "duplicated_by" })),
    )
    .await;
    assert_eq!(
        links(&w, &w.boss, two.0, ui).await.last().unwrap(),
        &("duplicates".to_string(), "Old UI ticket".to_string())
    );

    req(
        &w.app,
        &w.boss,
        Method::PATCH,
        &format!("{C}/boards/{}/cards/{api}", one.0),
        Some(json!({ "done": true })),
    )
    .await;
    assert_eq!(
        tile(&w, &w.boss, two.0, ui).await["blocked"],
        false,
        "a done blocker no longer blocks"
    );

    req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/boards/{}/cards/{api}/links", one.0),
        Some(json!({ "card": ui, "kind": "relates" })),
    )
    .await;
    assert_eq!(
        links(&w, &w.boss, two.0, ui).await,
        [
            ("duplicates".to_string(), "Old UI ticket".to_string()),
            ("relates".to_string(), "Build API".to_string())
        ],
        "linking the same pair again replaces the kind"
    );

    req(
        &w.app,
        &w.boss,
        Method::DELETE,
        &format!("{C}/boards/{}/cards/{ui}/links/{api}", two.0),
        None,
    )
    .await;
    assert_eq!(
        links(&w, &w.boss, one.0, api).await,
        vec![],
        "removing a link removes it from both cards"
    );
    assert_eq!(links(&w, &w.boss, two.0, ui).await.len(), 1);
    let activity = req(
        &w.app,
        &w.boss,
        Method::GET,
        &format!("{C}/boards/{}/cards/{api}", one.0),
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
    assert!(
        kinds.contains(&"link_added") && kinds.contains(&"link_removed"),
        "{kinds:?}"
    );
}

#[tokio::test]
async fn links_only_reach_cards_the_person_can_see() {
    let w = world().await;
    let one = board(&w, w.project, "Sprint").await;
    let secret = board(&w, w.project, "Secret").await;
    let other_project = req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "Ops", "owner_id": w.boss.id })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let elsewhere = board(&w, other_project, "Elsewhere").await;
    let a = card(&w, one, "Visible card").await;
    let hidden = card(&w, secret, "Hidden blocker").await;
    let far = card(&w, elsewhere, "Other project card").await;
    let archived = card(&w, one, "Archived card").await;
    let shut = board(&w, w.project, "Closed").await;
    let closed = card(&w, shut, "On a closed board").await;
    req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/boards/{}/close", shut.0),
        None,
    )
    .await;
    req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/boards/{}/cards/{archived}/archive", one.0),
        None,
    )
    .await;
    req(
        &w.app,
        &w.boss,
        Method::PUT,
        &format!("{C}/boards/{}/access", secret.0),
        Some(json!({ "restricted": true, "people": [w.boss.id] })),
    )
    .await;
    let path = format!("{C}/boards/{}/cards/{a}/links", one.0);

    for (who, target, want) in [
        (&w.dev, hidden, StatusCode::NOT_FOUND),
        (&w.boss, far, StatusCode::NOT_FOUND),
        (&w.boss, archived, StatusCode::NOT_FOUND),
        (&w.boss, closed, StatusCode::NOT_FOUND),
        (&w.boss, a, StatusCode::BAD_REQUEST),
        (&w.viewer, hidden, StatusCode::FORBIDDEN),
    ] {
        assert_eq!(
            call(
                &w.app,
                who,
                Method::POST,
                &path,
                Some(json!({ "card": target, "kind": "relates" }))
            )
            .await
            .status(),
            want,
            "card {target}"
        );
    }
    assert_eq!(
        call(
            &w.app,
            &w.boss,
            Method::POST,
            &path,
            Some(json!({ "card": hidden, "kind": "owns" }))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );

    req(
        &w.app,
        &w.boss,
        Method::POST,
        &path,
        Some(json!({ "card": hidden, "kind": "blocked_by" })),
    )
    .await;
    assert_eq!(links(&w, &w.boss, one.0, a).await.len(), 1);
    assert!(
        links(&w, &w.dev, one.0, a).await.is_empty(),
        "a card on a board you cannot see is left out"
    );
    assert_eq!(tile(&w, &w.boss, one.0, a).await["blocked"], true);
    let dev_tile = tile(&w, &w.dev, one.0, a).await;
    assert!(
        dev_tile["blocked"] != true && dev_tile["links"] != 1,
        "nothing about the hidden card shows on the tile: {dev_tile}"
    );
    assert_eq!(
        call(
            &w.app,
            &w.dev,
            Method::DELETE,
            &format!("{path}/{hidden}"),
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );

    let found = req(
        &w.app,
        &w.dev,
        Method::GET,
        &format!("{C}/boards/{}/cards/{a}/link-targets?q=card", one.0),
        None,
    )
    .await;
    let titles: Vec<&str> = found["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["card"]["title"].as_str().unwrap())
        .collect();
    assert!(
        titles.is_empty(),
        "no hidden, other-project, archived or self cards are offered: {titles:?}"
    );
    let b = card(&w, one, "Another card").await;
    let found = req(
        &w.app,
        &w.boss,
        Method::GET,
        &format!("{C}/boards/{}/cards/{a}/link-targets?q=another", one.0),
        None,
    )
    .await;
    assert_eq!(found["cards"][0]["card"]["id"], b);
    let found = req(
        &w.app,
        &w.boss,
        Method::GET,
        &format!("{C}/boards/{}/cards/{a}/link-targets", one.0),
        None,
    )
    .await;
    assert!(
        found["cards"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["card"]["id"] != hidden),
        "already linked cards are not offered again"
    );
}
