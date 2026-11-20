mod common;

use axum::body::Body;
use axum::http::{Method, Request, Response, StatusCode, header};
use common::{TestApp, Who, body_json, body_text, call, person};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";

struct World {
    app: TestApp,
    boss: Who,
    lead: Who,
    dev: Who,
    project_a: i64,
    project_b: i64,
    board_a: i64,
    board_b: i64,
    list_a: i64,
}

async fn req(app: &TestApp, who: &Who, method: Method, path: &str, body: Option<Value>) -> Value {
    let res = call(app, who, method, path, body).await;
    serde_json::from_str(&body_text(res).await).unwrap_or(Value::Null)
}

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let b = app.company("company-b", "active").await;
    let boss = person(&app, "boss@a.example").await;
    let lead = person(&app, "lead@a.example").await;
    let dev = person(&app, "dev@a.example").await;
    app.member(a, boss.id, "owner").await;
    app.member(b, lead.id, "owner").await;
    for w in [&lead, &dev] {
        app.member(a, w.id, "member").await;
    }
    let mut projects = Vec::new();
    for name in ["Alpha", "Beta"] {
        let p = req(
            &app,
            &boss,
            Method::POST,
            &format!("{C}/projects"),
            Some(json!({ "name": name, "owner_id": lead.id })),
        )
        .await["id"]
            .as_i64()
            .unwrap();
        req(
            &app,
            &lead,
            Method::POST,
            &format!("{C}/projects/{p}/invites"),
            Some(json!({ "email": "dev@a.example", "role": "member" })),
        )
        .await;
        projects.push(p);
    }
    let mut boards = Vec::new();
    for p in &projects {
        boards.push(
            req(
                &app,
                &lead,
                Method::POST,
                &format!("{C}/projects/{p}/boards"),
                Some(json!({ "name": "Board" })),
            )
            .await["id"]
                .as_i64()
                .unwrap(),
        );
    }
    let list_a = req(
        &app,
        &lead,
        Method::GET,
        &format!("{C}/boards/{}", boards[0]),
        None,
    )
    .await["lists"][0]["id"]
        .as_i64()
        .unwrap();
    World {
        app,
        boss,
        lead,
        dev,
        project_a: projects[0],
        project_b: projects[1],
        board_a: boards[0],
        board_b: boards[1],
        list_a,
    }
}

async fn with_token(
    app: &TestApp,
    token: &str,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Response<Body> {
    let r = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    let r = match body {
        Some(b) => r
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string())),
        None => r.body(Body::empty()),
    };
    app.send(r.unwrap()).await
}

