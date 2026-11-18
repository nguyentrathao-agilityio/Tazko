mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, body_json, call, person};
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn zen_choices_are_remembered_for_the_person() {
    let app = TestApp::new();
    let me = person(&app, "mai@example.com").await;
    let v = body_json(app.get("/api/v1/me", Some(&me.cookie)).await).await;
    assert_eq!(v["settings"]["zen"], false);
    assert_eq!(v["settings"]["zen_density"], "comfortable");

    let res = call(
        &app,
        &me,
        Method::PATCH,
        "/api/v1/me/settings",
        Some(json!({ "zen": true, "zen_density": "minimal" })),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let other = person(&app, "jo@example.com").await;
    assert_eq!(
        body_json(app.get("/api/v1/me/settings", Some(&other.cookie)).await).await["zen"],
        false
    );

    let again = app.sign_in(me.id).await;
    let v = body_json(app.get("/api/v1/me", Some(&again)).await).await;
    assert_eq!(v["settings"]["zen"], true);
    assert_eq!(v["settings"]["zen_density"], "minimal");

    for bad in [
        json!({ "zen": "yes" }),
        json!({ "zen_density": "tiny" }),
        json!({ "theme": "x", "nope": 1 }),
    ] {
        let res = call(
            &app,
            &me,
            Method::PATCH,
            "/api/v1/me/settings",
            Some(bad.clone()),
        )
        .await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{bad}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn settings_and_profile_are_the_same_in_every_company() {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let b = app.company("company-b", "active").await;
    let boss_a = person(&app, "boss@a.example").await;
    let boss_b = person(&app, "boss@b.example").await;
    let me = person(&app, "mai@example.com").await;
    app.member(a, boss_a.id, "owner").await;
    app.member(b, boss_b.id, "owner").await;
    app.member(a, me.id, "member").await;
    app.member(b, me.id, "member").await;
    for (boss, slug) in [(&boss_a, "company-a"), (&boss_b, "company-b")] {
        let res = call(
            &app,
            boss,
            Method::POST,
            &format!("/api/v1/c/{slug}/projects"),
            Some(json!({ "name": "P", "owner_id": me.id })),
        )
        .await;
        assert_eq!(res.status(), StatusCode::CREATED);
    }

    let change = json!({ "theme": "dark", "zoom": 120, "time_zone": "Asia/Ho_Chi_Minh", "date_format": "DMY", "time_format": "24", "week_start": "sun", "reduce_motion": true, "label_style": "bars", "density": "compact", "job_title": "Designer" });
    let res = call(
        &app,
        &me,
        Method::PATCH,
        "/api/v1/me/settings",
        Some(change.clone()),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let res = call(
        &app,
        &me,
        Method::PATCH,
        "/api/v1/me/profile",
        Some(json!({ "name": "Mai T. Tran" })),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let settings =
        body_json(app.get("/api/v1/me", Some(&me.cookie)).await).await["settings"].clone();
    for (k, v) in change.as_object().unwrap() {
        assert_eq!(&settings[k], v, "{k}");
    }
    for slug in ["company-a", "company-b"] {
        let mine = body_json(
            call(
                &app,
                &me,
                Method::GET,
                &format!("/api/v1/c/{slug}/my-projects"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(
            mine["projects"][0]["members"][0]["name"], "Mai T. Tran",
            "{slug}"
        );
    }

    for bad in [
        json!({ "zoom": 75 }),
        json!({ "zoom": 125 }),
        json!({ "time_zone": "../../etc" }),
        json!({ "date_format": "YMD" }),
        json!({ "job_title": "x".repeat(81) }),
    ] {
        let res = call(
            &app,
            &me,
            Method::PATCH,
            "/api/v1/me/settings",
            Some(bad.clone()),
        )
        .await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{bad}");
    }
    let res = call(
        &app,
        &me,
        Method::PATCH,
        "/api/v1/me/profile",
        Some(json!({ "name": "  " })),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn people_see_and_end_their_own_sessions() {
    let app = TestApp::new();
    let me = person(&app, "mai@example.com").await;
    let other_device = app.sign_in(me.id).await;
    let third = app.sign_in(me.id).await;
    let stranger = person(&app, "jo@example.com").await;

    let list = body_json(call(&app, &me, Method::GET, "/api/v1/me/sessions", None).await).await;
    let sessions = list["sessions"].as_array().unwrap().clone();
    assert_eq!(sessions.len(), 3);
    let current = sessions.iter().find(|s| s["current"] == true).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let other = sessions.iter().find(|s| s["current"] == false).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let res = call(
        &app,
        &me,
        Method::DELETE,
        &format!("/api/v1/me/sessions/{current}"),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let res = call(
        &app,
        &stranger,
        Method::DELETE,
        &format!("/api/v1/me/sessions/{other}"),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let res = call(
        &app,
        &me,
        Method::DELETE,
        &format!("/api/v1/me/sessions/{other}"),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res = call(&app, &me, Method::POST, "/api/v1/me/sessions/others", None).await;
    assert_eq!(body_json(res).await["ended"], 1);
    for cookie in [&other_device, &third] {
        assert_eq!(
            app.get("/api/v1/me", Some(cookie)).await.status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        app.get("/api/v1/me", Some(&me.cookie)).await.status(),
        StatusCode::OK
    );
}
