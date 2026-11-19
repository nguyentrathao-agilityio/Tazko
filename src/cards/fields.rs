use std::collections::HashMap;

use axum::Router;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::{patch, post, put};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::card_in_board;
use crate::api::COMPANY_PREFIX;
use crate::boards::Board;
use crate::error::{ApiError, bad};
use crate::events::{self, At};
use crate::projects::{Act, path_id};
use crate::state::AppState;

pub const TYPES: &[&str] = &["text", "number", "date", "select", "check"];
pub const MAX_FIELDS: i64 = 20;
const MAX_OPTIONS: usize = 20;
const MAX_TEXT: usize = 200;

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}/boards/{{board}}{s}");
    Router::new()
        .route(&p("/fields"), post(create))
        .route(&p("/fields/{field}"), patch(change).delete(remove))
        .route(&p("/cards/{card}/fields/{field}"), put(set_value))
}

type Params = Path<HashMap<String, String>>;

pub struct Def {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub options: Vec<String>,
    pub show_on_card: bool,
}

impl Def {
    pub fn json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "type": self.kind,
            "options": self.options,
            "show_on_card": self.show_on_card,
        })
    }
}

fn def_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Def> {
    let options: String = r.get(3)?;
    Ok(Def {
        id: r.get(0)?,
        name: r.get(1)?,
        kind: r.get(2)?,
        options: serde_json::from_str(&options).unwrap_or_default(),
        show_on_card: r.get(4)?,
    })
}

const DEF_COLUMNS: &str = "id, name, type, options, show_on_card";

pub fn defs(c: &Connection, board: i64) -> rusqlite::Result<Vec<Def>> {
    let mut stmt = c.prepare(&format!(
        "SELECT {DEF_COLUMNS} FROM custom_field_defs WHERE board_id = ?1 ORDER BY position, id"
    ))?;
    stmt.query_map([board], def_row)?.collect()
}

fn def(c: &Connection, board: i64, field: i64) -> rusqlite::Result<Option<Def>> {
    c.query_row(
        &format!("SELECT {DEF_COLUMNS} FROM custom_field_defs WHERE id = ?1 AND board_id = ?2"),
        params![field, board],
        def_row,
    )
    .optional()
}

pub fn shown(kind: &str, raw: &str) -> Value {
    match kind {
        "number" => raw
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        "check" => Value::Bool(raw == "1"),
        _ => Value::String(raw.to_string()),
    }
}

pub fn card_values(
    c: &Connection,
    card: i64,
    only_front: bool,
) -> rusqlite::Result<Map<String, Value>> {
    let mut stmt = c.prepare(
        "SELECT d.id, d.type, v.value FROM custom_field_values v JOIN custom_field_defs d ON d.id = v.field_id
         WHERE v.card_id = ?1 AND (?2 = 0 OR d.show_on_card = 1)",
    )?;
    let rows: Vec<(i64, String, String)> = stmt
        .query_map(params![card, only_front], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows
        .into_iter()
        .map(|(id, kind, raw)| (id.to_string(), shown(&kind, &raw)))
        .collect())
}

pub fn board_values(
    c: &Connection,
    board: i64,
) -> rusqlite::Result<HashMap<i64, Map<String, Value>>> {
    let mut stmt = c.prepare(
        "SELECT v.card_id, d.id, d.type, v.value FROM custom_field_values v
         JOIN custom_field_defs d ON d.id = v.field_id
         WHERE d.board_id = ?1",
    )?;
    let mut out: HashMap<i64, Map<String, Value>> = HashMap::new();
    let rows = stmt.query_map([board], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    for row in rows {
        let (card, field, kind, raw) = row?;
        out.entry(card)
            .or_default()
            .insert(field.to_string(), shown(&kind, &raw));
    }
    Ok(out)
}

fn clean_text(raw: &str, what: &str, max: usize) -> Result<String, ApiError> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(bad(&format!("enter {what}")));
    }

    if t.chars().count() > max || t.chars().any(char::is_control) {
        return Err(bad(&format!("{what} is too long")));
    }

    Ok(t.to_string())
}

fn clean_options(raw: &[String]) -> Result<Vec<String>, ApiError> {
    let mut out: Vec<String> = Vec::new();
    for o in raw {
        let o = clean_text(o, "an option", 40)?;
        if !out.iter().any(|x| x.eq_ignore_ascii_case(&o)) {
            out.push(o);
        }
    }
    if out.len() > MAX_OPTIONS {
        return Err(bad("a dropdown can have up to 20 options"));
    }

    Ok(out)
}

