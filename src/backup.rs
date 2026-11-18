use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::DataLayout;
use crate::db::{CONTROL, Migrations, TENANT, schema_version, valid_slug};
use crate::session::now;

pub const DEFAULT_KEEP_DAYS: i64 = 30;
const PARTIAL: &str = ".partial-";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub created_at: i64,
    pub version: String,
    pub databases: usize,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Status {
    pub at: i64,
    pub ok: bool,
    pub snapshot: Option<String>,
    pub databases: usize,
    pub files: usize,
    pub bytes: u64,
    pub problems: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CheckReport {
    pub databases: usize,
    pub files: usize,
    pub problems: Vec<String>,
}

impl CheckReport {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

pub fn stamp(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

fn stamp_to_unix(name: &str) -> Option<i64> {
    let (date, time) = name.split_once('-')?;
    if date.len() != 8
        || time.len() != 6
        || !date.bytes().chain(time.bytes()).all(|b| b.is_ascii_digit())
    {
        return None;
    }

    let y: i64 = date[0..4].parse().ok()?;
    let m: i64 = date[4..6].parse().ok()?;
    let d: i64 = date[6..8].parse().ok()?;
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let h: i64 = time[0..2].parse().ok()?;
    let mi: i64 = time[2..4].parse().ok()?;
    let s: i64 = time[4..6].parse().ok()?;
    Some(days * 86_400 + h * 3600 + mi * 60 + s)
}

fn copy_db(from: &Path, to: &Path) -> rusqlite::Result<()> {
    let src = Connection::open_with_flags(
        from,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    src.busy_timeout(Duration::from_secs(10))?;
    src.backup(rusqlite::MAIN_DB, to, None)
}

fn copy_tree(from: &Path, to: &Path, files: &mut usize, bytes: &mut u64) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target, files, bytes)?;
        } else if entry.file_type()?.is_file() {
            *bytes += fs::copy(entry.path(), &target)?;
            *files += 1;
        }
    }

    Ok(())
}

fn companies(orgs: &Path) -> io::Result<Vec<String>> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(orgs) {
        for e in entries {
            let e = e?;
            let name = e.file_name().to_string_lossy().to_string();
            if e.file_type()?.is_dir() && valid_slug(&name) && e.path().join("tazko.db").is_file() {
                out.push(name);
            }
        }
    }

    out.sort();
    Ok(out)
}

fn err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

pub fn snapshot(data: &Path, to: &Path, at: i64) -> io::Result<(PathBuf, Manifest)> {
    let layout = DataLayout::new(data);
    if !layout.control_db.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} has no control.db", data.display()),
        ));
    }

    fs::create_dir_all(to)?;
    let name = stamp(at);
    let partial = to.join(format!("{PARTIAL}{name}"));
    let _ = fs::remove_dir_all(&partial);
    fs::create_dir_all(partial.join("orgs"))?;

    let mut manifest = Manifest {
        created_at: at,
        version: env!("CARGO_PKG_VERSION").to_string(),
        databases: 0,
        files: 0,
        bytes: 0,
    };
    copy_db(&layout.control_db, &partial.join("control.db")).map_err(err)?;
    manifest.databases += 1;
    for slug in companies(&layout.orgs)? {
        let dir = partial.join("orgs").join(&slug);
        fs::create_dir_all(&dir)?;
        copy_db(
            &layout.orgs.join(&slug).join("tazko.db"),
            &dir.join("tazko.db"),
        )
        .map_err(err)?;
        manifest.databases += 1;
        let files = layout.orgs.join(&slug).join("files");
        if files.is_dir() {
            copy_tree(
                &files,
                &dir.join("files"),
                &mut manifest.files,
                &mut manifest.bytes,
            )?;
        }
    }

    for entry in walk(&partial)? {
        if entry.extension().is_some_and(|e| e == "db") {
            manifest.bytes += fs::metadata(&entry)?.len();
        }
    }

    fs::write(
        partial.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).map_err(err)?,
    )?;
    let done = to.join(&name);
    if done.exists() {
        fs::remove_dir_all(&done)?;
    }

    fs::rename(&partial, &done)?;
    Ok((done, manifest))
}

fn walk(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir)? {
        let e = e?;
        if e.file_type()?.is_dir() {
            out.extend(walk(&e.path())?);
        } else {
            out.push(e.path());
        }
    }

    Ok(out)
}

pub fn snapshots(dir: &Path) -> io::Result<Vec<(String, i64)>> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries {
            let name = e?.file_name().to_string_lossy().to_string();
            if let Some(at) = stamp_to_unix(&name) {
                out.push((name, at));
            }
        }
    }

    out.sort();
    Ok(out)
}

pub fn prune(dir: &Path, keep_days: i64, now: i64) -> io::Result<Vec<String>> {
    let mut removed = Vec::new();
    let all = snapshots(dir)?;
    let newest = all.last().map(|(n, _)| n.clone());
    for (name, at) in all {
        if Some(&name) != newest.as_ref() && now - at > keep_days * 86_400 {
            fs::remove_dir_all(dir.join(&name))?;
            removed.push(name);
        }
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().starts_with(PARTIAL) {
                let _ = fs::remove_dir_all(e.path());
            }
        }
    }

    Ok(removed)
}

