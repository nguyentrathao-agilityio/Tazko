mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{TestApp, Who, body_json, body_text, call, person};
use serde_json::{Value, json};

const A: &str = "/api/v1/c/company-a";
const B: &str = "/api/v1/c/company-b";

async fn req(app: &TestApp, who: &Who, method: Method, path: &str, body: Option<Value>) -> Value {
    let res = call(app, who, method.clone(), path, body).await;
    assert!(
        res.status().is_success(),
        "{method} {path} -> {}",
        res.status()
    );
    serde_json::from_str(&body_text(res).await).unwrap_or(Value::Null)
}

async fn send_file(
    app: &TestApp,
    who: &Who,
    path: &str,
    file: &[u8],
    people: Option<Value>,
) -> (StatusCode, Value) {
    let mut r = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::COOKIE, &who.cookie)
        .header(tazko::auth::CSRF_HEADER, &who.csrf)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(p) = people {
        r = r.header("tazko-people", p.to_string());
    }
    let res = app.send(r.body(Body::from(file.to_vec())).unwrap()).await;
    let status = res.status();
    (
        status,
        serde_json::from_str(&body_text(res).await).unwrap_or(Value::Null),
    )
}

async fn import(
    app: &TestApp,
    who: &Who,
    path: &str,
    file: &[u8],
    choices: Value,
) -> (StatusCode, Value) {
    let mut head = choices.to_string();
    head.pop();
    let sep = if head.len() > 1 { "," } else { "" };
    let mut body = format!("{head}{sep}\"file\":").into_bytes();
    body.extend_from_slice(file);
    body.push(b'}');
    send_file(app, who, path, &body, None).await
}

async fn project(app: &TestApp, owner: &Who, base: &str, name: &str) -> i64 {
    req(
        app,
        owner,
        Method::POST,
        &format!("{base}/projects"),
        Some(json!({ "name": name, "owner_id": owner.id })),
    )
    .await["id"]
        .as_i64()
        .unwrap()
}

async fn add_to_project(
    app: &TestApp,
    owner: &Who,
    base: &str,
    project: i64,
    email: &str,
    role: &str,
) {
    req(
        app,
        owner,
        Method::POST,
        &format!("{base}/projects/{project}/invites"),
        Some(json!({ "email": email, "role": role })),
    )
    .await;
}

fn sample() -> Value {
    let people = json!([
        { "key": "p1", "name": "Mai Tran", "email": "mai@a.example" },
        { "key": "p2", "name": "Dev Lee", "email": "dev@a.example" },
        { "key": "p3", "name": "Former Intern", "email": "intern@gone.example" }
    ]);
    let labels = json!([
        { "key": "l1", "name": "Bug", "color": 3 },
        { "key": "l2", "name": "Design", "color": 2 },
        { "key": "l3", "name": "Backend", "color": 1 }
    ]);
    let titles = ["Ideas", "This week", "Doing", "Waiting on client", "Done"];
    let mut n = 0;
    let lists: Vec<Value> = titles
        .iter()
        .enumerate()
        .map(|(li, t)| {
            let count = if li == 0 { 28 } else { 24 };
            let cards: Vec<Value> = (0..count)
                .map(|_| {
                    n += 1;
                    json!({
                        "title": format!("Card {n}"),
                        "description_html": format!("<p>About <b>card {n}</b></p>"),
                        "labels": if n % 3 == 0 { json!(["l1", "l3"]) } else { json!(["l2"]) },
                        "members": if n % 2 == 0 { json!(["p1"]) } else { json!(["p2", "p3"]) },
                        "start_at": if n % 4 == 0 { json!(1_790_000_000 + n) } else { Value::Null },
                        "due_at": 1_795_000_000 + n * 3600,
                        "done": n % 5 == 0,
                        "archived": n % 11 == 0,
                        "checklists": [{ "title": "Steps", "items": [
                            { "text": "First", "done": true, "assignee": "p1", "due_at": 1_795_100_000 },
                            { "text": "Second", "done": false, "assignee": "p3", "due_at": null }
                        ] }],
                        "comments": [
                            { "author": "p2", "author_name": "Dev Lee", "text": format!("Note {n}"), "created_at": 1_780_000_000 + n },
                            { "author": "p3", "author_name": "Former Intern", "text": "Old note", "created_at": 1_780_000_500 + n }
                        ],
                        "links": [{ "name": "Spec", "url": format!("https://example.com/spec/{n}") }],
                        "files": []
                    })
                })
                .collect();
            json!({ "title": t, "archived": false, "cards": cards })
        })
        .collect();
    json!({
        "format": "tazko-board-export", "version": 1, "exported_at": 1_795_000_000,
        "board": { "name": "Marketing Q4", "description": "Quarter plan", "background": "dusk" },
        "labels": labels, "people": people, "lists": lists
    })
}

