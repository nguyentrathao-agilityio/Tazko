pub mod export;
pub mod file;

use std::collections::{BTreeMap, HashMap};

use axum::Router;
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::value::RawValue;
use serde_json::{Value, json};
use tokio::sync::Semaphore;

use crate::admin::audit;
use crate::api::COMPANY_PREFIX;
use crate::boards::Board;
use crate::error::{ApiError, bad};
use crate::events::{self, At};
use crate::order;
use crate::projects::{Act, Project};
use crate::session::now;
use crate::state::AppState;
use file::{BoardFile, Counts};

const MAX_REQUEST: u64 = file::MAX_FILE_BYTES as u64 + 2 * 1024 * 1024;

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}{s}");
    Router::new()
        .route(&p("/boards/{board}/export"), get(export_board))
        .route(&p("/projects/{project}/imports/preview"), post(preview))
        .route(&p("/projects/{project}/imports"), post(import))
}

async fn export_board(board: Board) -> Result<Response, ApiError> {
    let (id, info) = (board.id, board.info.clone());
    let name = export::file_name(&info.name);
    let v = board
        .project
        .db()
        .read(move |c| Ok(export::board(c, id, &info, now())?))
        .await?;
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{name}\""))
        .map_err(|_| ApiError::NotFound)?;
    Ok((
        [
            (header::CONTENT_DISPOSITION, disposition),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
        ],
        Json(v),
    )
        .into_response())
}

#[derive(Deserialize)]
struct FileName {
    name: Option<String>,
}

#[derive(Clone)]
struct Candidate {
    id: i64,
    name: String,
    email: String,
    editor: bool,
}

fn candidates(c: &Connection, project: i64) -> rusqlite::Result<Vec<Candidate>> {
    c.prepare(
        "SELECT p.id, p.name, p.email, m.role != 'observer' FROM project_members m JOIN people p ON p.id = m.user_id
         WHERE m.project_id = ?1 ORDER BY p.name COLLATE NOCASE, p.id",
    )?
    .query_map([project], |r| {
        Ok(Candidate {
            id: r.get(0)?,
            name: r.get(1)?,
            email: r.get(2)?,
            editor: r.get(3)?,
        })
    })?
    .collect()
}

fn suggest(person: &file::Person, all: &[Candidate]) -> Option<i64> {
    let by_email = (!person.email.is_empty())
        .then(|| {
            all.iter()
                .find(|c| c.email.eq_ignore_ascii_case(&person.email))
        })
        .flatten();
    let by_name = || {
        (!person.name.is_empty())
            .then(|| {
                all.iter()
                    .find(|c| c.name.eq_ignore_ascii_case(&person.name))
            })
            .flatten()
    };
    by_email.or_else(by_name).map(|c| c.id)
}

fn counts_json(n: &Counts) -> Value {
    json!({
        "lists": n.lists, "cards": n.cards, "archived": n.archived, "labels": n.labels,
        "checklists": n.checklists, "items": n.items, "comments": n.comments,
        "links": n.links, "files": n.files,
    })
}

static PARSING: Semaphore = Semaphore::const_new(2);

async fn parse_bytes(bytes: Vec<u8>, file_name: &str) -> Result<BoardFile, ApiError> {
    let _turn = PARSING
        .acquire()
        .await
        .map_err(|_| bad("the file could not be read"))?;
    let name = file_name.to_string();
    tokio::task::spawn_blocking(move || file::parse(&bytes, &name))
        .await
        .map_err(|_| bad("the file could not be read"))?
        .map_err(|e| bad(&e))
}

async fn preview(
    State(state): State<AppState>,
    project: Project,
    headers: HeaderMap,
    Query(q): Query<FileName>,
    body: Body,
) -> Result<Json<Value>, ApiError> {
    project.writing(&headers, Act::CreateBoard)?;
    crate::projects::sync_person(&state, &project.company).await?;
    let file_name = q.name.unwrap_or_default();
    let bytes = crate::files::read_limited(body, MAX_REQUEST).await?;
    let parsed = parse_bytes(bytes, &file_name).await?;
    let pid = project.id;
    let all = project.db().read(move |c| Ok(candidates(c, pid)?)).await?;
    let people: Vec<Value> = parsed
        .people
        .iter()
        .map(|p| json!({ "key": p.key, "name": p.name, "email": p.email, "match": suggest(p, &all) }))
        .collect();
    Ok(Json(json!({
        "format": parsed.format,
        "name": parsed.name,
        "lists": parsed.lists.iter().filter(|l| !l.archived).map(|l| l.title.clone()).collect::<Vec<_>>(),
        "counts": counts_json(&parsed.counts()),
        "colours": parsed.source_colours,
        "people": people,
        "candidates": all.iter().map(|c| json!({ "id": c.id, "name": c.name, "email": c.email, "editor": c.editor })).collect::<Vec<_>>(),
        "project": project.info.name,
    })))
}

