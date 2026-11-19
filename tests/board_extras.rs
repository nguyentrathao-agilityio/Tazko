mod common;

use axum::body::Body;
use axum::http::{Method, Request, Response, StatusCode, header};
use common::{TestApp, Who, body_json, body_text, call, person};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";
const PDF: &[u8] = b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\n";

struct World {
    app: TestApp,
    lead: Who,
    dev: Who,
    viewer: Who,
    plain: Who,
    board: i64,
    other: i64,
    card: i64,
}

async fn req(app: &TestApp, who: &Who, method: Method, path: &str, body: Option<Value>) -> Value {
    let res = call(app, who, method, path, body).await;
    serde_json::from_str(&body_text(res).await).unwrap_or(Value::Null)
}

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let boss = person(&app, "boss@a.example").await;
    let lead = person(&app, "lead@a.example").await;
    let dev = person(&app, "dev@a.example").await;
    let viewer = person(&app, "viewer@a.example").await;
    let plain = person(&app, "plain@a.example").await;
    app.member(a, boss.id, "owner").await;
    for w in [&lead, &dev, &viewer, &plain] {
        app.member(a, w.id, "member").await;
    }
    let project = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "P", "owner_id": lead.id })),
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
            &lead,
            Method::POST,
            &format!("{C}/projects/{project}/invites"),
            Some(json!({ "email": e, "role": role })),
        )
        .await;
    }
    let mut boards = Vec::new();
    for name in ["B", "Other"] {
        boards.push(
            req(
                &app,
                &lead,
                Method::POST,
                &format!("{C}/projects/{project}/boards"),
                Some(json!({ "name": name })),
            )
            .await["id"]
                .as_i64()
                .unwrap(),
        );
    }
    let list = req(
        &app,
        &lead,
        Method::GET,
        &format!("{C}/boards/{}", boards[0]),
        None,
    )
    .await["lists"][0]["id"]
        .as_i64()
        .unwrap();
    let card = req(
        &app,
        &lead,
        Method::POST,
        &format!("{C}/boards/{}/cards", boards[0]),
        Some(json!({ "list_id": list, "title": "Hero" })),
    )
    .await["cards"][0]["id"]
        .as_i64()
        .unwrap();
    World {
        app,
        lead,
        dev,
        viewer,
        plain,
        board: boards[0],
        other: boards[1],
        card,
    }
}

impl World {
    async fn upload(&self, who: &Who, board: i64, name: &str, bytes: Vec<u8>) -> Response<Body> {
        let r = Request::builder()
            .method(Method::POST)
            .uri(format!(
                "{C}/boards/{board}/background?name={}",
                form_urlencoded::byte_serialize(name.as_bytes()).collect::<String>()
            ))
            .header(header::COOKIE, &who.cookie)
            .header(tazko::auth::CSRF_HEADER, &who.csrf)
            .body(Body::from(bytes))
            .unwrap();
        self.app.send(r).await
    }

    async fn board(&self, who: &Who) -> Value {
        req(
            &self.app,
            who,
            Method::GET,
            &format!("{C}/boards/{}", self.board),
            None,
        )
        .await
    }
}

async fn error(res: Response<Body>) -> (StatusCode, String) {
    let status = res.status();
    (
        status,
        body_json(res).await["error"]
            .as_str()
            .unwrap_or("")
            .to_string(),
    )
}

fn png_of(len: usize) -> Vec<u8> {
    let mut v = PNG.to_vec();
    v.resize(len, 0);
    v
}

