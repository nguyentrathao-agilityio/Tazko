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
        Migration {
            version: 3,
            name: "0003_personal_tokens.sql",
            sql: include_str!("../../migrations/control/0003_personal_tokens.sql"),
        },
        Migration {
            version: 4,
            name: "0004_admin_templates.sql",
            sql: include_str!("../../migrations/control/0004_admin_templates.sql"),
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
        Migration {
            version: 7,
            name: "0007_attachments.sql",
            sql: include_str!("../../migrations/tenant/0007_attachments.sql"),
        },
        Migration {
            version: 8,
            name: "0008_notifications.sql",
            sql: include_str!("../../migrations/tenant/0008_notifications.sql"),
        },
        Migration {
            version: 9,
            name: "0009_fields_reactions_backgrounds.sql",
            sql: include_str!("../../migrations/tenant/0009_fields_reactions_backgrounds.sql"),
        },
        Migration {
            version: 10,
            name: "0010_api_tokens.sql",
            sql: include_str!("../../migrations/tenant/0010_api_tokens.sql"),
        },
        Migration {
            version: 11,
            name: "0011_templates.sql",
            sql: include_str!("../../migrations/tenant/0011_templates.sql"),
        },
        Migration {
            version: 12,
            name: "0012_touch_cards.sql",
            sql: include_str!("../../migrations/tenant/0012_touch_cards.sql"),
        },
        Migration {
            version: 13,
            name: "0013_item_meta.sql",
            sql: include_str!("../../migrations/tenant/0013_item_meta.sql"),
        },
        Migration {
            version: 14,
            name: "0014_item_note_text.sql",
            sql: include_str!("../../migrations/tenant/0014_item_note_text.sql"),
        },
        Migration {
            version: 15,
            name: "0015_recurring_cards.sql",
            sql: include_str!("../../migrations/tenant/0015_recurring_cards.sql"),
        },
        Migration {
            version: 16,
            name: "0016_events_by_card.sql",
            sql: include_str!("../../migrations/tenant/0016_events_by_card.sql"),
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
