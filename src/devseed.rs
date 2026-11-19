use axum::body::Body;
use axum::http::{Method, Request, header};
use http_body_util::BodyExt;
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::auth::CSRF_HEADER;
use crate::session::{self, now};
use crate::state::AppState;

pub const SLUG: &str = "tazko-demo";
const DAY: i64 = 86_400;

#[derive(Debug, PartialEq, Eq)]
pub enum Seeded {
    Created { board: i64, roadmap: i64 },
    AlreadyThere,
}

pub fn local(public_url: &str) -> bool {
    let rest = public_url
        .strip_prefix("http://")
        .or_else(|| public_url.strip_prefix("https://"))
        .unwrap_or("");
    let host = rest.split('/').next().unwrap_or("");
    let host = if host.starts_with('[') {
        host.split(']')
            .next()
            .map(|h| format!("{h}]"))
            .unwrap_or_default()
    } else {
        host.split(':').next().unwrap_or("").to_string()
    };
    host == "localhost"
        || host == "127.0.0.1"
        || host == "[::1"
        || host == "[::1]"
        || host.ends_with(".localhost")
        || host.ends_with(".test")
}

struct Client {
    state: AppState,
    cookie: String,
    csrf: String,
}

impl Client {
    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value, String> {
        let mut req = Request::builder()
            .method(method.clone())
            .uri(path)
            .header(header::COOKIE, &self.cookie)
            .header(CSRF_HEADER, &self.csrf)
            .header(header::ACCEPT, "application/json");
        let body = match body {
            Some(b) => {
                req = req.header(header::CONTENT_TYPE, "application/json");
                Body::from(b.to_string())
            }
            None => Body::empty(),
        };
        let res = crate::http::router(self.state.clone())
            .oneshot(req.body(body).map_err(|e| e.to_string())?)
            .await
            .unwrap_or_else(|never| match never {});
        let status = res.status();
        let bytes = res
            .into_body()
            .collect()
            .await
            .map_err(|e| e.to_string())?
            .to_bytes();
        let text = String::from_utf8_lossy(&bytes).to_string();
        if !status.is_success() {
            return Err(format!("{method} {path}: {status} {text}"));
        }

        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    }
}

