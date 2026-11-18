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

pub fn board_activity(
    c: &Connection,
    board: i64,
    before: Option<i64>,
    limit: i64,
) -> rusqlite::Result<Vec<serde_json::Value>> {
    let mut stmt = c.prepare(
        "SELECT e.id, e.type, e.payload, e.created_at, e.card_id, k.title,
                p.id, p.name, p.email
         FROM events e
         LEFT JOIN cards k ON k.id = e.card_id
         LEFT JOIN people p ON p.id = e.actor_id
         WHERE e.board_id = ?1 AND e.id < ?2 AND e.type NOT IN ('board_starred', 'board_unstarred')
         ORDER BY e.id DESC LIMIT ?3",
    )?;
    stmt.query_map(params![board, before.unwrap_or(i64::MAX), limit], |r| {
        let actor = match r.get::<_, Option<i64>>(6)? {
            Some(id) => serde_json::json!({
                "id": id,
                "name": r.get::<_, String>(7)?,
                "email": r.get::<_, String>(8)?,
            }),
            None => serde_json::Value::Null,
        };
        Ok(serde_json::json!({
            "id": r.get::<_, i64>(0)?,
            "type": r.get::<_, String>(1)?,
            "payload": serde_json::from_str::<serde_json::Value>(&r.get::<_, String>(2)?).unwrap_or(serde_json::Value::Null),
            "created_at": r.get::<_, i64>(3)?,
            "card": r.get::<_, Option<i64>>(4)?.map(|id| serde_json::json!({ "id": id, "title": r.get::<_, Option<String>>(5).ok().flatten() })),
            "actor": actor,
        }))
    })?
    .collect()
}
