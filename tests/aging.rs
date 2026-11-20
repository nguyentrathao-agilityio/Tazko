mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, Who, call, person};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";
const DAY: i64 = 86_400;

async fn req(app: &TestApp, who: &Who, method: Method, path: &str, body: Option<Value>) -> Value {
    let res = call(app, who, method.clone(), path, body).await;
    assert!(
        res.status().is_success(),
        "{method} {path} -> {}",
        res.status()
    );
    serde_json::from_str(&common::body_text(res).await).unwrap_or(Value::Null)
}

async fn age(app: &TestApp, card: i64, days: i64) {
    let db = app.state.dbs.tenant("company-a").unwrap();
    db.write(move |c| {
        Ok(c.execute(
            "UPDATE cards SET updated_at = unixepoch() - ?2 WHERE id = ?1",
            rusqlite::params![card, days * DAY],
        )?)
    })
    .await
    .unwrap();
}

async fn stale(app: &TestApp, who: &Who) -> Vec<String> {
    let r = req(
        app,
        who,
        Method::GET,
        &format!("{C}/search?q=is:stale"),
        None,
    )
    .await;
    let mut t: Vec<String> = r["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap().to_string())
        .collect();
    t.sort();
    t
}

#[tokio::test]
async fn aging_is_a_project_setting_and_is_stale_finds_quiet_open_cards() {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
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
    let p = req(
        &app,
        &boss,
        Method::GET,
        &format!("{C}/projects/{project}"),
        None,
    )
    .await;
    assert_eq!(
        (p["aging"].clone(), p["aging_days"].clone()),
        (json!(true), json!(7)),
        "aging is on at 7 days by default"
    );

    let b = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects/{project}/boards"),
        Some(json!({ "name": "Sprint" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let list =
        req(&app, &boss, Method::GET, &format!("{C}/boards/{b}"), None).await["lists"][0]["id"]
            .as_i64()
            .unwrap();
    let mut ids = Vec::new();
    for title in ["Quiet", "Fresh", "Quiet but done", "Old"] {
        ids.push(
            req(
                &app,
                &boss,
                Method::POST,
                &format!("{C}/boards/{b}/cards"),
                Some(json!({ "list_id": list, "title": title })),
            )
            .await["cards"][0]["id"]
                .as_i64()
                .unwrap(),
        );
    }
    req(
        &app,
        &boss,
        Method::PATCH,
        &format!("{C}/boards/{b}/cards/{}", ids[2]),
        Some(json!({ "done": true })),
    )
    .await;
    age(&app, ids[0], 8).await;
    age(&app, ids[2], 8).await;
    age(&app, ids[3], 40).await;
    assert_eq!(
        stale(&app, &dev).await,
        ["Old", "Quiet"],
        "open cards quiet for 7+ days; done cards are never stale"
    );

    let board = req(&app, &boss, Method::GET, &format!("{C}/boards/{b}"), None).await;
    assert_eq!(
        (board["aging"].clone(), board["aging_days"].clone()),
        (json!(true), json!(7))
    );

    req(
        &app,
        &dev,
        Method::POST,
        &format!("{C}/boards/{b}/cards/{}/comments", ids[0]),
        Some(json!({ "body": "still on it" })),
    )
    .await;
    assert_eq!(
        stale(&app, &dev).await,
        ["Old"],
        "a comment brings a card back"
    );

    req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/boards/{b}/cards/{}/links", ids[1]),
        Some(json!({ "card": ids[3], "kind": "relates" })),
    )
    .await;
    assert!(
        stale(&app, &dev).await.is_empty(),
        "linking cards counts as activity on both"
    );
    age(&app, ids[3], 40).await;
    let before = req(&app, &dev, Method::GET, &format!("{C}/boards/{b}"), None).await["last_event"]
        .as_i64()
        .unwrap();
    req(
        &app,
        &boss,
        Method::PATCH,
        &format!("{C}/projects/{project}"),
        Some(json!({ "aging_days": 30 })),
    )
    .await;
    let after = req(&app, &dev, Method::GET, &format!("{C}/boards/{b}"), None).await["last_event"]
        .as_i64()
        .unwrap();
    assert!(after > before, "open boards hear about the new setting");
    assert_eq!(stale(&app, &dev).await, ["Old"]);
    req(
        &app,
        &boss,
        Method::PATCH,
        &format!("{C}/projects/{project}"),
        Some(json!({ "aging_days": 3 })),
    )
    .await;
    req(
        &app,
        &boss,
        Method::PATCH,
        &format!("{C}/projects/{project}"),
        Some(json!({ "aging": false })),
    )
    .await;
    assert!(
        stale(&app, &dev).await.is_empty(),
        "nothing is stale while aging is off"
    );
    let board = req(&app, &boss, Method::GET, &format!("{C}/boards/{b}"), None).await;
    assert_eq!(
        (board["aging"].clone(), board["aging_days"].clone()),
        (json!(false), json!(3))
    );

    assert_eq!(
        call(
            &app,
            &boss,
            Method::PATCH,
            &format!("{C}/projects/{project}"),
            Some(json!({ "aging_days": 5 }))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            &dev,
            Method::PATCH,
            &format!("{C}/projects/{project}"),
            Some(json!({ "aging": true }))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN,
        "only owners change aging"
    );
}