fn normalise(export: &Value) -> Value {
    let label_name = |k: &Value| {
        export["labels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["key"] == *k)
            .map(|l| l["name"].clone())
            .unwrap_or(Value::Null)
    };
    let email = |k: &Value| {
        export["people"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["key"] == *k)
            .map(|p| p["email"].clone())
            .unwrap_or(Value::Null)
    };
    let mut lists = export["lists"].clone();
    for list in lists.as_array_mut().unwrap() {
        for card in list["cards"].as_array_mut().unwrap() {
            card["labels"] = card["labels"]
                .as_array()
                .unwrap()
                .iter()
                .map(label_name)
                .collect();
            card["members"] = card["members"]
                .as_array()
                .unwrap()
                .iter()
                .map(email)
                .collect();
            for k in card["checklists"].as_array_mut().unwrap() {
                for i in k["items"].as_array_mut().unwrap() {
                    i["assignee"] = email(&i["assignee"]);
                }
            }
            for m in card["comments"].as_array_mut().unwrap() {
                m["author"] = email(&m["author"]);
                if !m["author"].is_null() {
                    m["author_name"] = Value::Null;
                }
            }
        }
    }
    json!({
        "name": export["board"]["name"],
        "description": export["board"]["description"],
        "labels": export["labels"].as_array().unwrap().iter().map(|l| json!([l["name"], l["color"]])).collect::<Vec<_>>(),
        "lists": lists,
    })
}

async fn export_of(app: &TestApp, who: &Who, base: &str, board: i64) -> Value {
    let res = call(
        app,
        who,
        Method::GET,
        &format!("{base}/boards/{board}/export"),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        res.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment; filename=\"tazko-"),
    );
    body_json(res).await
}

