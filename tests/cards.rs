mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, Who, body_text, call, person};
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";

struct World {
    app: TestApp,
    member: Who,
    observer: Who,
    board: i64,
    lists: Vec<i64>,
}

async fn req(
    app: &TestApp,
    who: &Who,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let res = call(app, who, method, path, body).await;
    let status = res.status();
    let text = body_text(res).await;
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let boss = person(&app, "boss@a.example").await;
    let member = person(&app, "dev@a.example").await;
    let observer = person(&app, "viewer@a.example").await;
    app.member(a, boss.id, "owner").await;
    app.member(a, member.id, "member").await;
    app.member(a, observer.id, "member").await;
    let (_, v) = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "P", "owner_id": member.id })),
    )
    .await;
    let project = v["id"].as_i64().unwrap();
    req(
        &app,
        &member,
        Method::POST,
        &format!("{C}/projects/{project}/invites"),
        Some(json!({ "email": "viewer@a.example", "role": "observer" })),
    )
    .await;
    let (_, v) = req(
        &app,
        &member,
        Method::POST,
        &format!("{C}/projects/{project}/boards"),
        Some(json!({ "name": "B" })),
    )
    .await;
    let board = v["id"].as_i64().unwrap();
    let (_, v) = req(
        &app,
        &member,
        Method::GET,
        &format!("{C}/boards/{board}"),
        None,
    )
    .await;
    let lists = v["lists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["id"].as_i64().unwrap())
        .collect();
    World {
        app,
        member,
        observer,
        board,
        lists,
    }
}

impl World {
    fn base(&self) -> String {
        format!("{C}/boards/{}", self.board)
    }

