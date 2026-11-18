mod common;

use std::process::Command;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use axum::response::Response;
use common::{TestApp, body_json};
use rusqlite::params;
use serde_json::{Value, json};
use tazko::admin::trash;
use tazko::auth::CSRF_HEADER;

struct Admin {
    app: TestApp,
    cookie: String,
    csrf: String,
    id: i64,
}

async fn admin() -> Admin {
    let app = TestApp::new();
    let id = app.user("root@example.com").await;
    app.sql(move |c| c.execute("UPDATE users SET is_super_admin = 1 WHERE id = ?1", [id]))
        .await;
    let cookie = app.sign_in(id).await;
    let csrf = body_json(app.get("/api/v1/me", Some(&cookie)).await).await["csrf_token"]
        .as_str()
        .unwrap()
        .to_string();
    Admin {
        app,
        cookie,
        csrf,
        id,
    }
}

impl Admin {
    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> Response {
        self.call_as(&self.cookie, Some(&self.csrf), method, path, body)
            .await
    }

    async fn call_as(
        &self,
        cookie: &str,
        csrf: Option<&str>,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Response {
        let mut r = Request::builder()
            .method(method)
            .uri(path)
            .header(header::COOKIE, cookie);
        if let Some(t) = csrf {
            r = r.header(CSRF_HEADER, t);
        }
        let req = match body {
            Some(b) => r
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string())),
            None => r.body(Body::empty()),
        };
        self.app.send(req.unwrap()).await
    }

    async fn audit(&self) -> Vec<(String, String)> {
        self.app
            .sql(|c| {
                let mut s =
                    c.prepare("SELECT action, coalesce(target, '') FROM audit_log ORDER BY id")?;
                let r = s
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(r)
            })
            .await
    }

    async fn new_actions(&self, before: usize) -> Vec<String> {
        self.audit()
            .await
            .into_iter()
            .skip(before)
            .map(|(a, _)| a)
            .collect()
    }
}