struct Options {
    archived: bool,
    comments: bool,
    links: bool,
}

#[derive(Deserialize)]
struct ImportBody {
    #[serde(default)]
    name: String,
    #[serde(default = "yes")]
    archived: bool,
    #[serde(default = "yes")]
    comments: bool,
    #[serde(default = "yes")]
    links: bool,
    #[serde(default)]
    people: BTreeMap<String, Option<i64>>,
    file: Box<RawValue>,
}

fn yes() -> bool {
    true
}

struct Plan {
    file: BoardFile,
    options: Options,
    people: HashMap<String, Candidate>,
}

fn plan_people(
    file: &BoardFile,
    mapping: &BTreeMap<String, Option<i64>>,
    all: &[Candidate],
) -> Result<HashMap<String, Candidate>, ApiError> {
    let by_id: HashMap<i64, &Candidate> = all.iter().map(|c| (c.id, c)).collect();
    let mut out = HashMap::new();
    for person in &file.people {
        let chosen = match mapping.get(&person.key) {
            Some(choice) => *choice,
            None => suggest(person, all),
        };
        if let Some(id) = chosen {
            let found = by_id
                .get(&id)
                .ok_or_else(|| bad("match people with members of this project"))?;
            out.insert(person.key.clone(), (*found).clone());
        }
    }
    Ok(out)
}

async fn import(
    State(state): State<AppState>,
    project: Project,
    headers: HeaderMap,
    body: Body,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = project.writing(&headers, Act::CreateBoard)?;
    crate::projects::sync_person(&state, &project.company).await?;
    let bytes = crate::files::read_limited(body, MAX_REQUEST).await?;
    let request: ImportBody =
        serde_json::from_slice(&bytes).map_err(|_| bad("send the file with the import choices"))?;
    drop(bytes);
    if request.people.len() > file::MAX_PEOPLE {
        return Err(bad("too many people choices"));
    }

    let file_name = file::line(&request.name, 200);
    let options = Options {
        archived: request.archived,
        comments: request.comments,
        links: request.links,
    };
    let mapping = request.people;
    let parsed = parse_bytes(request.file.get().as_bytes().to_vec(), &file_name).await?;
    let pid = project.id;
    let all = project.db().read(move |c| Ok(candidates(c, pid)?)).await?;
    let people = plan_people(&parsed, &mapping, &all)?;
    let background = project.info.background.clone();
    let plan = Plan {
        file: parsed,
        options,
        people,
    };
    let at = now();
    let fname = file_name.clone();
    let (board, name, done) = project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let result = write(&tx, pid, by, &background, &plan, &fname, at)?;
            tx.commit()?;
            Ok(result)
        })
        .await?;

    let (org, target) = (project.company.org_id, project.info.name.clone());
    let detail = json!({ "board": board, "project": pid, "counts": counts_json(&done) });
    let audited = state
        .dbs
        .control()
        .write(move |c| {
            audit::record(c, Some(by), "board_imported", Some(org), &target, detail)?;
            Ok(())
        })
        .await;
    if let Err(e) = audited {
        eprintln!("tazko: the audit entry for an import could not be written: {e:?}");
    }

    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": board, "name": name, "counts": counts_json(&done) })),
    ))
}

