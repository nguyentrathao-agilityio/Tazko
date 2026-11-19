pub mod cards;

use std::collections::HashMap;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::{delete, get, post, put};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::api::COMPANY_PREFIX;
use crate::auth::check_csrf;
use crate::boards::Board;
use crate::db::Database;
use crate::error::{ApiError, bad};
use crate::events;
use crate::order;
use crate::projects::{Act, BACKGROUNDS, path_id};
use crate::state::AppState;
use crate::tenant::{Company, CompanyRole};

pub const MAX_LISTS: usize = 50;
pub const MAX_CARDS: usize = 500;
const MAX_TEMPLATES: i64 = 100;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TemplateLabel {
    pub name: String,
    pub color: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TemplateCard {
    pub title: String,
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TemplateList {
    pub title: String,
    #[serde(default)]
    pub cards: Vec<TemplateCard>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TemplateField {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub show_on_card: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TemplateData {
    #[serde(default)]
    pub labels: Vec<TemplateLabel>,
    #[serde(default)]
    pub lists: Vec<TemplateList>,
    #[serde(default)]
    pub fields: Vec<TemplateField>,
}

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}{s}");
    Router::new()
        .route(&p("/templates"), get(list))
        .route(&p("/templates/{template}"), delete(remove))
        .route(&p("/templates/{template}/offer"), put(offer))
        .route(&p("/admin-templates/{template}"), delete(withdraw))
        .route(&p("/boards/{board}/save-template"), post(save))
        .merge(cards::routes())
}

pub fn snapshot(c: &Connection, board: i64) -> rusqlite::Result<TemplateData> {
    let mut stmt =
        c.prepare("SELECT id, name, color FROM labels WHERE board_id = ?1 ORDER BY position, id")?;
    let labels: Vec<(i64, String, i64)> = stmt
        .query_map([board], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let names: HashMap<i64, String> = labels.iter().map(|(id, n, _)| (*id, n.clone())).collect();
    let mut stmt = c.prepare(
        "SELECT id, title FROM lists WHERE board_id = ?1 AND archived_at IS NULL ORDER BY position LIMIT ?2",
    )?;
    let lists: Vec<(i64, String)> = stmt
        .query_map(params![board, MAX_LISTS as i64], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut out = Vec::new();
    let mut total = 0;
    for (list, title) in lists {
        let mut stmt = c.prepare(
            "SELECT id, title FROM cards WHERE list_id = ?1 AND archived_at IS NULL ORDER BY position",
        )?;
        let rows: Vec<(i64, String)> = stmt
            .query_map([list], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        let mut cards = Vec::new();
        for (card, t) in rows {
            if total >= MAX_CARDS {
                break;
            }

            total += 1;
            let mut stmt = c.prepare("SELECT label_id FROM card_labels WHERE card_id = ?1")?;
            let mut labels: Vec<String> = stmt
                .query_map([card], |r| r.get::<_, i64>(0))?
                .filter_map(Result::ok)
                .filter_map(|l| names.get(&l).cloned())
                .filter(|n| !n.is_empty())
                .collect();
            labels.sort();
            cards.push(TemplateCard { title: t, labels });
        }
        out.push(TemplateList { title, cards });
    }
    let fields = crate::cards::fields::defs(c, board)?
        .into_iter()
        .map(|d| TemplateField {
            name: d.name,
            kind: d.kind,
            options: d.options,
            show_on_card: d.show_on_card,
        })
        .collect();
    Ok(TemplateData {
        labels: labels
            .into_iter()
            .map(|(_, name, color)| TemplateLabel { name, color })
            .collect(),
        lists: out,
        fields,
    })
}

pub fn apply(c: &Connection, board: i64, data: &TemplateData, by: i64) -> rusqlite::Result<()> {
    let mut label_ids: HashMap<String, i64> = HashMap::new();
    for (i, l) in data.labels.iter().enumerate() {
        c.execute(
            "INSERT INTO labels (board_id, name, color, position) VALUES (?1, ?2, ?3, ?4)",
            params![board, l.name, l.color.clamp(0, 7), i as i64],
        )?;
        if !l.name.is_empty() {
            label_ids
                .entry(l.name.clone())
                .or_insert(c.last_insert_rowid());
        }
    }
    for (list, pos) in data.lists.iter().zip(order::spread(data.lists.len())) {
        c.execute(
            "INSERT INTO lists (board_id, title, position) VALUES (?1, ?2, ?3)",
            params![board, list.title, pos],
        )?;
        let list_id = c.last_insert_rowid();
        for (card, cpos) in list.cards.iter().zip(order::spread(list.cards.len())) {
            c.execute(
                "INSERT INTO cards (board_id, list_id, title, position, created_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![board, list_id, card.title, cpos, by],
            )?;
            let id = c.last_insert_rowid();
            for name in &card.labels {
                if let Some(l) = label_ids.get(name) {
                    c.execute(
                        "INSERT INTO card_labels (card_id, label_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                        params![id, l],
                    )?;
                }
            }
        }
    }
    for (i, f) in data.fields.iter().enumerate() {
        c.execute(
            "INSERT INTO custom_field_defs (board_id, name, type, options, show_on_card, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![board, f.name, f.kind, json!(f.options).to_string(), f.show_on_card, i as i64],
        )?;
    }
    Ok(())
}

fn clean(s: &str, max: usize) -> Option<String> {
    let t = s.trim();
    (!t.is_empty() && t.chars().count() <= max && !t.chars().any(char::is_control))
        .then(|| t.to_string())
}

pub fn validate(data: &TemplateData) -> Result<(), ApiError> {
    if data.lists.is_empty() || data.lists.len() > MAX_LISTS {
        return Err(bad("a template has 1 to 50 lists"));
    }

    if data.lists.iter().map(|l| l.cards.len()).sum::<usize>() > MAX_CARDS {
        return Err(bad("a template has up to 500 starter cards"));
    }

    let ok_lists = data.lists.iter().all(|l| {
        clean(&l.title, 200).is_some() && l.cards.iter().all(|c| clean(&c.title, 500).is_some())
    });
    let ok_labels =
        data.labels.len() <= 50 && data.labels.iter().all(|l| l.name.chars().count() <= 40);
    let ok_fields = data.fields.len() as i64 <= crate::cards::fields::MAX_FIELDS
        && data.fields.iter().all(|f| {
            crate::cards::fields::TYPES.contains(&f.kind.as_str())
                && clean(&f.name, 40).is_some()
                && f.options.len() <= 20
        });
    if ok_lists && ok_labels && ok_fields {
        Ok(())
    } else {
        Err(bad("this template is not valid"))
    }
}

fn summary(
    id: String,
    name: String,
    description: String,
    background: String,
    data: &str,
    by: String,
    admin: bool,
) -> Value {
    let d: TemplateData = serde_json::from_str(data).unwrap_or_default();
    json!({
        "id": id,
        "name": name,
        "description": description,
        "background": background,
        "admin": admin,
        "by": by,
        "lists": d.lists.iter().map(|l| json!({ "title": l.title, "cards": l.cards.iter().map(|c| c.title.clone()).collect::<Vec<_>>() })).collect::<Vec<_>>(),
        "labels": d.labels.len(),
        "fields": d.fields.len(),
    })
}

pub enum Source {
    Company(i64),
    Admin(i64),
}

pub fn parse_id(raw: &str) -> Option<Source> {
    let (kind, n) = raw.split_once(':')?;
    let n = n.parse::<i64>().ok().filter(|n| *n > 0)?;
    match kind {
        "c" => Some(Source::Company(n)),
        "a" => Some(Source::Admin(n)),
        _ => None,
    }
}

pub async fn load(
    state: &AppState,
    company_db: &Database,
    raw: &str,
) -> Result<(TemplateData, String), ApiError> {
    let source = parse_id(raw).ok_or(ApiError::NotFound)?;
    let row: Option<(String, String)> = match source {
        Source::Company(id) => {
            company_db
                .read(move |c| {
                    Ok(c.query_row(
                        "SELECT data, background FROM board_templates WHERE id = ?1",
                        [id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?)
                })
                .await?
        }
        Source::Admin(id) => {
            state
                .dbs
                .control()
                .read(move |c| {
                    Ok(c.query_row(
                        "SELECT data, background FROM admin_templates WHERE id = ?1",
                        [id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?)
                })
                .await?
        }
    };
    let (data, background) = row.ok_or(ApiError::NotFound)?;
    let data: TemplateData =
        serde_json::from_str(&data).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok((data, background))
}

async fn list(State(state): State<AppState>, co: Company) -> Result<Json<Value>, ApiError> {
    let mine = co
        .db
        .read(|c| {
            let mut stmt = c.prepare(
                "SELECT t.id, t.name, t.description, t.background, t.data, coalesce(nullif(p.name, ''), p.email, '')
                 FROM board_templates t LEFT JOIN people p ON p.id = t.created_by ORDER BY t.id DESC",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?))
                })?
                .collect::<Result<Vec<(i64, String, String, String, String, String)>, _>>()?;
            Ok(rows)
        })
        .await?;
    let offered = state
        .dbs
        .control()
        .read(|c| {
            let mut stmt =
                c.prepare("SELECT id, name, description, background, data FROM admin_templates ORDER BY id DESC")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, String>(4)?)))?
                .collect::<Result<Vec<(i64, String, String, String, String)>, _>>()?;
            Ok(rows)
        })
        .await?;
    let mut out: Vec<Value> = mine
        .into_iter()
        .map(|(id, n, d, b, data, _)| {
            summary(format!("c:{id}"), n, d, b, &data, co.name.clone(), false)
        })
        .collect();
    out.extend(offered.into_iter().map(|(id, n, d, b, data)| {
        summary(
            format!("a:{id}"),
            n,
            d,
            b,
            &data,
            "Tazko admin".into(),
            true,
        )
    }));
    let can_offer =
        co.user.0.is_super_admin && co.role == CompanyRole::Owner && co.user.0.token.is_none();
    Ok(Json(
        json!({ "templates": out, "can_offer": can_offer, "company": co.name }),
    ))
}

#[derive(Deserialize)]
struct SaveTemplate {
    name: String,
    #[serde(default)]
    description: String,
}

async fn save(
    board: Board,
    headers: HeaderMap,
    Json(body): Json<SaveTemplate>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::Manage)?;
    let name = clean(&body.name, 80).ok_or_else(|| bad("give the template a name"))?;
    let description = body.description.trim().to_string();
    if description.chars().count() > 300 || description.chars().any(char::is_control) {
        return Err(bad("the description is too long"));
    }

    let background = if BACKGROUNDS.contains(&board.info.background.as_str()) {
        board.info.background.clone()
    } else {
        board.project.info.background.clone()
    };
    let (id, at) = (board.id, board.at());
    let saved = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let count: i64 = tx.query_row("SELECT count(*) FROM board_templates", [], |r| r.get(0))?;
            if count >= MAX_TEMPLATES {
                return Ok(Err(bad("this company has 100 templates; delete some first")));
            }

            let data = snapshot(&tx, id)?;
            if data.lists.is_empty() {
                return Ok(Err(bad("add a list before saving a template")));
            }

            tx.execute(
                "INSERT INTO board_templates (name, description, background, data, created_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![name, description, background, json!(data).to_string(), by],
            )?;
            let template = tx.last_insert_rowid();
            events::record(&tx, by, "board_saved_as_template", at, json!({ "template": template, "name": name }))?;
            tx.commit()?;
            Ok(Ok(json!({ "id": format!("c:{template}"), "name": name })))
        })
        .await??;
    Ok((StatusCode::CREATED, Json(saved)))
}

async fn remove(
    co: Company,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    check_csrf(&co.user, &headers)?;
    let user = co.user.0.user_id;
    let owner = co.role == CompanyRole::Owner;
    let id = path_id(&params, "template")?;
    let n = co
        .db
        .write(move |c| {
            Ok(c.execute(
                "DELETE FROM board_templates WHERE id = ?1 AND (?2 OR created_by = ?3)",
                params![id, owner, user],
            )?)
        })
        .await?;
    if n == 0 {
        return Err(ApiError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn offer(
    State(state): State<AppState>,
    co: Company,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    check_csrf(&co.user, &headers)?;
    if !co.user.0.is_super_admin || co.role != CompanyRole::Owner || co.user.0.token.is_some() {
        return Err(ApiError::Forbidden);
    }

    let id = path_id(&params, "template")?;
    let row: Option<(String, String, String, String)> = co
        .db
        .read(move |c| {
            Ok(c.query_row(
                "SELECT name, description, background, data FROM board_templates WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?)
        })
        .await?;
    let (name, description, background, data) = row.ok_or(ApiError::NotFound)?;
    let by = co.user.0.user_id;
    let org = co.org_id;
    let new = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            tx.execute(
                "INSERT INTO admin_templates (name, description, background, data, created_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![name, description, background, data, by],
            )?;
            let new = tx.last_insert_rowid();
            crate::admin::audit::record(&tx, Some(by), "template_offered", Some(org), &name, json!({ "template": new }))?;
            tx.commit()?;
            Ok(new)
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": format!("a:{new}") })),
    ))
}

async fn withdraw(
    State(state): State<AppState>,
    co: Company,
    headers: HeaderMap,
    Path(params): Path<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    check_csrf(&co.user, &headers)?;
    if !co.user.0.is_super_admin || co.user.0.token.is_some() {
        return Err(ApiError::Forbidden);
    }

    let id = path_id(&params, "template")?;
    let by = co.user.0.user_id;
    let n = state
        .dbs
        .control()
        .write(move |c| {
            let tx = c.transaction()?;
            let n = tx.execute("DELETE FROM admin_templates WHERE id = ?1", [id])?;
            if n > 0 {
                crate::admin::audit::record(
                    &tx,
                    Some(by),
                    "template_withdrawn",
                    None,
                    "",
                    json!({ "template": id }),
                )?;
            }
            tx.commit()?;
            Ok(n)
        })
        .await?;
    if n == 0 {
        return Err(ApiError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}
