mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, Who, body_json, call, person};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";

struct World {
    app: TestApp,
    project: i64,
    owner: Who,
    member: Who,
    observer: Who,
    company_owner: Who,
}

async fn email_of(app: &TestApp, id: i64) -> String {
    app.sql(move |c| c.query_row("SELECT email FROM users WHERE id = ?1", [id], |r| r.get(0)))
        .await
}

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let company_owner = person(&app, "boss@a.example").await;
    let owner = person(&app, "lead@a.example").await;
    let member = person(&app, "dev@a.example").await;
    let observer = person(&app, "viewer@a.example").await;
    app.member(a, company_owner.id, "owner").await;
    for w in [&owner, &member, &observer] {
        app.member(a, w.id, "member").await;
    }
    let res = call(
        &app,
        &company_owner,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "Website", "owner_id": owner.id })),
    )
    .await;
    let project = body_json(res).await["id"].as_i64().unwrap();
    for (w, role) in [(&member, "member"), (&observer, "observer")] {
        let email = email_of(&app, w.id).await;
        call(
            &app,
            &owner,
            Method::POST,
            &format!("{C}/projects/{project}/invites"),
            Some(json!({ "email": email, "role": role })),
        )
        .await;
    }
    World {
        app,
        project,
        owner,
        member,
        observer,
        company_owner,
    }
}

