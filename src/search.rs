use axum::Router;
use axum::extract::{Query, State};
use axum::response::Json;
use axum::routing::get;
use rusqlite::types::Value as Sql;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::COMPANY_PREFIX;
use crate::boards::VISIBLE;
use crate::error::{ApiError, bad};
use crate::session::now;
use crate::state::AppState;
use crate::tenant::Company;

pub const LIMIT: i64 = 50;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    pub words: Vec<String>,
    pub members: Vec<String>,
    pub labels: Vec<String>,
    pub overdue: bool,
    pub archived: bool,
    pub open: bool,
    pub done: bool,
    pub attachments: bool,
    pub mine: bool,
    pub unknown: Vec<String>,
}

impl Parsed {
    fn filtered(&self) -> bool {
        !self.members.is_empty()
            || !self.labels.is_empty()
            || self.overdue
            || self.archived
            || self.open
            || self.done
            || self.attachments
            || self.mine
    }
}

fn clean(term: &str) -> String {
    term.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.' || *c == '@')
        .collect::<String>()
        .to_lowercase()
}

pub fn parse(q: &str) -> Parsed {
    let mut p = Parsed::default();
    for raw in q.split_whitespace().take(20) {
        let lower = raw.to_lowercase();
        match lower.as_str() {
            "is:overdue" => p.overdue = true,
            "is:archived" => p.archived = true,
            "is:open" => p.open = true,
            "is:done" | "is:complete" => p.done = true,
            "has:attachments" => p.attachments = true,
            "@me" => p.mine = true,
            t if t.starts_with('@') && t.len() > 1 => p.members.push(clean(&t[1..])),
            t if t.starts_with('#') && t.len() > 1 => p.labels.push(clean(&t[1..])),
            t if t.starts_with("is:") || t.starts_with("has:") => p.unknown.push(t.to_string()),
            t => {
                let w: String = t
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .map(|c| if c == 'đ' { 'd' } else { c })
                    .collect();
                if !w.is_empty() {
                    p.words.push(w);
                }
            }
        }
    }
    p.members.retain(|m| !m.is_empty());
    p.labels.retain(|l| !l.is_empty());
    p
}