pub fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }

    let num = |r: std::ops::Range<usize>| s[r].parse::<u32>().ok();
    let (Some(y), Some(m), Some(d)) = (num(0..4), num(5..7), num(8..10)) else {
        return false;
    };
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => return false,
    };
    (1900..=2999).contains(&y) && (1..=days).contains(&d)
}

pub fn value_for(def: &Def, v: &Value) -> Result<Option<String>, ApiError> {
    if v.is_null() || v.as_str().is_some_and(|s| s.trim().is_empty()) {
        return Ok(None);
    }

    match def.kind.as_str() {
        "text" => v
            .as_str()
            .ok_or_else(|| bad("enter text"))
            .and_then(|s| clean_text(s, "the value", MAX_TEXT))
            .map(Some),
        "number" => {
            let n = match v {
                Value::Number(n) => n.as_f64(),
                Value::String(s) => s.trim().parse::<f64>().ok(),
                _ => None,
            };
            match n {
                Some(n) if n.is_finite() && n.abs() < 1e15 => Ok(Some(n.to_string())),
                _ => Err(bad("enter a number")),
            }
        }
        "date" => match v.as_str() {
            Some(s) if valid_date(s.trim()) => Ok(Some(s.trim().to_string())),
            _ => Err(bad("enter a date as YYYY-MM-DD")),
        },
        "select" => match v.as_str() {
            Some(s) if def.options.iter().any(|o| o == s) => Ok(Some(s.to_string())),
            _ => Err(bad("choose one of the options")),
        },
        "check" => match v.as_bool() {
            Some(true) => Ok(Some("1".into())),
            Some(false) => Ok(None),
            None => Err(bad("a checkbox is true or false")),
        },
        _ => Err(bad("unknown field type")),
    }
}

#[derive(Deserialize)]
struct NewField {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    options: Vec<String>,
    #[serde(default)]
    show_on_card: bool,
}

async fn create(
    board: Board,
    headers: HeaderMap,
    Json(body): Json<NewField>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let name = clean_text(&body.name, "a field name", 40)?;
    if !TYPES.contains(&body.kind.as_str()) {
        return Err(bad("choose text, number, date, dropdown or checkbox"));
    }

    let options = if body.kind == "select" {
        let o = clean_options(&body.options)?;
        if o.is_empty() {
            return Err(bad("a dropdown needs at least one option"));
        }

        o
    } else {
        Vec::new()
    };
    let (id, at) = (board.id, board.at());
    let saved = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let count: i64 = tx.query_row(
                "SELECT count(*) FROM custom_field_defs WHERE board_id = ?1",
                [id],
                |r| r.get(0),
            )?;
            if count >= MAX_FIELDS {
                return Ok(Err(bad("a board can have up to 20 custom fields")));
            }

            tx.execute(
                "INSERT INTO custom_field_defs (board_id, name, type, options, show_on_card, position)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, name, body.kind, json!(options).to_string(), body.show_on_card, count],
            )?;
            let field = tx.last_insert_rowid();
            events::record(&tx, by, "field_created", at, json!({ "field": field, "name": name }))?;
            let d = def(&tx, id, field)?.map(|d| d.json());
            tx.commit()?;
            Ok(Ok(d))
        })
        .await??;
    Ok((StatusCode::CREATED, Json(saved.unwrap_or(Value::Null))))
}

#[derive(Deserialize)]
struct FieldChange {
    name: Option<String>,
    options: Option<Vec<String>>,
    show_on_card: Option<bool>,
}

