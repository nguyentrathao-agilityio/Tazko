use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use super::{DbError, DbResult};

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub struct Migrations {
    pub folder: &'static str,
    pub list: &'static [Migration],
}

impl Migrations {
    pub fn latest(&self) -> i64 {
        self.list.last().map_or(0, |m| m.version)
    }
}

pub static CONTROL: Migrations = Migrations {
    folder: "migrations/control",
    list: &[
        Migration {
            version: 1,
            name: "0001_initial.sql",
            sql: include_str!("../../migrations/control/0001_initial.sql"),
        },
        Migration {
            version: 2,
            name: "0002_project_invites.sql",
            sql: include_str!("../../migrations/control/0002_project_invites.sql"),
        },
    ],
};

pub static TENANT: Migrations = Migrations {
    folder: "migrations/tenant",
    list: &[
        Migration {
            version: 1,
            name: "0001_initial.sql",
            sql: include_str!("../../migrations/tenant/0001_initial.sql"),
        },
        Migration {
            version: 2,
            name: "0002_projects.sql",
            sql: include_str!("../../migrations/tenant/0002_projects.sql"),
        },
        Migration {
            version: 3,
            name: "0003_boards.sql",
            sql: include_str!("../../migrations/tenant/0003_boards.sql"),
        },
        Migration {
            version: 4,
            name: "0004_cards.sql",
            sql: include_str!("../../migrations/tenant/0004_cards.sql"),
        },
        Migration {
            version: 5,
            name: "0005_card_details.sql",
            sql: include_str!("../../migrations/tenant/0005_card_details.sql"),
        },
        Migration {
            version: 6,
            name: "0006_search.sql",
            sql: include_str!("../../migrations/tenant/0006_search.sql"),
        },
    ],
};

pub fn open_connection(path: &Path) -> DbResult<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

pub fn schema_version(conn: &Connection) -> DbResult<i64> {
    Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
}

pub fn migrate(conn: &mut Connection, migrations: &Migrations) -> DbResult<i64> {
    let current = schema_version(conn)?;
    let known = migrations.latest();
    if current > known {
        return Err(DbError::TooNew {
            found: current,
            known,
        });
    }

    for m in migrations.list.iter().filter(|m| m.version > current) {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)?;
        tx.pragma_update(None, "user_version", m.version)?;
        tx.commit()?;
    }

    schema_version(conn)
}
