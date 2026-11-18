mod common;

use axum::http::Method;
use common::{TestApp, Who, body_text, call, person};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";

async fn req(app: &TestApp, who: &Who, method: Method, path: &str, body: Option<Value>) -> Value {
    let res = call(app, who, method, path, body).await;
    serde_json::from_str(&body_text(res).await).unwrap_or(Value::Null)
}

struct World {
    app: TestApp,
    boss: Who,
    owner: Who,
    member: Who,
    other: Who,
    open_board: i64,
    secret_board: i64,
    other_board: i64,
}

async fn board_with_cards(
    app: &TestApp,
    who: &Who,
    project: i64,
    name: &str,
    restricted: bool,
    cards: &[&str],
) -> (i64, i64) {
    let b = req(
        app,
        who,
        Method::POST,
        &format!("{C}/projects/{project}/boards"),
        Some(json!({ "name": name, "restricted": restricted })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let v = req(app, who, Method::GET, &format!("{C}/boards/{b}"), None).await;
    let list = v["lists"][0]["id"].as_i64().unwrap();
    let r = req(
        app,
        who,
        Method::POST,
        &format!("{C}/boards/{b}/cards"),
        Some(json!({ "list_id": list, "titles": cards })),
    )
    .await;
    (b, r["cards"][0]["id"].as_i64().unwrap())
}

async fn world() -> World {
    let app = TestApp::new();
    let a = app.company("company-a", "active").await;
    let b = app.company("company-b", "active").await;
    let boss = person(&app, "boss@a.example").await;
    let owner = person(&app, "lead@a.example").await;
    let member = person(&app, "dev@a.example").await;
    let other = person(&app, "solo@a.example").await;
    app.member(a, boss.id, "owner").await;
    for w in [&owner, &member, &other] {
        app.member(a, w.id, "member").await;
    }
    app.member(b, member.id, "member").await;

    let p1 = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "Web", "owner_id": owner.id })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    req(
        &app,
        &owner,
        Method::POST,
        &format!("{C}/projects/{p1}/invites"),
        Some(json!({ "email": "dev@a.example", "role": "member" })),
    )
    .await;
    let p2 = req(
        &app,
        &boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "Private", "owner_id": other.id })),
    )
    .await["id"]
        .as_i64()
        .unwrap();

    let (open_board, hero) = board_with_cards(
        &app,
        &owner,
        p1,
        "Launch board",
        false,
        &["Hero banner rework", "Pricing table", "Footer links"],
    )
    .await;
    let (secret_board, _) = board_with_cards(
        &app,
        &owner,
        p1,
        "Salaries",
        true,
        &["Hero salary review", "Bonus plan"],
    )
    .await;
    let (other_board, _) =
        board_with_cards(&app, &other, p2, "Side", false, &["Hero side quest"]).await;

    let path = format!("{C}/boards/{open_board}/cards/{hero}");
    req(&app, &owner, Method::PATCH, &path, Some(json!({ "description_html": "<p>New <b>gradient</b> for the brand</p>", "due_at": 1_000_000 }))).await;
    req(
        &app,
        &owner,
        Method::PUT,
        &format!("{path}/members/{}", member.id),
        Some(json!({ "on": true })),
    )
    .await;
    let labels = req(
        &app,
        &owner,
        Method::GET,
        &format!("{C}/boards/{open_board}"),
        None,
    )
    .await["labels"]
        .clone();
    let bug = labels
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "Bug")
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    req(
        &app,
        &owner,
        Method::PUT,
        &format!("{path}/labels/{bug}"),
        Some(json!({ "on": true })),
    )
    .await;

    let company_b = app.state.dbs.tenant("company-b").unwrap();
    company_b
        .write(|c| {
            c.execute_batch(
                "INSERT INTO projects (name) VALUES ('B');
                 INSERT INTO boards (project_id, name) VALUES (1, 'Hero B');
                 INSERT INTO lists (board_id, title, position) VALUES (1, 'L', 'V');
                 INSERT INTO cards (board_id, list_id, title, position) VALUES (1, 1, 'Hero in another company', 'V');",
            )?;
            Ok(())
        })
        .await
        .unwrap();
    World {
        app,
        boss,
        owner,
        member,
        other,
        open_board,
        secret_board,
        other_board,
    }
}

