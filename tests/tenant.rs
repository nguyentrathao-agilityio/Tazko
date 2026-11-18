mod common;

use axum::http::StatusCode;
use common::{TestApp, body_json, body_text};
use tazko::api::{COMPANY_GET_PATHS, COMPANY_PREFIX};

fn company_paths(slug: &str) -> Vec<String> {
    let base = COMPANY_PREFIX.replace("{company}", slug);
    let mut paths: Vec<String> = COMPANY_GET_PATHS
        .iter()
        .map(|p| format!("{base}{p}"))
        .collect();
    paths.push(format!("{base}/"));
    paths.push(format!("{base}/anything"));
    paths
}

struct Two {
    app: TestApp,
    alice: String,
}

async fn two_companies() -> Two {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let b = app.company("company-b", "active").await;
    let alice = app.user("alice@example.com").await;
    let bob = app.user("bob@example.com").await;
    app.member(a, alice, "member").await;
    app.member(b, bob, "owner").await;
    let alice = app.sign_in(alice).await;
    Two { app, alice }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_member_of_company_a_gets_not_found_for_every_company_b_address() {
    let Two { app, alice } = two_companies().await;
    let missing = body_text(
        app.get(
            &COMPANY_PREFIX.replace("{company}", "no-such-company"),
            Some(&alice),
        )
        .await,
    )
    .await;

    for path in company_paths("company-b") {
        let res = app.get(&path, Some(&alice)).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{path}");
        assert_eq!(
            body_text(res).await,
            missing,
            "{path} differs from a company that does not exist"
        );
    }

    assert!(
        !app.state.dbs.is_open("company-b"),
        "company B's file was opened"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_member_reaches_their_own_company_file() {
    let Two { app, alice } = two_companies().await;
    let res = app
        .get(
            &COMPANY_PREFIX.replace("{company}", "company-a"),
            Some(&alice),
        )
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_json(res).await;
    assert_eq!(body["slug"], "company-a");
    assert_eq!(body["role"], "member");
    assert_eq!(body["schema_version"], 1);
    assert!(
        app.dir
            .path()
            .join("data/orgs/company-a/tazko.db")
            .is_file()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_session_means_sign_in() {
    let Two { app, .. } = two_companies().await;
    for cookie in [
        None,
        Some("tazko_session=not-a-token"),
        Some(&format!("tazko_session={}", "ab".repeat(32))[..]),
    ] {
        let res = app
            .get(&COMPANY_PREFIX.replace("{company}", "company-a"), cookie)
            .await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{cookie:?}");
        assert_eq!(
            app.get("/api/v1/me", cookie).await.status(),
            StatusCode::UNAUTHORIZED
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expired_sessions_and_disabled_people_are_signed_out() {
    let Two { app, alice } = two_companies().await;
    let path = COMPANY_PREFIX.replace("{company}", "company-a");
    assert_eq!(app.get(&path, Some(&alice)).await.status(), StatusCode::OK);

    app.sql(|c| c.execute("UPDATE sessions SET expires_at = unixepoch() - 1", []))
        .await;
    assert_eq!(
        app.get(&path, Some(&alice)).await.status(),
        StatusCode::UNAUTHORIZED
    );

    let bob = app
        .sql(|c| {
            c.query_row(
                "SELECT id FROM users WHERE email = 'bob@example.com'",
                [],
                |r| r.get::<_, i64>(0),
            )
        })
        .await;
    let bob_cookie = app.sign_in(bob).await;
    app.sql(move |c| c.execute("UPDATE users SET disabled = 1 WHERE id = ?1", [bob]))
        .await;
    assert_eq!(
        app.get("/api/v1/me", Some(&bob_cookie)).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suspended_or_deleted_companies_are_not_found_even_for_members() {
    let app = TestApp::new();
    let alice = app.user("alice@example.com").await;
    for (slug, status) in [("paused", "suspended"), ("gone", "deleted")] {
        let org = app.company(slug, status).await;
        app.member(org, alice, "owner").await;
    }
    let cookie = app.sign_in(alice).await;
    for slug in ["paused", "gone"] {
        let res = app
            .get(&COMPANY_PREFIX.replace("{company}", slug), Some(&cookie))
            .await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{slug}");
    }
    assert_eq!(
        body_json(app.get("/api/v1/me", Some(&cookie)).await).await["companies"],
        serde_json::json!([])
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_super_admin_does_not_get_into_companies_they_are_not_in() {
    let Two { app, .. } = two_companies().await;
    let admin = app.user("admin@example.com").await;
    app.sql(move |c| c.execute("UPDATE users SET is_super_admin = 1 WHERE id = ?1", [admin]))
        .await;
    let cookie = app.sign_in(admin).await;
    for path in company_paths("company-b") {
        assert_eq!(
            app.get(&path, Some(&cookie)).await.status(),
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bad_company_names_in_the_address_are_not_found() {
    let Two { app, alice } = two_companies().await;
    for slug in ["..", "Company-A", "a%2F..%2Fb", "-x"] {
        let res = app
            .get(&COMPANY_PREFIX.replace("{company}", slug), Some(&alice))
            .await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{slug}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn me_lists_only_the_companies_the_person_belongs_to() {
    let Two { app, alice } = two_companies().await;
    let body = body_json(app.get("/api/v1/me", Some(&alice)).await).await;
    assert_eq!(body["email"], "alice@example.com");
    let slugs: Vec<&str> = body["companies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["slug"].as_str().unwrap())
        .collect();
    assert_eq!(slugs, vec!["company-a"]);
}
