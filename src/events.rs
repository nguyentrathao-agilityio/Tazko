use rusqlite::{Connection, params};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default)]
pub struct At {
    pub project: Option<i64>,
    pub board: Option<i64>,
    pub card: Option<i64>,
}

impl At {
    pub fn project(id: i64) -> Self {
        At {
            project: Some(id),
            ..At::default()
        }
    }

    pub fn board(project: i64, board: i64) -> Self {
        At {
            project: Some(project),
            board: Some(board),
            card: None,
        }
    }

    pub fn card(project: i64, board: i64, card: i64) -> Self {
        At {
            project: Some(project),
            board: Some(board),
            card: Some(card),
        }
    }
}

pub fn record(
    c: &Connection,
    actor: i64,
    kind: &str,
    at: At,
    payload: Value,
) -> rusqlite::Result<i64> {
    c.execute(
        "INSERT INTO events (type, actor_id, project_id, board_id, card_id, payload) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![kind, actor, at.project, at.board, at.card, payload.to_string()],
    )?;
    Ok(c.last_insert_rowid())
}