async fn change(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<FieldChange>,
) -> Result<Json<Value>, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let field = path_id(&params, "field")?;
    let name = body
        .name
        .as_deref()
        .map(|n| clean_text(n, "a field name", 40))
        .transpose()?;
    let options = body.options.as_deref().map(clean_options).transpose()?;
    let (id, at) = (board.id, board.at());
    let saved = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(mut d) = def(&tx, id, field)? else {
                return Ok(Err(ApiError::NotFound));
            };
            if let Some(n) = name {
                d.name = n;
            }

            if let Some(o) = options {
                if d.kind != "select" {
                    return Ok(Err(bad("only dropdowns have options")));
                }

                if o.is_empty() {
                    return Ok(Err(bad("a dropdown needs at least one option")));
                }

                d.options = o;
                let mut stmt = tx.prepare("SELECT card_id, value FROM custom_field_values WHERE field_id = ?1")?;
                let gone: Vec<i64> = stmt
                    .query_map([field], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
                    .filter_map(Result::ok)
                    .filter(|(_, v)| !d.options.contains(v))
                    .map(|(card, _)| card)
                    .collect();
                drop(stmt);
                for card in gone {
                    tx.execute(
                        "DELETE FROM custom_field_values WHERE field_id = ?1 AND card_id = ?2",
                        params![field, card],
                    )?;
                }
            }

            if let Some(s) = body.show_on_card {
                d.show_on_card = s;
            }

            tx.execute(
                "UPDATE custom_field_defs SET name = ?2, options = ?3, show_on_card = ?4 WHERE id = ?1",
                params![field, d.name, json!(d.options).to_string(), d.show_on_card],
            )?;
            events::record(&tx, by, "field_changed", at, json!({ "field": field, "name": d.name }))?;
            tx.commit()?;
            Ok(Ok(d.json()))
        })
        .await??;
    Ok(Json(saved))
}

async fn remove(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let field = path_id(&params, "field")?;
    let (id, at) = (board.id, board.at());
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let Some(d) = def(&tx, id, field)? else {
                return Ok(Err(ApiError::NotFound));
            };
            tx.execute("DELETE FROM custom_field_defs WHERE id = ?1", [field])?;
            events::record(
                &tx,
                by,
                "field_deleted",
                at,
                json!({ "field": field, "name": d.name }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SetValue {
    #[serde(default)]
    value: Value,
}

async fn set_value(
    board: Board,
    headers: HeaderMap,
    Path(params): Params,
    Json(body): Json<SetValue>,
) -> Result<Json<Value>, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let card = path_id(&params, "card")?;
    let field = path_id(&params, "field")?;
    let (id, pid) = (board.id, board.project.id);
    let saved = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            if card_in_board(&tx, id, card, false)?.is_none() {
                return Ok(Err(ApiError::NotFound));
            }

            let Some(d) = def(&tx, id, field)? else {
                return Ok(Err(ApiError::NotFound));
            };
            let value = match value_for(&d, &body.value) {
                Ok(v) => v,
                Err(e) => return Ok(Err(e)),
            };
            match &value {
                Some(v) => tx.execute(
                    "INSERT INTO custom_field_values (card_id, field_id, value) VALUES (?1, ?2, ?3)
                     ON CONFLICT (card_id, field_id) DO UPDATE SET value = excluded.value",
                    params![card, field, v],
                )?,
                None => tx.execute(
                    "DELETE FROM custom_field_values WHERE card_id = ?1 AND field_id = ?2",
                    params![card, field],
                )?,
            };
            tx.execute(
                "UPDATE cards SET updated_at = unixepoch() WHERE id = ?1",
                [card],
            )?;
            events::record(
                &tx,
                by,
                "field_set",
                At::card(pid, id, card),
                json!({ "field": field, "name": d.name }),
            )?;
            tx.commit()?;
            Ok(Ok(
                json!({ "value": value.map(|v| shown(&d.kind, &v)).unwrap_or(Value::Null) }),
            ))
        })
        .await??;
    Ok(Json(saved))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(kind: &str) -> Def {
        Def {
            id: 1,
            name: "F".into(),
            kind: kind.into(),
            options: vec!["Low".into(), "High".into()],
            show_on_card: false,
        }
    }

    #[test]
    fn values_are_checked_against_the_field_type() {
        assert_eq!(
            value_for(&d("number"), &json!(3.5)).unwrap(),
            Some("3.5".into())
        );
        assert_eq!(
            value_for(&d("number"), &json!("12")).unwrap(),
            Some("12".into())
        );
        assert!(value_for(&d("number"), &json!("twelve")).is_err());
        assert_eq!(
            value_for(&d("date"), &json!("2026-02-28")).unwrap(),
            Some("2026-02-28".into())
        );
        assert!(value_for(&d("date"), &json!("2026-02-30")).is_err());
        assert!(value_for(&d("select"), &json!("Medium")).is_err());
        assert_eq!(
            value_for(&d("select"), &json!("High")).unwrap(),
            Some("High".into())
        );
        assert_eq!(value_for(&d("check"), &json!(false)).unwrap(), None);
        assert_eq!(value_for(&d("text"), &json!("  ")).unwrap(), None);
        assert!(value_for(&d("text"), &json!("x".repeat(201))).is_err());
        assert_eq!(shown("number", "3.5"), json!(3.5));
        assert_eq!(shown("check", "1"), json!(true));
    }
}
