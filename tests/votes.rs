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

async fn tile(app: &TestApp, who: &Who, b: i64, card: i64) -> Value {
    let d = req(app, who, Method::GET, &format!("{C}/boards/{b}"), None).await;
    d["lists"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|l| l["cards"].as_array().unwrap().clone())
        .find(|c| c["id"] == card)
        .unwrap()
}

#[tokio::test]
async fn one_vote_per_person_shown_on_tile_and_card_and_turned_off_by_owners() {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let boss = person(&app, "boss@a.example").await;
    let dev = person(&app, "dev@a.example").await;
    let viewer = person(&app, "viewer@a.example").await;
    let outsider = person(&app, "out@a.example").await;
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
    assert_eq!(
        req(
            &app,
            &boss,
            Method::GET,
            &format!("{C}/projects/{project}"),
            None
        )
        .await["voting"],
        false,
        "voting is off until an owner turns it on"
    );
    let b0 = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects/{project}/boards"),
        Some(json!({ "name": "Before" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let l0 =
        req(&app, &boss, Method::GET, &format!("{C}/boards/{b0}"), None).await["lists"][0]["id"]
            .as_i64()
            .unwrap();
    let c0 = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/boards/{b0}/cards"),
        Some(json!({ "list_id": l0, "title": "Early" })),
    )
    .await["cards"][0]["id"]
        .as_i64()
        .unwrap();
    assert_eq!(
        call(
            &app,
            &dev,
            Method::PUT,
            &format!("{C}/boards/{b0}/cards/{c0}/vote"),
            Some(json!({ "on": true }))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            &boss,
            Method::POST,
            &format!("{C}/boards/{b0}/lists/{l0}/sort"),
            Some(json!({ "by": "votes" }))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST,
        "no sorting by votes while voting is off"
    );
    req(
        &app,
        &boss,
        Method::PATCH,
        &format!("{C}/projects/{project}"),
        Some(json!({ "voting": true })),
    )
    .await;
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
    let mut cards = Vec::new();
    for title in ["Few", "Many"] {
        cards.push(
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
    let (few, many) = (cards[0], cards[1]);
    let vote = |card: i64| format!("{C}/boards/{b}/cards/{card}/vote");

    req(
        &app,
        &dev,
        Method::PUT,
        &vote(many),
        Some(json!({ "on": true })),
    )
    .await;
    req(
        &app,
        &dev,
        Method::PUT,
        &vote(many),
        Some(json!({ "on": true })),
    )
    .await;
    req(
        &app,
        &boss,
        Method::PUT,
        &vote(many),
        Some(json!({ "on": true })),
    )
    .await;
    let t = tile(&app, &dev, b, many).await;
    assert_eq!(
        (t["votes"].clone(), t["voted"].clone()),
        (json!(2), json!(true)),
        "one vote per person"
    );
    assert_eq!(tile(&app, &viewer, b, many).await["voted"], false);
    assert!(
        tile(&app, &dev, b, few).await.get("votes").is_none(),
        "no votes, no count"
    );
    let detail = req(
        &app,
        &viewer,
        Method::GET,
        &format!("{C}/boards/{b}/cards/{many}"),
        None,
    )
    .await;
    let mut names: Vec<&str> = detail["voters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["email"].as_str().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["boss@a.example", "dev@a.example"],
        "everyone who sees the card sees who voted"
    );
    assert_eq!(
        req(&app, &boss, Method::GET, &format!("{C}/boards/{b}"), None).await["voting"],
        true
    );

    req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/boards/{b}/lists/{list}/sort"),
        Some(json!({ "by": "votes" })),
    )
    .await;
    let order: Vec<i64> =
        req(&app, &boss, Method::GET, &format!("{C}/boards/{b}"), None).await["lists"][0]["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_i64().unwrap())
            .collect();
    assert_eq!(order, [many, few], "sort by votes, most first");

    req(
        &app,
        &dev,
        Method::PUT,
        &vote(many),
        Some(json!({ "on": false })),
    )
    .await;
    let t = tile(&app, &dev, b, many).await;
    assert_eq!(
        (t["votes"].clone(), t["voted"].clone()),
        (json!(1), json!(false)),
        "voting again removes the vote"
    );
    let activity = req(
        &app,
        &boss,
        Method::GET,
        &format!("{C}/boards/{b}/cards/{many}"),
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
        kinds.contains(&"card_voted") && kinds.contains(&"card_unvoted"),
        "{kinds:?}"
    );

    assert_eq!(
        call(
            &app,
            &viewer,
            Method::PUT,
            &vote(few),
            Some(json!({ "on": true }))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN,
        "observers cannot vote"
    );
    assert_eq!(
        call(
            &app,
            &outsider,
            Method::PUT,
            &vote(few),
            Some(json!({ "on": true }))
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/boards/{b}/cards/{few}/archive"),
        None,
    )
    .await;
    assert_eq!(
        call(
            &app,
            &dev,
            Method::PUT,
            &vote(few),
            Some(json!({ "on": true }))
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );

    req(
        &app,
        &boss,
        Method::PUT,
        &format!("{C}/boards/{b}/cards/{many}/template"),
        Some(json!({ "on": true })),
    )
    .await;
    assert_eq!(
        call(
            &app,
            &dev,
            Method::PUT,
            &vote(many),
            Some(json!({ "on": true }))
        )
        .await
        .status(),
        StatusCode::NOT_FOUND,
        "template cards take no votes"
    );
    req(
        &app,
        &boss,
        Method::PUT,
        &format!("{C}/boards/{b}/cards/{many}/template"),
        Some(json!({ "on": false })),
    )
    .await;
    assert_eq!(
        call(
            &app,
            &dev,
            Method::PATCH,
            &format!("{C}/projects/{project}"),
            Some(json!({ "voting": false }))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN,
        "members cannot turn voting off"
    );
    req(
        &app,
        &boss,
        Method::PATCH,
        &format!("{C}/projects/{project}"),
        Some(json!({ "voting": false })),
    )
    .await;
    assert_eq!(
        call(
            &app,
            &dev,
            Method::PUT,
            &vote(many),
            Some(json!({ "on": true }))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        req(&app, &boss, Method::GET, &format!("{C}/boards/{b}"), None).await["voting"],
        false
    );
}
