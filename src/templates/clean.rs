use rusqlite::{Connection, params};
use serde_json::json;

use super::TemplateData;
use crate::admin::audit;
use crate::db::{Databases, DbError, DbResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cleaned {
    pub company: String,
    pub template: i64,
    pub name: String,
    pub board: i64,
    pub cards: usize,
    pub shared_copies: usize,
}

const COPIES: &str = "FROM admin_templates
     WHERE data = ?2 AND id IN (SELECT json_extract(detail, '$.template') FROM audit_log
                                WHERE action = 'template_offered' AND org_id = ?1)";

pub async fn restricted(dbs: &Databases, apply: bool) -> DbResult<Vec<Cleaned>> {
    let companies: Vec<(i64, String)> = dbs
        .control()
        .read(|c| {
            let mut stmt =
                c.prepare("SELECT id, slug FROM orgs WHERE status != 'deleted' ORDER BY slug")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?;
            Ok(rows)
        })
        .await?;

    let mut out = vec![];
    for (org, company) in companies {
        let db = match dbs.tenant(&company) {
            Ok(db) => db,
            Err(DbError::Missing(_)) => continue,
            Err(e) => return Err(e),
        };
        let found = db
            .read(|c| Ok(with_cards_from_restricted_boards(c)?))
            .await?;
        for (mut cleaned, data, without) in found {
            cleaned.company = company.clone();
            let (template, cards, name) = (cleaned.template, cleaned.cards, cleaned.name.clone());
            cleaned.shared_copies = if apply {
                let (d, w) = (data.clone(), without.clone());
                db.write(move |c| {
                    c.execute(
                        "UPDATE board_templates SET data = ?3 WHERE id = ?1 AND data = ?2",
                        params![template, d, w],
                    )?;
                    Ok(())
                })
                .await?;
                dbs
                    .control()
                    .write(move |c| {
                        let tx = c.transaction()?;
                        let copies = tx.execute(
                            &format!("UPDATE admin_templates SET data = ?3 WHERE id IN (SELECT id {COPIES})"),
                            params![org, data, without],
                        )?;
                        audit::record(
                            &tx,
                            None,
                            "template_cards_removed",
                            Some(org),
                            &name,
                            json!({ "template": format!("c:{template}"), "cards": cards, "shared_copies": copies, "via": "command line" }),
                        )?;
                        tx.commit()?;
                        Ok(copies)
                    })
                    .await?
            } else {
                dbs.control()
                    .read(move |c| {
                        Ok(c.query_row(
                            &format!("SELECT count(*) {COPIES}"),
                            params![org, data],
                            |r| r.get::<_, i64>(0),
                        )? as usize)
                    })
                    .await?
            };
            out.push(cleaned);
        }
    }
    Ok(out)
}

fn with_cards_from_restricted_boards(
    c: &Connection,
) -> rusqlite::Result<Vec<(Cleaned, String, String)>> {
    let mut stmt = c.prepare(
        "SELECT DISTINCT t.id, t.name, t.data, b.id
         FROM board_templates t
         JOIN events e ON e.type = 'board_saved_as_template' AND json_extract(e.payload, '$.template') = t.id
         JOIN boards b ON b.id = e.board_id
         WHERE b.restricted = 1
         ORDER BY t.id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(template, name, data, board)| {
            let mut parsed: TemplateData = serde_json::from_str(&data).ok()?;
            let cards: usize = parsed.lists.iter().map(|l| l.cards.len()).sum();
            if cards == 0 {
                return None;
            }

            parsed.lists.iter_mut().for_each(|l| l.cards.clear());
            let without = serde_json::to_string(&parsed).ok()?;
            let cleaned = Cleaned {
                company: String::new(),
                template,
                name,
                board,
                cards,
                shared_copies: 0,
            };
            Some((cleaned, data, without))
        })
        .collect())
}
