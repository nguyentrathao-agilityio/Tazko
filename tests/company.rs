mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use axum::response::Response;
use common::{TestApp, body_json};
use rusqlite::params;
use serde_json::{Value, json};
use tazko::auth::CSRF_HEADER;
use tazko::company::OWNER_ROUTES;

struct Who {
    id: i64,
    cookie: String,
    csrf: String,
}

async fn person(app: &TestApp, email: &str) -> Who {
    let id = app.user(email).await;
    let cookie = app.sign_in(id).await;
    let csrf = body_json(app.get("/api/v1/me", Some(&cookie)).await).await["csrf_token"]
        .as_str()
        .unwrap()
        .to_string();
    Who { id, cookie, csrf }
}

async fn call(
    app: &TestApp,
    who: &Who,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Response {
    let r = Request::builder()
        .method(method)
        .uri(path)
        .header(header::COOKIE, &who.cookie)
        .header(CSRF_HEADER, &who.csrf);
    let req = match body {
        Some(b) => r
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string())),
        None => r.body(Body::empty()),
    };
    app.send(req.unwrap()).await
}

fn body_for(method: &Method, path: &str) -> Option<Value> {
    match (method.as_str(), path) {
        ("POST", "/invites") => Some(json!({ "email": "new@example.com", "role": "member" })),
        ("PATCH", _) => Some(json!({ "role": "member" })),
        _ => None,
    }
}

struct World {
    app: TestApp,
    a: i64,
    b: i64,
    owner: Who,
    member: Who,
}

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let b = app.company("company-b", "active").await;
    let owner = person(&app, "owner@a.example").await;
    let member = person(&app, "member@a.example").await;
    app.member(a, owner.id, "owner").await;
    app.member(a, member.id, "member").await;
    World {
        app,
        a,
        b,
        owner,
        member,
    }
}

