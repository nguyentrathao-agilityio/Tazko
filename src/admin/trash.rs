use std::io;
use std::path::PathBuf;

use rusqlite::params;

use crate::config::DataLayout;
use crate::db::{Database, DbResult};

pub const KEEP_SECS: i64 = 30 * 24 * 60 * 60;

pub fn move_to_trash(layout: &DataLayout, slug: &str, at: i64) -> io::Result<Option<PathBuf>> {
    let from = layout.orgs.join(slug);
    if !from.exists() {
        return Ok(None);
    }

    std::fs::create_dir_all(&layout.trash)?;
    let to = layout.trash.join(format!("{slug}.{at}"));
    std::fs::rename(&from, &to)?;
    Ok(Some(to))
}

pub fn deleted_at(name: &str) -> Option<i64> {
    let (slug, at) = name.rsplit_once('.')?;
    if slug.is_empty() {
        return None;
    }

    at.parse().ok()
}

pub async fn purge(layout: &DataLayout, control: &Database, now: i64) -> DbResult<Vec<String>> {
    let mut removed = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&layout.trash) {
        for e in entries.filter_map(Result::ok) {
            let name = e.file_name().to_string_lossy().to_string();
            if deleted_at(&name).is_some_and(|at| now - at >= KEEP_SECS)
                && std::fs::remove_dir_all(e.path()).is_ok()
            {
                removed.push(name);
            }
        }
    }

    control
        .write(move |c| {
            c.execute(
                "DELETE FROM orgs WHERE status = 'deleted' AND deleted_at IS NOT NULL AND deleted_at <= ?1",
                params![now - KEEP_SECS],
            )?;
            Ok(())
        })
        .await?;
    Ok(removed)
}
