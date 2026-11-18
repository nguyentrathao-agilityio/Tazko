use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use tazko::config::DataLayout;
use tazko::db::{
    CONTROL, Databases, DbError, Migration, Migrations, TENANT, migrate, open_connection,
    schema_version, valid_slug,
};

fn tables(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap();
    stmt.query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn layout(dir: &Path) -> DataLayout {
    let layout = DataLayout::new(dir.join("data"));
    layout.create().unwrap();
    layout
}

#[test]
fn control_db_migrates_fresh_and_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("control.db");
    let mut conn = open_connection(&path).unwrap();
    assert_eq!(migrate(&mut conn, &CONTROL).unwrap(), CONTROL.latest());
    drop(conn);

    let mut conn = open_connection(&path).unwrap();
    assert_eq!(migrate(&mut conn, &CONTROL).unwrap(), CONTROL.latest());
    let names = tables(&conn);
    for t in [
        "users",
        "orgs",
        "org_members",
        "invites",
        "sessions",
        "audit_log",
        "user_settings",
        "personal_tokens",
        "device_codes",
        "email_queue",
    ] {
        assert!(names.iter().any(|n| n == t), "{t} missing from {names:?}");
    }
}

#[test]
fn tenant_db_migrates_fresh_and_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tazko.db");
    for _ in 0..2 {
        let mut conn = open_connection(&path).unwrap();
        assert_eq!(migrate(&mut conn, &TENANT).unwrap(), TENANT.latest());
    }
    let conn = open_connection(&path).unwrap();
    let names = tables(&conn);
    assert!(names.contains(&"events".to_string()));
    for table in names {
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info(?1)")
            .unwrap()
            .query_map([&table], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(
            !cols.iter().any(|c| c.starts_with("org")),
            "{table} has a company column: {cols:?}"
        );
    }
}

#[test]
fn every_migration_file_is_listed_in_order() {
    for set in [&CONTROL, &TENANT] {
        let mut files: Vec<String> = fs::read_dir(set.folder)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.ends_with(".sql"))
            .collect();
        files.sort();
        let listed: Vec<&str> = set.list.iter().map(|m| m.name).collect();
        assert_eq!(files, listed, "{}", set.folder);
        for (i, m) in set.list.iter().enumerate() {
            assert_eq!(m.version, i as i64 + 1);
            assert!(m.name.starts_with(&format!("{:04}_", m.version)));
        }
    }
}

#[test]
fn connections_use_wal_normal_sync_and_foreign_keys() {
    let dir = tempfile::tempdir().unwrap();
    let conn = open_connection(&dir.path().join("x.db")).unwrap();
    let mode: String = conn
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    let sync: i64 = conn
        .pragma_query_value(None, "synchronous", |r| r.get(0))
        .unwrap();
    let fk: i64 = conn
        .pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .unwrap();
    assert_eq!((mode.as_str(), sync, fk), ("wal", 1, 1));
}

#[test]
fn a_database_newer_than_the_program_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = open_connection(&dir.path().join("x.db")).unwrap();
    conn.pragma_update(None, "user_version", 99).unwrap();
    assert!(matches!(
        migrate(&mut conn, &CONTROL),
        Err(DbError::TooNew { found: 99, .. })
    ));
}

#[test]
fn a_failing_migration_leaves_the_file_at_the_previous_version() {
    static BROKEN: Migrations = Migrations {
        folder: "",
        list: &[
            Migration {
                version: 1,
                name: "0001.sql",
                sql: "CREATE TABLE a (x);",
            },
            Migration {
                version: 2,
                name: "0002.sql",
                sql: "CREATE TABLE b (x); CREATE TABLE a (x);",
            },
        ],
    };
    let dir = tempfile::tempdir().unwrap();
    let mut conn = open_connection(&dir.path().join("x.db")).unwrap();
    assert!(migrate(&mut conn, &BROKEN).is_err());
    assert_eq!(schema_version(&conn).unwrap(), 1);
    assert!(!tables(&conn).contains(&"b".to_string()));
}