fn write(
    c: &Connection,
    project: i64,
    by: i64,
    background: &str,
    plan: &Plan,
    file_name: &str,
    at: i64,
) -> rusqlite::Result<(i64, String, Counts)> {
    let f = &plan.file;
    let o = &plan.options;
    let name = f.name.clone();
    let board = crate::boards::insert_board(c, project, &name, background, false, by)?;
    if !f.description.is_empty() {
        c.execute(
            "UPDATE boards SET description = ?2 WHERE id = ?1",
            params![board, f.description],
        )?;
    }

    let mut labels: HashMap<&str, i64> = HashMap::new();
    for (i, l) in f.labels.iter().enumerate() {
        c.execute(
            "INSERT INTO labels (board_id, name, color, position) VALUES (?1, ?2, ?3, ?4)",
            params![board, l.name, l.color, i as i64],
        )?;
        labels.insert(l.key.as_str(), c.last_insert_rowid());
    }

    let person = |key: &str, editor: bool| -> Option<i64> {
        plan.people
            .get(key)
            .filter(|p| p.editor || !editor)
            .map(|p| p.id)
    };
    let mut done = Counts {
        labels: f.labels.len(),
        ..Counts::default()
    };
    let lists: Vec<&file::List> = f
        .lists
        .iter()
        .filter(|l| o.archived || !l.archived)
        .collect();
    for (list, pos) in lists.iter().zip(order::spread(lists.len())) {
        c.execute(
            "INSERT INTO lists (board_id, title, position, archived_at) VALUES (?1, ?2, ?3, ?4)",
            params![board, list.title, pos, list.archived.then_some(at)],
        )?;
        let list_id = c.last_insert_rowid();
        if !list.archived {
            done.lists += 1;
        }

        let cards: Vec<&file::Card> = list
            .cards
            .iter()
            .filter(|k| o.archived || !k.archived)
            .collect();
        for (card, cpos) in cards.iter().zip(order::spread(cards.len())) {
            let archived = card.archived || list.archived;
            c.execute(
                "INSERT INTO cards (board_id, list_id, title, position, created_by, description, description_text,
                                    start_at, due_at, done, archived_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    board,
                    list_id,
                    card.title,
                    cpos,
                    by,
                    card.description,
                    crate::sanitize::plain_text(&card.description),
                    card.start_at,
                    card.due_at,
                    card.done,
                    card.archived.then_some(at)
                ],
            )?;
            let id = c.last_insert_rowid();
            if archived {
                done.archived += 1;
            } else {
                done.cards += 1;
            }

            for l in &card.labels {
                if let Some(label) = labels.get(l.as_str()) {
                    c.execute(
                        "INSERT INTO card_labels (card_id, label_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                        params![id, label],
                    )?;
                }
            }
            for m in &card.members {
                if let Some(user) = person(m, true) {
                    c.execute(
                        "INSERT INTO card_members (card_id, user_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                        params![id, user],
                    )?;
                }
            }
            for (k, kpos) in card
                .checklists
                .iter()
                .zip(order::spread(card.checklists.len()))
            {
                c.execute(
                    "INSERT INTO checklists (card_id, title, position) VALUES (?1, ?2, ?3)",
                    params![
                        id,
                        if k.title.is_empty() {
                            "Checklist"
                        } else {
                            &k.title
                        },
                        kpos
                    ],
                )?;
                let checklist = c.last_insert_rowid();
                done.checklists += 1;
                for (item, ipos) in k.items.iter().zip(order::spread(k.items.len())) {
                    c.execute(
                        "INSERT INTO checklist_items (checklist_id, text, done, position, assignee_id, due_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            checklist,
                            item.text,
                            item.done,
                            ipos,
                            item.assignee.as_deref().and_then(|a| person(a, true)),
                            item.due_at
                        ],
                    )?;
                    done.items += 1;
                }
            }
            if o.comments {
                for m in &card.comments {
                    let author = m.author.as_deref().and_then(|a| person(a, false));
                    let shown = if m.author_name.is_empty() {
                        m.author
                            .as_deref()
                            .and_then(|a| plan.people.get(a))
                            .map(|p| {
                                if p.name.is_empty() {
                                    p.email.clone()
                                } else {
                                    p.name.clone()
                                }
                            })
                            .unwrap_or_else(|| "Unknown".into())
                    } else {
                        m.author_name.clone()
                    };
                    c.execute(
                        "INSERT INTO comments (card_id, user_id, body, created_at, author_name) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            id,
                            author,
                            m.text,
                            m.created_at.unwrap_or(at),
                            shown
                        ],
                    )?;
                    done.comments += 1;
                }
            }
            if o.links {
                for link in &card.links {
                    c.execute(
                        "INSERT INTO attachments (card_id, kind, name, url, created_by) VALUES (?1, 'link', ?2, ?3, ?4)",
                        params![id, link.name, link.url, by],
                    )?;
                    done.links += 1;
                }
            }
            done.files += card.files;
        }
    }

    let matched: BTreeMap<&str, i64> = plan
        .people
        .iter()
        .map(|(k, p)| (k.as_str(), p.id))
        .collect();
    c.execute(
        "INSERT INTO imports (board_id, file_name, format, counts, people, created_by, finished_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            board,
            file_name,
            f.format,
            counts_json(&done).to_string(),
            json!(matched).to_string(),
            by,
            at
        ],
    )?;
    events::record(
        c,
        by,
        "board_imported",
        At::board(project, board),
        json!({ "name": name, "file": file_name, "cards": done.cards, "lists": done.lists, "archived": done.archived }),
    )?;
    Ok((board, name, done))
}
