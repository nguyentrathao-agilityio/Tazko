mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, Who, body_json, call, person};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum R {
    CompanyOwner,
    CompanyMember,
    Owner,
    Member,
    Observer,
    Outsider,
}

const ALL: [R; 6] = [
    R::Outsider,
    R::CompanyMember,
    R::CompanyOwner,
    R::Observer,
    R::Member,
    R::Owner,
];

struct World {
    app: TestApp,
    project: i64,
    company_owner: Who,
    company_member: Who,
    owner: Who,
    member: Who,
    observer: Who,
    outsider: Who,
}

impl World {
    fn who(&self, r: R) -> &Who {
        match r {
            R::CompanyOwner => &self.company_owner,
            R::CompanyMember => &self.company_member,
            R::Owner => &self.owner,
            R::Member => &self.member,
            R::Observer => &self.observer,
            R::Outsider => &self.outsider,
        }
    }

    async fn call(&self, r: R, method: Method, path: &str, body: Option<Value>) -> StatusCode {
        call(&self.app, self.who(r), method, path, body)
            .await
            .status()
    }
}

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let b = app.company("company-b", "active").await;
    let company_owner = person(&app, "boss@a.example").await;
    let company_member = person(&app, "plain@a.example").await;
    let owner = person(&app, "lead@a.example").await;
    let member = person(&app, "dev@a.example").await;
    let observer = person(&app, "viewer@gmail.com").await;
    let outsider = person(&app, "other@b.example").await;
    app.member(a, company_owner.id, "owner").await;
    for w in [&company_member, &owner, &member, &observer] {
        app.member(a, w.id, "member").await;
    }
    app.member(b, outsider.id, "owner").await;

    let res = call(
        &app,
        &company_owner,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "Website relaunch", "owner_id": owner.id })),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let project = body_json(res).await["id"].as_i64().unwrap();
    for (w, role) in [(&member, "member"), (&observer, "observer")] {
        let email: String = {
            let id = w.id;
            app.sql(move |c| {
                c.query_row("SELECT email FROM users WHERE id = ?1", [id], |r| r.get(0))
            })
            .await
        };
        let res = call(
            &app,
            &owner,
            Method::POST,
            &format!("{C}/projects/{project}/invites"),
            Some(json!({ "email": email, "role": role })),
        )
        .await;
        assert_eq!(body_json(res).await["result"], "added");
    }

    World {
        app,
        project,
        company_owner,
        company_member,
        owner,
        member,
        observer,
        outsider,
    }
}

fn denied(r: R) -> StatusCode {
    match r {
        R::Outsider => StatusCode::NOT_FOUND,
        _ => StatusCode::FORBIDDEN,
    }
}

fn hidden(_: R) -> StatusCode {
    StatusCode::NOT_FOUND
}

struct Case {
    name: &'static str,
    method: Method,
    path: fn(&World) -> String,
    body: fn(&World) -> Option<Value>,
    allowed: &'static [R],
    ok: StatusCode,
    refused: fn(R) -> StatusCode,
}