async fn ok(res: Response) {
    let status = res.status();
    assert!(
        status.is_success(),
        "{status}: {}",
        common::body_text(res).await
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_admin_action_writes_an_audit_row() {
    let a = admin().await;
    let mut seen = Vec::new();
    let mut check = Vec::new();
    let n = a.audit().await.len();
    let res = a
        .call(
            Method::POST,
            "/api/v1/admin/companies",
            Some(json!({ "name": "Acme Studio", "owner_email": "Owner@Acme.Example" })),
        )
        .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    check.push((
        a.new_actions(n).await,
        vec!["company_created", "invite_created"],
    ));

    let n = a.audit().await.len();
    ok(a.call(
        Method::PATCH,
        "/api/v1/admin/companies/acme-studio",
        Some(json!({ "name": "Acme", "storage_gb": 5 })),
    )
    .await)
    .await;
    check.push((a.new_actions(n).await, vec!["company_updated"]));

    let n = a.audit().await.len();
    let res = a
        .call(
            Method::POST,
            "/api/v1/admin/companies/acme-studio/invites",
            Some(json!({ "email": "mai@acme.example", "role": "member" })),
        )
        .await;
    assert_eq!(body_json(res).await["result"], "invited");
    check.push((a.new_actions(n).await, vec!["invite_created"]));

    let invite: i64 = a
        .app
        .sql(|c| {
            c.query_row(
                "SELECT id FROM invites WHERE email = 'mai@acme.example'",
                [],
                |r| r.get(0),
            )
        })
        .await;
    let n = a.audit().await.len();
    ok(a.call(
        Method::DELETE,
        &format!("/api/v1/admin/invites/{invite}"),
        None,
    )
    .await)
    .await;
    check.push((a.new_actions(n).await, vec!["invite_revoked"]));

    let jo = a.app.user("jo@gmail.com").await;
    let keeper = a.app.user("keeper@acme.example").await;
    let org: i64 = a
        .app
        .sql(|c| {
            c.query_row("SELECT id FROM orgs WHERE slug = 'acme-studio'", [], |r| {
                r.get(0)
            })
        })
        .await;
    a.app.member(org, keeper, "owner").await;
    let n = a.audit().await.len();
    let res = a
        .call(
            Method::POST,
            "/api/v1/admin/companies/acme-studio/invites",
            Some(json!({ "email": "jo@gmail.com", "role": "member" })),
        )
        .await;
    assert_eq!(body_json(res).await["result"], "added");
    check.push((a.new_actions(n).await, vec!["invite_created"]));

    let n = a.audit().await.len();
    ok(a.call(
        Method::PATCH,
        &format!("/api/v1/admin/companies/acme-studio/members/{jo}"),
        Some(json!({ "role": "owner" })),
    )
    .await)
    .await;
    check.push((a.new_actions(n).await, vec!["role_changed"]));

    let n = a.audit().await.len();
    ok(a.call(
        Method::DELETE,
        &format!("/api/v1/admin/companies/acme-studio/members/{jo}"),
        None,
    )
    .await)
    .await;
    check.push((a.new_actions(n).await, vec!["member_removed"]));

    for (path, action) in [
        ("suspend", "company_suspended"),
        ("resume", "company_resumed"),
    ] {
        let n = a.audit().await.len();
        a.call(
            Method::POST,
            &format!("/api/v1/admin/companies/acme-studio/{path}"),
            None,
        )
        .await;
        check.push((a.new_actions(n).await, vec![action]));
    }

    for (body, action) in [
        (json!({ "is_super_admin": true }), "super_admin_granted"),
        (json!({ "is_super_admin": false }), "super_admin_revoked"),
        (json!({ "disabled": true }), "person_disabled"),
        (json!({ "disabled": false }), "person_enabled"),
    ] {
        let n = a.audit().await.len();
        a.call(
            Method::PATCH,
            &format!("/api/v1/admin/people/{jo}"),
            Some(body),
        )
        .await;
        check.push((a.new_actions(n).await, vec![action]));
    }

    let n = a.audit().await.len();
    ok(a.call(
        Method::DELETE,
        "/api/v1/admin/companies/acme-studio",
        Some(json!({ "confirm": "acme-studio" })),
    )
    .await)
    .await;
    check.push((a.new_actions(n).await, vec!["company_deleted"]));

    for (got, want) in &check {
        assert_eq!(got, want);
        seen.extend(got.iter().cloned());
    }

    let actor_less: i64 = a
        .app
        .sql(|c| {
            c.query_row(
                "SELECT count(*) FROM audit_log WHERE actor_id IS NULL",
                [],
                |r| r.get(0),
            )
        })
        .await;
    assert_eq!(actor_less, 0, "every admin action names who did it");

    for kind in ["company", "invites", "people"] {
        for action in tazko::admin::audit::actions_of(kind).unwrap() {
            assert!(
                seen.iter().any(|s| s == action),
                "{action} was never exercised"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn admin_writes_need_the_csrf_token_and_leave_no_trace_without_it() {
    let a = admin().await;
    for csrf in [None, Some("wrong")] {
        let res = a
            .call_as(
                &a.cookie,
                csrf,
                Method::POST,
                "/api/v1/admin/companies",
                Some(json!({ "name": "X", "owner_email": "x@example.com" })),
            )
            .await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }
    assert!(a.audit().await.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn people_who_are_not_super_admins_cannot_see_the_console() {
    let a = admin().await;
    ok(a.call(
        Method::POST,
        "/api/v1/admin/companies",
        Some(json!({ "name": "Acme", "owner_email": "o@acme.example" })),
    )
    .await)
    .await;
    let member = a.app.user("member@example.com").await;
    let org: i64 = a
        .app
        .sql(|c| c.query_row("SELECT id FROM orgs", [], |r| r.get(0)))
        .await;
    a.app.member(org, member, "owner").await;
    let cookie = a.app.sign_in(member).await;
    let csrf = body_json(a.app.get("/api/v1/me", Some(&cookie)).await).await["csrf_token"]
        .as_str()
        .unwrap()
        .to_string();

    for path in [
        "/api/v1/admin/companies",
        "/api/v1/admin/companies/acme",
        "/api/v1/admin/people",
        "/api/v1/admin/invites",
        "/api/v1/admin/audit",
        "/api/v1/admin/server",
    ] {
        assert_eq!(
            a.app.get(path, Some(&cookie)).await.status(),
            StatusCode::NOT_FOUND,
            "{path}"
        );
        assert_eq!(
            a.app.get(path, None).await.status(),
            StatusCode::UNAUTHORIZED,
            "{path}"
        );
    }

    for (method, path, body) in [
        (
            Method::POST,
            "/api/v1/admin/companies",
            json!({ "name": "Y", "owner_email": "y@example.com" }),
        ),
        (
            Method::POST,
            "/api/v1/admin/companies/acme/suspend",
            json!({}),
        ),
        (
            Method::DELETE,
            "/api/v1/admin/companies/acme",
            json!({ "confirm": "acme" }),
        ),
        (
            Method::PATCH,
            &format!("/api/v1/admin/people/{member}"),
            json!({ "is_super_admin": true }),
        ),
    ] {
        let res = a
            .call_as(&cookie, Some(&csrf), method, path, Some(body))
            .await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{path}");
    }
    let sup: bool = a
        .app
        .sql(move |c| {
            c.query_row(
                "SELECT is_super_admin FROM users WHERE id = ?1",
                [member],
                |r| r.get(0),
            )
        })
        .await;
    assert!(!sup);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn creating_a_company_makes_its_folder_and_invites_the_first_owner() {
    let a = admin().await;
    let res = a
        .call(
            Method::POST,
            "/api/v1/admin/companies",
            Some(json!({ "name": "Acme Studio", "slug": "acme", "owner_email": "owner@acme.example", "storage_gb": 3 })),
        )
        .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    assert!(a.app.dir.path().join("data/orgs/acme/tazko.db").is_file());

    let list = body_json(a.app.get("/api/v1/admin/companies", Some(&a.cookie)).await).await;
    let c = &list["companies"][0];
    assert_eq!(
        (
            c["slug"].as_str(),
            c["status"].as_str(),
            c["boards"].as_i64()
        ),
        (Some("acme"), Some("active"), Some(0))
    );
    assert_eq!(c["storage_limit"].as_i64(), Some(3 * 1024 * 1024 * 1024));
    assert!(c["storage_used"].as_i64().unwrap() > 0);

    let detail = body_json(
        a.app
            .get("/api/v1/admin/companies/acme", Some(&a.cookie))
            .await,
    )
    .await;
    assert_eq!(detail["invites"][0]["email"], "owner@acme.example");
    assert_eq!(detail["invites"][0]["role"], "owner");
    assert_eq!(detail["invites"][0]["invited_by"], "root@example.com");
    assert_eq!(detail["projects"], json!([]));

    let again = a
        .call(
            Method::POST,
            "/api/v1/admin/companies",
            Some(json!({ "name": "Other", "slug": "acme", "owner_email": "x@example.com" })),
        )
        .await;
    assert_eq!(again.status(), StatusCode::CONFLICT);

    for body in [
        json!({ "name": "", "owner_email": "x@example.com" }),
        json!({ "name": "X", "slug": "../etc", "owner_email": "x@example.com" }),
        json!({ "name": "X", "owner_email": "not-an-email" }),
        json!({ "name": "X", "owner_email": "x@example.com", "storage_gb": 0 }),
    ] {
        let res = a
            .call(Method::POST, "/api/v1/admin/companies", Some(body.clone()))
            .await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{body}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deleting_a_company_needs_its_short_name_and_moves_it_to_trash() {
    let a = admin().await;
    ok(a.call(
        Method::POST,
        "/api/v1/admin/companies",
        Some(json!({ "name": "Acme", "owner_email": "o@acme.example" })),
    )
    .await)
    .await;
    let member = a.app.user("m@acme.example").await;
    let org: i64 = a
        .app
        .sql(|c| c.query_row("SELECT id FROM orgs", [], |r| r.get(0)))
        .await;
    a.app.member(org, member, "member").await;
    let member_cookie = a.app.sign_in(member).await;
    assert_eq!(
        a.app
            .get("/api/v1/c/acme", Some(&member_cookie))
            .await
            .status(),
        StatusCode::OK
    );

    let res = a
        .call(
            Method::DELETE,
            "/api/v1/admin/companies/acme",
            Some(json!({ "confirm": "Acme" })),
        )
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(a.app.dir.path().join("data/orgs/acme").exists());

    let res = a
        .call(
            Method::DELETE,
            "/api/v1/admin/companies/acme",
            Some(json!({ "confirm": "acme" })),
        )
        .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert!(!a.app.dir.path().join("data/orgs/acme").exists());
    let trashed: Vec<String> = std::fs::read_dir(a.app.dir.path().join("data/trash"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(trashed.len(), 1);
    assert!(trashed[0].starts_with("acme."));
    assert!(trashed[0].len() > 5);

    assert_eq!(
        a.app
            .get("/api/v1/c/acme", Some(&member_cookie))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let list = body_json(a.app.get("/api/v1/admin/companies", Some(&a.cookie)).await).await;
    assert_eq!(list["companies"], json!([]));
    let pending = body_json(a.app.get("/api/v1/admin/invites", Some(&a.cookie)).await).await;
    assert_eq!(pending["invites"], json!([]));
    assert!(
        !a.app.dir.path().join("data/orgs/acme").exists(),
        "listing must not recreate the folder"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn trash_is_emptied_after_thirty_days() {
    let a = admin().await;
    ok(a.call(
        Method::POST,
        "/api/v1/admin/companies",
        Some(json!({ "name": "Acme", "owner_email": "o@acme.example" })),
    )
    .await)
    .await;
    ok(a.call(
        Method::DELETE,
        "/api/v1/admin/companies/acme",
        Some(json!({ "confirm": "acme" })),
    )
    .await)
    .await;
    let layout = a.app.state.dbs.layout().clone();
    let control = a.app.state.dbs.control().clone();
    let now = tazko::session::now();

    assert!(
        trash::purge(&layout, &control, now + trash::KEEP_SECS - 60)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(std::fs::read_dir(&layout.trash).unwrap().count(), 1);

    let removed = trash::purge(&layout, &control, now + trash::KEEP_SECS + 60)
        .await
        .unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(std::fs::read_dir(&layout.trash).unwrap().count(), 0);
    let orgs: i64 = a
        .app
        .sql(|c| c.query_row("SELECT count(*) FROM orgs", [], |r| r.get(0)))
        .await;
    assert_eq!(orgs, 0);

    let res = a
        .call(
            Method::POST,
            "/api/v1/admin/companies",
            Some(json!({ "name": "Acme", "owner_email": "o@acme.example" })),
        )
        .await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "the short name is free again"
    );
    assert_eq!(trash::deleted_at("acme.1700000000"), Some(1_700_000_000));
    assert_eq!(trash::deleted_at("noise"), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_company_keeps_at_least_one_owner() {
    let a = admin().await;
    ok(a.call(
        Method::POST,
        "/api/v1/admin/companies",
        Some(json!({ "name": "Acme", "owner_email": "o@acme.example" })),
    )
    .await)
    .await;
    let owner = a.app.user("solo@acme.example").await;
    let org: i64 = a
        .app
        .sql(|c| c.query_row("SELECT id FROM orgs", [], |r| r.get(0)))
        .await;
    a.app.member(org, owner, "owner").await;
    let path = format!("/api/v1/admin/companies/acme/members/{owner}");
    assert_eq!(
        a.call(Method::PATCH, &path, Some(json!({ "role": "member" })))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        a.call(Method::DELETE, &path, None).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        a.call(Method::PATCH, &path, Some(json!({ "role": "boss" })))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn admins_cannot_lock_themselves_out_and_disabling_ends_sessions() {
    let a = admin().await;
    let me = format!("/api/v1/admin/people/{}", a.id);
    for body in [
        json!({ "is_super_admin": false }),
        json!({ "disabled": true }),
    ] {
        assert_eq!(
            a.call(Method::PATCH, &me, Some(body)).await.status(),
            StatusCode::BAD_REQUEST
        );
    }

    let other = a.app.user("other@example.com").await;
    let cookie = a.app.sign_in(other).await;
    assert_eq!(
        a.app.get("/api/v1/me", Some(&cookie)).await.status(),
        StatusCode::OK
    );
    ok(a.call(
        Method::PATCH,
        &format!("/api/v1/admin/people/{other}"),
        Some(json!({ "disabled": true })),
    )
    .await)
    .await;
    let sessions: i64 = a
        .app
        .sql(move |c| {
            c.query_row(
                "SELECT count(*) FROM sessions WHERE user_id = ?1",
                [other],
                |r| r.get(0),
            )
        })
        .await;
    assert_eq!(sessions, 0);
    assert_eq!(
        a.app.get("/api/v1/me", Some(&cookie)).await.status(),
        StatusCode::UNAUTHORIZED
    );

    let people = body_json(a.app.get("/api/v1/admin/people", Some(&a.cookie)).await).await;
    let row = people["people"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["email"] == "other@example.com")
        .unwrap();
    assert_eq!(row["disabled"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_audit_log_filters_by_kind_and_names_the_actor() {
    let a = admin().await;
    ok(a.call(
        Method::POST,
        "/api/v1/admin/companies",
        Some(json!({ "name": "Acme", "owner_email": "o@acme.example" })),
    )
    .await)
    .await;
    a.app
        .sql(|c| {
            c.execute(
                "INSERT INTO audit_log (action, target, detail) VALUES ('sign_in_refused', 'jo.lee@gmail.com', '{\"reason\":\"not_invited\"}')",
                [],
            )
        })
        .await;

    let all = body_json(a.app.get("/api/v1/admin/audit", Some(&a.cookie)).await).await;
    assert_eq!(all["entries"].as_array().unwrap().len(), 3);
    assert_eq!(all["entries"][0]["kind"], "refused");

    let refused = body_json(
        a.app
            .get("/api/v1/admin/audit?kind=refused", Some(&a.cookie))
            .await,
    )
    .await;
    let e = &refused["entries"].as_array().unwrap();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0]["target"], "jo.lee@gmail.com");
    assert_eq!(e[0]["detail"]["reason"], "not_invited");

    let company = body_json(
        a.app
            .get("/api/v1/admin/audit?kind=company", Some(&a.cookie))
            .await,
    )
    .await;
    assert_eq!(company["entries"][0]["actor"], "root@example.com");
    assert_eq!(company["entries"][0]["company"], "Acme");

    assert_eq!(
        a.app
            .get("/api/v1/admin/audit?kind=nope", Some(&a.cookie))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_server_page_reports_version_uptime_and_data_size() {
    let a = admin().await;
    let s = body_json(a.app.get("/api/v1/admin/server", Some(&a.cookie)).await).await;
    assert_eq!(s["version"], env!("CARGO_PKG_VERSION"));
    assert!(s["data_bytes"].as_u64().unwrap() > 0);
    assert_eq!(s["email"], "off");
    assert_eq!(s["public_url"], "http://localhost:8791");
}

#[test]
fn the_command_line_makes_the_first_super_admin() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let run = |email: &str| {
        Command::new(env!("CARGO_BIN_EXE_tazko"))
            .args(["admin", "--data"])
            .arg(&data)
            .args(["--email", email])
            .output()
            .unwrap()
    };
    let out = run("Boss@Example.com");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("boss@example.com is now a super admin"));
    let out = run("boss@example.com");
    assert!(String::from_utf8_lossy(&out.stdout).contains("already a super admin"));
    assert!(!run("not an email").status.success());

    let conn = rusqlite::Connection::open(data.join("control.db")).unwrap();
    let (sup, audits): (bool, i64) = conn
        .query_row(
            "SELECT is_super_admin, (SELECT count(*) FROM audit_log WHERE action = 'super_admin_granted' AND target = ?1) FROM users WHERE email = ?1",
            params!["boss@example.com"],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((sup, audits), (true, 1));
}