impl World {
    async fn req(
        &self,
        who: &Who,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let res = call(&self.app, who, method, path, body).await;
        let status = res.status();
        let text = common::body_text(res).await;
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    async fn board(&self, who: &Who, name: &str, restricted: bool) -> i64 {
        let (s, b) = self
            .req(
                who,
                Method::POST,
                &format!("{C}/projects/{}/boards", self.project),
                Some(json!({ "name": name, "restricted": restricted })),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{b}");
        b["id"].as_i64().unwrap()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_new_board_starts_with_three_lists_and_the_project_background() {
    let w = world().await;
    let b = w.board(&w.member, "Sprint 15", false).await;
    let (s, v) = w
        .req(&w.observer, Method::GET, &format!("{C}/boards/{b}"), None)
        .await;
    assert_eq!(s, StatusCode::OK);
    let titles: Vec<&str> = v["lists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["To do", "Doing", "Done"]);
    assert_eq!(v["background"], "dusk");
    assert_eq!(v["can"]["edit"], false);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn people_not_on_a_restricted_board_cannot_see_it_even_by_address() {
    let w = world().await;
    let open = w.board(&w.owner, "Roadmap", false).await;
    let secret = w.board(&w.owner, "Salaries", true).await;
    let base = format!("{C}/boards/{secret}");
    let lists = w.req(&w.owner, Method::GET, &base, None).await.1["lists"].clone();
    let list = lists[0]["id"].as_i64().unwrap();

    for who in [&w.member, &w.observer, &w.company_owner] {
        let attempts: Vec<(Method, String, Option<Value>)> = vec![
            (Method::GET, base.clone(), None),
            (Method::PATCH, base.clone(), Some(json!({ "name": "Mine" }))),
            (Method::GET, format!("{base}/archive"), None),
            (
                Method::PUT,
                format!("{base}/star"),
                Some(json!({ "starred": true })),
            ),
            (
                Method::POST,
                format!("{base}/copy"),
                Some(json!({ "name": "Leak" })),
            ),
            (
                Method::POST,
                format!("{base}/lists"),
                Some(json!({ "title": "x" })),
            ),
            (
                Method::PATCH,
                format!("{base}/lists/{list}"),
                Some(json!({ "title": "x" })),
            ),
            (Method::POST, format!("{base}/lists/{list}/archive"), None),
            (
                Method::PUT,
                format!("{base}/access"),
                Some(json!({ "restricted": false })),
            ),
        ];
        for (m, path, body) in attempts {
            let (s, _) = w.req(who, m.clone(), &path, body).await;
            assert_eq!(s, StatusCode::NOT_FOUND, "{m} {path}");
        }
    }

    let (_, v) = w
        .req(
            &w.member,
            Method::GET,
            &format!("{C}/projects/{}/boards", w.project),
            None,
        )
        .await;
    let ids: Vec<i64> = v["boards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, vec![open]);

    let (s, _) = w
        .req(
            &w.owner,
            Method::PUT,
            &format!("{base}/access"),
            Some(json!({ "restricted": true, "people": [w.member.id] })),
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert_eq!(
        w.req(&w.member, Method::GET, &base, None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        w.req(&w.observer, Method::GET, &base, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        w.req(&w.owner, Method::GET, &base, None).await.0,
        StatusCode::OK,
        "the owner who restricts keeps access"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_project_members_choose_restricted_people_and_only_owners_restrict() {
    let w = world().await;
    let b = w.board(&w.member, "Ops", false).await;
    let path = format!("{C}/boards/{b}/access");
    let (s, _) = w
        .req(
            &w.member,
            Method::PUT,
            &path,
            Some(json!({ "restricted": true })),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = w
        .req(
            &w.owner,
            Method::PUT,
            &path,
            Some(json!({ "restricted": true, "people": [w.company_owner.id] })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn observers_cannot_change_boards_or_lists() {
    let w = world().await;
    let b = w.board(&w.owner, "Plan", false).await;
    let base = format!("{C}/boards/{b}");
    let list = w.req(&w.owner, Method::GET, &base, None).await.1["lists"][0]["id"]
        .as_i64()
        .unwrap();
    let writes: Vec<(Method, String, Option<Value>)> = vec![
        (
            Method::POST,
            format!("{C}/projects/{}/boards", w.project),
            Some(json!({ "name": "x" })),
        ),
        (Method::PATCH, base.clone(), Some(json!({ "name": "x" }))),
        (Method::POST, format!("{base}/close"), None),
        (
            Method::POST,
            format!("{base}/copy"),
            Some(json!({ "name": "x" })),
        ),
        (
            Method::POST,
            format!("{base}/lists"),
            Some(json!({ "title": "x" })),
        ),
        (
            Method::PATCH,
            format!("{base}/lists/{list}"),
            Some(json!({ "title": "x" })),
        ),
        (
            Method::POST,
            format!("{base}/lists/{list}/move"),
            Some(json!({ "after": null })),
        ),
        (Method::POST, format!("{base}/lists/{list}/archive"), None),
        (
            Method::POST,
            format!("{base}/lists/{list}/copy"),
            Some(json!({ "title": "x" })),
        ),
    ];
    for (m, path, body) in writes {
        let (s, _) = w.req(&w.observer, m.clone(), &path, body).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{m} {path}");
    }
    let (s, _) = w
        .req(
            &w.observer,
            Method::PUT,
            &format!("{base}/star"),
            Some(json!({ "starred": true })),
        )
        .await;
    assert_eq!(
        s,
        StatusCode::NO_CONTENT,
        "anyone who can see a board may star it"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn members_create_boards_only_while_the_switch_is_on_and_only_owners_close_them() {
    let w = world().await;
    let b = w.board(&w.member, "Mine", false).await;
    let (s, _) = w
        .req(
            &w.member,
            Method::POST,
            &format!("{C}/boards/{b}/close"),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    w.req(
        &w.owner,
        Method::PATCH,
        &format!("{C}/projects/{}", w.project),
        Some(json!({ "members_create_boards": false })),
    )
    .await;
    let (s, _) = w
        .req(
            &w.member,
            Method::POST,
            &format!("{C}/projects/{}/boards", w.project),
            Some(json!({ "name": "No" })),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    let (s, _) = w
        .req(
            &w.owner,
            Method::POST,
            &format!("{C}/boards/{b}/close"),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, v) = w
        .req(
            &w.member,
            Method::POST,
            &format!("{C}/boards/{b}/lists"),
            Some(json!({ "title": "Late" })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    w.req(
        &w.owner,
        Method::POST,
        &format!("{C}/boards/{b}/reopen"),
        None,
    )
    .await;
    let (s, _) = w
        .req(
            &w.member,
            Method::POST,
            &format!("{C}/boards/{b}/lists"),
            Some(json!({ "title": "Late" })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lists_are_created_moved_archived_restored_and_copied_in_order() {
    let w = world().await;
    let b = w.board(&w.member, "Flow", false).await;
    let base = format!("{C}/boards/{b}");
    let titles = |v: &Value| -> Vec<String> {
        v["lists"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["title"].as_str().unwrap().to_string())
            .collect()
    };
    let ids = |v: &Value| -> Vec<i64> {
        v["lists"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["id"].as_i64().unwrap())
            .collect()
    };
    let v = w.req(&w.member, Method::GET, &base, None).await.1;
    let [todo, doing, done]: [i64; 3] = ids(&v).try_into().unwrap();

    w.req(
        &w.member,
        Method::POST,
        &format!("{base}/lists"),
        Some(json!({ "title": "Review", "after": doing, "at_end": false })),
    )
    .await;
    w.req(
        &w.member,
        Method::POST,
        &format!("{base}/lists/{done}/move"),
        Some(json!({ "after": null })),
    )
    .await;
    w.req(
        &w.member,
        Method::PATCH,
        &format!("{base}/lists/{todo}"),
        Some(json!({ "title": "Backlog" })),
    )
    .await;
    let v = w.req(&w.member, Method::GET, &base, None).await.1;
    assert_eq!(titles(&v), vec!["Done", "Backlog", "Doing", "Review"]);

    w.req(
        &w.member,
        Method::POST,
        &format!("{base}/lists/{doing}/archive"),
        None,
    )
    .await;
    let v = w.req(&w.member, Method::GET, &base, None).await.1;
    assert_eq!(titles(&v), vec!["Done", "Backlog", "Review"]);
    let archived = w
        .req(&w.member, Method::GET, &format!("{base}/archive"), None)
        .await
        .1;
    assert_eq!(archived["lists"][0]["title"], "Doing");
    w.req(
        &w.member,
        Method::POST,
        &format!("{base}/lists/{doing}/restore"),
        None,
    )
    .await;
    w.req(
        &w.member,
        Method::POST,
        &format!("{base}/lists/{todo}/copy"),
        Some(json!({ "title": "Backlog 2" })),
    )
    .await;
    let v = w.req(&w.member, Method::GET, &base, None).await.1;
    assert_eq!(
        titles(&v),
        vec!["Done", "Backlog", "Backlog 2", "Review", "Doing"]
    );

    let (s, _) = w
        .req(
            &w.member,
            Method::POST,
            &format!("{base}/lists/{todo}/move"),
            Some(json!({ "after": todo })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn starred_boards_follow_the_person_and_copies_keep_lists_and_access() {
    let w = world().await;
    let b = w.board(&w.owner, "Launch", true).await;
    w.req(
        &w.owner,
        Method::PUT,
        &format!("{C}/boards/{b}/access"),
        Some(json!({ "restricted": true, "people": [w.member.id] })),
    )
    .await;
    w.req(
        &w.member,
        Method::PUT,
        &format!("{C}/boards/{b}/star"),
        Some(json!({ "starred": true })),
    )
    .await;
    let mine = w
        .req(&w.member, Method::GET, &format!("{C}/starred"), None)
        .await
        .1;
    assert_eq!(mine["boards"][0]["name"], "Launch");
    let theirs = w
        .req(&w.owner, Method::GET, &format!("{C}/starred"), None)
        .await
        .1;
    assert_eq!(theirs["boards"], json!([]));

    let (s, v) = w
        .req(
            &w.owner,
            Method::POST,
            &format!("{C}/boards/{b}/copy"),
            Some(json!({ "name": "Launch copy" })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED);
    let copy = v["id"].as_i64().unwrap();
    let v = w
        .req(&w.member, Method::GET, &format!("{C}/boards/{copy}"), None)
        .await
        .1;
    assert_eq!(v["restricted"], true);
    assert_eq!(v["lists"].as_array().unwrap().len(), 3);
    assert_eq!(
        w.req(
            &w.observer,
            Method::GET,
            &format!("{C}/boards/{copy}"),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn removing_someone_from_the_project_takes_them_off_restricted_boards() {
    let w = world().await;
    let b = w.board(&w.owner, "Inner", true).await;
    w.req(
        &w.owner,
        Method::PUT,
        &format!("{C}/boards/{b}/access"),
        Some(json!({ "restricted": true, "people": [w.member.id] })),
    )
    .await;
    w.req(
        &w.owner,
        Method::DELETE,
        &format!("{C}/projects/{}/members/{}", w.project, w.member.id),
        None,
    )
    .await;
    let email = email_of(&w.app, w.member.id).await;
    w.req(
        &w.owner,
        Method::POST,
        &format!("{C}/projects/{}/invites", w.project),
        Some(json!({ "email": email, "role": "member" })),
    )
    .await;
    assert_eq!(
        w.req(&w.member, Method::GET, &format!("{C}/boards/{b}"), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