#[test]
fn company_names_in_paths_are_checked() {
    for good in ["acme", "acme-2", "a"] {
        assert!(valid_slug(good), "{good}");
    }
    for bad in [
        "",
        "../acme",
        "Acme",
        "a/b",
        "-a",
        "a-",
        "a.b",
        &"a".repeat(41),
    ] {
        assert!(!valid_slug(bad), "{bad}");
    }

    let dir = tempfile::tempdir().unwrap();
    let dbs = Databases::open(layout(dir.path())).unwrap();
    assert!(matches!(dbs.tenant("../escape"), Err(DbError::BadSlug(_))));
    assert!(matches!(
        dbs.create_tenant("../escape"),
        Err(DbError::BadSlug(_))
    ));
    assert!(matches!(
        dbs.tenant("not-created"),
        Err(DbError::Missing(_))
    ));
    assert!(
        !dir.path().join("data/orgs/not-created").exists(),
        "opening must not create a company folder"
    );
    assert!(!dir.path().join("escape").exists());
    dbs.close();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn each_company_gets_its_own_migrated_file() {
    let dir = tempfile::tempdir().unwrap();
    let dbs = Databases::open(layout(dir.path())).unwrap();
    let a = dbs.create_tenant("acme").unwrap();
    assert_eq!(a.path(), dir.path().join("data/orgs/acme/tazko.db"));
    assert!(dir.path().join("data/orgs/acme/files").is_dir());
    let version = a.read(schema_version).await.unwrap();
    assert_eq!(version, TENANT.latest());
    assert_eq!(dbs.tenant("acme").unwrap().path(), a.path());

    let control = dbs.control().read(schema_version).await.unwrap();
    assert_eq!(control, CONTROL.latest());
    dbs.close();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slow_write_in_company_a_does_not_block_company_b() {
    let dir = tempfile::tempdir().unwrap();
    let dbs = Databases::open(layout(dir.path())).unwrap();
    let a = dbs.create_tenant("company-a").unwrap();
    let b = dbs.create_tenant("company-b").unwrap();

    let slow = tokio::spawn({
        let a = a.clone();
        async move {
            a.write(|c| {
                let tx = c.transaction()?;
                tx.execute("INSERT INTO events (type) VALUES ('slow')", [])?;
                std::thread::sleep(Duration::from_millis(800));
                tx.commit()?;
                Ok(())
            })
            .await
        }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let start = Instant::now();
    b.write(|c| Ok(c.execute("INSERT INTO events (type) VALUES ('fast')", [])?))
        .await
        .unwrap();
    let b_took = start.elapsed();

    let start = Instant::now();
    let seen: i64 = a
        .read(|c| Ok(c.query_row("SELECT count(*) FROM events", [], |r| r.get(0))?))
        .await
        .unwrap();
    let a_read_took = start.elapsed();

    assert!(
        !slow.is_finished(),
        "the slow write ended before the checks ran"
    );
    assert!(
        b_took < Duration::from_millis(300),
        "company B waited {b_took:?}"
    );
    assert!(
        a_read_took < Duration::from_millis(300),
        "reading company A waited {a_read_took:?}"
    );
    assert_eq!(seen, 0, "a reader saw an uncommitted write");

    slow.await.unwrap().unwrap();
    dbs.close();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn writes_to_one_company_are_serialised_without_busy_errors() {
    let dir = tempfile::tempdir().unwrap();
    let dbs = Databases::open(layout(dir.path())).unwrap();
    let a = dbs.create_tenant("acme").unwrap();
    let jobs: Vec<_> = (0..50)
        .map(|i| {
            let a = a.clone();
            tokio::spawn(async move {
                a.write(move |c| {
                    Ok(c.execute("INSERT INTO events (type) VALUES (?1)", [format!("e{i}")])?)
                })
                .await
            })
        })
        .collect();
    for j in jobs {
        j.await.unwrap().unwrap();
    }
    let n: i64 = a
        .read(|c| Ok(c.query_row("SELECT count(*) FROM events", [], |r| r.get(0))?))
        .await
        .unwrap();
    assert_eq!(n, 50);
    dbs.close();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn readers_cannot_write() {
    let dir = tempfile::tempdir().unwrap();
    let dbs = Databases::open(layout(dir.path())).unwrap();
    let a = dbs.create_tenant("acme").unwrap();
    let res = a
        .read(|c| Ok(c.execute("INSERT INTO events (type) VALUES ('x')", [])?))
        .await;
    assert!(res.is_err());
    dbs.close();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writes_after_close_report_closed() {
    let dir = tempfile::tempdir().unwrap();
    let dbs = Databases::open(layout(dir.path())).unwrap();
    let a = dbs.create_tenant("acme").unwrap();
    dbs.forget("acme");
    assert!(matches!(a.write(|_| Ok(())).await, Err(DbError::Closed)));
    dbs.close();
}
