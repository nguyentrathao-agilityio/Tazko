use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use serde_json::json;

use crate::state::AppState;
use crate::web::{self, Asset};

const RESERVED: [&str; 4] = ["/api/", "/auth/", "/cli/", "/ws"];

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(crate::api::routes())
        .merge(crate::auth::routes())
        .merge(crate::admin::routes())
        .merge(crate::company::routes())
        .merge(crate::projects::routes())
        .fallback(static_file)
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "sqlite": rusqlite::version(),
    }))
}

async fn static_file(method: Method, uri: Uri, headers: HeaderMap) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    let path = uri.path();
    if RESERVED.iter().any(|p| path.starts_with(p)) {
        return (StatusCode::NOT_FOUND, Json(json!({ "error": "not found" }))).into_response();
    }

    let lookup = if path == "/" { "/index.html" } else { path };
    match web::find(lookup) {
        Some(asset) => serve(asset, &headers, cache_for(lookup)),
        None if path.starts_with("/assets/") || has_extension(path) => {
            StatusCode::NOT_FOUND.into_response()
        }
        None => serve(web::index(), &headers, "no-cache"),
    }
}

fn cache_for(path: &str) -> &'static str {
    if path.starts_with("/assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

fn has_extension(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
}

fn accepts_brotli(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::ACCEPT_ENCODING)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|enc| {
            let mut parts = enc.trim().split(';');
            let name = parts.next().unwrap_or("").trim();
            let refused = parts.any(|q| matches!(q.trim(), "q=0" | "q=0.0" | "q=0.00" | "q=0.000"));
            name.eq_ignore_ascii_case("br") && !refused
        })
}

fn serve(asset: &'static Asset, headers: &HeaderMap, cache: &'static str) -> Response {
    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, asset.mime)
        .header(header::CACHE_CONTROL, cache)
        .header(header::VARY, HeaderValue::from_static("accept-encoding"))
        .header("x-content-type-options", "nosniff");

    let body = if accepts_brotli(headers) {
        builder = builder.header(header::CONTENT_ENCODING, "br");
        Body::from(asset.br)
    } else {
        Body::from(web::decompress(asset))
    };
    builder.body(body).expect("static response")
}