pub fn restore(from: &Path, snapshot: Option<&str>, to: &Path) -> io::Result<String> {
    let name = match snapshot {
        Some(s) => s.to_string(),
        None => snapshots(from)?.pop().map(|(n, _)| n).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("no backups in {}", from.display()),
            )
        })?,
    };
    if stamp_to_unix(&name).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} is not a backup name"),
        ));
    }

    let src = from.join(&name);
    if !src.join("control.db").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} is not a complete backup", src.display()),
        ));
    }

    if to.exists() && fs::read_dir(to)?.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} is not empty; restore into a new folder", to.display()),
        ));
    }

    let layout = DataLayout::new(to);
    layout.create()?;
    fs::copy(src.join("control.db"), &layout.control_db)?;
    let mut files = 0;
    let mut bytes = 0;
    if src.join("orgs").is_dir() {
        copy_tree(&src.join("orgs"), &layout.orgs, &mut files, &mut bytes)?;
    }

    Ok(name)
}

fn check_db(path: &Path, migrations: &Migrations, problems: &mut Vec<String>) {
    let label = path.display().to_string();
    let conn = match Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(c) => c,
        Err(e) => {
            problems.push(format!("{label}: cannot open: {e}"));
            return;
        }
    };
    match conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)) {
        Ok(s) if s == "ok" => {}
        Ok(s) => problems.push(format!("{label}: integrity check says {s}")),
        Err(e) => problems.push(format!("{label}: integrity check failed: {e}")),
    }

    match schema_version(&conn) {
        Ok(v) if v > migrations.latest() => problems.push(format!(
            "{label}: schema version {v} is newer than this program knows"
        )),
        Ok(_) => {}
        Err(e) => problems.push(format!("{label}: cannot read the schema version: {e}")),
    }
}

fn is_hash_name(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn check(data: &Path) -> CheckReport {
    let layout = DataLayout::new(data);
    let mut report = CheckReport::default();
    if !layout.control_db.is_file() {
        report
            .problems
            .push(format!("{} has no control.db", data.display()));
        return report;
    }

    check_db(&layout.control_db, &CONTROL, &mut report.problems);
    report.databases += 1;
    let slugs = match companies(&layout.orgs) {
        Ok(s) => s,
        Err(e) => {
            report
                .problems
                .push(format!("cannot list {}: {e}", layout.orgs.display()));
            return report;
        }
    };
    for slug in slugs {
        let dir = layout.orgs.join(&slug);
        check_db(&dir.join("tazko.db"), &TENANT, &mut report.problems);
        report.databases += 1;
        let files = dir.join("files");
        if !files.is_dir() {
            continue;
        }

        match walk(&files) {
            Ok(all) => {
                for f in all {
                    report.files += 1;
                    let name = f
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if !is_hash_name(&name) {
                        report
                            .problems
                            .push(format!("{}: file name is not a content hash", f.display()));
                        continue;
                    }

                    match fs::read(&f) {
                        Ok(bytes) => {
                            let hash: String = Sha256::digest(&bytes)
                                .iter()
                                .map(|b| format!("{b:02x}"))
                                .collect();
                            if hash != name {
                                report.problems.push(format!(
                                    "{}: content does not match its hash",
                                    f.display()
                                ));
                            }
                        }
                        Err(e) => report
                            .problems
                            .push(format!("{}: cannot read: {e}", f.display())),
                    }
                }
            }
            Err(e) => report
                .problems
                .push(format!("cannot list {}: {e}", files.display())),
        }
    }

    report
}

pub fn status_dir(data: &Path) -> PathBuf {
    data.join("status")
}

pub fn write_status(data: &Path, kind: &str, status: &Status) -> io::Result<()> {
    let dir = status_dir(data);
    fs::create_dir_all(&dir)?;
    let tmp = dir.join(format!(".{kind}.json.tmp"));
    fs::write(&tmp, serde_json::to_vec_pretty(status).map_err(err)?)?;
    fs::rename(tmp, dir.join(format!("{kind}.json")))
}

pub fn read_status(data: &Path, kind: &str) -> Option<Status> {
    let text = fs::read(status_dir(data).join(format!("{kind}.json"))).ok()?;
    serde_json::from_slice(&text).ok()
}

pub fn run_backup(data: &Path, to: &Path, keep_days: i64) -> Status {
    let at = now();
    let mut status = Status {
        at,
        ok: false,
        snapshot: None,
        databases: 0,
        files: 0,
        bytes: 0,
        problems: Vec::new(),
    };
    match snapshot(data, to, at) {
        Ok((path, manifest)) => {
            status.snapshot = path.file_name().map(|n| n.to_string_lossy().to_string());
            status.databases = manifest.databases;
            status.files = manifest.files;
            status.bytes = manifest.bytes;
            let report = check(&path);
            status.problems = report.problems;
            if let Err(e) = prune(to, keep_days, at) {
                status
                    .problems
                    .push(format!("cannot remove old backups: {e}"));
            }
        }
        Err(e) => status.problems.push(e.to_string()),
    }

    status.ok = status.problems.is_empty();
    status
}

pub fn run_restore_test(from: &Path, scratch: &Path) -> Status {
    let mut status = Status {
        at: now(),
        ok: false,
        snapshot: None,
        databases: 0,
        files: 0,
        bytes: 0,
        problems: Vec::new(),
    };
    let target = scratch.join(format!("tazko-restore-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&target);
    match restore(from, None, &target) {
        Ok(name) => {
            status.snapshot = Some(name);
            let report = check(&target);
            status.databases = report.databases;
            status.files = report.files;
            status.problems = report.problems;
        }
        Err(e) => status.problems.push(e.to_string()),
    }

    let _ = fs::remove_dir_all(&target);
    status.ok = status.problems.is_empty();
    status
}