    async fn add(&self, list: i64, titles: &[&str]) -> Vec<i64> {
        let (s, v) = req(
            &self.app,
            &self.member,
            Method::POST,
            &format!("{}/cards", self.base()),
            Some(json!({ "list_id": list, "titles": titles })),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED, "{v}");
        v["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_i64().unwrap())
            .collect()
    }

    async fn layout(&self) -> Vec<(i64, Vec<i64>, Vec<String>)> {
        let (_, v) = req(&self.app, &self.member, Method::GET, &self.base(), None).await;
        v["lists"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| {
                let cards = l["cards"].as_array().unwrap();
                (
                    l["id"].as_i64().unwrap(),
                    cards.iter().map(|c| c["id"].as_i64().unwrap()).collect(),
                    cards
                        .iter()
                        .map(|c| c["position"].as_str().unwrap().to_string())
                        .collect(),
                )
            })
            .collect()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn moving_200_cards_makes_no_ordering_collisions() {
    let w = world().await;
    let titles: Vec<String> = (0..200).map(|i| format!("Card {i}")).collect();
    let refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    let mut ids = w.add(w.lists[0], &refs[..100]).await;
    ids.extend(w.add(w.lists[0], &refs[100..]).await);
    let mut expected: Vec<Vec<i64>> = vec![ids, Vec::new(), Vec::new()];
    let mut rng = rand::rngs::StdRng::seed_from_u64(15);

    for round in 0..260 {
        let from = *[0usize, 1, 2]
            .iter()
            .filter(|i| !expected[**i].is_empty())
            .collect::<Vec<_>>()
            .choose(&mut rng)
            .unwrap();
        let idx = rng.gen_range(0..expected[*from].len());
        let card = expected[*from].remove(idx);
        let to = rng.gen_range(0..3);
        let slot = if round % 5 == 0 {
            0
        } else {
            rng.gen_range(0..=expected[to].len())
        };
        let after = if slot == 0 {
            None
        } else {
            Some(expected[to][slot - 1])
        };
        let (s, v) = req(
            &w.app,
            &w.member,
            Method::POST,
            &format!("{}/cards/{card}/move", w.base()),
            Some(json!({ "list_id": w.lists[to], "after": after })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        expected[to].insert(slot, card);
    }

    let layout = w.layout().await;
    for (i, (_, cards, positions)) in layout.iter().enumerate() {
        assert_eq!(cards, &expected[i], "list {i} order");
        let mut sorted = positions.clone();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            positions.len(),
            "list {i} has two cards on one key"
        );
        assert!(positions.windows(2).all(|p| p[0] < p[1]));
    }
    let total: usize = layout.iter().map(|l| l.1.len()).sum();
    assert_eq!(total, 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pasting_several_lines_makes_one_card_per_line_in_order() {
    let w = world().await;
    w.add(w.lists[1], &["First"]).await;
    let ids = w
        .add(w.lists[1], &["Buy milk", "  ", "Call Jo", "Book room"])
        .await;
    assert_eq!(ids.len(), 3);
    let (s, v) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{}/cards", w.base()),
        Some(json!({ "list_id": w.lists[1], "titles": ["Top 1", "Top 2"], "top": true })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let (_, v) = req(&w.app, &w.member, Method::GET, &w.base(), None).await;
    let titles: Vec<&str> = v["lists"][1]["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        vec![
            "Top 1",
            "Top 2",
            "First",
            "Buy milk",
            "Call Jo",
            "Book room"
        ]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn list_actions_sort_move_all_and_archive_all() {
    let w = world().await;
    w.add(w.lists[0], &["pear", "Apple", "mango"]).await;
    let base = w.base();
    let (s, _) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{base}/lists/{}/sort", w.lists[0]),
        Some(json!({ "by": "name" })),
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (_, v) = req(&w.app, &w.member, Method::GET, &base, None).await;
    let titles: Vec<&str> = v["lists"][0]["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Apple", "mango", "pear"]);

    req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{base}/lists/{}/move-cards", w.lists[0]),
        Some(json!({ "to": w.lists[2] })),
    )
    .await;
    let layout = w.layout().await;
    assert!(layout[0].1.is_empty());
    assert_eq!(layout[2].1.len(), 3);

    req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{base}/lists/{}/archive-cards", w.lists[2]),
        None,
    )
    .await;
    let (_, v) = req(
        &w.app,
        &w.member,
        Method::GET,
        &format!("{base}/archive"),
        None,
    )
    .await;
    assert_eq!(v["cards"].as_array().unwrap().len(), 3);
    let card = v["cards"][0]["id"].as_i64().unwrap();
    req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{base}/cards/{card}/restore"),
        None,
    )
    .await;
    assert_eq!(w.layout().await[2].1, vec![card]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn copying_a_list_or_board_copies_its_open_cards() {
    let w = world().await;
    let ids = w.add(w.lists[0], &["One", "Two"]).await;
    let base = w.base();
    req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{base}/cards/{}/archive", ids[1]),
        None,
    )
    .await;
    let (s, v) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{base}/lists/{}/copy", w.lists[0]),
        Some(json!({ "title": "Copy" })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let layout = w.layout().await;
    assert_eq!(layout[1].1.len(), 1, "only open cards are copied");

    let (s, v) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{base}/copy"),
        Some(json!({ "name": "B2" })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let copy = v["id"].as_i64().unwrap();
    let (_, v) = req(
        &w.app,
        &w.member,
        Method::GET,
        &format!("{C}/boards/{copy}"),
        None,
    )
    .await;
    let n: usize = v["lists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["cards"].as_array().unwrap().len())
        .sum();
    assert_eq!(n, 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cards_stay_on_their_board_and_observers_cannot_change_them() {
    let w = world().await;
    let ids = w.add(w.lists[0], &["Mine"]).await;
    let card = ids[0];
    let base = w.base();
    let writes: Vec<(Method, String, Option<Value>)> = vec![
        (
            Method::POST,
            format!("{base}/cards"),
            Some(json!({ "list_id": w.lists[0], "title": "x" })),
        ),
        (
            Method::PATCH,
            format!("{base}/cards/{card}"),
            Some(json!({ "title": "x" })),
        ),
        (
            Method::POST,
            format!("{base}/cards/{card}/move"),
            Some(json!({ "list_id": w.lists[1], "after": null })),
        ),
        (Method::POST, format!("{base}/cards/{card}/archive"), None),
        (
            Method::POST,
            format!("{base}/cards/{card}/copy"),
            Some(json!({ "title": "x", "list_id": w.lists[0] })),
        ),
        (
            Method::POST,
            format!("{base}/lists/{}/sort", w.lists[0]),
            Some(json!({ "by": "name" })),
        ),
        (
            Method::POST,
            format!("{base}/lists/{}/move-cards", w.lists[0]),
            Some(json!({ "to": w.lists[1] })),
        ),
        (
            Method::POST,
            format!("{base}/lists/{}/archive-cards", w.lists[0]),
            None,
        ),
    ];
    for (m, path, body) in writes {
        let (s, _) = req(&w.app, &w.observer, m.clone(), &path, body).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{m} {path}");
    }

    let (_, v) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{C}/projects/1/boards"),
        Some(json!({ "name": "Other" })),
    )
    .await;
    let other = v["id"].as_i64().unwrap();
    let (_, v) = req(
        &w.app,
        &w.member,
        Method::GET,
        &format!("{C}/boards/{other}"),
        None,
    )
    .await;
    let other_list = v["lists"][0]["id"].as_i64().unwrap();
    let (s, _) = req(
        &w.app,
        &w.member,
        Method::PATCH,
        &format!("{C}/boards/{other}/cards/{card}"),
        Some(json!({ "title": "x" })),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::NOT_FOUND,
        "a card is reached only through its own board"
    );
    let (s, _) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{base}/cards/{card}/move"),
        Some(json!({ "list_id": other_list, "after": null })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn changes_inside_a_card_move_its_last_updated_time() {
    let app = common::TestApp::new();
    let a = app.company("company-a", "active").await;
    let owner = common::person(&app, "lead@a.example").await;
    app.member(a, owner.id, "owner").await;
    let c = "/api/v1/c/company-a";
    let res = common::call(
        &app,
        &owner,
        axum::http::Method::POST,
        &format!("{c}/projects"),
        Some(serde_json::json!({ "name": "P", "owner_id": owner.id })),
    )
    .await;
    let project = common::body_json(res).await["id"].as_i64().unwrap();
    let res = common::call(
        &app,
        &owner,
        axum::http::Method::POST,
        &format!("{c}/projects/{project}/boards"),
        Some(serde_json::json!({ "name": "B" })),
    )
    .await;
    let board = common::body_json(res).await["id"].as_i64().unwrap();
    let view = common::body_json(
        common::call(
            &app,
            &owner,
            axum::http::Method::GET,
            &format!("{c}/boards/{board}"),
            None,
        )
        .await,
    )
    .await;
    let list = view["lists"][0]["id"].as_i64().unwrap();
    let res = common::call(
        &app,
        &owner,
        axum::http::Method::POST,
        &format!("{c}/boards/{board}/cards"),
        Some(serde_json::json!({ "list_id": list, "title": "T" })),
    )
    .await;
    let card = common::body_json(res).await["cards"][0]["id"]
        .as_i64()
        .unwrap();
    let db = app.state.dbs.tenant("company-a").unwrap();
    let set_old = |db: tazko::db::Database| async move {
        db.write(move |x| {
            x.execute("UPDATE cards SET updated_at = 1 WHERE id = ?1", [card])?;
            Ok(())
        })
        .await
        .unwrap()
    };
    let stamp = |db: tazko::db::Database| async move {
        db.read(move |x| {
            Ok(
                x.query_row("SELECT updated_at FROM cards WHERE id = ?1", [card], |r| {
                    r.get::<_, i64>(0)
                })?,
            )
        })
        .await
        .unwrap()
    };
    set_old(db.clone()).await;
    common::call(
        &app,
        &owner,
        axum::http::Method::POST,
        &format!("{c}/boards/{board}/cards/{card}/comments"),
        Some(serde_json::json!({ "body": "hi" })),
    )
    .await;
    assert!(stamp(db.clone()).await > 1, "comment");
    set_old(db.clone()).await;
    let label = view["labels"][0]["id"].as_i64().unwrap();
    common::call(
        &app,
        &owner,
        axum::http::Method::PUT,
        &format!("{c}/boards/{board}/cards/{card}/labels/{label}"),
        Some(serde_json::json!({ "on": true })),
    )
    .await;
    assert!(stamp(db.clone()).await > 1, "label");
    set_old(db.clone()).await;
    let k = common::body_json(
        common::call(
            &app,
            &owner,
            axum::http::Method::POST,
            &format!("{c}/boards/{board}/cards/{card}/checklists"),
            Some(serde_json::json!({ "title": "K" })),
        )
        .await,
    )
    .await["id"]
        .as_i64()
        .unwrap();
    set_old(db.clone()).await;
    common::call(
        &app,
        &owner,
        axum::http::Method::POST,
        &format!("{c}/boards/{board}/checklists/{k}/items"),
        Some(serde_json::json!({ "text": "x" })),
    )
    .await;
    assert!(stamp(db).await > 1, "checklist item");
}

#[tokio::test]
async fn quick_add_sets_labels_people_and_due_date_in_one_step() {
    let w = world().await;
    let (s, label) = req(
        &w.app,
        &w.member,
        Method::POST,
        &format!("{}/labels", w.base()),
        Some(json!({ "name": "bug", "color": 1 })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{label}");
    let label = label["id"].as_i64().unwrap();
    let path = format!("{}/cards", w.base());
    let new = |extra: Value| {
        let mut card = json!({ "title": "Fix login" });
        card.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let body = json!({ "list_id": w.lists[0], "items": [card, { "title": "Second line" }] });
        req(&w.app, &w.member, Method::POST, &path, Some(body))
    };

    let (s, v) =
        new(json!({ "labels": [label], "members": [w.member.id], "due_at": 1_900_000_000 })).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let card = v["cards"][0]["id"].as_i64().unwrap();
    let (_, c) = req(
        &w.app,
        &w.member,
        Method::GET,
        &format!("{}/cards/{card}", w.base()),
        None,
    )
    .await;
    assert_eq!(c["labels"], json!([label]), "{c}");
    assert_eq!(c["members"], json!([w.member.id]), "{c}");
    assert_eq!(c["due_at"], 1_900_000_000);

    for (extra, why) in [
        (json!({ "labels": [9999] }), "label from another board"),
        (json!({ "members": [w.observer.id] }), "observer"),
        (json!({ "due_at": -5 }), "bad date"),
    ] {
        let (s, _) = new(extra).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{why}");
    }
    let (_, v) = req(&w.app, &w.member, Method::GET, &w.base(), None).await;
    let count = v["lists"][0]["cards"].as_array().unwrap().len();
    assert_eq!(count, 2, "refused quick adds create nothing");
    let (_, activity) = req(
        &w.app,
        &w.member,
        Method::GET,
        &format!("{}/activity", w.base()),
        None,
    )
    .await;
    let kinds = activity.to_string();
    assert!(
        kinds.contains("label_added") && kinds.contains("member_assigned"),
        "{kinds}"
    );
}
