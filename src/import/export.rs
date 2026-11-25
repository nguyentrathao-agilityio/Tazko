use std::collections::BTreeSet;

use rusqlite::Connection;
use serde_json::{Value, json};

use super::file::{TAZKO_FORMAT, TAZKO_VERSION};
use crate::boards::BoardInfo;

fn key(person: i64) -> String {
    format!("p{person}")
}

fn rows<T>(
    c: &Connection,
    sql: &str,
    p: impl rusqlite::Params,
    f: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> rusqlite::Result<Vec<T>> {
    c.prepare_cached(sql)?.query_map(p, f)?.collect()
}

pub fn board(c: &Connection, board: i64, info: &BoardInfo, at: i64) -> rusqlite::Result<Value> {
    let labels = rows(
        c,
        "SELECT id, name, color FROM labels WHERE board_id = ?1 ORDER BY position, id",
        [board],
        |r| {
            Ok(
                json!({ "key": format!("l{}", r.get::<_, i64>(0)?), "name": r.get::<_, String>(1)?, "color": r.get::<_, i64>(2)? }),
            )
        },
    )?;
    let lists = rows(
        c,
        "SELECT id, title, archived_at IS NOT NULL FROM lists WHERE board_id = ?1 ORDER BY position, id",
        [board],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, bool>(2)?,
            ))
        },
    )?;
    let mut people = BTreeSet::new();
    let mut out = Vec::with_capacity(lists.len());
    for (list, title, archived) in lists {
        let mut cards = Vec::new();
        for card in rows(
            c,
            "SELECT id, title, description, start_at, due_at, done, archived_at IS NOT NULL FROM cards
             WHERE list_id = ?1 AND is_template = 0 ORDER BY position, id",
            [list],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    json!({
                        "title": r.get::<_, String>(1)?,
                        "description_html": r.get::<_, String>(2)?,
                        "start_at": r.get::<_, Option<i64>>(3)?,
                        "due_at": r.get::<_, Option<i64>>(4)?,
                        "done": r.get::<_, bool>(5)?,
                        "archived": r.get::<_, bool>(6)?,
                    }),
                ))
            },
        )? {
            cards.push(card_detail(c, card, &mut people)?);
        }
        out.push(json!({ "title": title, "archived": archived, "cards": cards }));
    }

    let mut known = Vec::new();
    for person in people {
        let row = c.query_row(
            "SELECT name, email FROM people WHERE id = ?1",
            [person],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        );
        if let Ok((name, email)) = row {
            known.push(json!({ "key": key(person), "name": name, "email": email }));
        }
    }

    Ok(json!({
        "format": TAZKO_FORMAT,
        "version": TAZKO_VERSION,
        "exported_at": at,
        "board": { "name": info.name, "description": info.description, "background": info.background },
        "labels": labels,
        "people": known,
        "lists": out,
    }))
}

fn card_detail(
    c: &Connection,
    (card, mut v): (i64, Value),
    people: &mut BTreeSet<i64>,
) -> rusqlite::Result<Value> {
    v["labels"] = json!(rows(
        c,
        "SELECT label_id FROM card_labels WHERE card_id = ?1 ORDER BY label_id",
        [card],
        |r| Ok(format!("l{}", r.get::<_, i64>(0)?)),
    )?);
    let members = rows(
        c,
        "SELECT user_id FROM card_members WHERE card_id = ?1 ORDER BY user_id",
        [card],
        |r| r.get::<_, i64>(0),
    )?;
    people.extend(members.iter().copied());
    v["members"] = json!(members.into_iter().map(key).collect::<Vec<_>>());

    let mut checklists = Vec::new();
    for (id, title) in rows(
        c,
        "SELECT id, title FROM checklists WHERE card_id = ?1 ORDER BY position, id",
        [card],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
    )? {
        let items = rows(
            c,
            "SELECT text, done, assignee_id, due_at FROM checklist_items WHERE checklist_id = ?1 ORDER BY position, id",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, bool>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            },
        )?;
        let items: Vec<Value> = items
            .into_iter()
            .map(|(text, done, assignee, due)| {
                people.extend(assignee);
                json!({ "text": text, "done": done, "assignee": assignee.map(key), "due_at": due })
            })
            .collect();
        checklists.push(json!({ "title": title, "items": items }));
    }
    v["checklists"] = json!(checklists);

    let comments = rows(
        c,
        "SELECT m.user_id, COALESCE(p.name, m.author_name, ''), m.body, m.created_at FROM comments m
         LEFT JOIN people p ON p.id = m.user_id WHERE m.card_id = ?1 ORDER BY m.id",
        [card],
        |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?)),
    )?;
    v["comments"] = json!(
        comments
            .into_iter()
            .map(|(user, name, text, at)| {
                people.extend(user);
                json!({ "author": user.map(key), "author_name": name, "text": text, "created_at": at })
            })
            .collect::<Vec<_>>()
    );

    let attachments = rows(
        c,
        "SELECT kind, name, url FROM attachments WHERE card_id = ?1 ORDER BY id",
        [card],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        },
    )?;
    let mut links = Vec::new();
    let mut files = Vec::new();
    for (kind, name, url) in attachments {
        match (kind.as_str(), url) {
            ("link", Some(url)) => links.push(json!({ "name": name, "url": url })),
            _ => files.push(json!({ "name": name })),
        }
    }
    v["links"] = json!(links);
    v["files"] = json!(files);
    Ok(v)
}

pub fn file_name(board_name: &str) -> String {
    let slug: String = board_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "tazko-board.json".into()
    } else {
        format!("tazko-{slug}.json")
    }
}
