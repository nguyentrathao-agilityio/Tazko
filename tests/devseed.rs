mod common;

use common::TestApp;
use tazko::devseed::{SLUG, Seeded, seed};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn seeding_twice_adds_the_demo_company_once() {
    let app = TestApp::new();
    let owner = app.user("owner@example.com").await;
    app.sql(move |c| c.execute("UPDATE users SET is_super_admin = 1 WHERE id = ?1", [owner]))
        .await;

    let first = seed(&app.state, Some("owner@example.com")).await.unwrap();
    let Seeded::Created { board, roadmap } = first else {
        panic!("{first:?}");
    };
    assert_eq!(
        seed(&app.state, Some("owner@example.com")).await.unwrap(),
        Seeded::AlreadyThere
    );
    assert_eq!(seed(&app.state, None).await.unwrap(), Seeded::AlreadyThere);

    let db = app.state.dbs.tenant(SLUG).unwrap();
    let (cards, lists, events, found, sessions_left) = {
        let counts = db
            .read(move |c| {
                let n = |sql: &str| -> rusqlite::Result<i64> { c.query_row(sql, [], |r| r.get(0)) };
                Ok((
                    n("SELECT count(*) FROM cards")?,
                    c.query_row("SELECT group_concat(title, ',') FROM (SELECT title FROM lists WHERE board_id = ?1 ORDER BY position)", [board], |r| r.get::<_, String>(0))?,
                    n("SELECT count(*) FROM events")?,
                    n("SELECT count(*) FROM cards_fts WHERE cards_fts MATCH '\"duong\"*'")?,
                ))
            })
            .await
            .unwrap();
        let left: i64 = app
            .sql(|c| c.query_row("SELECT count(*) FROM sessions", [], |r| r.get(0)))
            .await;
        (counts.0, counts.1, counts.2, counts.3, left)
    };
    assert_eq!(cards, 20);
    assert_eq!(lists, "Backlog,To do,In progress,Review,Done");
    assert!(events > 50);
    assert_eq!(found, 1, "Vietnamese text is found without accents");
    assert_eq!(sessions_left, 0, "the seed's own session is removed");
    assert!(roadmap > board);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn seeding_needs_a_super_admin() {
    let app = TestApp::new();
    app.user("plain@example.com").await;
    assert!(seed(&app.state, Some("plain@example.com")).await.is_err());
    assert!(seed(&app.state, None).await.is_err());
    assert!(!app.state.dbs.layout().orgs.join(SLUG).exists());
}
