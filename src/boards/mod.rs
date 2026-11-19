pub mod lists;

use std::collections::HashMap;

use axum::Router;
use axum::extract::{FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::{get, post, put};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::admin::validate;
use crate::api::COMPANY_PREFIX;
use crate::auth::check_csrf;
use crate::error::{ApiError, bad};
use crate::events::{self, At};
use crate::order;
use crate::projects::{Act, BACKGROUNDS, Project, path_id};
use crate::state::AppState;
use crate::tenant::Company;

pub const DEFAULT_LISTS: [&str; 3] = ["To do", "Doing", "Done"];

#[derive(Debug, Clone)]
pub struct BoardInfo {
    pub name: String,
    pub description: String,
    pub background: String,
    pub restricted: bool,
    pub closed: bool,
}

pub struct Board {
    pub project: Project,
    pub id: i64,
    pub info: BoardInfo,
}

pub const VISIBLE: &str = "(b.restricted = 0 OR EXISTS (SELECT 1 FROM board_access a WHERE a.board_id = b.id AND a.user_id = ?2))";

pub fn visible(c: &Connection, board: i64, user: i64) -> rusqlite::Result<bool> {
    c.query_row(
        &format!("SELECT EXISTS (SELECT 1 FROM boards b WHERE b.id = ?1 AND {VISIBLE})"),
        params![board, user],
        |r| r.get(0),
    )
}

impl Board {
    pub fn user_id(&self) -> i64 {
        self.project.user_id()
    }

    pub fn can(&self, act: Act) -> bool {
        match act {
            Act::Read => true,
            Act::Edit => !self.info.closed && self.project.can(Act::Edit),
            other => self.project.can(other),
        }
    }

    pub fn writing(&self, headers: &HeaderMap, act: Act) -> Result<i64, ApiError> {
        check_csrf(&self.project.company.user, headers)?;
        if self.can(act) {
            Ok(self.user_id())
        } else if act == Act::Edit && self.info.closed && self.project.can(Act::Edit) {
            Err(bad("this board is closed; reopen it first"))
        } else {
            Err(ApiError::Forbidden)
        }
    }

    pub fn at(&self) -> At {
        At::board(self.project.id, self.id)
    }

    pub async fn load(company: Company, id: i64) -> Result<Board, ApiError> {
        let user = company.user.0.user_id;
        let found = company
            .db
            .read(move |c| {
                let row = c
                    .query_row(
                        "SELECT project_id, name, description, background, restricted, closed_at IS NOT NULL
                         FROM boards WHERE id = ?1",
                        [id],
                        |r| {
                            Ok((
                                r.get::<_, i64>(0)?,
                                BoardInfo {
                                    name: r.get(1)?,
                                    description: r.get(2)?,
                                    background: r.get(3)?,
                                    restricted: r.get(4)?,
                                    closed: r.get(5)?,
                                },
                            ))
                        },
                    )
                    .optional()?;
                let Some((project, info)) = row else {
                    return Ok(None);
                };
                Ok(visible(c, id, user)?.then_some((project, info)))
            })
            .await?;
        let (project_id, info) = found.ok_or(ApiError::NotFound)?;
        let project = Project::load(company, project_id).await?;
        Ok(Board { project, id, info })
    }
}

impl FromRequestParts<AppState> for Board {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let company = Company::from_request_parts(parts, state).await?;
        let Path(params) = Path::<HashMap<String, String>>::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::NotFound)?;
        let id = path_id(&params, "board")?;
        Board::load(company, id).await
    }
}

pub fn routes() -> Router<AppState> {
    let p = |s: &str| format!("{COMPANY_PREFIX}{s}");
    Router::new()
        .route(&p("/starred"), get(starred))
        .route(
            &p("/projects/{project}/boards"),
            get(project_boards).post(create),
        )
        .route(&p("/boards/{board}"), get(view).patch(update))
        .route(&p("/boards/{board}/close"), post(close))
        .route(&p("/boards/{board}/reopen"), post(reopen))
        .route(&p("/boards/{board}/star"), put(star))
        .route(&p("/boards/{board}/access"), put(access))
        .route(&p("/boards/{board}/copy"), post(copy))
        .route(&p("/boards/{board}/archive"), get(archived))
        .route(&p("/boards/{board}/activity"), get(activity))
        .merge(lists::routes())
}

fn tile(r: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, i64>(0)?,
        "name": r.get::<_, String>(1)?,
        "background": r.get::<_, String>(2)?,
        "restricted": r.get::<_, bool>(3)?,
        "closed": r.get::<_, bool>(4)?,
        "starred": r.get::<_, bool>(5)?,
        "lists": r.get::<_, i64>(6)?,
        "cards": r.get::<_, i64>(7)?,
        "project_id": r.get::<_, i64>(8)?,
    }))
}

