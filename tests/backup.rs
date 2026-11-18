mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::{TestApp, body_json};
use sha2::{Digest, Sha256};
use tazko::backup::{self, Status};
use tazko::config::DataLayout;
use tazko::db::Databases;

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

async fn sample(data: &Path) {
    let layout = DataLayout::new(data);
    layout.create().unwrap();
    let dbs = Databases::open(layout.clone()).unwrap();
    dbs.control()
        .write(|c| {
            c.execute_batch(
                "INSERT INTO users (email, name) VALUES ('a@example.com', 'A');
                 INSERT INTO orgs (slug, name) VALUES ('acme', 'Acme'), ('north', 'North');",
            )?;
            Ok(())
        })
        .await
        .unwrap();
    for slug in ["acme", "north"] {
        let db = dbs.create_tenant(slug).unwrap();
        db.write(|c| {
            for i in 0..50 {
                c.execute(
                    "INSERT INTO events (type, payload) VALUES ('x', ?1)",
                    [format!("{{\"n\":{i}}}")],
                )?;
            }
            Ok(())
        })
        .await
        .unwrap();
    }
    let content = b"an attached file";
    let dir = layout.orgs.join("acme/files/ab");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(hash(content)), content).unwrap();
    dbs.close();
}

fn count(db: &Path, sql: &str) -> i64 {
    rusqlite::Connection::open(db)
        .unwrap()
        .query_row(sql, [], |r| r.get(0))
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_backup_restored_into_a_fresh_folder_passes_the_check() {
    let dir = tempfile::tempdir().unwrap();
    let (data, backups, fresh) = (
        dir.path().join("data"),
        dir.path().join("backups"),
        dir.path().join("data.new"),
    );
    sample(&data).await;

    let status = backup::run_backup(&data, &backups, 30);
    assert!(status.ok, "{status:?}");
    assert_eq!((status.databases, status.files), (3, 1));
    let name = status.snapshot.clone().unwrap();
    let manifest: backup::Manifest =
        serde_json::from_slice(&fs::read(backups.join(&name).join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest.databases, 3);

    assert_eq!(backup::restore(&backups, None, &fresh).unwrap(), name);
    let report = backup::check(&fresh);
    assert!(report.ok(), "{:?}", report.problems);
    assert_eq!((report.databases, report.files), (3, 1));
    assert_eq!(
        count(&fresh.join("control.db"), "SELECT count(*) FROM orgs"),
        2
    );
    assert_eq!(
        count(
            &fresh.join("orgs/north/tazko.db"),
            "SELECT count(*) FROM events"
        ),
        50
    );
    assert!(
        !fresh.join("secret").exists(),
        "the server secret is not part of a backup"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_backup_taken_during_writes_is_consistent() {
    let dir = tempfile::tempdir().unwrap();
    let (data, backups) = (dir.path().join("data"), dir.path().join("backups"));
    sample(&data).await;
    let dbs = Databases::open(DataLayout::new(&data)).unwrap();
    let acme = dbs.tenant("acme").unwrap();
    let writer = tokio::spawn(async move {
        for i in 0..200 {
            acme.write(move |c| {
                Ok(c.execute("INSERT INTO events (type) VALUES (?1)", [format!("w{i}")])?)
            })
            .await
            .unwrap();
        }
    });
    let status = tokio::task::spawn_blocking({
        let (data, backups) = (data.clone(), backups.clone());
        move || backup::run_backup(&data, &backups, 30)
    })
    .await
    .unwrap();
    writer.await.unwrap();
    dbs.close();
    assert!(status.ok, "{status:?}");
    let n = count(
        &backups
            .join(status.snapshot.unwrap())
            .join("orgs/acme/tazko.db"),
        "SELECT count(*) FROM events",
    );
    assert!((50..=250).contains(&n), "{n}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_check_finds_damaged_databases_and_files() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    sample(&data).await;
    assert!(backup::check(&data).ok());

    let file = fs::read_dir(data.join("orgs/acme/files/ab"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(&file, b"changed").unwrap();
    fs::write(data.join("orgs/acme/files/ab/notes.txt"), b"x").unwrap();
    let db = data.join("orgs/north/tazko.db");
    let mut bytes = fs::read(&db).unwrap();
    let len = bytes.len();
    for b in &mut bytes[4096.min(len / 2)..len] {
        *b = 0x5a;
    }
    fs::write(&db, bytes).unwrap();

    let report = backup::check(&data);
    let text = report.problems.join("\n");
    assert!(text.contains("does not match its hash"), "{text}");
    assert!(text.contains("not a content hash"), "{text}");
    assert!(text.contains("north"), "{text}");
}

#[test]
fn old_snapshots_are_removed_but_the_newest_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let now = 1_800_000_000;
    for days_ago in [0, 10, 29, 31, 45] {
        fs::create_dir_all(dir.path().join(backup::stamp(now - days_ago * 86_400))).unwrap();
    }
    fs::create_dir_all(dir.path().join(".partial-20250101-000000")).unwrap();
    let removed = backup::prune(dir.path(), 30, now).unwrap();
    assert_eq!(removed.len(), 2);
    let left: Vec<String> = backup::snapshots(dir.path())
        .unwrap()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert_eq!(left.len(), 3);
    assert!(!dir.path().join(".partial-20250101-000000").exists());

    let only = tempfile::tempdir().unwrap();
    fs::create_dir_all(only.path().join(backup::stamp(now - 400 * 86_400))).unwrap();
    assert!(
        backup::prune(only.path(), 30, now).unwrap().is_empty(),
        "the newest backup always stays"
    );
}

#[test]
fn snapshot_names_are_utc_times() {
    assert_eq!(backup::stamp(0), "19700101-000000");
    assert_eq!(backup::stamp(1_700_000_000), "20231114-221320");
    assert_eq!(backup::stamp(951_782_400), "20000229-000000");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restore_refuses_a_folder_that_is_in_use_and_unknown_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let (data, backups) = (dir.path().join("data"), dir.path().join("backups"));
    sample(&data).await;
    assert!(backup::run_backup(&data, &backups, 30).ok);
    assert!(
        backup::restore(&backups, None, &data).is_err(),
        "never over live data"
    );
    assert!(backup::restore(&backups, Some("../data"), &dir.path().join("x")).is_err());
    assert!(backup::restore(&dir.path().join("none"), None, &dir.path().join("y")).is_err());
    std::fs::create_dir_all(dir.path().join("elsewhere")).unwrap();
    std::os::unix::fs::symlink(dir.path().join("elsewhere"), dir.path().join("link")).unwrap();
    assert!(
        backup::restore(&backups, None, &dir.path().join("link")).is_err(),
        "never through a symbolic link"
    );
    assert_eq!(
        std::fs::read_dir(dir.path().join("elsewhere"))
            .unwrap()
            .count(),
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_restore_test_is_recorded_and_shown_on_the_server_page() {
    let app = TestApp::new();
    let data = app.state.dbs.layout().root.clone();
    let backups = app.dir.path().join("backups");
    let b = backup::run_backup(&data, &backups, 30);
    backup::write_status(&data, "backup", &b).unwrap();
    let r = backup::run_restore_test(&backups, app.dir.path());
    backup::write_status(&data, "restore-test", &r).unwrap();
    assert!(r.ok, "{r:?}");
    let leftovers: Vec<_> = fs::read_dir(app.dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("tazko-restore-test")
        })
        .collect();
    assert!(leftovers.is_empty(), "the scratch copy is removed");

    let admin = app.user("root@example.com").await;
    app.sql(move |c| c.execute("UPDATE users SET is_super_admin = 1 WHERE id = ?1", [admin]))
        .await;
    let cookie = app.sign_in(admin).await;
    let s = body_json(app.get("/api/v1/admin/server", Some(&cookie)).await).await;
    let last: Status = serde_json::from_value(s["last_backup"].clone()).unwrap();
    assert!(last.ok);
    assert_eq!(last.snapshot, b.snapshot);
    assert_eq!(s["last_restore_test"]["ok"], true);
}

fn tazko(args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tazko"));
    for (k, _) in std::env::vars() {
        if k.starts_with("TAZKO_") {
            cmd.env_remove(k);
        }
    }
    cmd.args(args).output().unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_command_line_backs_up_checks_restores_and_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let p = |s: &str| dir.path().join(s).to_string_lossy().to_string();
    sample(&dir.path().join("data")).await;

    let out = tazko(&[
        "backup",
        "--data",
        &p("data"),
        "--to",
        &p("backups"),
        "--keep-days",
        "30",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("backup ok:"));
    assert!(dir.path().join("data/status/backup.json").is_file());

    let out = tazko(&["check", "--data", &p("data")]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "check ok: 3 databases, 1 files"
    );

    let out = tazko(&[
        "backup",
        "restore",
        "--from",
        &p("backups"),
        "--to",
        &p("data.new"),
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(tazko(&["check", "--data", &p("data.new")]).status.success());

    let out = tazko(&[
        "backup",
        "verify",
        "--from",
        &p("backups"),
        "--data",
        &p("data"),
        "--scratch",
        &p(""),
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dir.path().join("data/status/restore-test.json").is_file());

    assert!(!tazko(&["check", "--data", &p("missing")]).status.success());
    assert!(
        !tazko(&["backup", "--data", &p("data")]).status.success(),
        "--to is required"
    );
}
