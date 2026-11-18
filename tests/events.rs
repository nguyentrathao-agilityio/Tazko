mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, Who, body_text, call, person};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";

struct World {
    app: TestApp,
    boss: Who,
    owner: Who,
    member: Who,
}

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let boss = person(&app, "boss@a.example").await;
    let owner = person(&app, "lead@a.example").await;
    let member = person(&app, "dev@a.example").await;
    app.member(a, boss.id, "owner").await;
    app.member(a, owner.id, "member").await;
    app.member(a, member.id, "member").await;
    World {
        app,
        boss,
        owner,
        member,
    }
}

async fn events(
    app: &TestApp,
) -> (
    i64,
    Option<(String, Option<i64>, Option<i64>, Option<i64>, Option<i64>)>,
) {
    let db = app.state.dbs.tenant("company-a").unwrap();
    db.read(|c| {
        let n: i64 = c.query_row("SELECT count(*) FROM events", [], |r| r.get(0))?;
        let last = c
            .query_row(
                "SELECT type, actor_id, project_id, board_id, card_id FROM events ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .ok();
        Ok((n, last))
    })
    .await
    .unwrap()
}

#[derive(Clone, Copy)]
enum Scope {
    Project,
    Board,
    Card,
}

async fn one(
    w: &World,
    who: &Who,
    method: Method,
    path: &str,
    body: Option<Value>,
    kind: &str,
    scope: Scope,
) -> Value {
    let (before, _) = events(&w.app).await;
    let res = call(&w.app, who, method.clone(), path, body).await;
    let status = res.status();
    let text = body_text(res).await;
    assert!(status.is_success(), "{method} {path}: {status} {text}");
    let (after, last) = events(&w.app).await;
    assert_eq!(
        after - before,
        1,
        "{method} {path} should write exactly one event"
    );
    let (t, actor, project, board, card) = last.unwrap();
    assert_eq!(t, kind, "{method} {path}");
    assert_eq!(actor, Some(who.id), "{method} {path} names who did it");
    assert!(project.is_some(), "{method} {path} has a project");
    match scope {
        Scope::Project => {}
        Scope::Board => assert!(board.is_some(), "{method} {path} has a board"),
        Scope::Card => assert!(
            board.is_some() && card.is_some(),
            "{method} {path} has a card"
        ),
    }
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

async fn none(w: &World, who: &Who, method: Method, path: &str, body: Option<Value>) -> StatusCode {
    let (before, _) = events(&w.app).await;
    let status = call(&w.app, who, method, path, body).await.status();
    let (after, _) = events(&w.app).await;
    assert_eq!(after, before, "a refused change writes no event");
    status
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn each_change_writes_exactly_one_event_in_its_transaction() {
    use Scope::*;
    let w = world().await;
    let (o, m, boss) = (&w.owner, &w.member, &w.boss);

    let p = one(
        &w,
        boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "Site", "owner_id": o.id })),
        "project_created",
        Project,
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let pp = format!("{C}/projects/{p}");
    one(
        &w,
        o,
        Method::PATCH,
        &pp,
        Some(json!({ "description": "x" })),
        "project_updated",
        Project,
    )
    .await;
    one(
        &w,
        o,
        Method::POST,
        &format!("{pp}/invites"),
        Some(json!({ "email": "dev@a.example", "role": "observer" })),
        "member_added",
        Project,
    )
    .await;
    one(
        &w,
        o,
        Method::PATCH,
        &format!("{pp}/members/{}", m.id),
        Some(json!({ "role": "member" })),
        "member_role_changed",
        Project,
    )
    .await;
    one(
        &w,
        boss,
        Method::POST,
        &format!("{pp}/owners"),
        Some(json!({ "user_id": boss.id })),
        "owner_added",
        Project,
    )
    .await;
    one(
        &w,
        boss,
        Method::POST,
        &format!("{pp}/archive"),
        None,
        "project_archived",
        Project,
    )
    .await;
    one(
        &w,
        boss,
        Method::POST,
        &format!("{pp}/restore"),
        None,
        "project_restored",
        Project,
    )
    .await;

    let b = one(
        &w,
        m,
        Method::POST,
        &format!("{pp}/boards"),
        Some(json!({ "name": "B" })),
        "board_created",
        Board,
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let bp = format!("{C}/boards/{b}");
    one(
        &w,
        m,
        Method::PATCH,
        &bp,
        Some(json!({ "name": "B2" })),
        "board_updated",
        Board,
    )
    .await;
    one(
        &w,
        m,
        Method::PUT,
        &format!("{bp}/star"),
        Some(json!({ "starred": true })),
        "board_starred",
        Board,
    )
    .await;
    one(
        &w,
        o,
        Method::PUT,
        &format!("{bp}/access"),
        Some(json!({ "restricted": true, "people": [m.id] })),
        "board_access_changed",
        Board,
    )
    .await;
    one(
        &w,
        o,
        Method::POST,
        &format!("{bp}/close"),
        None,
        "board_closed",
        Board,
    )
    .await;
    one(
        &w,
        o,
        Method::POST,
        &format!("{bp}/reopen"),
        None,
        "board_reopened",
        Board,
    )
    .await;
    one(
        &w,
        m,
        Method::POST,
        &format!("{bp}/copy"),
        Some(json!({ "name": "B3" })),
        "board_created",
        Board,
    )
    .await;

    let list = one(
        &w,
        m,
        Method::POST,
        &format!("{bp}/lists"),
        Some(json!({ "title": "L" })),
        "list_created",
        Board,
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let (_, v) = {
        let res = call(&w.app, m, Method::GET, &bp, None).await;
        (
            (),
            serde_json::from_str::<Value>(&body_text(res).await).unwrap(),
        )
    };
    let first = v["lists"][0]["id"].as_i64().unwrap();
    let lp = format!("{bp}/lists/{list}");
    one(
        &w,
        m,
        Method::PATCH,
        &lp,
        Some(json!({ "title": "L2" })),
        "list_renamed",
        Board,
    )
    .await;
    one(
        &w,
        m,
        Method::POST,
        &format!("{lp}/move"),
        Some(json!({ "after": null })),
        "list_moved",
        Board,
    )
    .await;
    one(
        &w,
        m,
        Method::POST,
        &format!("{lp}/copy"),
        Some(json!({ "title": "L3" })),
        "list_copied",
        Board,
    )
    .await;
    one(
        &w,
        m,
        Method::POST,
        &format!("{lp}/archive"),
        None,
        "list_archived",
        Board,
    )
    .await;
    one(
        &w,
        m,
        Method::POST,
        &format!("{lp}/restore"),
        None,
        "list_restored",
        Board,
    )
    .await;

    let card = one(
        &w,
        m,
        Method::POST,
        &format!("{bp}/cards"),
        Some(json!({ "list_id": list, "title": "C" })),
        "card_created",
        Card,
    )
    .await["cards"][0]["id"]
        .as_i64()
        .unwrap();
    one(
        &w,
        m,
        Method::POST,
        &format!("{bp}/cards"),
        Some(json!({ "list_id": list, "titles": ["a", "b"] })),
        "cards_created",
        Board,
    )
    .await;
    let cp = format!("{bp}/cards/{card}");
    one(
        &w,
        m,
        Method::PATCH,
        &cp,
        Some(json!({ "title": "C2" })),
        "card_renamed",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::PATCH,
        &cp,
        Some(json!({ "done": true })),
        "card_done",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::PATCH,
        &cp,
        Some(json!({ "description_html": "<p>x</p>", "due_at": 2_000_000_000 })),
        "card_updated",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::POST,
        &format!("{cp}/move"),
        Some(json!({ "list_id": first, "after": null })),
        "card_moved",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::POST,
        &format!("{cp}/copy"),
        Some(json!({ "title": "C3", "list_id": first })),
        "card_copied",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::POST,
        &format!("{cp}/archive"),
        None,
        "card_archived",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::POST,
        &format!("{cp}/restore"),
        None,
        "card_restored",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::POST,
        &format!("{lp}/sort"),
        Some(json!({ "by": "name" })),
        "list_sorted",
        Board,
    )
    .await;
    one(
        &w,
        m,
        Method::POST,
        &format!("{lp}/move-cards"),
        Some(json!({ "to": first })),
        "cards_moved",
        Board,
    )
    .await;

    let label = one(
        &w,
        m,
        Method::POST,
        &format!("{bp}/labels"),
        Some(json!({ "name": "Ops", "color": 2 })),
        "label_created",
        Board,
    )
    .await["id"]
        .as_i64()
        .unwrap();
    one(
        &w,
        m,
        Method::PATCH,
        &format!("{bp}/labels/{label}"),
        Some(json!({ "name": "Ops2" })),
        "label_changed",
        Board,
    )
    .await;
    one(
        &w,
        m,
        Method::PUT,
        &format!("{cp}/labels/{label}"),
        Some(json!({ "on": true })),
        "label_added",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::PUT,
        &format!("{cp}/labels/{label}"),
        Some(json!({ "on": false })),
        "label_removed",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::DELETE,
        &format!("{bp}/labels/{label}"),
        None,
        "label_deleted",
        Board,
    )
    .await;
    one(
        &w,
        m,
        Method::PUT,
        &format!("{cp}/members/{}", m.id),
        Some(json!({ "on": true })),
        "member_assigned",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::PUT,
        &format!("{cp}/members/{}", m.id),
        Some(json!({ "on": false })),
        "member_unassigned",
        Card,
    )
    .await;

    let ck = one(
        &w,
        m,
        Method::POST,
        &format!("{cp}/checklists"),
        Some(json!({ "title": "K" })),
        "checklist_added",
        Card,
    )
    .await["id"]
        .as_i64()
        .unwrap();
    one(
        &w,
        m,
        Method::POST,
        &format!("{cp}/checklists"),
        Some(json!({ "title": "K2" })),
        "checklist_added",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::PATCH,
        &format!("{bp}/checklists/{ck}"),
        Some(json!({ "title": "K1" })),
        "checklist_renamed",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::POST,
        &format!("{bp}/checklists/{ck}/move"),
        Some(json!({ "after": null })),
        "checklist_moved",
        Card,
    )
    .await;
    let item = one(
        &w,
        m,
        Method::POST,
        &format!("{bp}/checklists/{ck}/items"),
        Some(json!({ "text": "i" })),
        "item_added",
        Card,
    )
    .await["id"]
        .as_i64()
        .unwrap();
    one(
        &w,
        m,
        Method::POST,
        &format!("{bp}/checklists/{ck}/items"),
        Some(json!({ "text": "j" })),
        "item_added",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::PATCH,
        &format!("{bp}/items/{item}"),
        Some(json!({ "done": true })),
        "item_checked",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::PATCH,
        &format!("{bp}/items/{item}"),
        Some(json!({ "done": false })),
        "item_unchecked",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::PATCH,
        &format!("{bp}/items/{item}"),
        Some(json!({ "text": "i2" })),
        "item_changed",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::POST,
        &format!("{bp}/items/{item}/move"),
        Some(json!({ "after": null })),
        "item_moved",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::DELETE,
        &format!("{bp}/items/{item}"),
        None,
        "item_deleted",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::DELETE,
        &format!("{bp}/checklists/{ck}"),
        None,
        "checklist_deleted",
        Card,
    )
    .await;

    let cm = one(
        &w,
        m,
        Method::POST,
        &format!("{cp}/comments"),
        Some(json!({ "body": "hi" })),
        "comment_added",
        Card,
    )
    .await["id"]
        .as_i64()
        .unwrap();
    one(
        &w,
        m,
        Method::PATCH,
        &format!("{bp}/comments/{cm}"),
        Some(json!({ "body": "hey" })),
        "comment_edited",
        Card,
    )
    .await;
    one(
        &w,
        m,
        Method::DELETE,
        &format!("{bp}/comments/{cm}"),
        None,
        "comment_deleted",
        Card,
    )
    .await;

    one(
        &w,
        m,
        Method::POST,
        &format!("{lp}/archive-cards"),
        None,
        "cards_archived",
        Board,
    )
    .await;
    one(
        &w,
        o,
        Method::DELETE,
        &format!("{pp}/members/{}", boss.id),
        None,
        "member_removed",
        Project,
    )
    .await;
    one(
        &w,
        boss,
        Method::DELETE,
        &pp,
        Some(json!({ "confirm": "Site" })),
        "project_deleted",
        Project,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn refused_and_unchanged_requests_write_no_event() {
    let w = world().await;
    let res = call(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "Site", "owner_id": w.owner.id })),
    )
    .await;
    let p: i64 = serde_json::from_str::<Value>(&body_text(res).await).unwrap()["id"]
        .as_i64()
        .unwrap();
    let res = call(
        &w.app,
        &w.owner,
        Method::POST,
        &format!("{C}/projects/{p}/boards"),
        Some(json!({ "name": "B" })),
    )
    .await;
    let b: i64 = serde_json::from_str::<Value>(&body_text(res).await).unwrap()["id"]
        .as_i64()
        .unwrap();
    let bp = format!("{C}/boards/{b}");

    assert_eq!(
        none(
            &w,
            &w.member,
            Method::PATCH,
            &bp,
            Some(json!({ "name": "x" }))
        )
        .await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        none(
            &w,
            &w.owner,
            Method::PATCH,
            &bp,
            Some(json!({ "name": "" }))
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        none(
            &w,
            &w.owner,
            Method::POST,
            &format!("{bp}/lists/999/move"),
            Some(json!({ "after": null }))
        )
        .await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        none(
            &w,
            &w.owner,
            Method::POST,
            &format!("{bp}/cards"),
            Some(json!({ "list_id": 999, "title": "x" }))
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        none(
            &w,
            &w.owner,
            Method::DELETE,
            &format!("{C}/projects/{p}/members/{}", w.owner.id),
            None
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    call(
        &w.app,
        &w.owner,
        Method::PUT,
        &format!("{bp}/star"),
        Some(json!({ "starred": true })),
    )
    .await;
    assert_eq!(
        none(
            &w,
            &w.owner,
            Method::PUT,
            &format!("{bp}/star"),
            Some(json!({ "starred": true }))
        )
        .await,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn board_activity_lists_changes_newest_first_without_personal_stars() {
    let w = world().await;
    let res = call(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "Site", "owner_id": w.owner.id })),
    )
    .await;
    let p: i64 = serde_json::from_str::<Value>(&body_text(res).await).unwrap()["id"]
        .as_i64()
        .unwrap();
    let res = call(
        &w.app,
        &w.owner,
        Method::POST,
        &format!("{C}/projects/{p}/boards"),
        Some(json!({ "name": "B" })),
    )
    .await;
    let b: i64 = serde_json::from_str::<Value>(&body_text(res).await).unwrap()["id"]
        .as_i64()
        .unwrap();
    let bp = format!("{C}/boards/{b}");
    let res = call(&w.app, &w.owner, Method::GET, &bp, None).await;
    let list = serde_json::from_str::<Value>(&body_text(res).await).unwrap()["lists"][0]["id"]
        .as_i64()
        .unwrap();
    for i in 0..60 {
        call(
            &w.app,
            &w.owner,
            Method::POST,
            &format!("{bp}/cards"),
            Some(json!({ "list_id": list, "title": format!("C{i}") })),
        )
        .await;
    }
    call(
        &w.app,
        &w.owner,
        Method::PUT,
        &format!("{bp}/star"),
        Some(json!({ "starred": true })),
    )
    .await;

    let res = call(
        &w.app,
        &w.owner,
        Method::GET,
        &format!("{bp}/activity"),
        None,
    )
    .await;
    let page: Value = serde_json::from_str(&body_text(res).await).unwrap();
    let entries = page["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 50);
    assert_eq!(entries[0]["type"], "card_created");
    assert_eq!(entries[0]["card"]["title"], "C59");
    assert_eq!(entries[0]["actor"]["id"], w.owner.id);
    let last = entries[49]["id"].as_i64().unwrap();
    let res = call(
        &w.app,
        &w.owner,
        Method::GET,
        &format!("{bp}/activity?before={last}"),
        None,
    )
    .await;
    let more: Value = serde_json::from_str(&body_text(res).await).unwrap();
    assert_eq!(
        more["entries"].as_array().unwrap().len(),
        11,
        "60 cards and the board itself"
    );
    assert!(
        more["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["type"] != "board_starred")
    );
}