const TILE: &str = "SELECT b.id, b.name, b.background, b.restricted, b.closed_at IS NOT NULL,
        EXISTS (SELECT 1 FROM board_stars s WHERE s.board_id = b.id AND s.user_id = ?2),
        (SELECT count(*) FROM lists l WHERE l.board_id = b.id AND l.archived_at IS NULL),
        (SELECT count(*) FROM cards k JOIN lists l ON l.id = k.list_id
          WHERE k.board_id = b.id AND k.archived_at IS NULL AND l.archived_at IS NULL),
        b.project_id
    FROM boards b";

async fn project_boards(project: Project) -> Result<Json<Value>, ApiError> {
    let (id, user) = (project.id, project.user_id());
    let boards = project
        .db()
        .read(move |c| {
            let mut stmt = c.prepare(&format!(
                "{TILE} WHERE b.project_id = ?1 AND {VISIBLE} ORDER BY b.id"
            ))?;
            let rows = stmt
                .query_map(params![id, user], tile)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(Json(
        json!({ "boards": boards, "can": project.permissions() }),
    ))
}

async fn starred(State(state): State<AppState>, company: Company) -> Result<Json<Value>, ApiError> {
    crate::projects::sync_person(&state, &company).await?;
    let user = company.user.0.user_id;
    let boards = company
        .db
        .read(move |c| {
            let mut stmt = c.prepare(&format!(
                "{TILE} JOIN projects p ON p.id = b.project_id
                 JOIN project_members m ON m.project_id = p.id AND m.user_id = ?2
                 JOIN board_stars st ON st.board_id = b.id AND st.user_id = ?2
                 WHERE ?1 = ?1 AND p.deleted_at IS NULL AND p.archived_at IS NULL AND b.closed_at IS NULL AND {VISIBLE}
                 ORDER BY st.created_at"
            ))?;
            let rows = stmt
                .query_map(params![0, user], tile)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;
    Ok(Json(json!({ "boards": boards })))
}

#[derive(Deserialize)]
struct NewBoard {
    name: String,
    background: Option<String>,
    #[serde(default)]
    restricted: bool,
}

fn background(raw: Option<&str>, fallback: &str) -> Result<String, ApiError> {
    match raw {
        None => Ok(fallback.to_string()),
        Some(b) if BACKGROUNDS.contains(&b) => Ok(b.to_string()),
        Some(_) => Err(bad("choose one of the backgrounds")),
    }
}

pub fn insert_board(
    tx: &Connection,
    project: i64,
    name: &str,
    background: &str,
    restricted: bool,
    by: i64,
) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT INTO boards (project_id, name, background, restricted, created_by) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![project, name, background, restricted, by],
    )?;
    let id = tx.last_insert_rowid();
    if restricted {
        tx.execute(
            "INSERT INTO board_access (board_id, user_id) VALUES (?1, ?2)",
            params![id, by],
        )?;
    }

    Ok(id)
}

async fn create(
    project: Project,
    headers: HeaderMap,
    Json(body): Json<NewBoard>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = project.writing(&headers, Act::CreateBoard)?;
    let name = validate::name(&body.name).ok_or_else(|| bad("enter a board title"))?;
    let bg = background(body.background.as_deref(), &project.info.background)?;
    let pid = project.id;
    let restricted = body.restricted;
    let id = project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let id = insert_board(&tx, pid, &name, &bg, restricted, by)?;
            crate::cards::detail::insert_default_labels(&tx, id)?;
            for (title, pos) in DEFAULT_LISTS.iter().zip(order::spread(DEFAULT_LISTS.len())) {
                tx.execute(
                    "INSERT INTO lists (board_id, title, position) VALUES (?1, ?2, ?3)",
                    params![id, title, pos],
                )?;
            }
            events::record(
                &tx,
                by,
                "board_created",
                At::board(pid, id),
                json!({ "name": name, "restricted": restricted }),
            )?;
            tx.commit()?;
            Ok(id)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

async fn view(board: Board) -> Result<Json<Value>, ApiError> {
    let (id, pid, user) = (board.id, board.project.id, board.user_id());
    let restricted = board.info.restricted;
    let (lists, people, starred, archived, labels, last_event) = board
        .project
        .db()
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT id, title, position FROM lists WHERE board_id = ?1 AND archived_at IS NULL ORDER BY position",
            )?;
            let mut cards = crate::cards::summaries(c, id)?;
            let lists = stmt
                .query_map([id], |r| {
                    let list: i64 = r.get(0)?;
                    Ok(json!({
                        "id": list,
                        "title": r.get::<_, String>(1)?,
                        "position": r.get::<_, String>(2)?,
                        "cards": cards.remove(&list).unwrap_or_default(),
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut stmt = c.prepare(
                "SELECT pe.id, pe.name, pe.email, m.role,
                        EXISTS (SELECT 1 FROM board_access a WHERE a.board_id = ?2 AND a.user_id = pe.id)
                 FROM project_members m JOIN people pe ON pe.id = m.user_id
                 WHERE m.project_id = ?1
                 ORDER BY m.role = 'owner' DESC, pe.name = '', pe.name COLLATE NOCASE, pe.email",
            )?;
            let people = stmt
                .query_map(params![pid, id], |r| {
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "name": r.get::<_, String>(1)?,
                        "email": r.get::<_, String>(2)?,
                        "role": r.get::<_, String>(3)?,
                        "access": !restricted || r.get::<_, bool>(4)?,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let starred: bool = c.query_row(
                "SELECT EXISTS (SELECT 1 FROM board_stars WHERE board_id = ?1 AND user_id = ?2)",
                params![id, user],
                |r| r.get(0),
            )?;
            let watching: bool = c.query_row(
                "SELECT EXISTS (SELECT 1 FROM board_watchers WHERE board_id = ?1 AND user_id = ?2)",
                params![id, user],
                |r| r.get(0),
            )?;
            let archived: i64 = c.query_row(
                "SELECT count(*) FROM lists WHERE board_id = ?1 AND archived_at IS NOT NULL",
                [id],
                |r| r.get(0),
            )?;
            let labels = crate::cards::detail::labels_of_board(c, id)?;
            let last_event = crate::realtime::last_event(c)?;
            Ok((lists, people, (starred, watching), archived, labels, last_event))
        })
        .await?;
    let i = &board.info;
    let mut can = board.project.permissions();
    can["edit"] = json!(board.can(Act::Edit));
    Ok(Json(json!({
        "id": board.id,
        "name": i.name,
        "description": i.description,
        "background": i.background,
        "restricted": i.restricted,
        "closed": i.closed,
        "starred": starred.0,
        "watching": starred.1,
        "project": { "id": board.project.id, "name": board.project.info.name },
        "role": board.project.role.as_str(),
        "can": can,
        "people": people,
        "lists": lists,
        "labels": labels,
        "archived_lists": archived,
        "last_event": last_event,
    })))
}

#[derive(Deserialize)]
struct BoardChange {
    name: Option<String>,
    description: Option<String>,
    background: Option<String>,
}

async fn update(
    board: Board,
    headers: HeaderMap,
    Json(body): Json<BoardChange>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Edit)?;
    let mut info = board.info.clone();
    if let Some(n) = &body.name {
        info.name = validate::name(n).ok_or_else(|| bad("enter a board title"))?;
    }

    if let Some(d) = &body.description {
        if d.chars().count() > 5000 {
            return Err(bad("the description is too long"));
        }

        info.description = d.trim().to_string();
    }

    info.background = background(body.background.as_deref(), &info.background)?;
    let (id, at) = (board.id, board.at());
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            tx.execute(
                "UPDATE boards SET name = ?2, description = ?3, background = ?4 WHERE id = ?1",
                params![id, info.name, info.description, info.background],
            )?;
            events::record(
                &tx,
                by,
                "board_updated",
                at,
                json!({ "name": info.name, "background": info.background }),
            )?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_closed(board: Board, headers: HeaderMap, close: bool) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Manage)?;
    let (id, at) = (board.id, board.at());
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let when = close.then(crate::session::now);
            tx.execute(
                "UPDATE boards SET closed_at = ?2 WHERE id = ?1",
                params![id, when],
            )?;
            let kind = if close {
                "board_closed"
            } else {
                "board_reopened"
            };
            events::record(&tx, by, kind, at, json!({}))?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn close(board: Board, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    set_closed(board, headers, true).await
}

async fn reopen(board: Board, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    set_closed(board, headers, false).await
}

#[derive(Deserialize)]
struct Star {
    starred: bool,
}

async fn star(
    board: Board,
    headers: HeaderMap,
    Json(body): Json<Star>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Read)?;
    let (id, at) = (board.id, board.at());
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let changed = if body.starred {
                tx.execute(
                    "INSERT INTO board_stars (board_id, user_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                    params![id, by],
                )?
            } else {
                tx.execute(
                    "DELETE FROM board_stars WHERE board_id = ?1 AND user_id = ?2",
                    params![id, by],
                )?
            };
            if changed > 0 {
                let kind = if body.starred { "board_starred" } else { "board_unstarred" };
                events::record(&tx, by, kind, at, json!({}))?;
            }
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct Access {
    restricted: bool,
    #[serde(default)]
    people: Vec<i64>,
}

async fn access(
    board: Board,
    headers: HeaderMap,
    Json(body): Json<Access>,
) -> Result<StatusCode, ApiError> {
    let by = board.writing(&headers, Act::Manage)?;
    let (id, pid, at) = (board.id, board.project.id, board.at());
    let mut people = body.people.clone();
    if body.restricted && !people.contains(&by) {
        people.push(by);
    }

    let restricted = body.restricted;
    board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            for person in &people {
                let member: bool = tx.query_row(
                    "SELECT EXISTS (SELECT 1 FROM project_members WHERE project_id = ?1 AND user_id = ?2)",
                    params![pid, person],
                    |r| r.get(0),
                )?;
                if !member {
                    return Ok(Err(bad("choose people from this project")));
                }
            }

            tx.execute("UPDATE boards SET restricted = ?2 WHERE id = ?1", params![id, restricted])?;
            tx.execute("DELETE FROM board_access WHERE board_id = ?1", [id])?;
            if restricted {
                for person in &people {
                    tx.execute(
                        "INSERT INTO board_access (board_id, user_id) VALUES (?1, ?2)",
                        params![id, person],
                    )?;
                }
            }

            events::record(
                &tx,
                by,
                "board_access_changed",
                at,
                json!({ "restricted": restricted, "people": if restricted { people.clone() } else { Vec::new() } }),
            )?;
            tx.commit()?;
            Ok(Ok(()))
        })
        .await??;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct CopyBoard {
    name: String,
}

async fn copy(
    board: Board,
    headers: HeaderMap,
    Json(body): Json<CopyBoard>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let by = board.writing(&headers, Act::CreateBoard)?;
    let name = validate::name(&body.name).ok_or_else(|| bad("enter a board title"))?;
    let (from, pid) = (board.id, board.project.id);
    let info = board.info.clone();
    let id = board
        .project
        .db()
        .write(move |c| {
            let tx = c.transaction()?;
            let id = insert_board(&tx, pid, &name, &info.background, info.restricted, by)?;
            tx.execute("UPDATE boards SET description = ?2 WHERE id = ?1", params![id, info.description])?;
            if info.restricted {
                tx.execute(
                    "INSERT INTO board_access (board_id, user_id) SELECT ?2, user_id FROM board_access WHERE board_id = ?1
                     ON CONFLICT DO NOTHING",
                    params![from, id],
                )?;
            }

            let labels = crate::cards::copy_labels(&tx, from, id)?;
            let lists: Vec<(i64, String, String)> = {
                let mut stmt = tx.prepare(
                    "SELECT id, title, position FROM lists WHERE board_id = ?1 AND archived_at IS NULL ORDER BY position",
                )?;
                stmt.query_map([from], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                    .collect::<Result<_, _>>()?
            };
            for (list, title, pos) in lists {
                tx.execute(
                    "INSERT INTO lists (board_id, title, position) VALUES (?1, ?2, ?3)",
                    params![id, title, pos],
                )?;
                crate::cards::copy_cards(&tx, list, tx.last_insert_rowid(), Some(&labels))?;
            }
            events::record(&tx, by, "board_created", At::board(pid, id), json!({ "name": name, "copied_from": from }))?;
            tx.commit()?;
            Ok(id)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

async fn archived(board: Board) -> Result<Json<Value>, ApiError> {
    let id = board.id;
    let (lists, cards) = board
        .project
        .db()
        .read(move |c| {
            let mut stmt = c.prepare(
                "SELECT id, title, archived_at FROM lists WHERE board_id = ?1 AND archived_at IS NOT NULL ORDER BY archived_at DESC",
            )?;
            let rows = stmt
                .query_map([id], |r| {
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "title": r.get::<_, String>(1)?,
                        "archived_at": r.get::<_, i64>(2)?,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut stmt = c.prepare(
                "SELECT k.id, k.title, k.archived_at, l.title FROM cards k JOIN lists l ON l.id = k.list_id
                 WHERE k.board_id = ?1 AND k.archived_at IS NOT NULL ORDER BY k.archived_at DESC LIMIT 500",
            )?;
            let cards = stmt
                .query_map([id], |r| {
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "title": r.get::<_, String>(1)?,
                        "archived_at": r.get::<_, i64>(2)?,
                        "list": r.get::<_, String>(3)?,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok((rows, cards))
        })
        .await?;
    Ok(Json(json!({ "lists": lists, "cards": cards })))
}

#[derive(Deserialize)]
struct Page {
    before: Option<i64>,
}

async fn activity(
    board: Board,
    axum::extract::Query(page): axum::extract::Query<Page>,
) -> Result<Json<Value>, ApiError> {
    let id = board.id;
    let entries = board
        .project
        .db()
        .read(move |c| Ok(events::board_activity(c, id, page.before, 50)?))
        .await?;
    Ok(Json(json!({ "entries": entries })))
}
