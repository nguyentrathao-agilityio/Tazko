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
    owner: Who,
    member: Who,
    observer: Who,
    outsider: Who,
    board: i64,
    other_board: i64,
    card: i64,
    other_card: i64,
}

async fn req(app: &TestApp, who: &Who, method: Method, path: &str, body: Option<Value>) -> Value {
    let res = call(app, who, method, path, body).await;
    serde_json::from_str(&body_text(res).await).unwrap_or(Value::Null)
}

async fn first_card(app: &TestApp, who: &Who, board: i64, title: &str) -> i64 {
    let list =
        req(app, who, Method::GET, &format!("{C}/boards/{board}"), None).await["lists"][0]["id"]
            .as_i64()
            .unwrap();
    req(
        app,
        who,
        Method::POST,
        &format!("{C}/boards/{board}/cards"),
        Some(json!({ "list_id": list, "title": title })),
    )
    .await["cards"][0]["id"]
        .as_i64()
        .unwrap()
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
    let project = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "P", "owner_id": owner.id })),
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
            &owner,
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
                &owner,
                Method::POST,
                &format!("{C}/projects/{project}/boards"),
                Some(json!({ "name": name })),
            )
            .await["id"]
                .as_i64()
                .unwrap(),
        );
    }
    let card = first_card(&app, &owner, boards[0], "Hero").await;
    let other_card = first_card(&app, &owner, boards[1], "Elsewhere").await;
    World {
        app,
        owner,
        member,
        observer,
        outsider,
        board: boards[0],
        other_board: boards[1],
        card,
        other_card,
    }
}

