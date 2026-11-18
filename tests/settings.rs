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
