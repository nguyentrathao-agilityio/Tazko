use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tazko::{http::router, web};
use tower::ServiceExt;

async fn get(path: &str, accept_br: bool) -> axum::response::Response {
    let mut req = Request::get(path);
    if accept_br {
        req = req.header(header::ACCEPT_ENCODING, "gzip, deflate, br");
    }
    router()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap()
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
    let res = router().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
}
