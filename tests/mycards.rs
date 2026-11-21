mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{TestApp, Who, body_json, body_text, call, person};
use serde_json::{Value, json};

async fn req(app: &TestApp, who: &Who, method: Method, path: &str, body: Option<Value>) -> Value {
    let res = call(app, who, method.clone(), path, body).await;
    assert!(
        res.status().is_success(),
        "{method} {path} -> {}",
        res.status()
    );
    serde_json::from_str(&body_text(res).await).unwrap_or(Value::Null)
}

async fn board_with_card(
    app: &TestApp,
    owner: &Who,
    base: &str,
    project: i64,
    name: &str,
    title: &str,
    assignee: i64,
) -> (i64, i64) {
    let board = req(
        app,
        owner,
        Method::POST,
        &format!("{base}/projects/{project}/boards"),
        Some(json!({ "name": name })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let list = req(
        app,
        owner,
        Method::GET,
        &format!("{base}/boards/{board}"),
        None,
    )
    .await["lists"][0]["id"]
        .as_i64()
        .unwrap();
    let card = req(
        app,
        owner,
        Method::POST,
        &format!("{base}/boards/{board}/cards"),
        Some(json!({ "list_id": list, "title": title })),
    )
    .await["cards"][0]["id"]
        .as_i64()
        .unwrap();
    req(
        app,
        owner,
        Method::PUT,
        &format!("{base}/boards/{board}/cards/{card}/members/{assignee}"),
        Some(json!({ "on": true })),
    )
    .await;
    (board, card)
}

fn titles(v: &Value) -> Vec<String> {
    let mut t: Vec<String> = v["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap().to_string())
        .collect();
    t.sort();
    t
}

#[tokio::test]
async fn my_cards_lists_only_projects_the_person_belongs_to() {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let b = app.company("company-b", "active").await;
    let boss = person(&app, "boss@a.example").await;
    let dev = person(&app, "dev@a.example").await;
    app.member(a, boss.id, "owner").await;
    app.member(b, boss.id, "owner").await;
    app.member(a, dev.id, "member").await;
    app.member(b, dev.id, "member").await;
    let base_a = "/api/v1/c/company-a";
    let base_b = "/api/v1/c/company-b";
    let p1 = req(
        &app,
        &boss,
        Method::POST,
        &format!("{base_a}/projects"),
        Some(json!({ "name": "Web", "owner_id": boss.id })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    req(
        &app,
        &boss,
        Method::POST,
        &format!("{base_a}/projects/{p1}/invites"),
        Some(json!({ "email": "dev@a.example", "role": "member" })),
    )
    .await;
    let (_, _) = board_with_card(&app, &boss, base_a, p1, "Sprint", "Visible card", dev.id).await;
    let (hidden, _) =
        board_with_card(&app, &boss, base_a, p1, "Hidden", "Restricted card", dev.id).await;
    let (closed, _) =
        board_with_card(&app, &boss, base_a, p1, "Old", "Closed board card", dev.id).await;
    let (_, archived) =
        board_with_card(&app, &boss, base_a, p1, "Arch", "Archived card", dev.id).await;
    let arch_board = req(
        &app,
        &boss,
        Method::GET,
        &format!("{base_a}/projects/{p1}/boards"),
        None,
    )
    .await["boards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["name"] == "Arch")
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    req(
        &app,
        &boss,
        Method::POST,
        &format!("{base_a}/boards/{arch_board}/cards/{archived}/archive"),
        None,
    )
    .await;
    req(
        &app,
        &boss,
        Method::PUT,
        &format!("{base_a}/boards/{hidden}/access"),
        Some(json!({ "restricted": true, "people": [] })),
    )
    .await;
    req(
        &app,
        &boss,
        Method::POST,
        &format!("{base_a}/boards/{closed}/close"),
        None,
    )
    .await;

    let p2 = req(
        &app,
        &boss,
        Method::POST,
        &format!("{base_a}/projects"),
        Some(json!({ "name": "Ops", "owner_id": boss.id })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    req(
        &app,
        &boss,
        Method::POST,
        &format!("{base_a}/projects/{p2}/invites"),
        Some(json!({ "email": "dev@a.example", "role": "member" })),
    )
    .await;
    let (_, _) = board_with_card(
        &app,
        &boss,
        base_a,
        p2,
        "Ops board",
        "Card in a project I leave",
        dev.id,
    )
    .await;
    req(
        &app,
        &boss,
        Method::DELETE,
        &format!("{base_a}/projects/{p2}/members/{}", dev.id),
        None,
    )
    .await;

    let pb = req(
        &app,
        &boss,
        Method::POST,
        &format!("{base_b}/projects"),
        Some(json!({ "name": "Labs", "owner_id": boss.id })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    req(
        &app,
        &boss,
        Method::POST,
        &format!("{base_b}/projects/{pb}/invites"),
        Some(json!({ "email": "dev@a.example", "role": "member" })),
    )
    .await;
    let (_, _) = board_with_card(&app, &boss, base_b, pb, "Lab", "Card in company B", dev.id).await;
    let (_, _) = board_with_card(
        &app,
        &boss,
        base_b,
        pb,
        "Lab 2",
        "Not assigned to me",
        boss.id,
    )
    .await;

    let only_a = body_json(
        app.get("/api/v1/me/cards?company=company-a", Some(&dev.cookie))
            .await,
    )
    .await;
    assert_eq!(titles(&only_a), ["Visible card"]);
    assert_eq!(only_a["cards"][0]["company"]["slug"], "company-a");
    assert_eq!(only_a["cards"][0]["project"]["name"], "Web");
    assert_eq!(only_a["cards"][0]["can_edit"], true);

    let watcher = person(&app, "obs@a.example").await;
    app.member(a, watcher.id, "member").await;
    req(
        &app,
        &boss,
        Method::POST,
        &format!("{base_a}/projects/{p1}/invites"),
        Some(json!({ "email": "obs@a.example", "role": "member" })),
    )
    .await;
    board_with_card(
        &app,
        &boss,
        base_a,
        p1,
        "Watch",
        "Observed card",
        watcher.id,
    )
    .await;
    req(
        &app,
        &boss,
        Method::PATCH,
        &format!("{base_a}/projects/{p1}/members/{}", watcher.id),
        Some(json!({ "role": "observer" })),
    )
    .await;
    let seen = body_json(
        app.get("/api/v1/me/cards?company=company-a", Some(&watcher.cookie))
            .await,
    )
    .await;
    assert_eq!(titles(&seen), ["Observed card"]);
    assert_eq!(
        seen["cards"][0]["can_edit"], false,
        "observers see their cards read-only"
    );
    let all = body_json(app.get("/api/v1/me/cards", Some(&dev.cookie)).await).await;
    assert_eq!(titles(&all), ["Card in company B", "Visible card"]);

    app.sql(|c| {
        c.execute(
            "UPDATE orgs SET status = 'suspended' WHERE slug = 'company-b'",
            [],
        )
    })
    .await;
    let all = body_json(app.get("/api/v1/me/cards", Some(&dev.cookie)).await).await;
    assert_eq!(
        titles(&all),
        ["Visible card"],
        "a suspended company drops out"
    );

    let outsider = person(&app, "plain@a.example").await;
    assert_eq!(
        app.get("/api/v1/me/cards?company=company-a", Some(&outsider.cookie))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get("/api/v1/me/cards?company=../x", Some(&dev.cookie))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get("/api/v1/me/cards", None).await.status(),
        StatusCode::UNAUTHORIZED
    );

    let p3 = req(
        &app,
        &boss,
        Method::POST,
        &format!("{base_a}/projects"),
        Some(json!({ "name": "Docs", "owner_id": boss.id })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    req(
        &app,
        &boss,
        Method::POST,
        &format!("{base_a}/projects/{p3}/invites"),
        Some(json!({ "email": "dev@a.example", "role": "member" })),
    )
    .await;
    let (docs_board, docs_card) =
        board_with_card(&app, &boss, base_a, p3, "Docs board", "Docs card", dev.id).await;
    req(
        &app,
        &boss,
        Method::PATCH,
        &format!("{base_a}/boards/{docs_board}/cards/{docs_card}"),
        Some(json!({ "due_at": tazko::session::now() - 3600 })),
    )
    .await;

    for (projects, expected) in [
        (None, vec!["Docs card", "Visible card"]),
        (
            Some(json!([{ "company": "company-a", "project": p1 }])),
            vec!["Visible card"],
        ),
    ] {
        let mut body = json!({ "name": "script", "scope": "read" });
        if let Some(p) = projects {
            body["projects"] = p;
        }
        let token = req(&app, &dev, Method::POST, "/api/v1/me/tokens", Some(body)).await["token"]
            .as_str()
            .unwrap()
            .to_string();
        let r = Request::builder()
            .uri("/api/v1/me/cards")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            titles(&body_json(app.send(r).await).await),
            expected,
            "a token sees only the projects it was given"
        );
    }

    for (window, expected) in [
        ("overdue", vec!["Docs card"]),
        ("today", vec![]),
        ("week", vec![]),
        ("none", vec!["Visible card"]),
    ] {
        let found = body_json(
            app.get(
                &format!("/api/v1/me/cards?company=company-a&due={window}"),
                Some(&dev.cookie),
            )
            .await,
        )
        .await;
        assert_eq!(titles(&found), expected, "due={window}");
    }
    assert_eq!(
        app.get("/api/v1/me/cards?due=soon", Some(&dev.cookie))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
}
