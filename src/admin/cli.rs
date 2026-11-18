use std::path::Path;

use rusqlite::{OptionalExtension, params};
use serde_json::json;

use super::{audit, validate};
use crate::config::DataLayout;
use crate::db::{CONTROL, Database, DbResult};

#[derive(Debug, PartialEq, Eq)]
pub enum Granted {
    Added,
    Promoted,
    Already,
}

pub async fn grant_super_admin(data: &Path, raw_email: &str) -> Result<(String, Granted), String> {
    let email = validate::email(raw_email)
        .ok_or_else(|| format!("{raw_email:?} is not a valid email address"))?;
    let layout = DataLayout::new(data);
    layout
        .create()
        .map_err(|e| format!("cannot prepare {}: {e}", data.display()))?;
    let db = Database::open(&layout.control_db, &CONTROL).map_err(|e| e.to_string())?;
    let result = grant(&db, email.clone()).await.map_err(|e| e.to_string());
    db.close();
    Ok((email, result?))
}

async fn grant(db: &Database, email: String) -> DbResult<Granted> {
    db.write(move |c| {
        let tx = c.transaction()?;
        let found: Option<(i64, bool)> = tx
            .query_row(
                "SELECT id, is_super_admin FROM users WHERE email = ?1",
                [&email],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let outcome = match found {
            Some((_, true)) => Granted::Already,
            Some((id, false)) => {
                tx.execute(
                    "UPDATE users SET is_super_admin = 1, disabled = 0 WHERE id = ?1",
                    [id],
                )?;
                Granted::Promoted
            }
            None => {
                tx.execute(
                    "INSERT INTO users (email, is_super_admin) VALUES (?1, 1)",
                    params![email],
                )?;
                Granted::Added
            }
        };
        if outcome != Granted::Already {
            audit::record(
                &tx,
                None,
                "super_admin_granted",
                None,
                &email,
                json!({ "via": "command line" }),
            )?;
        }

        tx.commit()?;
        Ok(outcome)
    })
    .await
}