#[tokio::test]
async fn a_124_card_export_round_trips_into_another_company() {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let b = app.company("company-b", "active").await;
    let mai = person(&app, "mai@a.example").await;
    let dev = person(&app, "dev@a.example").await;
    for org in [a, b] {
        app.member(org, mai.id, "owner").await;
        app.member(org, dev.id, "member").await;
    }
    let pa = project(&app, &mai, A, "Web").await;
    add_to_project(&app, &mai, A, pa, "dev@a.example", "member").await;
    let file = sample().to_string();

    let (status, preview) = send_file(
        &app,
        &mai,
        &format!("{A}/projects/{pa}/imports/preview?name=marketing-q4.json"),
        file.as_bytes(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(
        preview["counts"]["cards"].as_i64().unwrap()
            + preview["counts"]["archived"].as_i64().unwrap(),
        124
    );
    assert_eq!(
        preview["lists"],
        json!(["Ideas", "This week", "Doing", "Waiting on client", "Done"])
    );
    let matches: Vec<Value> = preview["people"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["match"].clone())
        .collect();
    assert_eq!(
        matches,
        [json!(mai.id), json!(dev.id), Value::Null],
        "matched by email, the intern is not here"
    );

    let (status, made) = import(
        &app,
        &mai,
        &format!("{A}/projects/{pa}/imports"),
        file.as_bytes(),
        json!({ "name": "marketing-q4.json" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{made}");
    assert_eq!(
        made["counts"]["cards"].as_i64().unwrap() + made["counts"]["archived"].as_i64().unwrap(),
        124
    );
    let first = export_of(&app, &mai, A, made["id"].as_i64().unwrap()).await;

    let mut expected = normalise(&sample());
    for list in expected["lists"].as_array_mut().unwrap() {
        for card in list["cards"].as_array_mut().unwrap() {
            card["members"] = card["members"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|m| m != &&json!("intern@gone.example"))
                .cloned()
                .collect();
            for k in card["checklists"].as_array_mut().unwrap() {
                for i in k["items"].as_array_mut().unwrap() {
                    if i["assignee"] == "intern@gone.example" {
                        i["assignee"] = Value::Null;
                    }
                }
            }
            for m in card["comments"].as_array_mut().unwrap() {
                if m["author"] == "intern@gone.example" {
                    m["author"] = Value::Null;
                    m["author_name"] = json!("Former Intern");
                }
            }
        }
    }
    assert_eq!(
        normalise(&first),
        expected,
        "lists, labels, checklists, dates, comments and people survive"
    );

    let pb = project(&app, &mai, B, "Copy").await;
    add_to_project(&app, &mai, B, pb, "dev@a.example", "member").await;
    let (status, again) = import(
        &app,
        &mai,
        &format!("{B}/projects/{pb}/imports"),
        first.to_string().as_bytes(),
        json!({ "name": "again.json" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{again}");
    let second = export_of(&app, &mai, B, again["id"].as_i64().unwrap()).await;
    assert_eq!(
        normalise(&second),
        normalise(&first),
        "exporting again and importing into another company gives the same board"
    );

    let board = again["id"].as_i64().unwrap();
    let view = req(
        &app,
        &mai,
        Method::GET,
        &format!("{B}/boards/{board}"),
        None,
    )
    .await;
    let card = view["lists"][0]["cards"][0]["id"].as_i64().unwrap();
    let detail = req(
        &app,
        &mai,
        Method::GET,
        &format!("{B}/boards/{board}/cards/{card}"),
        None,
    )
    .await;
    let authors: Vec<Value> = detail["comments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["author"]["name"].clone())
        .collect();
    assert!(
        detail["comments"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["imported"] == true),
        "imported comments are marked as imported"
    );
    assert!(
        authors.contains(&json!("Former Intern")),
        "an unmatched author shows by name: {authors:?}"
    );

    let activity = req(
        &app,
        &mai,
        Method::GET,
        &format!("{B}/boards/{board}/activity"),
        None,
    )
    .await;
    assert!(activity.to_string().contains("board_imported"));
    let audit = app
        .sql(|c| {
            c.query_row(
                "SELECT count(*) FROM audit_log WHERE action = 'board_imported'",
                [],
                |r| r.get::<_, i64>(0),
            )
        })
        .await;
    assert_eq!(audit, 2);
}

#[tokio::test]
async fn generic_files_match_people_and_options_leave_things_out() {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let mai = person(&app, "mai@a.example").await;
    app.member(a, mai.id, "owner").await;
    let pa = project(&app, &mai, A, "Web").await;
    let file = json!({
        "name": "From elsewhere",
        "labels": [{ "id": "x1", "name": "Bug", "color": "red_dark" }],
        "lists": [{ "id": "L1", "name": "Ideas", "pos": 1 }],
        "cards": [
            { "id": "c1", "name": "Open card", "idList": "L1", "pos": 1, "idMembers": ["m1"], "idLabels": ["x1"],
              "attachments": [{ "name": "Doc", "url": "https://example.com/doc" }] },
            { "id": "c2", "name": "Archived card", "idList": "L1", "pos": 2, "closed": true }
        ],
        "members": [{ "id": "m1", "fullName": "MAI TRAN", "username": "mai" }],
        "actions": [{ "type": "commentCard", "date": "2026-01-02T03:04:05Z", "idMemberCreator": "m1",
                      "data": { "text": "Hello", "card": { "id": "c1" } } }]
    })
    .to_string();
    req(
        &app,
        &mai,
        Method::PATCH,
        "/api/v1/me/profile",
        Some(json!({ "name": "Mai Tran" })),
    )
    .await;

    let (status, preview) = send_file(
        &app,
        &mai,
        &format!("{A}/projects/{pa}/imports/preview"),
        file.as_bytes(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["format"], "generic");
    assert_eq!(
        preview["people"][0]["match"],
        json!(mai.id),
        "matched by name when there is no email"
    );

    let (status, made) = import(
        &app,
        &mai,
        &format!("{A}/projects/{pa}/imports"),
        file.as_bytes(),
        json!({ "archived": false, "comments": false, "links": false }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{made}");
    assert_eq!(made["counts"]["archived"], 0);
    let board = made["id"].as_i64().unwrap();
    let view = req(
        &app,
        &mai,
        Method::GET,
        &format!("{A}/boards/{board}"),
        None,
    )
    .await;
    let cards = view["lists"][0]["cards"].as_array().unwrap();
    assert_eq!(cards.len(), 1);
    let card = cards[0]["id"].as_i64().unwrap();
    let detail = req(
        &app,
        &mai,
        Method::GET,
        &format!("{A}/boards/{board}/cards/{card}"),
        None,
    )
    .await;
    assert_eq!(detail["members"], json!([mai.id]));
    assert_eq!(detail["comments"], json!([]));
    assert_eq!(detail["attachments"], json!([]));

    let (status, _) = import(
        &app,
        &mai,
        &format!("{A}/projects/{pa}/imports"),
        file.as_bytes(),
        json!({ "people": { "m1": 999_999 } }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "people must be members of the project"
    );
}

#[tokio::test]
async fn only_people_who_can_create_boards_import_and_exports_follow_board_access() {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let mai = person(&app, "mai@a.example").await;
    let obs = person(&app, "obs@a.example").await;
    let out = person(&app, "out@a.example").await;
    for (who, role) in [(&mai, "owner"), (&obs, "member"), (&out, "member")] {
        app.member(a, who.id, role).await;
    }
    let pa = project(&app, &mai, A, "Web").await;
    add_to_project(&app, &mai, A, pa, "obs@a.example", "observer").await;
    let file = sample().to_string();

    let (s, _) = import(
        &app,
        &obs,
        &format!("{A}/projects/{pa}/imports"),
        file.as_bytes(),
        json!({}),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = send_file(
        &app,
        &obs,
        &format!("{A}/projects/{pa}/imports/preview"),
        file.as_bytes(),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = import(
        &app,
        &out,
        &format!("{A}/projects/{pa}/imports"),
        file.as_bytes(),
        json!({}),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let hidden = req(
        &app,
        &mai,
        Method::POST,
        &format!("{A}/projects/{pa}/boards"),
        Some(json!({ "name": "Hidden", "restricted": true })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let res = call(
        &app,
        &obs,
        Method::GET,
        &format!("{A}/boards/{hidden}/export"),
        None,
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "a restricted board is not exported to people who cannot see it"
    );
    let res = call(
        &app,
        &out,
        Method::GET,
        &format!("{A}/boards/{hidden}/export"),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let (s, e) = import(
        &app,
        &mai,
        &format!("{A}/projects/{pa}/imports"),
        b"{\"hello\": 1}",
        json!({}),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(e["error"].as_str().unwrap().contains("not a board export"));
    let big = vec![b' '; 53 * 1024 * 1024];
    let (s, _) = send_file(
        &app,
        &mai,
        &format!("{A}/projects/{pa}/imports/preview"),
        &big,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn cards_in_an_archived_list_keep_their_own_state() {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let mai = person(&app, "mai@a.example").await;
    app.member(a, mai.id, "owner").await;
    let pa = project(&app, &mai, A, "Web").await;
    let file = json!({
        "format": "tazko-board-export", "version": 1,
        "board": { "name": "Old work" },
        "lists": [
            { "title": "Now", "cards": [{ "title": "Open" }] },
            { "title": "Old", "archived": true, "cards": [{ "title": "Kept open" }, { "title": "Gone", "archived": true }] }
        ]
    })
    .to_string();
    let (status, made) = import(
        &app,
        &mai,
        &format!("{A}/projects/{pa}/imports"),
        file.as_bytes(),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{made}");
    let board = made["id"].as_i64().unwrap();
    let out = export_of(&app, &mai, A, board).await;
    let rows: Vec<(String, bool, bool)> = out["lists"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|l| {
            l["cards"].as_array().unwrap().iter().map(move |c| {
                (
                    c["title"].as_str().unwrap().to_string(),
                    c["archived"] == true,
                    l["archived"] == true,
                )
            })
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("Open".to_string(), false, false),
            ("Kept open".to_string(), false, true),
            ("Gone".to_string(), true, true)
        ]
    );
}
