mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::TestApp;
use http_body_util::BodyExt;
use tazko::web;
use tower::ServiceExt;

async fn send(req: Request<Body>) -> axum::response::Response {
    let app = TestApp::new();
    app.router().oneshot(req).await.unwrap()
}

async fn get(path: &str, accept_br: bool) -> axum::response::Response {
    let mut req = Request::get(path);
    if accept_br {
        req = req.header(header::ACCEPT_ENCODING, "gzip, deflate, br");
    }
    send(req.body(Body::empty()).unwrap()).await
}

async fn text(res: axum::response::Response) -> String {
    String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap()
}

#[tokio::test]
async fn health_reports_ok_and_version() {
    let res = get("/health", false).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_str(&text(res).await).unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert!(body["sqlite"].as_str().unwrap().starts_with("3."));
}

#[tokio::test]
async fn root_serves_the_embedded_app() {
    let res = get("/", false).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        res.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert!(text(res).await.contains("<title>Tazko</title>"));
}

#[tokio::test]
async fn unknown_app_routes_fall_back_to_index() {
    let res = get("/admin/companies", false).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-cache");
    assert!(text(res).await.contains("<title>Tazko</title>"));
}

#[tokio::test]
async fn missing_files_and_api_routes_are_not_found() {
    assert_eq!(
        get("/assets/missing-123.js", false).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get("/robots-missing.txt", false).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get("/api/v1/nothing", false).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get("/auth/nothing", false).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn brotli_is_sent_only_when_accepted() {
    let res = get("/", true).await;
    assert_eq!(res.headers()[header::CONTENT_ENCODING], "br");
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(body.as_ref(), web::index().br);

    let plain = get("/", false).await;
    assert!(plain.headers().get(header::CONTENT_ENCODING).is_none());
}

#[tokio::test]
async fn hashed_assets_are_cached_for_a_year() {
    let Some(asset) = web::ASSETS.iter().find(|a| a.path.starts_with("/assets/")) else {
        return;
    };
    let res = get(asset.path, true).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
}

#[tokio::test]
async fn non_get_requests_to_the_app_are_refused() {
    let req = Request::post("/").body(Body::empty()).unwrap();
    let res = send(req).await;
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn every_response_carries_the_browser_security_headers() {
    let app = TestApp::new();
    let page = app.get("/", None).await;
    let policy = page.headers()[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap()
        .to_string();
    assert!(policy.contains("default-src 'self'"));
    assert!(policy.contains("script-src 'self'"));
    assert!(policy.contains("frame-ancestors 'none'"));
    assert!(policy.contains("object-src 'none'"));
    assert!(!policy.contains("unsafe-eval"));
    for res in [
        page,
        app.get("/api/v1/me", None).await,
        app.get("/health", None).await,
        app.get("/api/docs", None).await,
    ] {
        let h = res.headers();
        assert_eq!(h[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(h[header::REFERRER_POLICY], "same-origin");
        assert_eq!(h[header::X_FRAME_OPTIONS], "DENY");
        assert!(h.get(header::STRICT_TRANSPORT_SECURITY).is_none());
    }

    let api = app.get("/api/v1/me", None).await;
    assert!(api.headers().get(header::CONTENT_SECURITY_POLICY).is_none());
    let docs = app.get("/api/docs", None).await;
    let docs_policy = docs.headers()[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap();
    assert!(docs_policy.starts_with("default-src 'none'"));
    assert!(docs_policy.contains("frame-ancestors 'none'"));
}

#[tokio::test]
async fn https_servers_ask_browsers_to_stay_on_https() {
    let app = TestApp::with_env("TAZKO_PUBLIC_URL=https://tazko.example.com\n");
    let res = app.get("/health", None).await;
    assert_eq!(
        res.headers()[header::STRICT_TRANSPORT_SECURITY],
        "max-age=31536000"
    );
}