async fn status(
    app: &TestApp,
    token: &str,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> StatusCode {
    with_token(app, token, method, path, body).await.status()
}

async fn personal(w: &World, who: &Who, scope: &str, projects: Option<Value>) -> String {
    let mut body = json!({ "name": "script", "scope": scope });
    if let Some(p) = projects {
        body["projects"] = p;
    }
    let res = call(&w.app, who, Method::POST, "/api/v1/me/tokens", Some(body)).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    body_json(res).await["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn a_project_limited_token_cannot_read_another_project() {
    let w = world().await;
    let token = personal(
        &w,
        &w.dev,
        "read",
        Some(json!([{ "company": "company-a", "project": w.project_a }])),
    )
    .await;
    assert_eq!(
        status(
            &w.app,
            &token,
            Method::GET,
            &format!("{C}/boards/{}", w.board_a),
            None
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        status(
            &w.app,
            &token,
            Method::GET,
            &format!("{C}/boards/{}", w.board_b),
            None
        )
        .await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(
            &w.app,
            &token,
            Method::GET,
            &format!("{C}/projects/{}/boards", w.project_b),
            None
        )
        .await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(
            &w.app,
            &token,
            Method::GET,
            &format!("{C}/projects/{}/members", w.project_b),
            None
        )
        .await,
        StatusCode::NOT_FOUND
    );
    for wide in [
        "/search?q=Board",
        "/notifications",
        "/starred",
        "/projects",
        "/storage",
    ] {
        assert_eq!(
            status(&w.app, &token, Method::GET, &format!("{C}{wide}"), None).await,
            StatusCode::FORBIDDEN,
            "{wide}"
        );
    }
    let mine = body_json(
        with_token(
            &w.app,
            &token,
            Method::GET,
            &format!("{C}/my-projects"),
            None,
        )
        .await,
    )
    .await;
    let names: Vec<&str> = mine["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Alpha"]);
    let me = body_json(with_token(&w.app, &token, Method::GET, "/api/v1/me", None).await).await;
    assert_eq!(me["companies"].as_array().unwrap().len(), 1);
    assert_eq!(me["csrf_token"], "");

    let company = {
        let res = call(
            &w.app,
            &w.lead,
            Method::POST,
            &format!("{C}/tokens"),
            Some(json!({ "name": "x", "scope": "read", "projects": [w.project_a] })),
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::FORBIDDEN,
            "only company owners make company tokens"
        );
        let res = call(
            &w.app,
            &w.boss,
            Method::POST,
            &format!("{C}/tokens"),
            Some(json!({ "name": "CI", "scope": "read", "projects": [w.project_a] })),
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::BAD_REQUEST,
            "the owner must belong to the projects"
        );
        req(
            &w.app,
            &w.lead,
            Method::POST,
            &format!("{C}/projects/{}/invites", w.project_a),
            Some(json!({ "email": "boss@a.example", "role": "member" })),
        )
        .await;
        let res = call(
            &w.app,
            &w.boss,
            Method::POST,
            &format!("{C}/tokens"),
            Some(json!({ "name": "CI", "scope": "read", "projects": [w.project_a] })),
        )
        .await;
        assert_eq!(res.status(), StatusCode::CREATED);
        body_json(res).await["token"].as_str().unwrap().to_string()
    };
    assert!(company.starts_with("tzk_c_company-a_"));
    let listed = req(&w.app, &w.boss, Method::GET, &format!("{C}/tokens"), None).await;
    assert_eq!(listed["tokens"][0]["name"], "CI");
    assert_eq!(
        listed["tokens"][0]["by"], "boss@a.example",
        "the list says who made each token"
    );
    assert!(
        !listed.to_string().contains(&company),
        "the secret is shown only once"
    );
    assert_eq!(
        status(
            &w.app,
            &company,
            Method::GET,
            &format!("{C}/boards/{}", w.board_a),
            None
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        status(
            &w.app,
            &company,
            Method::GET,
            &format!("{C}/boards/{}", w.board_b),
            None
        )
        .await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(
            &w.app,
            &company,
            Method::GET,
            "/api/v1/c/company-b/my-projects",
            None
        )
        .await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn scopes_limit_what_a_token_may_change() {
    let w = world().await;
    let read = personal(&w, &w.dev, "read", None).await;
    let write = personal(&w, &w.dev, "write", None).await;
    let card = json!({ "list_id": w.list_a, "title": "From a script" });
    let cards = format!("{C}/boards/{}/cards", w.board_a);
    assert_eq!(
        status(&w.app, &read, Method::POST, &cards, Some(card.clone())).await,
        StatusCode::FORBIDDEN
    );
    let made = body_json(with_token(&w.app, &write, Method::POST, &cards, Some(card)).await).await;
    let id = made["cards"][0]["id"].as_i64().unwrap();
    let comment = format!("{C}/boards/{}/cards/{id}/comments", w.board_a);
    assert_eq!(
        status(
            &w.app,
            &write,
            Method::POST,
            &comment,
            Some(json!({ "body": "done" }))
        )
        .await,
        StatusCode::CREATED
    );
    let new_board = format!("{C}/projects/{}/boards", w.project_a);
    assert_eq!(
        status(
            &w.app,
            &write,
            Method::POST,
            &new_board,
            Some(json!({ "name": "X" }))
        )
        .await,
        StatusCode::FORBIDDEN
    );
    let manage = personal(&w, &w.lead, "manage", None).await;
    assert_eq!(
        status(
            &w.app,
            &manage,
            Method::POST,
            &new_board,
            Some(json!({ "name": "X" }))
        )
        .await,
        StatusCode::CREATED
    );
    let dev_manage = personal(&w, &w.dev, "manage", None).await;
    let settings = format!("{C}/projects/{}", w.project_a);
    assert_eq!(
        status(
            &w.app,
            &dev_manage,
            Method::PATCH,
            &settings,
            Some(json!({ "name": "Renamed" }))
        )
        .await,
        StatusCode::FORBIDDEN,
        "a token never goes beyond its person"
    );
    assert_eq!(
        status(
            &w.app,
            &manage,
            Method::PATCH,
            &settings,
            Some(json!({ "name": "Renamed" }))
        )
        .await,
        StatusCode::NO_CONTENT
    );

    let boss = personal(&w, &w.boss, "manage", None).await;
    assert_eq!(
        status(&w.app, &boss, Method::GET, "/api/v1/admin/companies", None).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        status(&w.app, &write, Method::GET, "/api/v1/me/tokens", None).await,
        StatusCode::FORBIDDEN,
        "tokens cannot make tokens"
    );
    assert_eq!(
        status(
            &w.app,
            &write,
            Method::PATCH,
            "/api/v1/me/settings",
            Some(json!({ "theme": "dark" }))
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        status(
            &w.app,
            &write,
            Method::DELETE,
            "/api/v1/me/sessions/1",
            None
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        status(
            &w.app,
            &write,
            Method::GET,
            &format!("{C}/boards/{}/live", w.board_a),
            None
        )
        .await,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn tokens_are_stored_as_hashes_shown_once_and_revocable() {
    let w = world().await;
    let res = call(
        &w.app,
        &w.dev,
        Method::POST,
        "/api/v1/me/tokens",
        Some(json!({ "name": "laptop", "scope": "read", "expires_in_days": 30 })),
    )
    .await;
    let made = body_json(res).await;
    let token = made["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("tzk_p_") && token.len() > 60);
    let stored: Vec<String> = w
        .app
        .sql(|c| {
            let mut s = c.prepare("SELECT id_hash FROM personal_tokens")?;
            s.query_map([], |r| r.get(0))?
                .collect::<Result<Vec<String>, _>>()
        })
        .await;
    assert_eq!(stored, [tazko::session::hash_token(&token)]);
    let listed = req(&w.app, &w.dev, Method::GET, "/api/v1/me/tokens", None).await;
    assert_eq!(listed["tokens"][0]["name"], "laptop");
    assert!(
        listed["tokens"][0].get("token").is_none(),
        "the secret is shown only once"
    );
    assert!(!listed.to_string().contains(&token));
    let other = req(&w.app, &w.lead, Method::GET, "/api/v1/me/tokens", None).await;
    assert_eq!(other["tokens"].as_array().unwrap().len(), 0);

    let path = format!("/api/v1/me/tokens/{}", made["id"]);
    assert_eq!(
        call(&w.app, &w.lead, Method::DELETE, &path, None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(&w.app, &token, Method::GET, "/api/v1/me", None).await,
        StatusCode::OK
    );
    assert_eq!(
        call(&w.app, &w.dev, Method::DELETE, &path, None)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        status(&w.app, &token, Method::GET, "/api/v1/me", None).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(&w.app, "tzk_p_nonsense", Method::GET, "/api/v1/me", None).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(&w.app, "tzk_c_../../x_abc", Method::GET, "/api/v1/me", None).await,
        StatusCode::UNAUTHORIZED
    );

    let bad = call(&w.app, &w.dev, Method::POST, "/api/v1/me/tokens", Some(json!({ "name": "x", "scope": "read", "projects": [{ "company": "company-a", "project": 999 }] }))).await;
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn people_who_lose_access_lose_it_through_their_tokens() {
    let w = world().await;
    let token = personal(&w, &w.dev, "write", None).await;
    let board = format!("{C}/boards/{}", w.board_a);
    assert_eq!(
        status(&w.app, &token, Method::GET, &board, None).await,
        StatusCode::OK
    );
    req(
        &w.app,
        &w.lead,
        Method::DELETE,
        &format!("{C}/projects/{}/members/{}", w.project_a, w.dev.id),
        None,
    )
    .await;
    assert_eq!(
        status(&w.app, &token, Method::GET, &board, None).await,
        StatusCode::NOT_FOUND
    );
    req(
        &w.app,
        &w.lead,
        Method::PUT,
        &format!("{C}/boards/{}/access", w.board_b),
        Some(json!({ "restricted": true, "people": [] })),
    )
    .await;
    assert_eq!(
        status(
            &w.app,
            &token,
            Method::GET,
            &format!("{C}/boards/{}", w.board_b),
            None
        )
        .await,
        StatusCode::NOT_FOUND
    );

    req(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{C}/projects/{}/invites", w.project_a),
        Some(json!({ "email": "boss@a.example", "role": "member" })),
    )
    .await;
    let company = body_json(
        call(
            &w.app,
            &w.boss,
            Method::POST,
            &format!("{C}/tokens"),
            Some(json!({ "name": "CI", "scope": "read", "projects": [w.project_a] })),
        )
        .await,
    )
    .await["token"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        status(&w.app, &company, Method::GET, &board, None).await,
        StatusCode::OK
    );
    let boss = w.boss.id;
    w.app
        .sql(move |c| {
            c.execute(
                "UPDATE org_members SET role = 'member' WHERE user_id = ?1",
                [boss],
            )
        })
        .await;
    assert_eq!(
        status(&w.app, &company, Method::GET, &board, None).await,
        StatusCode::NOT_FOUND,
        "company tokens need their maker to stay an owner"
    );
    w.app
        .sql(move |c| c.execute("UPDATE users SET disabled = 1 WHERE id = ?1", [boss]))
        .await;
    assert_eq!(
        status(&w.app, &company, Method::GET, &board, None).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn a_token_over_its_rate_limit_gets_429() {
    let w = world().await;
    let token = personal(&w, &w.dev, "read", None).await;
    let path = format!("{C}/boards/{}", w.board_a);
    let mut limited = None;
    for _ in 0..(2 * tazko::api::tokens::RATE_PER_MINUTE + 2) {
        let res = with_token(&w.app, &token, Method::GET, &path, None).await;
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            limited = Some(res);
            break;
        }
        assert_eq!(res.status(), StatusCode::OK);
    }
    let res = limited.expect("a 429 within two minutes' worth of requests");
    assert!(res.headers().contains_key(header::RETRY_AFTER));
    assert!(
        body_json(res).await["error"]
            .as_str()
            .unwrap()
            .contains("too many requests")
    );
    let other = personal(&w, &w.lead, "read", None).await;
    assert_eq!(
        status(&w.app, &other, Method::GET, &path, None).await,
        StatusCode::OK,
        "limits are per token"
    );
}

#[tokio::test]
async fn a_bearer_header_never_falls_back_to_the_cookie_and_cookies_still_need_csrf() {
    let w = world().await;
    let cards = format!("{C}/boards/{}/cards", w.board_a);
    let r = Request::builder()
        .method(Method::POST)
        .uri(&cards)
        .header(header::COOKIE, &w.dev.cookie)
        .header(header::AUTHORIZATION, "Bearer tzk_p_wrong")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "list_id": w.list_a, "title": "x" }).to_string(),
        ))
        .unwrap();
    assert_eq!(w.app.send(r).await.status(), StatusCode::UNAUTHORIZED);
    let r = Request::builder()
        .method(Method::POST)
        .uri(&cards)
        .header(header::COOKIE, &w.dev.cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "list_id": w.list_a, "title": "x" }).to_string(),
        ))
        .unwrap();
    assert_eq!(w.app.send(r).await.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_api_reference_is_published() {
    let w = world().await;
    let spec = w.app.get("/api/v1/openapi.json", None).await;
    assert_eq!(spec.status(), StatusCode::OK);
    let spec = body_json(spec).await;
    assert_eq!(spec["openapi"], "3.1.0");
    assert!(
        spec["paths"]
            .get("/api/v1/c/{company}/boards/{board}/cards")
            .is_some()
    );
    let page = w.app.get("/api/docs", None).await;
    assert_eq!(page.status(), StatusCode::OK);
    assert!(
        page.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .contains("default-src 'none'")
    );
    assert!(body_text(page).await.contains("Tazko API"));
}

#[tokio::test]
async fn a_project_limited_owner_token_cannot_manage_other_projects() {
    let w = world().await;
    req(
        &w.app,
        &w.lead,
        Method::POST,
        &format!("{C}/projects/{}/invites", w.project_a),
        Some(json!({ "email": "boss@a.example", "role": "owner" })),
    )
    .await;
    let token = personal(
        &w,
        &w.boss,
        "manage",
        Some(json!([{ "company": "company-a", "project": w.project_a }])),
    )
    .await;
    for (method, path) in [
        (
            Method::POST,
            format!("{C}/projects/{}/archive", w.project_b),
        ),
        (
            Method::POST,
            format!("{C}/projects/{}/archive", w.project_a),
        ),
        (Method::DELETE, format!("{C}/projects/{}", w.project_b)),
        (Method::POST, format!("{C}/projects/{}/owners", w.project_b)),
        (Method::POST, format!("{C}/projects")),
    ] {
        let s = status(
            &w.app,
            &token,
            method.clone(),
            &path,
            Some(json!({ "user_id": w.boss.id, "name": "X", "owner_id": w.boss.id })),
        )
        .await;
        assert!(
            s == StatusCode::FORBIDDEN || s == StatusCode::NOT_FOUND,
            "{method} {path} gave {s}"
        );
    }
    let list = req(&w.app, &w.boss, Method::GET, &format!("{C}/projects"), None).await;
    let projects = list["projects"].as_array().unwrap();
    assert_eq!(projects.len(), 2, "{list}");
    assert!(projects.iter().all(|p| p["status"] == "active"), "{list}");
    let board_b = format!("{C}/boards/{}/watch", w.board_b);
    assert_eq!(
        status(
            &w.app,
            &token,
            Method::PUT,
            &board_b,
            Some(json!({ "on": true }))
        )
        .await,
        StatusCode::NOT_FOUND
    );
}
