use std::sync::LazyLock;

use axum::Router;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::Value;

use crate::state::AppState;

pub const OPENAPI: &str = include_str!("openapi.json");

static PAGE: LazyLock<String> = LazyLock::new(|| page(OPENAPI));

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/openapi.json", get(spec))
        .route("/api/docs", get(docs))
}

pub(crate) fn with_headers(body: String, kind: &'static str) -> Response {
    let mut res = body.into_response();
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(kind));
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    res
}

async fn spec() -> Response {
    with_headers(OPENAPI.to_string(), "application/json")
}

async fn docs() -> Response {
    with_headers(PAGE.clone(), "text/html; charset=utf-8")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn page(spec: &str) -> String {
    let doc: Value = serde_json::from_str(spec).unwrap_or(Value::Null);
    let info = &doc["info"];
    let mut rows = String::new();
    if let Some(paths) = doc["paths"].as_object() {
        for (path, ops) in paths {
            for (method, op) in ops.as_object().into_iter().flatten() {
                let body = op["requestBody"]["content"]["application/json"]["schema"]["$ref"]
                    .as_str()
                    .and_then(|r| r.rsplit('/').next())
                    .map(|name| {
                        let props = doc["components"]["schemas"][name]["properties"]
                            .as_object()
                            .map(|p| p.keys().cloned().collect::<Vec<_>>().join(", "))
                            .unwrap_or_default();
                        format!("<small>{}</small>", esc(&props))
                    })
                    .unwrap_or_default();
                rows.push_str(&format!(
                    "<tr><td><b class=\"m {m}\">{M}</b></td><td><code>{p}</code></td><td>{s}{b}</td><td>{sc}</td></tr>",
                    m = esc(method),
                    M = esc(&method.to_uppercase()),
                    p = esc(path),
                    s = esc(op["summary"].as_str().unwrap_or("")),
                    b = if body.is_empty() { String::new() } else { format!("<br>{body}") },
                    sc = esc(op["x-scope"].as_str().unwrap_or("")),
                ));
            }
        }
    }
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{t} reference</title><style>body{{font:15px/1.5 system-ui,sans-serif;margin:0 auto;max-width:1080px;padding:24px 16px;color:#1d1d2b;background:#f5f5fa}}h1{{margin:0 0 6px}}p{{color:#5e6a85;max-width:760px}}table{{border-collapse:collapse;width:100%;background:#fff;border-radius:12px;overflow:hidden}}td{{border-top:1px solid #e3e3ee;padding:8px 10px;vertical-align:top}}code{{font:13px ui-monospace,monospace}}small{{color:#5e6a85}}.m{{font:600 12px ui-monospace,monospace;padding:2px 6px;border-radius:5px;background:#eef}}.post{{background:#e3f6ec}}.patch,.put{{background:#fff3d6}}.delete{{background:#fde4e4}}@media(prefers-color-scheme:dark){{body{{background:#12121c;color:#e6e6f0}}table{{background:#1c1c2a}}td{{border-color:#2c2c3e}}.m{{background:#2a2a48}}}}</style></head><body><h1>{t}</h1><p>{d}</p><p><a href=\"/api/v1/openapi.json\">OpenAPI document</a></p><table>{rows}</table></body></html>",
        t = esc(info["title"].as_str().unwrap_or("API")),
        d = esc(info["description"].as_str().unwrap_or("")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_lists_every_documented_route() {
        let doc: Value = serde_json::from_str(OPENAPI).unwrap();
        let html = page(OPENAPI);
        for path in doc["paths"].as_object().unwrap().keys() {
            assert!(html.contains(&esc(path)), "{path}");
        }
        assert!(html.contains("Bearer"));
    }
}
