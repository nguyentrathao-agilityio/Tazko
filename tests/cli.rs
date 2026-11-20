mod common;

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::{TestApp, Who, body_text, call, person};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";

async fn post(app: &TestApp, path: &str, body: Value) -> (StatusCode, Value) {
    let res = app
        .send(
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await;
    let status = res.status();
    (
        status,
        serde_json::from_str(&body_text(res).await).unwrap_or(Value::Null),
    )
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
    (
        status,
        serde_json::from_str(&body_text(res).await).unwrap_or(Value::Null),
    )
}

async fn world() -> (TestApp, Who) {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let mai = person(&app, "mai@a.example").await;
    app.member(a, mai.id, "owner").await;
    (app, mai)
}

#[tokio::test]
async fn the_device_code_flow_hands_out_one_token_after_approval() {
    let (app, mai) = world().await;
    let (status, start) = post(
        &app,
        "/api/v1/cli/device",
        json!({ "client": "laptop\n(macos)" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let code = start["user_code"].as_str().unwrap().to_string();
    let device = start["device_code"].as_str().unwrap().to_string();
    assert_eq!(
        start["verification_uri_complete"],
        format!("http://localhost:8791/cli/authorize?code={code}")
    );

    assert_eq!(
        post(&app, "/api/v1/cli/token", json!({ "device_code": device }))
            .await
            .0,
        StatusCode::PRECONDITION_REQUIRED
    );
    assert_eq!(
        post(&app, "/api/v1/cli/token", json!({ "device_code": device }))
            .await
            .1["error"],
        "slow_down"
    );
    let (_, seen) = req(
        &app,
        &mai,
        Method::GET,
        &format!(
            "/api/v1/cli/device/{}",
            code.to_lowercase().replace('-', "%20")
        ),
        None,
    )
    .await;
    assert_eq!(
        (seen["client"].clone(), seen["status"].clone()),
        (json!("laptop(macos)"), json!("waiting"))
    );

    let anon = app
        .send(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/v1/cli/device/{code}/approve"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(anon.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        req(
            &app,
            &mai,
            Method::POST,
            &format!("/api/v1/cli/device/{code}/approve"),
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (status, got) = post(&app, "/api/v1/cli/token", json!({ "device_code": device })).await;
    assert_eq!(status, StatusCode::OK);
    let token = got["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("tzk_p_"));
    assert_eq!(got["email"], "mai@a.example");
    assert_eq!(
        post(&app, "/api/v1/cli/token", json!({ "device_code": device }))
            .await
            .0,
        StatusCode::GONE,
        "the token is handed out once"
    );

    let me = app
        .send(
            Request::builder()
                .uri("/api/v1/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(me.status(), StatusCode::OK);
    let (_, tokens) = req(&app, &mai, Method::GET, "/api/v1/me/tokens", None).await;
    assert_eq!(tokens["tokens"][0]["name"], "Command line on laptop(macos)");
    assert_eq!(tokens["tokens"][0]["scope"], "write");
    let approve_with_token = app
        .send(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/v1/cli/device/{code}/approve"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(
        approve_with_token.status(),
        StatusCode::FORBIDDEN,
        "a token cannot approve another sign-in"
    );
}

#[tokio::test]
async fn denied_expired_and_unknown_codes_give_no_token() {
    let (app, mai) = world().await;
    let start = post(&app, "/api/v1/cli/device", json!({})).await.1;
    let (code, device) = (
        start["user_code"].as_str().unwrap().to_string(),
        start["device_code"].as_str().unwrap().to_string(),
    );
    assert_eq!(
        req(
            &app,
            &mai,
            Method::POST,
            &format!("/api/v1/cli/device/{code}/deny"),
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        post(&app, "/api/v1/cli/token", json!({ "device_code": device }))
            .await
            .1["error"],
        "access_denied"
    );
    assert_eq!(
        req(
            &app,
            &mai,
            Method::POST,
            &format!("/api/v1/cli/device/{code}/approve"),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );

    let start = post(&app, "/api/v1/cli/device", json!({})).await.1;
    let device = start["device_code"].as_str().unwrap().to_string();
    app.sql(|c| c.execute("UPDATE device_codes SET expires_at = 1", []))
        .await;
    assert_eq!(
        post(&app, "/api/v1/cli/token", json!({ "device_code": device }))
            .await
            .0,
        StatusCode::GONE
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/cli/token",
            json!({ "device_code": "made-up" })
        )
        .await
        .0,
        StatusCode::GONE
    );
    assert_eq!(
        req(
            &app,
            &mai,
            Method::GET,
            "/api/v1/cli/device/ZZZZ-ZZZZ",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    let script = app.get("/cli/install.sh", None).await;
    assert_eq!(script.status(), StatusCode::OK);
    let text = body_text(script).await;
    assert!(text.contains("host=\"http://localhost:8791\"") && !text.contains("__TAZKO_"));
}

fn tazko(dir: &std::path::Path, host: &str, args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_tazko"))
        .args(args)
        .env("TAZKO_CONFIG_DIR", dir)
        .env("TAZKO_HOST", host)
        .env_remove("TAZKO_TOKEN")
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn the_command_line_signs_in_through_the_browser_and_makes_a_card() {
    let (app, mai) = world().await;
    let project = req(
        &app,
        &mai,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "Web", "owner_id": mai.id })),
    )
    .await
    .1["id"]
        .as_i64()
        .unwrap();
    let board = req(
        &app,
        &mai,
        Method::POST,
        &format!("{C}/projects/{project}/boards"),
        Some(json!({ "name": "Sprint 14" })),
    )
    .await
    .1["id"]
        .as_i64()
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let host = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let router = app.router();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();

    let (ok, _, err) = tazko(dir.path(), &host, &["board", "list"]);
    assert!(!ok && err.contains("not signed in"), "{err}");

    let mut child = Command::new(env!("CARGO_BIN_EXE_tazko"))
        .args(["auth", "login", "--host", &host, "--no-browser"])
        .env("TAZKO_CONFIG_DIR", dir.path())
        .env_remove("TAZKO_TOKEN")
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stderr.take().unwrap());
    let mut first = String::new();
    lines.read_line(&mut first).unwrap();
    let code = first.trim().rsplit(' ').next().unwrap().to_string();
    assert_eq!(code.len(), 9, "{first}");
    assert_eq!(
        req(
            &app,
            &mai,
            Method::POST,
            &format!("/api/v1/cli/device/{code}/approve"),
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let status = tokio::task::spawn_blocking(move || child.wait().unwrap())
        .await
        .unwrap();
    assert!(status.success());
    let token_file = dir.path().join("token");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&token_file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    let (ok, out, _) = tazko(dir.path(), &host, &["auth", "status"]);
    assert!(
        ok && out.contains("mai@a.example") && !out.contains("tzk_"),
        "{out}"
    );
    let (ok, out, err) = tazko(
        dir.path(),
        &host,
        &[
            "card",
            "create",
            "--board",
            "Sprint 14",
            "--title",
            "Fix footer links",
        ],
    );
    assert!(ok, "{err}");
    assert!(out.contains("Fix footer links"));
    let (ok, out, _) = tazko(
        dir.path(),
        &host,
        &[
            "card",
            "list",
            "--board",
            &board.to_string(),
            "--json",
            "title",
        ],
    );
    assert!(ok);
    assert_eq!(
        serde_json::from_str::<Value>(&out).unwrap(),
        json!([{ "title": "Fix footer links" }])
    );
    let (ok, out, _) = tazko(dir.path(), &host, &["api", "GET", "/me", "--jq", ".email"]);
    assert_eq!((ok, out.trim()), (true, "mai@a.example"));
    let (ok, _, err) = tazko(
        dir.path(),
        &host,
        &["card", "move", "1/999", "--to", "Nowhere"],
    );
    assert!(!ok && !err.is_empty());

    let (ok, _, _) = tazko(dir.path(), &host, &["auth", "logout"]);
    assert!(ok && !token_file.exists());
}