async fn audit_actions(app: &TestApp) -> Vec<(String, Option<i64>, Option<i64>)> {
    app.sql(|c| {
        let mut s = c.prepare("SELECT action, actor_id, org_id FROM audit_log ORDER BY id")?;
        let r = s
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(r)
    })
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn members_cannot_use_owner_pages_and_other_companies_are_not_found() {
    let w = world().await;
    for (method, path) in OWNER_ROUTES {
        let a = format!("/api/v1/c/company-a{path}");
        let res = call(
            &w.app,
            &w.member,
            method.clone(),
            &a,
            body_for(method, path),
        )
        .await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN, "member {method} {a}");

        let b = format!("/api/v1/c/company-b{path}");
        let res = call(&w.app, &w.owner, method.clone(), &b, body_for(method, path)).await;
        assert_eq!(
            res.status(),
            StatusCode::NOT_FOUND,
            "owner of A {method} {b}"
        );
    }
    assert!(audit_actions(&w.app).await.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_owner_cannot_reach_another_companys_invites_or_people_by_id() {
    let w = world().await;
    let outsider = w.app.user("outsider@b.example").await;
    w.app.member(w.b, outsider, "owner").await;
    let b = w.b;
    let invite: i64 = w
        .app
        .sql(move |c| {
            c.execute("INSERT INTO invites (org_id, email, company_role) VALUES (?1, 'x@b.example', 'member')", [b])?;
            Ok(c.last_insert_rowid())
        })
        .await;

    let res = call(
        &w.app,
        &w.owner,
        Method::DELETE,
        &format!("/api/v1/c/company-a/invites/{invite}"),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    for method in [Method::PATCH, Method::DELETE] {
        let path = format!("/api/v1/c/company-a/members/{outsider}");
        let res = call(
            &w.app,
            &w.owner,
            method.clone(),
            &path,
            body_for(&method, ""),
        )
        .await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{method}");
    }
    let revoked: Option<i64> = w
        .app
        .sql(move |c| {
            c.query_row(
                "SELECT revoked_at FROM invites WHERE id = ?1",
                [invite],
                |r| r.get(0),
            )
        })
        .await;
    assert_eq!(revoked, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn owner_changes_need_csrf_and_are_audited_with_the_owner_as_actor() {
    let w = world().await;
    let res = w
        .app
        .send(
            Request::post("/api/v1/c/company-a/invites")
                .header(header::COOKIE, &w.owner.cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "email": "x@a.example", "role": "member" }).to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let res = call(
        &w.app,
        &w.owner,
        Method::POST,
        "/api/v1/c/company-a/invites",
        Some(json!({ "email": "New@A.example", "role": "member" })),
    )
    .await;
    assert_eq!(body_json(res).await["result"], "invited");
    let invite: i64 = w
        .app
        .sql(|c| {
            c.query_row(
                "SELECT id FROM invites WHERE email = 'new@a.example'",
                [],
                |r| r.get(0),
            )
        })
        .await;
    let res = call(
        &w.app,
        &w.owner,
        Method::DELETE,
        &format!("/api/v1/c/company-a/invites/{invite}"),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let path = format!("/api/v1/c/company-a/members/{}", w.member.id);
    assert_eq!(
        call(
            &w.app,
            &w.owner,
            Method::PATCH,
            &path,
            Some(json!({ "role": "owner" }))
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&w.app, &w.owner, Method::DELETE, &path, None)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );

    let log = audit_actions(&w.app).await;
    let actions: Vec<&str> = log.iter().map(|(a, _, _)| a.as_str()).collect();
    assert_eq!(
        actions,
        vec![
            "invite_created",
            "invite_revoked",
            "role_changed",
            "member_removed"
        ]
    );
    assert!(
        log.iter()
            .all(|(_, actor, org)| *actor == Some(w.owner.id) && *org == Some(w.a))
    );

    let own = format!("/api/v1/c/company-a/members/{}", w.owner.id);
    let res = call(
        &w.app,
        &w.owner,
        Method::PATCH,
        &own,
        Some(json!({ "role": "member" })),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "the last owner stays"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_person_in_two_companies_only_sees_the_chosen_ones_data() {
    let w = world().await;
    let both = person(&w.app, "both@example.com").await;
    w.app.member(w.a, both.id, "owner").await;
    w.app.member(w.b, both.id, "owner").await;
    let only_b = w.app.user("only-b@b.example").await;
    w.app.member(w.b, only_b, "member").await;
    let b = w.b;
    w.app
        .sql(move |c| {
            c.execute("INSERT INTO invites (org_id, email, company_role) VALUES (?1, 'pending@b.example', 'member')", params![b])
        })
        .await;

    let a_db = w.app.state.dbs.tenant("company-a").unwrap();
    a_db.write(|c| {
        c.execute_batch(
            "CREATE TABLE projects (id INTEGER PRIMARY KEY, name TEXT NOT NULL, deleted_at INTEGER);
             INSERT INTO projects (name) VALUES ('Only in A');",
        )?;
        Ok(())
    })
    .await
    .unwrap();

    let me = body_json(w.app.get("/api/v1/me", Some(&both.cookie)).await).await;
    let slugs: Vec<&str> = me["companies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["slug"].as_str().unwrap())
        .collect();
    assert_eq!(slugs, vec!["company-a", "company-b"]);

    let emails = |v: &Value, key: &str| -> Vec<String> {
        v[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["email"].as_str().unwrap().to_string())
            .collect()
    };
    let a = body_json(
        call(
            &w.app,
            &both,
            Method::GET,
            "/api/v1/c/company-a/settings",
            None,
        )
        .await,
    )
    .await;
    let b = body_json(
        call(
            &w.app,
            &both,
            Method::GET,
            "/api/v1/c/company-b/settings",
            None,
        )
        .await,
    )
    .await;
    assert_eq!(a["name"], "COMPANY-A");
    assert_eq!(b["name"], "COMPANY-B");
    assert!(emails(&a, "members").contains(&"member@a.example".to_string()));
    assert!(!emails(&a, "members").contains(&"only-b@b.example".to_string()));
    assert!(emails(&b, "members").contains(&"only-b@b.example".to_string()));
    assert!(!emails(&b, "members").contains(&"member@a.example".to_string()));
    assert!(emails(&a, "invites").is_empty());
    assert_eq!(emails(&b, "invites"), vec!["pending@b.example"]);

    let pa = body_json(
        call(
            &w.app,
            &both,
            Method::GET,
            "/api/v1/c/company-a/projects",
            None,
        )
        .await,
    )
    .await;
    let pb = body_json(
        call(
            &w.app,
            &both,
            Method::GET,
            "/api/v1/c/company-b/projects",
            None,
        )
        .await,
    )
    .await;
    assert_eq!(pa["projects"], json!([{ "name": "Only in A" }]));
    assert_eq!(pb["projects"], json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_owner_cannot_tell_whether_an_address_already_has_an_account() {
    let w = world().await;
    let existing = person(&w.app, "existing@elsewhere.example").await;
    let off = w.app.user("off@elsewhere.example").await;
    w.app
        .sql(move |c| c.execute("UPDATE users SET disabled = 1 WHERE id = ?1", [off]))
        .await;

    for email in [
        "existing@elsewhere.example",
        "off@elsewhere.example",
        "nobody@elsewhere.example",
    ] {
        let res = call(
            &w.app,
            &w.owner,
            Method::POST,
            "/api/v1/c/company-a/invites",
            Some(json!({ "email": email, "role": "member" })),
        )
        .await;
        assert_eq!(res.status(), StatusCode::CREATED, "{email}");
        assert_eq!(
            body_json(res).await,
            json!({ "result": "invited" }),
            "{email}"
        );
    }

    let a = w.a;
    let app = &w.app;
    let members_of_a = async |id: i64| -> i64 {
        app.sql(move |c| {
            c.query_row(
                "SELECT count(*) FROM org_members WHERE org_id = ?1 AND user_id = ?2",
                params![a, id],
                |r| r.get(0),
            )
        })
        .await
    };
    assert_eq!(
        members_of_a(existing.id).await,
        0,
        "not added behind their back"
    );
    assert_eq!(members_of_a(off).await, 0);

    let me = body_json(w.app.get("/api/v1/me", Some(&existing.cookie)).await).await;
    let slugs: Vec<&str> = me["companies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["slug"].as_str().unwrap())
        .collect();
    assert_eq!(
        slugs,
        vec!["company-a"],
        "they join on their next page load"
    );
    assert_eq!(members_of_a(existing.id).await, 1);
    assert_eq!(members_of_a(off).await, 0);
}
