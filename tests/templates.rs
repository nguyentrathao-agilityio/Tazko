mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, Who, body_json, body_text, call, person};
use serde_json::{Value, json};

const A: &str = "/api/v1/c/company-a";
const B: &str = "/api/v1/c/company-b";

async fn req(app: &TestApp, who: &Who, method: Method, path: &str, body: Option<Value>) -> Value {
    let res = call(app, who, method, path, body).await;
    serde_json::from_str(&body_text(res).await).unwrap_or(Value::Null)
}

struct World {
    app: TestApp,
    admin: Who,
    lead: Who,
    dev: Who,
    viewer: Who,
    other: Who,
    board: i64,
    project_a: i64,
    project_b: i64,
}

async fn project(app: &TestApp, owner_of_company: &Who, base: &str, owner: &Who) -> i64 {
    req(
        app,
        owner_of_company,
        Method::POST,
        &format!("{base}/projects"),
        Some(json!({ "name": "P", "owner_id": owner.id })),
    )
    .await["id"]
        .as_i64()
        .unwrap()
}

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let b = app.company("company-b", "active").await;
    let admin = person(&app, "admin@a.example").await;
    let lead = person(&app, "lead@a.example").await;
    let dev = person(&app, "dev@a.example").await;
    let viewer = person(&app, "viewer@a.example").await;
    let other = person(&app, "owner@b.example").await;
    let admin_id = admin.id;
    app.sql(move |c| {
        c.execute(
            "UPDATE users SET is_super_admin = 1 WHERE id = ?1",
            [admin_id],
        )
    })
    .await;
    app.member(a, admin.id, "owner").await;
    app.member(b, other.id, "owner").await;
    for w in [&lead, &dev, &viewer] {
        app.member(a, w.id, "member").await;
    }
    let project_a = project(&app, &admin, A, &lead).await;
    for (e, role) in [
        ("dev@a.example", "member"),
        ("viewer@a.example", "observer"),
    ] {
        req(
            &app,
            &lead,
            Method::POST,
            &format!("{A}/projects/{project_a}/invites"),
            Some(json!({ "email": e, "role": role })),
        )
        .await;
    }
    let project_b = project(&app, &other, B, &other).await;
    let board = req(
        &app,
        &lead,
        Method::POST,
        &format!("{A}/projects/{project_a}/boards"),
        Some(json!({ "name": "Launch", "background": "sea" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    World {
        app,
        admin,
        lead,
        dev,
        viewer,
        other,
        board,
        project_a,
        project_b,
    }
}

fn shape(board: &Value) -> Value {
    let labels: Vec<Value> = board["labels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| json!([l["name"], l["color"]]))
        .collect();
    let names: std::collections::HashMap<i64, String> = board["labels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["id"].as_i64().unwrap(),
                l["name"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let lists: Vec<Value> = board["lists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            json!([
                l["title"],
                l["cards"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| {
                        let mut ls: Vec<String> = c["labels"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|id| names[&id.as_i64().unwrap()].clone())
                            .collect();
                        ls.sort();
                        json!([
                            c["title"],
                            ls,
                            c["members"],
                            c["due_at"],
                            c["comments"],
                            c["attachments"]
                        ])
                    })
                    .collect::<Vec<_>>()
            ])
        })
        .collect();
    let fields: Vec<Value> = board["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| json!([f["name"], f["type"], f["options"], f["show_on_card"]]))
        .collect();
    json!({ "background": board["background"], "labels": labels, "lists": lists, "fields": fields })
}

async fn fill(w: &World) {
    let base = format!("{A}/boards/{}", w.board);
    let board = req(&w.app, &w.lead, Method::GET, &base, None).await;
    let lists: Vec<i64> = board["lists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["id"].as_i64().unwrap())
        .collect();
    req(
        &w.app,
        &w.lead,
        Method::PATCH,
        &format!("{base}/lists/{}", lists[0]),
        Some(json!({ "title": "Ideas" })),
    )
    .await;
    req(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{base}/lists"),
        Some(json!({ "title": "Shipped" })),
    )
    .await;
    let bug = board["labels"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "Bug")
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let made = req(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{base}/cards"),
        Some(json!({ "list_id": lists[0], "titles": ["Hero copy", "Pricing page", "Analytics"] })),
    )
    .await;
    let first = made["cards"][0]["id"].as_i64().unwrap();
    req(
        &w.app,
        &w.lead,
        Method::PUT,
        &format!("{base}/cards/{first}/labels/{bug}"),
        Some(json!({ "on": true })),
    )
    .await;
    req(
        &w.app,
        &w.lead,
        Method::PUT,
        &format!("{base}/cards/{first}/members/{}", w.dev.id),
        Some(json!({ "on": true })),
    )
    .await;
    req(
        &w.app,
        &w.lead,
        Method::PATCH,
        &format!("{base}/cards/{first}"),
        Some(json!({ "due_at": 1_900_000_000 })),
    )
    .await;
    req(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{base}/cards/{first}/comments"),
        Some(json!({ "body": "secret note" })),
    )
    .await;
    req(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{base}/cards/{first}/attachments/link"),
        Some(json!({ "url": "https://example.com/private" })),
    )
    .await;
    req(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{base}/labels"),
        Some(json!({ "name": "Launch", "color": 6 })),
    )
    .await;
    req(&w.app, &w.lead, Method::POST, &format!("{base}/fields"), Some(json!({ "name": "Priority", "type": "select", "options": ["Low", "High"], "show_on_card": true }))).await;
    req(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{base}/cards"),
        Some(json!({ "list_id": lists[1], "title": "Plan the launch" })),
    )
    .await;
}

#[tokio::test]
async fn a_template_makes_an_identical_board_in_another_company() {
    let w = world().await;
    fill(&w).await;
    let base = format!("{A}/boards/{}", w.board);
    let save = format!("{base}/save-template");
    assert_eq!(
        call(
            &w.app,
            &w.dev,
            Method::POST,
            &save,
            Some(json!({ "name": "X" }))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &w.app,
            &w.viewer,
            Method::POST,
            &save,
            Some(json!({ "name": "X" }))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let saved = req(
        &w.app,
        &w.lead,
        Method::POST,
        &save,
        Some(json!({ "name": "Product launch", "description": "Lists, labels and starter cards" })),
    )
    .await;
    let id = saved["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("c:"));

    let in_b = req(
        &w.app,
        &w.other,
        Method::GET,
        &format!("{B}/templates"),
        None,
    )
    .await;
    assert!(
        in_b["templates"].as_array().unwrap().is_empty(),
        "a company's templates stay in that company"
    );
    let use_in_b = call(
        &w.app,
        &w.other,
        Method::POST,
        &format!("{B}/projects/{}/boards", w.project_b),
        Some(json!({ "name": "Copy", "template": id })),
    )
    .await;
    assert_eq!(use_in_b.status(), StatusCode::NOT_FOUND);

    let offer = format!("{A}/templates/{}/offer", id.trim_start_matches("c:"));
    assert_eq!(
        call(&w.app, &w.lead, Method::PUT, &offer, None)
            .await
            .status(),
        StatusCode::FORBIDDEN,
        "only a super admin offers templates"
    );
    let offered = body_json(call(&w.app, &w.admin, Method::PUT, &offer, None).await).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let listed = req(
        &w.app,
        &w.other,
        Method::GET,
        &format!("{B}/templates"),
        None,
    )
    .await;
    assert_eq!(listed["templates"][0]["name"], "Product launch");
    assert_eq!(listed["templates"][0]["admin"], true);
    assert_eq!(listed["templates"][0]["lists"][0]["cards"][0], "Hero copy");
    assert!(
        !listed.to_string().contains("secret note")
            && !listed.to_string().contains("dev@a.example")
    );

    let made = body_json(
        call(
            &w.app,
            &w.other,
            Method::POST,
            &format!("{B}/projects/{}/boards", w.project_b),
            Some(json!({ "name": "Launch", "template": offered })),
        )
        .await,
    )
    .await;
    let copy = req(
        &w.app,
        &w.other,
        Method::GET,
        &format!("{B}/boards/{}", made["id"]),
        None,
    )
    .await;
    let original = req(&w.app, &w.lead, Method::GET, &base, None).await;
    let (a, b) = (shape(&original), shape(&copy));
    assert_eq!(a["lists"].as_array().unwrap().len(), 4);
    assert_eq!(a["labels"], b["labels"]);
    assert_eq!(a["fields"], b["fields"]);
    assert_eq!(a["background"], b["background"]);
    let strip = |v: &Value| -> Value {
        json!(
            v["lists"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| json!([
                    l[0],
                    l[1].as_array()
                        .unwrap()
                        .iter()
                        .map(|c| json!([c[0], c[1]]))
                        .collect::<Vec<_>>()
                ]))
                .collect::<Vec<_>>()
        )
    };
    assert_eq!(strip(&a), strip(&b), "same lists, card titles and labels");
    for list in b["lists"].as_array().unwrap() {
        for card in list[1].as_array().unwrap() {
            assert_eq!(
                (
                    card[2].clone(),
                    card[3].clone(),
                    card[4].clone(),
                    card[5].clone()
                ),
                (json!([]), Value::Null, json!(0), json!(0)),
                "no people, dates, comments or files travel"
            );
        }
    }
    let same_company = body_json(
        call(
            &w.app,
            &w.lead,
            Method::POST,
            &format!("{A}/projects/{}/boards", w.project_a),
            Some(json!({ "name": "Again", "template": id })),
        )
        .await,
    )
    .await;
    let again = req(
        &w.app,
        &w.lead,
        Method::GET,
        &format!("{A}/boards/{}", same_company["id"]),
        None,
    )
    .await;
    assert_eq!(strip(&shape(&again)), strip(&a));

    assert_eq!(
        call(
            &w.app,
            &w.lead,
            Method::DELETE,
            &format!("{A}/admin-templates/{}", offered.trim_start_matches("a:")),
            None
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &w.app,
            &w.admin,
            Method::DELETE,
            &format!("{A}/admin-templates/{}", offered.trim_start_matches("a:")),
            None
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert!(
        req(
            &w.app,
            &w.other,
            Method::GET,
            &format!("{B}/templates"),
            None
        )
        .await["templates"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        call(
            &w.app,
            &w.dev,
            Method::DELETE,
            &format!("{A}/templates/{}", id.trim_start_matches("c:")),
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &w.app,
            &w.lead,
            Method::DELETE,
            &format!("{A}/templates/{}", id.trim_start_matches("c:")),
            None
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn a_card_made_into_a_template_makes_new_cards_until_unmarked() {
    let w = world().await;
    let base = format!("{A}/boards/{}", w.board);
    let board = req(&w.app, &w.lead, Method::GET, &base, None).await;
    let list = board["lists"][0]["id"].as_i64().unwrap();
    let bug = board["labels"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "Bug")
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let card = req(
        &w.app,
        &w.dev,
        Method::POST,
        &format!("{base}/cards"),
        Some(json!({ "list_id": list, "title": "Release checklist" })),
    )
    .await["cards"][0]["id"]
        .as_i64()
        .unwrap();
    req(
        &w.app,
        &w.dev,
        Method::PUT,
        &format!("{base}/cards/{card}/labels/{bug}"),
        Some(json!({ "on": true })),
    )
    .await;
    req(
        &w.app,
        &w.dev,
        Method::PUT,
        &format!("{base}/cards/{card}/members/{}", w.dev.id),
        Some(json!({ "on": true })),
    )
    .await;
    req(
        &w.app,
        &w.dev,
        Method::PATCH,
        &format!("{base}/cards/{card}"),
        Some(json!({ "due_at": 1_900_000_000, "description_markdown": "Steps we run" })),
    )
    .await;
    let k = req(
        &w.app,
        &w.dev,
        Method::POST,
        &format!("{base}/cards/{card}/checklists"),
        Some(json!({ "title": "Before release" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let item = req(
        &w.app,
        &w.dev,
        Method::POST,
        &format!("{base}/checklists/{k}/items"),
        Some(json!({ "text": "Changelog written" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    req(
        &w.app,
        &w.dev,
        Method::PATCH,
        &format!("{base}/items/{item}"),
        Some(json!({ "done": true })),
    )
    .await;

    let mark = format!("{base}/cards/{card}/template");
    assert_eq!(
        call(
            &w.app,
            &w.viewer,
            Method::PUT,
            &mark,
            Some(json!({ "on": true }))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &w.app,
            &w.dev,
            Method::PUT,
            &mark,
            Some(json!({ "on": true }))
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    let offered = req(
        &w.app,
        &w.viewer,
        Method::GET,
        &format!("{base}/card-templates"),
        None,
    )
    .await;
    assert_eq!(offered["saved"][0]["title"], "Release checklist");
    assert_eq!(offered["builtin"].as_array().unwrap().len(), 3);
    let tile = req(&w.app, &w.dev, Method::GET, &base, None).await;
    assert_eq!(tile["lists"][0]["cards"][0]["is_template"], true);

    let use_it = format!("{base}/card-templates/use");
    let new = body_json(
        call(
            &w.app,
            &w.dev,
            Method::POST,
            &use_it,
            Some(json!({ "list_id": list, "card": card })),
        )
        .await,
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let made = req(
        &w.app,
        &w.dev,
        Method::GET,
        &format!("{base}/cards/{new}"),
        None,
    )
    .await;
    assert_eq!(made["title"], "Release checklist");
    assert_eq!(made["labels"], json!([bug]));
    assert_eq!(made["checklists"][0]["title"], "Before release");
    assert_eq!(made["checklists"][0]["items"][0]["done"], false);
    assert!(
        made["description"]
            .as_str()
            .unwrap()
            .contains("Steps we run")
    );
    assert_eq!(
        (
            made["members"].clone(),
            made["due_at"].clone(),
            made["is_template"].clone()
        ),
        (json!([]), Value::Null, json!(false))
    );

    let builtin = body_json(
        call(
            &w.app,
            &w.dev,
            Method::POST,
            &use_it,
            Some(json!({ "list_id": list, "builtin": "bug", "title": "Login fails" })),
        )
        .await,
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let b = req(
        &w.app,
        &w.dev,
        Method::GET,
        &format!("{base}/cards/{builtin}"),
        None,
    )
    .await;
    assert_eq!(
        (
            b["title"].as_str(),
            b["labels"].clone(),
            b["checklists"][0]["items"].as_array().unwrap().len()
        ),
        (Some("Login fails"), json!([bug]), 3)
    );
    assert_eq!(
        call(
            &w.app,
            &w.viewer,
            Method::POST,
            &use_it,
            Some(json!({ "list_id": list, "builtin": "bug" }))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    call(
        &w.app,
        &w.dev,
        Method::PUT,
        &mark,
        Some(json!({ "on": false })),
    )
    .await;
    assert!(
        req(
            &w.app,
            &w.dev,
            Method::GET,
            &format!("{base}/card-templates"),
            None
        )
        .await["saved"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        call(
            &w.app,
            &w.dev,
            Method::POST,
            &use_it,
            Some(json!({ "list_id": list, "card": card }))
        )
        .await
        .status(),
        StatusCode::NOT_FOUND,
        "an unmarked card is no longer offered"
    );
    let other = req(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{A}/projects/{}/boards", w.project_a),
        Some(json!({ "name": "Other" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    call(
        &w.app,
        &w.dev,
        Method::PUT,
        &mark,
        Some(json!({ "on": true })),
    )
    .await;
    let other_list = req(
        &w.app,
        &w.lead,
        Method::GET,
        &format!("{A}/boards/{other}"),
        None,
    )
    .await["lists"][0]["id"]
        .as_i64()
        .unwrap();
    let cross = call(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{A}/boards/{other}/card-templates/use"),
        Some(json!({ "list_id": other_list, "card": card })),
    )
    .await;
    assert_eq!(
        cross.status(),
        StatusCode::NOT_FOUND,
        "card templates belong to their board"
    );
}

#[tokio::test]
async fn only_manage_tokens_delete_templates() {
    let w = world().await;
    fill(&w).await;
    let saved = req(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{A}/boards/{}/save-template", w.board),
        Some(json!({ "name": "T" })),
    )
    .await;
    let id = saved["id"]
        .as_str()
        .unwrap()
        .trim_start_matches("c:")
        .to_string();
    let token = |scope: &str| {
        let body = json!({ "name": "t", "scope": scope });
        let app = &w.app;
        let admin = &w.admin;
        async move {
            body_json(call(app, admin, Method::POST, "/api/v1/me/tokens", Some(body)).await).await["token"].as_str().unwrap().to_string()
        }
    };
    let path = format!("{A}/templates/{id}");
    let send = |t: String| {
        let r = axum::http::Request::builder()
            .method(Method::DELETE)
            .uri(&path)
            .header(axum::http::header::AUTHORIZATION, format!("Bearer {t}"))
            .body(axum::body::Body::empty())
            .unwrap();
        w.app.send(r)
    };
    assert_eq!(
        send(token("write").await).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(token("manage").await).await.status(),
        StatusCode::NO_CONTENT
    );
}