pub fn fts_query(words: &[String]) -> String {
    words
        .iter()
        .map(|w| format!("\"{}\"*", w.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

pub fn like(s: &str) -> String {
    format!(
        "%{}%",
        s.replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}

pub fn routes() -> Router<AppState> {
    Router::new().route(&format!("{COMPANY_PREFIX}/search"), get(search))
}

#[derive(Deserialize)]
struct Q {
    q: String,
}

async fn search(
    State(state): State<AppState>,
    company: Company,
    Query(query): Query<Q>,
) -> Result<Json<Value>, ApiError> {
    if query.q.len() > 500 {
        return Err(bad("the search is too long"));
    }

    crate::projects::sync_person(&state, &company).await?;
    let p = parse(&query.q);
    let user = company.user.0.user_id;
    if p.words.is_empty() && !p.filtered() {
        return Ok(Json(json!({ "boards": [], "cards": [], "total": 0 })));
    }

    let result = company
        .db
        .read(move |c| {
            let scope = format!(
                "JOIN boards b ON b.id = k.board_id
                 JOIN lists l ON l.id = k.list_id
                 JOIN projects pr ON pr.id = b.project_id
                 JOIN project_members pm ON pm.project_id = pr.id AND pm.user_id = ?2
                 WHERE pr.deleted_at IS NULL AND pr.archived_at IS NULL AND b.closed_at IS NULL AND {VISIBLE}"
            );
            let mut sql = format!(
                "SELECT k.id, k.title, b.id, b.name, b.background, l.title, k.due_at, k.done, k.archived_at IS NOT NULL, pr.name
                 FROM cards k {scope}"
            );
            let mut args: Vec<Sql> = vec![Sql::Integer(0), Sql::Integer(user)];
            if !p.words.is_empty() {
                args.push(Sql::Text(fts_query(&p.words)));
                sql.push_str(&format!(
                    " AND k.id IN (SELECT rowid FROM cards_fts WHERE cards_fts MATCH ?{})",
                    args.len()
                ));
            }
            sql.push_str(if p.archived {
                " AND (k.archived_at IS NOT NULL OR l.archived_at IS NOT NULL)"
            } else {
                " AND k.archived_at IS NULL AND l.archived_at IS NULL"
            });
            for m in &p.members {
                args.push(Sql::Text(like(m)));
                sql.push_str(&format!(
                    " AND EXISTS (SELECT 1 FROM card_members cm JOIN people pe ON pe.id = cm.user_id
                       WHERE cm.card_id = k.id AND (lower(pe.name) LIKE ?{n} ESCAPE '\\' OR lower(pe.email) LIKE ?{n} ESCAPE '\\'))",
                    n = args.len()
                ));
            }
            if p.mine {
                sql.push_str(" AND EXISTS (SELECT 1 FROM card_members cm WHERE cm.card_id = k.id AND cm.user_id = ?2)");
            }
            for l in &p.labels {
                args.push(Sql::Text(like(l)));
                sql.push_str(&format!(
                    " AND EXISTS (SELECT 1 FROM card_labels cl JOIN labels lb ON lb.id = cl.label_id
                       WHERE cl.card_id = k.id AND lower(lb.name) LIKE ?{} ESCAPE '\\')",
                    args.len()
                ));
            }
            if p.overdue {
                args.push(Sql::Integer(now()));
                sql.push_str(&format!(
                    " AND k.due_at IS NOT NULL AND k.due_at < ?{} AND k.done = 0",
                    args.len()
                ));
            }
            if p.open {
                sql.push_str(" AND k.done = 0");
            }
            if p.done {
                sql.push_str(" AND k.done = 1");
            }
            if p.attachments {
                sql.push_str(" AND EXISTS (SELECT 1 FROM attachments a WHERE a.card_id = k.id)");
            }
            if !p.unknown.is_empty() {
                sql.push_str(" AND 0");
            }
            sql.push_str(" ORDER BY k.updated_at DESC, k.id DESC");
            let count_sql = format!("SELECT count(*) FROM ({sql})");
            let total: i64 = c.query_row(&count_sql, rusqlite::params_from_iter(args.iter()), |r| r.get(0))?;
            sql.push_str(&format!(" LIMIT {LIMIT}"));
            let mut stmt = c.prepare(&sql)?;
            let cards = stmt
                .query_map(rusqlite::params_from_iter(args.iter()), |r| {
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "title": r.get::<_, String>(1)?,
                        "board": { "id": r.get::<_, i64>(2)?, "name": r.get::<_, String>(3)?, "background": r.get::<_, String>(4)? },
                        "list": r.get::<_, String>(5)?,
                        "due_at": r.get::<_, Option<i64>>(6)?,
                        "done": r.get::<_, bool>(7)?,
                        "archived": r.get::<_, bool>(8)?,
                        "project": r.get::<_, String>(9)?,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            let only_words = !p.filtered() && p.unknown.is_empty();
            let mut boards = Vec::new();
            if only_words && !p.words.is_empty() {
                let phrase = like(&p.words.join(" "));
                let mut stmt = c.prepare(&format!(
                    "SELECT b.id, b.name, b.background, pr.name FROM boards b
                     JOIN projects pr ON pr.id = b.project_id
                     JOIN project_members pm ON pm.project_id = pr.id AND pm.user_id = ?2
                     WHERE ?1 = 0 AND pr.deleted_at IS NULL AND pr.archived_at IS NULL AND b.closed_at IS NULL AND {VISIBLE}
                       AND lower(b.name) LIKE ?3 ESCAPE '\\'
                     ORDER BY b.name COLLATE NOCASE LIMIT 6"
                ))?;
                boards = stmt
                    .query_map(rusqlite::params![0, user, phrase], |r| {
                        Ok(json!({
                            "id": r.get::<_, i64>(0)?,
                            "name": r.get::<_, String>(1)?,
                            "background": r.get::<_, String>(2)?,
                            "project": r.get::<_, String>(3)?,
                        }))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
            }
            Ok(json!({ "boards": boards, "cards": cards, "total": total }))
        })
        .await?;
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operators_are_read_from_the_query() {
        let p = parse("hero @Mai #Bug is:overdue is:archived has:attachments @me launch!");
        assert_eq!(p.words, vec!["hero", "launch"]);
        assert_eq!(p.members, vec!["mai"]);
        assert_eq!(p.labels, vec!["bug"]);
        assert!(p.overdue && p.archived && p.attachments && p.mine);
        assert!(parse("is:weird").unknown.contains(&"is:weird".to_string()));
    }

    #[test]
    fn fts_query_quotes_each_word() {
        assert_eq!(
            fts_query(&["hero".into(), "tr\"ang".into()]),
            "\"hero\"* AND \"trang\"*"
        );
        assert_eq!(
            parse("NEAR(a b) OR x*").words,
            vec!["neara", "b", "or", "x"]
        );
    }
}
