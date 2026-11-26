mod common;

use std::path::Path;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{TestApp, person};

const COMPANY_PREFIX: &str = "/api/v1/c/{company}";

fn sources(dir: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(std::fs::read_to_string(path).unwrap());
        }
    }
}

fn call_text(src: &str, open: usize) -> &str {
    let mut depth = 0;
    let mut in_str = false;
    let bytes = src.as_bytes();
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'"' if bytes[i - 1] != b'\\' => in_str = !in_str,
            b'(' if !in_str => depth += 1,
            b')' if !in_str => {
                depth -= 1;
                if depth == 0 {
                    return &src[open + 1..i];
                }
            }
            _ => {}
        }
    }
    panic!("unclosed route call");
}

fn quoted(text: &str) -> Option<&str> {
    let start = text.find('"')? + 1;
    let end = start + text[start..].find('"')?;
    Some(&text[start..end])
}

fn resolve(template: &str) -> String {
    template
        .replace("{COMPANY_PREFIX}", COMPANY_PREFIX)
        .replace("{{", "{")
        .replace("}}", "}")
}

fn write_routes() -> Vec<(Method, String)> {
    let mut files = vec![];
    sources(
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")),
        &mut files,
    );
    let mut routes = vec![];
    for src in &files {
        let prefix = src
            .find("let p = |s: &str| format!(")
            .and_then(|at| quoted(&src[at..]))
            .map(str::to_string);
        let mut from = 0;
        while let Some(found) = src[from..].find(".route(") {
            let open = from + found + ".route".len();
            let text = call_text(src, open);
            from = open + text.len();
            let (path_expr, methods) = text.split_once(',').expect("route has a handler");
            let path = if path_expr.trim() == "COMPANY_PREFIX" {
                COMPANY_PREFIX.to_string()
            } else if path_expr.contains("p(\"") {
                let template = prefix.as_deref().expect("route uses p() without a prefix");
                resolve(&template.replace("{s}", quoted(path_expr).unwrap()))
            } else if let Some(text) = quoted(path_expr) {
                resolve(text)
            } else {
                let name = path_expr.trim();
                let decl = format!("const {name}: &str = ");
                let at = src
                    .find(&decl)
                    .expect("route path constant is declared in its file");
                resolve(quoted(&src[at..]).unwrap())
            };
            for (name, method) in [
                ("post(", Method::POST),
                ("put(", Method::PUT),
                ("patch(", Method::PATCH),
                ("delete(", Method::DELETE),
            ] {
                let called = methods
                    .match_indices(name)
                    .any(|(i, _)| i == 0 || !methods.as_bytes()[i - 1].is_ascii_alphanumeric());
                if called {
                    routes.push((method, path.clone()));
                }
            }
        }
    }
    routes
}

fn concrete(path: &str) -> String {
    let mut out = String::new();
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let end = start + rest[start..].find('}').unwrap();
        out.push_str(if &rest[start..=end] == "{company}" {
            "acme"
        } else {
            "1"
        });
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

#[tokio::test]
async fn every_write_route_refuses_a_cookie_request_without_the_csrf_token() {
    let routes = write_routes();
    assert!(
        routes.len() > 100,
        "found only {} write routes; the route scan is out of date",
        routes.len()
    );

    let app = TestApp::new();
    let org = app.company("acme", "active").await;
    let me = person(&app, "owner@acme.example").await;
    app.member(org, me.id, "owner").await;
    let mine = me.id;
    app.sql(move |c| c.execute("UPDATE users SET is_super_admin = 1 WHERE id = ?1", [mine]))
        .await;

    let send = |method: &Method, path: &str, extra: Option<(&'static str, String)>| {
        let mut r = Request::builder()
            .method(method.clone())
            .uri(concrete(path))
            .header(header::COOKIE, &me.cookie)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some((name, value)) = extra {
            r = r.header(name, value);
        }
        app.send(r.body(Body::from("{}")).unwrap())
    };
    let mut open = vec![];
    for (method, path) in &routes {
        if tazko::auth::CSRF_EXEMPT.contains(&path.as_str()) {
            continue;
        }

        let without = send(method, path, None).await.status();
        let basic = send(method, path, Some(("authorization", "Basic eDp5".into())))
            .await
            .status();
        if without != StatusCode::FORBIDDEN || basic != StatusCode::FORBIDDEN {
            open.push(format!(
                "{method} {path} answered {without} without the token and {basic} with basic auth"
            ));
        }
    }
    assert!(open.is_empty(), "{open:#?}");

    let unknown = send(
        &Method::POST,
        "/api/v1/not-a-route",
        Some((tazko::auth::CSRF_HEADER, me.csrf.clone())),
    )
    .await
    .status();
    assert_eq!(unknown, StatusCode::METHOD_NOT_ALLOWED);
    for (method, path) in &routes {
        let with = send(
            method,
            path,
            Some((tazko::auth::CSRF_HEADER, me.csrf.clone())),
        )
        .await
        .status();
        assert_ne!(
            with,
            StatusCode::METHOD_NOT_ALLOWED,
            "{method} {path} is not a real route; the scan is wrong"
        );
    }
}