#[tokio::test]
async fn a_bad_or_oversized_background_is_refused_clearly() {
    let w = world().await;
    let (status, msg) = error(
        w.upload(&w.dev, w.board, "sky.png", png_of(5 * 1024 * 1024 + 1))
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        msg,
        "background pictures can be up to 5.0 MB; this one is larger"
    );
    let (status, msg) = error(
        w.upload(
            &w.dev,
            w.board,
            "sky.png",
            b"MZ\x90\0 not a picture".to_vec(),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(msg.contains("named .png but it is not an image"), "{msg}");
    let (status, msg) = error(w.upload(&w.dev, w.board, "brief.pdf", PDF.to_vec()).await).await;
    assert_eq!(
        (status, msg.as_str()),
        (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "choose a PNG, JPEG, GIF or WebP picture for the background"
        )
    );
    let (status, msg) = error(
        w.upload(&w.dev, w.board, "logo.svg", b"<svg/>".to_vec())
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(msg.contains(".svg files cannot be uploaded"), "{msg}");
    assert_eq!(
        error(w.upload(&w.dev, w.board, "e.png", Vec::new()).await)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        w.upload(&w.viewer, w.board, "sky.png", PNG.to_vec())
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        w.upload(&w.plain, w.board, "sky.png", PNG.to_vec())
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        w.board(&w.dev).await["background"],
        "dusk",
        "nothing changed"
    );

    let ok = w
        .upload(&w.dev, w.board, "sky.png", png_of(5 * 1024 * 1024))
        .await;
    assert_eq!(ok.status(), StatusCode::OK);
    let bg = body_json(ok).await["background"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(bg.starts_with("img:"));
    assert_eq!(w.board(&w.viewer).await["background"], bg);

    let used = body_json(
        w.app
            .get(&format!("{C}/storage"), Some(&w.dev.cookie))
            .await,
    )
    .await["used"]
        .as_u64()
        .unwrap();
    w.app
        .sql(move |c| {
            c.execute(
                "UPDATE orgs SET storage_limit = ?1 WHERE slug = 'company-a'",
                [used as i64 + 10],
            )
        })
        .await;
    let (status, msg) = error(w.upload(&w.dev, w.board, "other.png", png_of(2000)).await).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(msg.starts_with("this company has used"), "{msg}");
}

#[tokio::test]
async fn uploaded_backgrounds_are_served_only_to_people_who_see_the_board() {
    let w = world().await;
    let bg =
        body_json(w.upload(&w.dev, w.board, "sky.png", PNG.to_vec()).await).await["background"]
            .as_str()
            .unwrap()
            .to_string();
    let image = bg.trim_start_matches("img:");
    let path = format!("{C}/boards/{}/background/{image}", w.board);
    let res = w.app.get(&path, Some(&w.viewer.cookie)).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(res.headers()["x-content-type-options"], "nosniff");
    assert!(
        res.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("inline;")
    );
    assert_eq!(
        w.app.get(&path, Some(&w.plain.cookie)).await.status(),
        StatusCode::NOT_FOUND
    );
    let through_other = format!("{C}/boards/{}/background/{image}", w.other);
    assert_eq!(
        w.app
            .get(&through_other, Some(&w.dev.cookie))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );

    let other = format!("{C}/boards/{}", w.other);
    let (status, msg) = error(
        call(
            &w.app,
            &w.dev,
            Method::PATCH,
            &other,
            Some(json!({ "background": bg })),
        )
        .await,
    )
    .await;
    assert_eq!(
        (status, msg.as_str()),
        (StatusCode::BAD_REQUEST, "choose one of the backgrounds")
    );

    let copy = req(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{C}/boards/{}/copy", w.board),
        Some(json!({ "name": "Copy" })),
    )
    .await;
    let copied = req(
        &w.app,
        &w.lead,
        Method::GET,
        &format!("{C}/boards/{}", copy["id"]),
        None,
    )
    .await;
    assert_eq!(
        copied["background"], "dusk",
        "a copy does not borrow another board's picture"
    );

    req(
        &w.app,
        &w.dev,
        Method::PATCH,
        &format!("{C}/boards/{}", w.board),
        Some(json!({ "background": "sea" })),
    )
    .await;
    assert_eq!(
        w.app.get(&path, Some(&w.dev.cookie)).await.status(),
        StatusCode::NOT_FOUND
    );
    let dir = w.app.state.dbs.layout().orgs.join("company-a");
    let db = w.app.state.dbs.tenant("company-a").unwrap();
    assert_eq!(
        tazko::files::sweep(&db, dir).await.unwrap(),
        1,
        "the unused picture is cleaned up"
    );
}

#[tokio::test]
async fn custom_fields_have_typed_values_and_follow_edit_rights() {
    let w = world().await;
    let fields = format!("{C}/boards/{}/fields", w.board);
    let viewer_try = call(
        &w.app,
        &w.viewer,
        Method::POST,
        &fields,
        Some(json!({ "name": "Priority", "type": "select", "options": ["Low"] })),
    )
    .await;
    assert_eq!(viewer_try.status(), StatusCode::FORBIDDEN);
    let bad = error(
        call(
            &w.app,
            &w.dev,
            Method::POST,
            &fields,
            Some(json!({ "name": "X", "type": "formula" })),
        )
        .await,
    )
    .await;
    assert_eq!(bad.0, StatusCode::BAD_REQUEST);
    let bad = error(
        call(
            &w.app,
            &w.dev,
            Method::POST,
            &fields,
            Some(json!({ "name": "X", "type": "select", "options": [] })),
        )
        .await,
    )
    .await;
    assert_eq!(bad.1, "a dropdown needs at least one option");

    let prio = req(&w.app, &w.dev, Method::POST, &fields, Some(json!({ "name": "Priority", "type": "select", "options": ["Low", "High", "low"], "show_on_card": true }))).await;
    assert_eq!(prio["options"], json!(["Low", "High"]));
    let points = req(
        &w.app,
        &w.dev,
        Method::POST,
        &fields,
        Some(json!({ "name": "Points", "type": "number" })),
    )
    .await;
    let ok = req(
        &w.app,
        &w.dev,
        Method::POST,
        &fields,
        Some(json!({ "name": "Signed off", "type": "check", "show_on_card": true })),
    )
    .await;
    let value = |f: &Value| format!("{C}/boards/{}/cards/{}/fields/{}", w.board, w.card, f["id"]);
    assert_eq!(
        error(
            call(
                &w.app,
                &w.dev,
                Method::PUT,
                &value(&prio),
                Some(json!({ "value": "Medium" }))
            )
            .await
        )
        .await
        .1,
        "choose one of the options"
    );
    assert_eq!(
        error(
            call(
                &w.app,
                &w.dev,
                Method::PUT,
                &value(&points),
                Some(json!({ "value": "lots" }))
            )
            .await
        )
        .await
        .1,
        "enter a number"
    );
    assert_eq!(
        call(
            &w.app,
            &w.viewer,
            Method::PUT,
            &value(&points),
            Some(json!({ "value": 3 }))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    req(
        &w.app,
        &w.dev,
        Method::PUT,
        &value(&prio),
        Some(json!({ "value": "High" })),
    )
    .await;
    req(
        &w.app,
        &w.dev,
        Method::PUT,
        &value(&points),
        Some(json!({ "value": 5 })),
    )
    .await;
    req(
        &w.app,
        &w.dev,
        Method::PUT,
        &value(&ok),
        Some(json!({ "value": true })),
    )
    .await;

    let card = req(
        &w.app,
        &w.viewer,
        Method::GET,
        &format!("{C}/boards/{}/cards/{}", w.board, w.card),
        None,
    )
    .await;
    assert_eq!(card["fields"][prio["id"].to_string()], "High");
    assert_eq!(card["fields"][points["id"].to_string()], 5.0);
    let board = w.board(&w.viewer).await;
    assert_eq!(board["fields"].as_array().unwrap().len(), 3);
    let front = &board["lists"][0]["cards"][0]["fields"];
    assert_eq!(front[prio["id"].to_string()], "High");
    assert_eq!(front[ok["id"].to_string()], true);
    assert_eq!(
        front[points["id"].to_string()],
        5.0,
        "all values come with the board so it can filter by them"
    );

    req(
        &w.app,
        &w.dev,
        Method::PATCH,
        &format!("{fields}/{}", prio["id"]),
        Some(json!({ "options": ["Low"] })),
    )
    .await;
    let card = req(
        &w.app,
        &w.dev,
        Method::GET,
        &format!("{C}/boards/{}/cards/{}", w.board, w.card),
        None,
    )
    .await;
    assert!(
        card["fields"].get(prio["id"].to_string()).is_none(),
        "a removed option clears its values"
    );
    let other_field = format!("{C}/boards/{}/fields/{}", w.other, points["id"]);
    assert_eq!(
        call(&w.app, &w.dev, Method::DELETE, &other_field, None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &w.app,
            &w.dev,
            Method::DELETE,
            &format!("{fields}/{}", points["id"]),
            None
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    for i in 0..18 {
        req(
            &w.app,
            &w.dev,
            Method::POST,
            &fields,
            Some(json!({ "name": format!("F{i}"), "type": "text" })),
        )
        .await;
    }
    let full = error(
        call(
            &w.app,
            &w.dev,
            Method::POST,
            &fields,
            Some(json!({ "name": "One more", "type": "text" })),
        )
        .await,
    )
    .await;
    assert_eq!(full.1, "a board can have up to 20 custom fields");
}

#[tokio::test]
async fn reactions_on_comments() {
    let w = world().await;
    let comment = req(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{C}/boards/{}/cards/{}/comments", w.board, w.card),
        Some(json!({ "body": "Ship it" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let path = format!("{C}/boards/{}/comments/{comment}/reactions", w.board);
    assert_eq!(
        error(
            call(
                &w.app,
                &w.dev,
                Method::PUT,
                &path,
                Some(json!({ "emoji": "💩", "on": true }))
            )
            .await
        )
        .await
        .1,
        "choose one of the reactions"
    );
    assert_eq!(
        call(
            &w.app,
            &w.viewer,
            Method::PUT,
            &path,
            Some(json!({ "emoji": "👍", "on": true }))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &w.app,
            &w.plain,
            Method::PUT,
            &path,
            Some(json!({ "emoji": "👍", "on": true }))
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let wrong = format!("{C}/boards/{}/comments/{comment}/reactions", w.other);
    assert_eq!(
        call(
            &w.app,
            &w.dev,
            Method::PUT,
            &wrong,
            Some(json!({ "emoji": "👍", "on": true }))
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    req(
        &w.app,
        &w.dev,
        Method::PUT,
        &path,
        Some(json!({ "emoji": "👍", "on": true })),
    )
    .await;
    req(
        &w.app,
        &w.dev,
        Method::PUT,
        &path,
        Some(json!({ "emoji": "👍", "on": true })),
    )
    .await;
    let r = req(
        &w.app,
        &w.lead,
        Method::PUT,
        &path,
        Some(json!({ "emoji": "👍", "on": true })),
    )
    .await;
    assert_eq!(r["reactions"][0]["count"], 2);
    req(
        &w.app,
        &w.lead,
        Method::PUT,
        &path,
        Some(json!({ "emoji": "🎉", "on": true })),
    )
    .await;
    req(
        &w.app,
        &w.lead,
        Method::PUT,
        &path,
        Some(json!({ "emoji": "🎉", "on": false })),
    )
    .await;
    let card = req(
        &w.app,
        &w.viewer,
        Method::GET,
        &format!("{C}/boards/{}/cards/{}", w.board, w.card),
        None,
    )
    .await;
    let rx = &card["comments"][0]["reactions"];
    assert_eq!(rx.as_array().unwrap().len(), 1);
    assert_eq!(
        (
            rx[0]["emoji"].as_str(),
            rx[0]["count"].as_i64(),
            rx[0]["mine"].as_bool()
        ),
        (Some("👍"), Some(2), Some(false))
    );
    let mine = req(
        &w.app,
        &w.dev,
        Method::GET,
        &format!("{C}/boards/{}/cards/{}", w.board, w.card),
        None,
    )
    .await;
    assert_eq!(mine["comments"][0]["reactions"][0]["mine"], true);
}