fn project_refusal(r: R) -> StatusCode {
    match r {
        R::Member | R::Observer => StatusCode::FORBIDDEN,
        _ => StatusCode::NOT_FOUND,
    }
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "open the project",
            method: Method::GET,
            path: |w| format!("{C}/projects/{}", w.project),
            body: |_| None,
            allowed: &[R::Owner, R::Member, R::Observer],
            ok: StatusCode::OK,
            refused: hidden,
        },
        Case {
            name: "see the project members",
            method: Method::GET,
            path: |w| format!("{C}/projects/{}/members", w.project),
            body: |_| None,
            allowed: &[R::Owner, R::Member, R::Observer],
            ok: StatusCode::OK,
            refused: hidden,
        },
        Case {
            name: "change project settings",
            method: Method::PATCH,
            path: |w| format!("{C}/projects/{}", w.project),
            body: |_| Some(json!({ "description": "New words", "members_invite": true })),
            allowed: &[R::Owner],
            ok: StatusCode::NO_CONTENT,
            refused: project_refusal,
        },
        Case {
            name: "invite to the project",
            method: Method::POST,
            path: |w| format!("{C}/projects/{}/invites", w.project),
            body: |_| Some(json!({ "email": "fresh@example.com", "role": "member" })),
            allowed: &[R::Owner],
            ok: StatusCode::CREATED,
            refused: project_refusal,
        },
        Case {
            name: "change a member's role",
            method: Method::PATCH,
            path: |w| format!("{C}/projects/{}/members/{}", w.project, w.observer.id),
            body: |_| Some(json!({ "role": "member" })),
            allowed: &[R::Owner],
            ok: StatusCode::NO_CONTENT,
            refused: project_refusal,
        },
        Case {
            name: "remove a member",
            method: Method::DELETE,
            path: |w| format!("{C}/projects/{}/members/{}", w.project, w.member.id),
            body: |_| None,
            allowed: &[R::Owner],
            ok: StatusCode::NO_CONTENT,
            refused: project_refusal,
        },
        Case {
            name: "list projects to manage",
            method: Method::GET,
            path: |_| format!("{C}/projects"),
            body: |_| None,
            allowed: &[R::CompanyOwner],
            ok: StatusCode::OK,
            refused: denied,
        },
        Case {
            name: "create a project",
            method: Method::POST,
            path: |_| format!("{C}/projects"),
            body: |w| Some(json!({ "name": "Another", "owner_id": w.member.id })),
            allowed: &[R::CompanyOwner],
            ok: StatusCode::CREATED,
            refused: denied,
        },
        Case {
            name: "archive a project",
            method: Method::POST,
            path: |w| format!("{C}/projects/{}/archive", w.project),
            body: |_| None,
            allowed: &[R::CompanyOwner],
            ok: StatusCode::NO_CONTENT,
            refused: denied,
        },
        Case {
            name: "restore a project",
            method: Method::POST,
            path: |w| format!("{C}/projects/{}/restore", w.project),
            body: |_| None,
            allowed: &[R::CompanyOwner],
            ok: StatusCode::NO_CONTENT,
            refused: denied,
        },
        Case {
            name: "delete a project",
            method: Method::DELETE,
            path: |w| format!("{C}/projects/{}", w.project),
            body: |_| Some(json!({ "confirm": "Website relaunch" })),
            allowed: &[R::CompanyOwner],
            ok: StatusCode::NO_CONTENT,
            refused: |r| match r {
                R::Outsider => StatusCode::NOT_FOUND,
                _ => StatusCode::FORBIDDEN,
            },
        },
        Case {
            name: "add a project owner",
            method: Method::POST,
            path: |w| format!("{C}/projects/{}/owners", w.project),
            body: |w| Some(json!({ "user_id": w.member.id })),
            allowed: &[R::CompanyOwner],
            ok: StatusCode::NO_CONTENT,
            refused: denied,
        },
    ]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_role_gets_exactly_the_access_the_plan_gives_it() {
    for case in cases() {
        let w = world().await;
        let path = (case.path)(&w);
        let mut order: Vec<R> = ALL
            .iter()
            .copied()
            .filter(|r| !case.allowed.contains(r))
            .collect();
        order.extend(case.allowed.iter().copied());
        for r in order {
            let got = w.call(r, case.method.clone(), &path, (case.body)(&w)).await;
            let want = if case.allowed.contains(&r) {
                case.ok
            } else {
                (case.refused)(r)
            };
            assert_eq!(
                got, want,
                "{:?} {} ({} {})",
                r, case.name, case.method, path
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn observers_cannot_change_anything() {
    let w = world().await;
    let p = w.project;
    let writes: Vec<(Method, String, Option<Value>)> = vec![
        (
            Method::PATCH,
            format!("{C}/projects/{p}"),
            Some(json!({ "name": "Mine now" })),
        ),
        (
            Method::POST,
            format!("{C}/projects/{p}/invites"),
            Some(json!({ "email": "x@example.com", "role": "observer" })),
        ),
        (
            Method::PATCH,
            format!("{C}/projects/{p}/members/{}", w.observer.id),
            Some(json!({ "role": "owner" })),
        ),
        (
            Method::DELETE,
            format!("{C}/projects/{p}/members/{}", w.member.id),
            None,
        ),
    ];
    for (m, path, body) in writes {
        assert_eq!(
            w.call(R::Observer, m.clone(), &path, body).await,
            StatusCode::FORBIDDEN,
            "{m} {path}"
        );
    }
    let view = body_json(
        call(
            &w.app,
            &w.observer,
            Method::GET,
            &format!("{C}/projects/{p}"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(view["role"], "observer");
    assert_eq!(
        view["can"],
        json!({ "edit": false, "manage": false, "invite": false, "create_board": false })
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn member_switches_decide_board_creation_and_inviting() {
    let w = world().await;
    let p = w.project;
    let view = body_json(
        call(
            &w.app,
            &w.member,
            Method::GET,
            &format!("{C}/projects/{p}"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(
        view["can"],
        json!({ "edit": true, "manage": false, "invite": false, "create_board": true })
    );

    w.call(
        R::Owner,
        Method::PATCH,
        &format!("{C}/projects/{p}"),
        Some(json!({ "members_create_boards": false, "members_invite": true })),
    )
    .await;
    let view = body_json(
        call(
            &w.app,
            &w.member,
            Method::GET,
            &format!("{C}/projects/{p}"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(view["can"]["create_board"], false);
    assert_eq!(view["can"]["invite"], true);

    let invite = |role: &str| json!({ "email": format!("{role}@example.com"), "role": role });
    let path = format!("{C}/projects/{p}/invites");
    assert_eq!(
        w.call(R::Member, Method::POST, &path, Some(invite("owner")))
            .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        w.call(R::Member, Method::POST, &path, Some(invite("observer")))
            .await,
        StatusCode::CREATED
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_company_owner_enters_a_project_only_by_adding_themselves_and_it_is_logged() {
    let w = world().await;
    let p = w.project;
    let open = format!("{C}/projects/{p}");
    assert_eq!(
        w.call(R::CompanyOwner, Method::GET, &open, None).await,
        StatusCode::NOT_FOUND
    );
    let listed = body_json(
        call(
            &w.app,
            &w.company_owner,
            Method::GET,
            &format!("{C}/projects"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(listed["projects"][0]["name"], "Website relaunch");
    assert_eq!(
        listed["projects"][0]["owners"][0]["email"],
        "lead@a.example"
    );

    let res = w
        .call(
            R::CompanyOwner,
            Method::POST,
            &format!("{C}/projects/{p}/owners"),
            Some(json!({ "user_id": w.company_owner.id })),
        )
        .await;
    assert_eq!(res, StatusCode::NO_CONTENT);
    assert_eq!(
        w.call(R::CompanyOwner, Method::GET, &open, None).await,
        StatusCode::OK
    );

    let actor = w.company_owner.id;
    let logged: i64 = w
        .app
        .sql(move |c| {
            c.query_row(
                "SELECT count(*) FROM audit_log WHERE action = 'project_joined' AND actor_id = ?1",
                [actor],
                |r| r.get(0),
            )
        })
        .await;
    assert_eq!(logged, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_company_member_cannot_create_a_project_or_see_one_they_are_not_in() {
    let w = world().await;
    let res = w
        .call(
            R::CompanyMember,
            Method::POST,
            &format!("{C}/projects"),
            Some(json!({ "name": "Side project", "owner_id": w.company_member.id })),
        )
        .await;
    assert_eq!(res, StatusCode::FORBIDDEN);
    let mine = body_json(
        call(
            &w.app,
            &w.company_member,
            Method::GET,
            &format!("{C}/my-projects"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(mine["projects"], json!([]));
    let mine = body_json(
        call(
            &w.app,
            &w.observer,
            Method::GET,
            &format!("{C}/my-projects"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(mine["projects"][0]["role"], "observer");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_project_keeps_at_least_one_owner() {
    let w = world().await;
    let p = w.project;
    let path = format!("{C}/projects/{p}/members/{}", w.owner.id);
    assert_eq!(
        w.call(
            R::Owner,
            Method::PATCH,
            &path,
            Some(json!({ "role": "member" }))
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        w.call(R::Owner, Method::DELETE, &path, None).await,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deleted_projects_wait_thirty_days_in_trash_then_go() {
    let w = world().await;
    let p = w.project;
    let wrong = w
        .call(
            R::CompanyOwner,
            Method::DELETE,
            &format!("{C}/projects/{p}"),
            Some(json!({ "confirm": "website" })),
        )
        .await;
    assert_eq!(wrong, StatusCode::BAD_REQUEST);
    let res = w
        .call(
            R::CompanyOwner,
            Method::DELETE,
            &format!("{C}/projects/{p}"),
            Some(json!({ "confirm": "Website relaunch" })),
        )
        .await;
    assert_eq!(res, StatusCode::NO_CONTENT);
    assert_eq!(
        w.call(R::Owner, Method::GET, &format!("{C}/projects/{p}"), None)
            .await,
        StatusCode::NOT_FOUND
    );

    let db = w.app.state.dbs.tenant("company-a").unwrap();
    let count = |db: tazko::db::Database| async move {
        db.read(|c| Ok(c.query_row("SELECT count(*) FROM projects", [], |r| r.get::<_, i64>(0))?))
            .await
            .unwrap()
    };
    let now = tazko::session::now();
    tazko::projects::purge(&db, now + 29 * 86_400)
        .await
        .unwrap();
    assert_eq!(count(db.clone()).await, 1);
    tazko::projects::purge(&db, now + 31 * 86_400)
        .await
        .unwrap();
    assert_eq!(count(db.clone()).await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_archived_project_is_closed_to_its_members_until_restored() {
    let w = world().await;
    let p = w.project;
    w.call(
        R::CompanyOwner,
        Method::POST,
        &format!("{C}/projects/{p}/archive"),
        None,
    )
    .await;
    assert_eq!(
        w.call(R::Owner, Method::GET, &format!("{C}/projects/{p}"), None)
            .await,
        StatusCode::NOT_FOUND
    );
    w.call(
        R::CompanyOwner,
        Method::POST,
        &format!("{C}/projects/{p}/restore"),
        None,
    )
    .await;
    assert_eq!(
        w.call(R::Owner, Method::GET, &format!("{C}/projects/{p}"), None)
            .await,
        StatusCode::OK
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn someone_invited_to_one_project_joins_the_company_and_sees_only_that_project() {
    let w = world().await;
    let p = w.project;
    let other = body_json(
        call(
            &w.app,
            &w.company_owner,
            Method::POST,
            &format!("{C}/projects"),
            Some(json!({ "name": "Hidden", "owner_id": w.owner.id })),
        )
        .await,
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let res = w
        .call(
            R::Owner,
            Method::POST,
            &format!("{C}/projects/{p}/invites"),
            Some(json!({ "email": "Guest@Gmail.com", "role": "observer" })),
        )
        .await;
    assert_eq!(res, StatusCode::CREATED);
    let listed = body_json(
        call(
            &w.app,
            &w.owner,
            Method::GET,
            &format!("{C}/projects/{p}/members"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(listed["invites"][0]["email"], "guest@gmail.com");
    assert_eq!(listed["invites"][0]["role"], "observer");

    let guest = person(&w.app, "guest@gmail.com").await;
    let me = body_json(w.app.get("/api/v1/me", Some(&guest.cookie)).await).await;
    assert_eq!(me["companies"][0]["role"], "member");
    let mine = body_json(
        call(
            &w.app,
            &guest,
            Method::GET,
            &format!("{C}/my-projects"),
            None,
        )
        .await,
    )
    .await;
    let names: Vec<&str> = mine["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Website relaunch"]);
    assert_eq!(
        call(
            &w.app,
            &guest,
            Method::GET,
            &format!("{C}/projects/{other}"),
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let again = body_json(
        call(
            &w.app,
            &guest,
            Method::GET,
            &format!("{C}/my-projects"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(again["projects"].as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_changes_need_the_csrf_token() {
    let w = world().await;
    let p = w.project;
    let res = w
        .app
        .send(
            axum::http::Request::patch(format!("{C}/projects/{p}"))
                .header(axum::http::header::COOKIE, &w.owner.cookie)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    json!({ "name": "Changed" }).to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