fn titles(v: &Value) -> Vec<String> {
    let mut t: Vec<String> = v["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap().to_string())
        .collect();
    t.sort();
    t
}

fn boards_of(v: &Value) -> Vec<i64> {
    v["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["board"]["id"].as_i64().unwrap())
        .collect()
}

async fn search(w: &World, who: &Who, q: &str) -> Value {
    req(
        &w.app,
        who,
        Method::GET,
        &format!("{C}/search?q={}", urlencode(q)),
        None,
    )
    .await
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn search_never_returns_cards_from_boards_the_person_cannot_see() {
    let w = world().await;
    let member = search(&w, &w.member, "hero").await;
    assert_eq!(titles(&member), vec!["Hero banner rework"]);
    assert!(boards_of(&member).iter().all(|b| *b == w.open_board));

    let owner = search(&w, &w.owner, "hero").await;
    assert_eq!(
        titles(&owner),
        vec!["Hero banner rework", "Hero salary review"]
    );

    let other = search(&w, &w.other, "hero").await;
    assert_eq!(titles(&other), vec!["Hero side quest"]);
    assert_eq!(boards_of(&other), vec![w.other_board]);

    let boss = search(&w, &w.boss, "hero").await;
    assert_eq!(
        boss["cards"],
        json!([]),
        "company owners see only projects they are in"
    );

    for who in [&w.member, &w.other, &w.boss] {
        for q in [
            "salary",
            "bonus",
            "salaries",
            "is:archived salary",
            "@dev",
            "#bug salary",
        ] {
            let v = search(&w, who, q).await;
            assert!(!boards_of(&v).contains(&w.secret_board), "{q}");
            assert!(
                v["boards"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|b| b["id"] != w.secret_board),
                "{q}"
            );
        }
    }
    assert!(titles(&search(&w, &w.member, "another").await).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn operators_filter_by_member_label_dates_and_archive() {
    let w = world().await;
    assert_eq!(
        titles(&search(&w, &w.member, "gradient").await),
        vec!["Hero banner rework"],
        "description text is indexed"
    );
    assert_eq!(
        titles(&search(&w, &w.member, "@dev").await),
        vec!["Hero banner rework"]
    );
    assert_eq!(
        titles(&search(&w, &w.member, "@me").await),
        vec!["Hero banner rework"]
    );
    assert_eq!(
        titles(&search(&w, &w.member, "#bug").await),
        vec!["Hero banner rework"]
    );
    assert_eq!(
        titles(&search(&w, &w.member, "is:overdue").await),
        vec!["Hero banner rework"]
    );
    assert!(titles(&search(&w, &w.member, "has:attachments").await).is_empty());
    assert_eq!(
        titles(&search(&w, &w.member, "pric").await),
        vec!["Pricing table"],
        "words match as prefixes"
    );

    let v = req(
        &w.app,
        &w.owner,
        Method::GET,
        &format!("{C}/boards/{}", w.open_board),
        None,
    )
    .await;
    let footer = v["lists"][0]["cards"][2]["id"].as_i64().unwrap();
    req(
        &w.app,
        &w.owner,
        Method::POST,
        &format!("{C}/boards/{}/cards/{footer}/archive", w.open_board),
        None,
    )
    .await;
    assert!(titles(&search(&w, &w.member, "footer").await).is_empty());
    assert_eq!(
        titles(&search(&w, &w.member, "footer is:archived").await),
        vec!["Footer links"]
    );

    let boards = search(&w, &w.member, "launch").await;
    assert_eq!(boards["boards"][0]["id"], w.open_board);
    let renamed = format!("{C}/boards/{}/cards/{footer}", w.open_board);
    req(
        &w.app,
        &w.owner,
        Method::POST,
        &format!("{renamed}/restore"),
        None,
    )
    .await;
    req(
        &w.app,
        &w.owner,
        Method::PATCH,
        &renamed,
        Some(json!({ "title": "Đường dẫn chân trang" })),
    )
    .await;
    assert_eq!(
        titles(&search(&w, &w.member, "duong").await),
        vec!["Đường dẫn chân trang"],
        "accents do not matter"
    );
}