impl World {
    async fn upload_to(
        &self,
        who: &Who,
        board: i64,
        card: i64,
        name: &str,
        bytes: Vec<u8>,
    ) -> Response<Body> {
        let r = Request::builder()
            .method(Method::POST)
            .uri(format!(
                "{C}/boards/{board}/cards/{card}/attachments?name={}",
                form_urlencoded::byte_serialize(name.as_bytes()).collect::<String>()
            ))
            .header(header::COOKIE, &who.cookie)
            .header(tazko::auth::CSRF_HEADER, &who.csrf)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(bytes))
            .unwrap();
        self.app.send(r).await
    }

    async fn upload(&self, who: &Who, name: &str, bytes: &[u8]) -> Response<Body> {
        self.upload_to(who, self.board, self.card, name, bytes.to_vec())
            .await
    }

    async fn get(&self, who: &Who, path: &str) -> Response<Body> {
        self.app.get(path, Some(&who.cookie)).await
    }

    fn file_path(&self, hash: &str) -> std::path::PathBuf {
        self.app
            .state
            .dbs
            .layout()
            .orgs
            .join("company-a/files")
            .join(&hash[..2])
            .join(hash)
    }

    async fn hashes(&self) -> Vec<String> {
        self.app
            .state
            .dbs
            .tenant("company-a")
            .unwrap()
            .read(|c| {
                let mut s =
                    c.prepare("SELECT hash FROM attachments WHERE hash IS NOT NULL ORDER BY id")?;
                Ok(s.query_map([], |r| r.get(0))?
                    .collect::<Result<Vec<String>, _>>()?)
            })
            .await
            .unwrap()
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

#[tokio::test]
async fn the_same_file_is_stored_once_under_its_content_hash() {
    let w = world().await;
    let first = w.upload(&w.member, "hero.png", PNG).await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first = body_json(first).await;
    assert_eq!(first["name"], "hero.png");
    assert_eq!(first["image"], true);
    assert_eq!(first["size"], PNG.len());
    let again = body_json(w.upload(&w.member, "copy of hero.png", PNG).await).await;

    let hashes = w.hashes().await;
    assert_eq!(hashes.len(), 2);
    assert_eq!(hashes[0], hashes[1]);
    assert_eq!(hashes[0], tazko::files::store::hash(PNG));
    assert_eq!(std::fs::read(w.file_path(&hashes[0])).unwrap(), PNG);
    let stored = std::fs::read_dir(w.file_path(&hashes[0]).parent().unwrap())
        .unwrap()
        .count();
    assert_eq!(stored, 1);

    let card = req(
        &w.app,
        &w.observer,
        Method::GET,
        &format!("{C}/boards/{}/cards/{}", w.board, w.card),
        None,
    )
    .await;
    assert_eq!(card["attachments"].as_array().unwrap().len(), 2);
    let board = req(
        &w.app,
        &w.observer,
        Method::GET,
        &format!("{C}/boards/{}", w.board),
        None,
    )
    .await;
    assert_eq!(board["lists"][0]["cards"][0]["attachments"], 2);
    let found = req(
        &w.app,
        &w.observer,
        Method::GET,
        &format!("{C}/search?q=has:attachments"),
        None,
    )
    .await;
    assert_eq!(found["cards"].as_array().unwrap().len(), 1);

    let res = w
        .get(
            &w.observer,
            &format!("{C}/boards/{}/attachments/{}", w.board, first["id"]),
        )
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let h = res.headers().clone();
    assert_eq!(h[header::CONTENT_TYPE], "image/png");
    assert_eq!(h["x-content-type-options"], "nosniff");
    assert!(
        h[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("inline;")
    );
    assert!(
        h[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .contains("sandbox")
    );
    assert_eq!(body_bytes(res).await, PNG);
    let res = w
        .get(
            &w.observer,
            &format!(
                "{C}/boards/{}/attachments/{}?download=1",
                w.board, first["id"]
            ),
        )
        .await;
    assert!(
        res.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment;")
    );

    let del = |id: &Value| format!("{C}/boards/{}/attachments/{id}", w.board);
    assert_eq!(
        call(&w.app, &w.member, Method::DELETE, &del(&first["id"]), None)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    assert!(
        w.file_path(&hashes[0]).exists(),
        "still used by the other attachment"
    );
    call(&w.app, &w.member, Method::DELETE, &del(&again["id"]), None).await;
    assert!(
        !w.file_path(&hashes[0]).exists(),
        "the last one removes the file"
    );

    let kinds: Vec<String> = w
        .app
        .state
        .dbs
        .tenant("company-a")
        .unwrap()
        .read(|c| {
            let mut s =
                c.prepare("SELECT type FROM events WHERE type LIKE 'attachment_%' ORDER BY id")?;
            Ok(s.query_map([], |r| r.get(0))?
                .collect::<Result<Vec<String>, _>>()?)
        })
        .await
        .unwrap();
    assert_eq!(
        kinds,
        [
            "attachment_added",
            "attachment_added",
            "attachment_removed",
            "attachment_removed"
        ]
    );
}

async fn body_bytes(res: Response<Body>) -> Vec<u8> {
    use http_body_util::BodyExt;
    res.into_body().collect().await.unwrap().to_bytes().to_vec()
}

#[tokio::test]
async fn an_upload_past_the_size_limit_or_the_company_storage_is_refused() {
    let w = world().await;
    let mut big = PNG.to_vec();
    big.resize(10 * 1024 * 1024 + 1, 0);
    let (status, msg) = error(w.upload(&w.member, "huge.png", &big).await).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(msg, "files can be up to 10.0 MB; this one is larger");
    let mut fits = PNG.to_vec();
    fits.resize(10 * 1024 * 1024, 0);
    assert_eq!(
        w.upload(&w.member, "fits.png", &fits).await.status(),
        StatusCode::CREATED
    );

    let res = w.upload(&w.member, "first.pdf", PDF).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let used = body_json(w.get(&w.member, &format!("{C}/storage")).await).await["used"]
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
    let storage = body_json(w.get(&w.member, &format!("{C}/storage")).await).await;
    assert_eq!(storage["limit"], used + 10);
    assert_eq!(storage["max_file"], 10 * 1024 * 1024);

    let (status, msg) = error(w.upload(&w.member, "notes.txt", &[b'a'; 100]).await).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(msg.starts_with("this company has used "), "{msg}");
    assert!(msg.contains("of its "), "{msg}");
    assert_eq!(
        w.upload(&w.member, "same pdf again.pdf", PDF)
            .await
            .status(),
        StatusCode::CREATED,
        "a file that is already stored takes no new space"
    );
}

#[tokio::test]
async fn a_file_renamed_to_look_like_an_image_is_refused() {
    let w = world().await;
    let (status, msg) = error(
        w.upload(&w.member, "cat.png", b"MZ\x90\0\x03\0\0\0 program bytes")
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(msg.contains("named .png but it is not an image"), "{msg}");
    let (_, msg) = error(w.upload(&w.member, "cat.jpg", PDF).await).await;
    assert!(msg.contains("really a PDF"), "{msg}");
    let (status, msg) = error(
        w.upload(&w.member, "logo.svg", b"<svg onload=alert(1)/>")
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(msg.contains(".svg files cannot be uploaded"), "{msg}");
    let (_, msg) = error(
        w.upload(&w.member, "page.html", b"<script>x</script>")
            .await,
    )
    .await;
    assert!(msg.contains(".html files cannot be uploaded"), "{msg}");
    let (_, msg) = error(w.upload(&w.member, "tool.exe", b"MZ\x90\0").await).await;
    assert_eq!(msg, ".exe files cannot be uploaded");
    let (status, msg) = error(w.upload(&w.member, "empty.txt", b"").await).await;
    assert_eq!(
        (status, msg.as_str()),
        (StatusCode::BAD_REQUEST, "this file is empty")
    );
    assert!(w.hashes().await.is_empty());
    let files = w.app.state.dbs.layout().orgs.join("company-a/files");
    assert!(!files.exists() || std::fs::read_dir(files).unwrap().count() == 0);

    let res = body_json(w.upload(&w.member, "../../control.db.pdf", PDF).await).await;
    assert_eq!(res["name"], "control.db.pdf");
    let pdf = w
        .get(
            &w.member,
            &format!("{C}/boards/{}/attachments/{}", w.board, res["id"]),
        )
        .await;
    assert_eq!(pdf.headers()[header::CONTENT_TYPE], "application/pdf");
    assert!(
        pdf.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment;")
    );
}

#[tokio::test]
async fn attachments_follow_the_card_access_rules() {
    let w = world().await;
    assert_eq!(
        w.upload(&w.observer, "a.png", PNG).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        w.upload(&w.outsider, "a.png", PNG).await.status(),
        StatusCode::NOT_FOUND
    );
    let wrong_board = w
        .upload_to(&w.member, w.board, w.other_card, "a.png", PNG.to_vec())
        .await;
    assert_eq!(wrong_board.status(), StatusCode::NOT_FOUND);

    let a = body_json(w.upload(&w.member, "a.png", PNG).await).await["id"]
        .as_i64()
        .unwrap();
    let path = format!("{C}/boards/{}/attachments/{a}", w.board);
    assert_eq!(w.get(&w.observer, &path).await.status(), StatusCode::OK);
    assert_eq!(
        w.get(&w.outsider, &path).await.status(),
        StatusCode::NOT_FOUND
    );
    let through_other = format!("{C}/boards/{}/attachments/{a}", w.other_board);
    assert_eq!(
        w.get(&w.member, &through_other).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&w.app, &w.observer, Method::DELETE, &path, None)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let link = format!("{C}/boards/{}/cards/{}/attachments/link", w.board, w.card);
    let body = json!({ "url": "https://example.com/brief" });
    assert_eq!(
        call(&w.app, &w.observer, Method::POST, &link, Some(body))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );

    req(
        &w.app,
        &w.owner,
        Method::PUT,
        &format!("{C}/boards/{}/access", w.board),
        Some(json!({ "restricted": true, "people": [] })),
    )
    .await;
    assert_eq!(
        w.get(&w.member, &path).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        w.upload(&w.member, "b.png", PNG).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(w.get(&w.owner, &path).await.status(), StatusCode::OK);

    let no_csrf = Request::builder()
        .method(Method::POST)
        .uri(format!(
            "{C}/boards/{}/cards/{}/attachments?name=x.png",
            w.board, w.card
        ))
        .header(header::COOKIE, &w.owner.cookie)
        .body(Body::from(PNG.to_vec()))
        .unwrap();
    assert_eq!(w.app.send(no_csrf).await.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn links_and_image_covers() {
    let w = world().await;
    let link = format!("{C}/boards/{}/cards/{}/attachments/link", w.board, w.card);
    for bad in ["javascript:alert(1)", "ftp://x", "https://", "https://a b"] {
        let res = call(
            &w.app,
            &w.member,
            Method::POST,
            &link,
            Some(json!({ "url": bad })),
        )
        .await;
        assert_eq!(
            error(res).await,
            (
                StatusCode::BAD_REQUEST,
                "enter a link that starts with https://".into()
            ),
            "{bad}"
        );
    }
    let made = body_json(
        call(
            &w.app,
            &w.member,
            Method::POST,
            &link,
            Some(json!({ "url": "https://example.com/brand", "name": "Brand guide" })),
        )
        .await,
    )
    .await;
    assert_eq!(
        (made["kind"].as_str(), made["name"].as_str()),
        (Some("link"), Some("Brand guide"))
    );
    let unnamed = body_json(
        call(
            &w.app,
            &w.member,
            Method::POST,
            &link,
            Some(json!({ "url": "https://example.com/x" })),
        )
        .await,
    )
    .await;
    assert_eq!(unnamed["name"], "https://example.com/x");
    let link_path = format!("{C}/boards/{}/attachments/{}", w.board, made["id"]);
    assert_eq!(
        w.get(&w.member, &link_path).await.status(),
        StatusCode::NOT_FOUND
    );

    let img = body_json(w.upload(&w.member, "shot.png", PNG).await).await["id"]
        .as_i64()
        .unwrap();
    let pdf = body_json(w.upload(&w.member, "spec.pdf", PDF).await).await["id"]
        .as_i64()
        .unwrap();
    let cover = |id: i64| format!("{C}/boards/{}/attachments/{id}/cover", w.board);
    let card_path = format!("{C}/boards/{}/cards/{}", w.board, w.card);
    let (status, msg) = error(call(&w.app, &w.member, Method::PUT, &cover(pdf), None).await).await;
    assert_eq!(
        (status, msg.as_str()),
        (StatusCode::BAD_REQUEST, "only images can be a cover")
    );
    req(
        &w.app,
        &w.member,
        Method::PATCH,
        &card_path,
        Some(json!({ "cover": "#4B4FD8" })),
    )
    .await;
    assert_eq!(
        call(&w.app, &w.member, Method::PUT, &cover(img), None)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    let card = req(&w.app, &w.member, Method::GET, &card_path, None).await;
    assert_eq!(
        (card["cover_image"].as_i64(), card["cover"].clone()),
        (Some(img), Value::Null)
    );
    let board = req(
        &w.app,
        &w.member,
        Method::GET,
        &format!("{C}/boards/{}", w.board),
        None,
    )
    .await;
    assert_eq!(board["lists"][0]["cards"][0]["cover_image"], img);

    req(
        &w.app,
        &w.member,
        Method::PATCH,
        &card_path,
        Some(json!({ "cover": "#0F8F7E" })),
    )
    .await;
    assert_eq!(
        req(&w.app, &w.member, Method::GET, &card_path, None).await["cover_image"],
        Value::Null
    );
    call(&w.app, &w.member, Method::PUT, &cover(img), None).await;
    call(
        &w.app,
        &w.member,
        Method::DELETE,
        &format!("{C}/boards/{}/attachments/{img}", w.board),
        None,
    )
    .await;
    assert_eq!(
        req(&w.app, &w.member, Method::GET, &card_path, None).await["cover_image"],
        Value::Null
    );
}

#[tokio::test]
async fn files_nobody_refers_to_are_swept() {
    let w = world().await;
    let id = body_json(w.upload(&w.member, "a.png", PNG).await).await["id"]
        .as_i64()
        .unwrap();
    let hash = tazko::files::store::hash(PNG);
    let dir = w.app.state.dbs.layout().orgs.join("company-a");
    tazko::files::store::put(&dir, &tazko::files::store::hash(b"stray"), b"stray").unwrap();
    let db = w.app.state.dbs.tenant("company-a").unwrap();
    assert_eq!(tazko::files::sweep(&db, dir.clone()).await.unwrap(), 1);
    assert!(w.file_path(&hash).exists());
    db.write(move |c| {
        c.execute("DELETE FROM attachments WHERE id = ?1", [id])?;
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(tazko::files::sweep(&db, dir).await.unwrap(), 1);
    assert!(!w.file_path(&hash).exists());
}