struct Card {
    list: &'static str,
    title: &'static str,
    description: Option<&'static str>,
    labels: &'static [&'static str],
    due_in_days: Option<i64>,
    done: bool,
    mine: bool,
    checklist: Option<(&'static str, &'static [(&'static str, bool)])>,
    comments: &'static [&'static str],
}

const fn card(list: &'static str, title: &'static str) -> Card {
    Card {
        list,
        title,
        description: None,
        labels: &[],
        due_in_days: None,
        done: false,
        mine: false,
        checklist: None,
        comments: &[],
    }
}

fn sprint() -> Vec<Card> {
    vec![
        Card {
            description: Some(
                "Map every page of the current site and note which ones we keep, merge or drop.\n\n- Export the page list\n- Mark traffic per page\n- Agree the new structure with marketing",
            ),
            labels: &["Research"],
            due_in_days: Some(-2),
            mine: true,
            checklist: Some((
                "Audit",
                &[
                    ("Export the page list", true),
                    ("Add traffic numbers", true),
                    ("Mark pages to drop", false),
                    ("Review with marketing", false),
                ],
            )),
            comments: &[
                "Analytics export is in the shared drive.",
                "Let's drop the old press pages.",
            ],
            ..card("Backlog", "Audit current navigation and sitemap")
        },
        Card {
            labels: &["Content"],
            due_in_days: Some(6),
            ..card("Backlog", "Write copy for the pricing page")
        },
        Card {
            labels: &["Backend"],
            ..card("Backlog", "Set up image CDN")
        },
        Card {
            description: Some(
                "Dịch toàn bộ trang **bảng giá** sang tiếng Việt, giữ nguyên đơn vị tiền tệ.",
            ),
            labels: &["Content"],
            due_in_days: Some(10),
            ..card("Backlog", "Dịch trang bảng giá sang tiếng Việt")
        },
        Card {
            labels: &["Frontend", "Bug"],
            ..card("To do", "Accessibility pass on forms")
        },
        Card {
            description: Some(
                "New hero with one headline, one sentence and one button.\n\nSee the brief: [hero brief](https://example.com/brief).",
            ),
            labels: &["Design", "Frontend"],
            due_in_days: Some(1),
            mine: true,
            checklist: Some((
                "Hero tasks",
                &[
                    ("Draft headline", true),
                    ("Pick photo", true),
                    ("Mobile layout", false),
                    ("Contrast check", false),
                    ("Copy review", false),
                    ("Ship to staging", false),
                    ("Final sign-off", false),
                ],
            )),
            comments: &[
                "Can we keep the gradient?",
                "Yes, but lighter.",
                "Updated the mock.",
                "Looks great, **ship it**",
                "One more tweak on mobile.",
            ],
            ..card("To do", "Redesign homepage hero")
        },
        Card {
            labels: &["Frontend"],
            due_in_days: Some(3),
            mine: true,
            ..card("In progress", "Build responsive nav component")
        },
        Card {
            description: Some(
                "Move all posts to the new CMS and keep the old addresses working with redirects.",
            ),
            labels: &["Backend", "Blocked"],
            due_in_days: Some(-1),
            checklist: Some((
                "Migration",
                &[
                    ("Export posts", true),
                    ("Map categories", false),
                    ("Write redirects", false),
                ],
            )),
            ..card("In progress", "Migrate blog to the new CMS")
        },
        Card {
            labels: &["Urgent"],
            ..card("In progress", "Kiểm tra đường dẫn chân trang")
        },
        Card {
            labels: &["Frontend"],
            due_in_days: Some(2),
            comments: &["Ready for review on staging."],
            ..card("Review", "Pricing table component")
        },
        Card {
            labels: &["Content"],
            ..card("Review", "Footer links and legal pages")
        },
        Card {
            labels: &["Backend"],
            done: true,
            due_in_days: Some(-4),
            ..card("Done", "Set up staging environment")
        },
        Card {
            labels: &["Design"],
            done: true,
            ..card("Done", "Choose type scale and colour tokens")
        },
        Card {
            labels: &["Research"],
            done: true,
            ..card("Done", "Kickoff and goals")
        },
        card("Done", "Cookie banner wording"),
    ]
}

fn roadmap() -> Vec<Card> {
    vec![
        Card {
            labels: &["Research"],
            ..card("Now", "Customer interviews, round two")
        },
        Card {
            labels: &["Frontend"],
            ..card("Now", "Dark mode for the marketing site")
        },
        card("Next", "Self-serve plan changes"),
        Card {
            description: Some(
                "Mở rộng sang thị trường Việt Nam: tên miền, thanh toán và hỗ trợ khách hàng.",
            ),
            ..card("Next", "Ra mắt phiên bản tiếng Việt")
        },
        card("Later", "Partner directory"),
    ]
}

async fn lists_of(client: &Client, board: i64) -> Result<Vec<(i64, String)>, String> {
    let v = client
        .call(
            Method::GET,
            &format!("/api/v1/c/{SLUG}/boards/{board}"),
            None,
        )
        .await?;
    Ok(v["lists"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|l| {
            (
                l["id"].as_i64().unwrap_or(0),
                l["title"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect())
}

async fn shape_lists(
    client: &Client,
    board: i64,
    wanted: &[&str],
) -> Result<Vec<(i64, String)>, String> {
    let b = format!("/api/v1/c/{SLUG}/boards/{board}");
    let existing = lists_of(client, board).await?;
    for (i, (id, _)) in existing.iter().enumerate() {
        if let Some(title) = wanted.get(i) {
            client
                .call(
                    Method::PATCH,
                    &format!("{b}/lists/{id}"),
                    Some(json!({ "title": title })),
                )
                .await?;
        }
    }
    for title in wanted.iter().skip(existing.len()) {
        client
            .call(
                Method::POST,
                &format!("{b}/lists"),
                Some(json!({ "title": title })),
            )
            .await?;
    }
    lists_of(client, board).await
}

async fn fill(
    client: &Client,
    board: i64,
    lists: &[(i64, String)],
    cards: Vec<Card>,
    me: i64,
) -> Result<(), String> {
    let b = format!("/api/v1/c/{SLUG}/boards/{board}");
    let v = client.call(Method::GET, &b, None).await?;
    let labels: Vec<(i64, String)> = v["labels"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|l| {
            (
                l["id"].as_i64().unwrap_or(0),
                l["name"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    let today = now();
    for c in cards {
        let list = lists
            .iter()
            .find(|(_, t)| t == c.list)
            .map(|(id, _)| *id)
            .ok_or("missing list")?;
        let created = client
            .call(
                Method::POST,
                &format!("{b}/cards"),
                Some(json!({ "list_id": list, "title": c.title })),
            )
            .await?;
        let id = created["cards"][0]["id"].as_i64().ok_or("no card id")?;
        let path = format!("{b}/cards/{id}");
        let mut change = serde_json::Map::new();
        if let Some(d) = c.description {
            change.insert("description_markdown".into(), json!(d));
        }
        if let Some(days) = c.due_in_days {
            change.insert("due_at".into(), json!(today + days * DAY));
        }
        if c.done {
            change.insert("done".into(), json!(true));
        }
        if !change.is_empty() {
            client
                .call(Method::PATCH, &path, Some(Value::Object(change)))
                .await?;
        }
        for name in c.labels {
            if let Some((label, _)) = labels.iter().find(|(_, n)| n == name) {
                client
                    .call(
                        Method::PUT,
                        &format!("{path}/labels/{label}"),
                        Some(json!({ "on": true })),
                    )
                    .await?;
            }
        }
        if c.mine {
            client
                .call(
                    Method::PUT,
                    &format!("{path}/members/{me}"),
                    Some(json!({ "on": true })),
                )
                .await?;
        }
        if let Some((title, items)) = c.checklist {
            let k = client
                .call(
                    Method::POST,
                    &format!("{path}/checklists"),
                    Some(json!({ "title": title })),
                )
                .await?;
            let k = k["id"].as_i64().ok_or("no checklist id")?;
            for (text, done) in items {
                let it = client
                    .call(
                        Method::POST,
                        &format!("{b}/checklists/{k}/items"),
                        Some(json!({ "text": text })),
                    )
                    .await?;
                if *done {
                    let item = it["id"].as_i64().ok_or("no item id")?;
                    client
                        .call(
                            Method::PATCH,
                            &format!("{b}/items/{item}"),
                            Some(json!({ "done": true })),
                        )
                        .await?;
                }
            }
        }
        for text in c.comments {
            client
                .call(
                    Method::POST,
                    &format!("{path}/comments"),
                    Some(json!({ "body": text })),
                )
                .await?;
        }
    }
    Ok(())
}

pub async fn seed(state: &AppState, owner: Option<&str>) -> Result<Seeded, String> {
    if !local(&state.settings.public_url) {
        return Err("tazko dev seed only runs against a local address (localhost, 127.0.0.1, *.localhost or *.test)".into());
    }

    let owner = owner.map(|o| o.trim().to_lowercase());
    let (exists, found) = state
        .dbs
        .control()
        .read(move |c| {
            let exists: bool = c.query_row("SELECT EXISTS (SELECT 1 FROM orgs WHERE slug = ?1)", [SLUG], |r| r.get(0))?;
            let found: Option<(i64, bool)> = match &owner {
                Some(email) => c
                    .query_row("SELECT id, is_super_admin FROM users WHERE email = ?1 AND disabled = 0", [email], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()?,
                None => c
                    .query_row("SELECT id, 1 FROM users WHERE is_super_admin = 1 AND disabled = 0 ORDER BY id LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()?,
            };
            Ok((exists, found))
        })
        .await
        .map_err(|e| e.to_string())?;
    if exists {
        return Ok(Seeded::AlreadyThere);
    }

    let (me, is_admin) =
        found.ok_or("no such person; create the super admin first with `tazko admin --email`")?;
    if !is_admin {
        return Err("the owner must be a super admin (`tazko admin --email`)".into());
    }

    let (token, csrf) = session::create(state.dbs.control(), me, Some("tazko dev seed".into()))
        .await
        .map_err(|e| e.to_string())?;
    let client = Client {
        state: state.clone(),
        cookie: format!("{}={token}", session::COOKIE),
        csrf,
    };
    let result = run(&client, me).await;
    let hash = session::hash_token(&token);
    let _ = state
        .dbs
        .control()
        .write(move |c| {
            c.execute("DELETE FROM sessions WHERE id_hash = ?1", [hash])?;
            Ok(())
        })
        .await;
    result
}

async fn run(client: &Client, me: i64) -> Result<Seeded, String> {
    let email = client.call(Method::GET, "/api/v1/me", None).await?["email"]
        .as_str()
        .unwrap_or("")
        .to_string();
    client
        .call(
            Method::POST,
            "/api/v1/admin/companies",
            Some(json!({ "name": "Tazko Demo", "slug": SLUG, "owner_email": email, "storage_gb": 5 })),
        )
        .await?;
    client.call(Method::GET, "/api/v1/me", None).await?;
    let c = format!("/api/v1/c/{SLUG}");
    let project = client
        .call(
            Method::POST,
            &format!("{c}/projects"),
            Some(json!({ "name": "Website relaunch", "owner_id": me })),
        )
        .await?["id"]
        .as_i64()
        .ok_or("no project id")?;
    client
        .call(
            Method::PATCH,
            &format!("{c}/projects/{project}"),
            Some(json!({ "description": "New marketing site and docs, launching next quarter", "color": "c0" })),
        )
        .await?;

    let board = client
        .call(
            Method::POST,
            &format!("{c}/projects/{project}/boards"),
            Some(json!({ "name": "Sprint 12", "background": "dusk" })),
        )
        .await?["id"]
        .as_i64()
        .ok_or("no board id")?;
    let lists = shape_lists(
        client,
        board,
        &["Backlog", "To do", "In progress", "Review", "Done"],
    )
    .await?;
    fill(client, board, &lists, sprint(), me).await?;
    client
        .call(
            Method::PUT,
            &format!("{c}/boards/{board}/star"),
            Some(json!({ "starred": true })),
        )
        .await?;
    client
        .call(
            Method::PATCH,
            &format!("{c}/boards/{board}"),
            Some(json!({ "description": "Two-week sprint for the website relaunch." })),
        )
        .await?;

    let roadmap = client
        .call(
            Method::POST,
            &format!("{c}/projects/{project}/boards"),
            Some(json!({ "name": "Roadmap", "background": "sea" })),
        )
        .await?["id"]
        .as_i64()
        .ok_or("no board id")?;
    let lists = shape_lists(client, roadmap, &["Now", "Next", "Later"]).await?;
    fill(client, roadmap, &lists, roadmap_cards(), me).await?;
    Ok(Seeded::Created { board, roadmap })
}

fn roadmap_cards() -> Vec<Card> {
    roadmap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_local_addresses_are_seeded() {
        for url in [
            "http://localhost:8791",
            "http://127.0.0.1:8791",
            "http://tazko.localhost",
            "https://tazko.test/",
            "http://[::1]:8791",
        ] {
            assert!(local(url), "{url}");
        }
        for url in [
            "https://tazko-dev.example.com",
            "http://localhost.example.com",
            "http://10.0.0.2:8791",
        ] {
            assert!(!local(url), "{url}");
        }
    }
}
