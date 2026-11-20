pub mod args;
pub mod jq;
pub mod store;

use std::io::IsTerminal;
use std::process::ExitCode;
use std::time::Duration;

use serde_json::{Value, json};

use args::{
    AuthAction, BoardAction, CardAction, ClientCommand, ConfigAction, MyAction, Out, ProjectAction,
};

const NOT_SIGNED_IN: u8 = 4;

pub enum Fail {
    Message(String),
    SignIn(String),
}

type Res<T> = Result<T, Fail>;

impl From<String> for Fail {
    fn from(s: String) -> Self {
        Fail::Message(s)
    }
}

impl From<&str> for Fail {
    fn from(s: &str) -> Self {
        Fail::Message(s.to_string())
    }
}

pub fn run(cmd: ClientCommand) -> ExitCode {
    match dispatch(cmd) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Fail::Message(m)) => {
            eprintln!("tazko: {m}");
            ExitCode::FAILURE
        }
        Err(Fail::SignIn(m)) => {
            eprintln!("tazko: {m}");
            ExitCode::from(NOT_SIGNED_IN)
        }
    }
}

pub fn normalise_host(raw: &str) -> Result<String, String> {
    let raw = raw.trim().trim_end_matches('/');
    let with_scheme = if raw.starts_with("https://") || raw.starts_with("http://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    };
    let local = ["http://localhost", "http://127.0.0.1", "http://[::1]"]
        .iter()
        .any(|p| with_scheme == *p || with_scheme.starts_with(&format!("{p}:")));
    if with_scheme.starts_with("http://") && !local {
        return Err(
            "the server address must use https:// (plain http is only for localhost)".into(),
        );
    }

    if with_scheme.len() <= "https://".len() || with_scheme.contains(['@', ' ', '?', '#']) {
        return Err(format!("{raw:?} is not a server address"));
    }

    Ok(with_scheme)
}

struct Api {
    host: String,
    token: Option<String>,
    agent: ureq::Agent,
}

fn agent() -> ureq::Agent {
    let _ = rustls::crypto::ring::default_provider().install_default();
    ureq::Agent::config_builder()
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .max_redirects(0)
        .user_agent(concat!("tazko-cli/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent()
}

impl Api {
    fn new(host_flag: Option<&str>, need_token: bool) -> Res<Api> {
        let cfg = store::load();
        let host = host_flag
            .map(str::to_string)
            .or_else(|| std::env::var("TAZKO_HOST").ok())
            .or(cfg.host)
            .ok_or_else(|| {
                Fail::SignIn("not signed in; run tazko auth login --host <server>".into())
            })?;
        let host = normalise_host(&host)?;
        let token = std::env::var("TAZKO_TOKEN")
            .ok()
            .filter(|t| !t.is_empty())
            .or_else(store::read_token);
        if need_token && token.is_none() {
            return Err(Fail::SignIn("not signed in; run tazko auth login".into()));
        }

        Ok(Api {
            host,
            token,
            agent: agent(),
        })
    }

    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Res<(u16, Value)> {
        let url = format!("{}{}", self.host, path);
        let mut req = ureq::http::Request::builder()
            .method(method)
            .uri(&url)
            .header("Accept", "application/json");
        if let Some(t) = &self.token {
            req = req.header("Authorization", format!("Bearer {t}"));
        }

        let result = match body {
            Some(b) => self.agent.run(
                req.header("Content-Type", "application/json")
                    .body(b.to_string())
                    .map_err(|e| e.to_string())?,
            ),
            None => self.agent.run(req.body(()).map_err(|e| e.to_string())?),
        };
        let mut res = result.map_err(|e| format!("cannot reach {}: {e}", self.host))?;
        let status = res.status().as_u16();
        let text = res.body_mut().read_to_string().unwrap_or_default();
        let value = serde_json::from_str(&text).unwrap_or(if text.is_empty() {
            Value::Null
        } else {
            Value::String(text)
        });
        Ok((status, value))
    }

    fn send(&self, method: &str, path: &str, body: Option<&Value>) -> Res<Value> {
        let (status, value) = self.call(method, path, body)?;
        match status {
            200..=299 => Ok(value),
            401 => Err(Fail::SignIn(
                "not signed in, or the token has expired; run tazko auth login".into(),
            )),
            _ => {
                let msg = value
                    .get("error")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("the server answered {status}"));
                Err(Fail::Message(msg))
            }
        }
    }

    fn get(&self, path: &str) -> Res<Value> {
        self.send("GET", path, None)
    }

    fn company(&self, flag: Option<&str>) -> Res<String> {
        if let Some(c) = flag.map(str::to_string).or(store::load().company) {
            return Ok(c);
        }

        let me = self.get("/api/v1/me")?;
        let slugs: Vec<String> = me["companies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["slug"].as_str().map(str::to_string))
            .collect();
        match slugs.as_slice() {
            [one] => Ok(one.clone()),
            [] => Err("you are not in any company on this server".into()),
            many => Err(format!("you are in several companies ({}); add --company or run tazko config set company <slug>", many.join(", ")).into()),
        }
    }
}

fn base(api: &Api, out: &Out) -> Res<String> {
    Ok(format!(
        "/api/v1/c/{}",
        api.company(out.company.as_deref())?
    ))
}

fn print(
    out: &Out,
    value: &Value,
    table: impl FnOnce(&Value) -> Vec<Vec<String>>,
    headers: &[&str],
) -> Res<()> {
    if let Some(filter) = &out.jq {
        for v in jq::apply(filter, value)? {
            println!("{}", jq::show(&v));
        }
        return Ok(());
    }

    if let Some(fields) = &out.json {
        let wanted: Vec<&str> = fields
            .split(',')
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .collect();
        let shown = if wanted.is_empty() {
            value.clone()
        } else {
            pick(value, &wanted)
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&shown).unwrap_or_default()
        );
        return Ok(());
    }

    let rows = table(value);
    if rows.is_empty() {
        println!("Nothing to show.");
        return Ok(());
    }

    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count().min(60));
        }
    }
    let line = |cells: Vec<String>| {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let c: String = c.chars().take(60).collect();
                format!("{c:<w$}", w = widths[i])
            })
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    if std::io::stdout().is_terminal() {
        println!(
            "{}",
            line(headers.iter().map(|h| h.to_uppercase()).collect())
        );
    }

    for row in rows {
        println!("{}", line(row));
    }
    Ok(())
}

fn pick(value: &Value, fields: &[&str]) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(|v| pick(v, fields)).collect()),
        Value::Object(map) => {
            let list = ["cards", "boards", "projects", "members", "lists"]
                .iter()
                .find_map(|k| map.get(*k).filter(|v| v.is_array()));
            match list {
                Some(inner) if !fields.iter().any(|f| map.contains_key(*f)) => pick(inner, fields),
                _ => Value::Object(
                    fields
                        .iter()
                        .filter_map(|f| map.get(*f).map(|v| (f.to_string(), v.clone())))
                        .collect(),
                ),
            }
        }
        other => other.clone(),
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn date(v: &Value) -> String {
    v.as_i64()
        .map(crate::mail::message::rfc2822_date)
        .map(|d| d[5..16].to_string())
        .unwrap_or_default()
}

fn rows(list: &Value, f: impl Fn(&Value) -> Vec<String>) -> Vec<Vec<String>> {
    list.as_array().into_iter().flatten().map(f).collect()
}

fn card_ref(raw: &str) -> Res<(i64, i64)> {
    let (b, c) = raw
        .split_once(['/', ':'])
        .ok_or("give the card as BOARD/CARD, for example 3/142")?;
    let n = |s: &str| {
        s.trim()
            .trim_start_matches(['b', 'c', '#'])
            .parse::<i64>()
            .map_err(|_| Fail::from("give the card as BOARD/CARD, for example 3/142"))
    };
    Ok((n(b)?, n(c)?))
}

fn boards(api: &Api, base: &str, project: Option<i64>) -> Res<Vec<Value>> {
    let projects = api.get(&format!("{base}/my-projects"))?;
    let mut out = vec![];
    for p in projects["projects"].as_array().into_iter().flatten() {
        let id = p["id"].as_i64().unwrap_or(0);
        if project.is_some_and(|want| want != id) {
            continue;
        }

        for b in api.get(&format!("{base}/projects/{id}/boards"))?["boards"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let mut b = b.clone();
            b["project"] = json!({ "id": id, "name": p["name"] });
            out.push(b);
        }
    }
    Ok(out)
}

fn board_id(api: &Api, base: &str, raw: &str) -> Res<i64> {
    if let Ok(id) = raw.trim_start_matches('b').parse::<i64>() {
        return Ok(id);
    }

    let found: Vec<i64> = boards(api, base, None)?
        .iter()
        .filter(|b| {
            b["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(raw))
        })
        .filter_map(|b| b["id"].as_i64())
        .collect();
    match found.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!("no board called {raw:?}").into()),
        _ => Err(format!("several boards are called {raw:?}; use its id").into()),
    }
}

fn list_id(board: &Value, raw: Option<&str>) -> Res<i64> {
    let lists = board["lists"].as_array().cloned().unwrap_or_default();
    let found = match raw {
        None => lists.first(),
        Some(r) => lists.iter().find(|l| {
            l["id"].as_i64().is_some_and(|id| id.to_string() == r)
                || l["title"]
                    .as_str()
                    .is_some_and(|t| t.eq_ignore_ascii_case(r))
        }),
    };
    found.and_then(|l| l["id"].as_i64()).ok_or_else(|| {
        Fail::Message(format!(
            "no list {:?} on this board",
            raw.unwrap_or("(first)")
        ))
    })
}

fn open_in_browser(url: &str) -> bool {
    let cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).status()
    } else if cfg!(windows) {
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .status()
    } else {
        std::process::Command::new("xdg-open").arg(url).status()
    };
    cmd.is_ok_and(|s| s.success())
}

fn host_name() -> String {
    let raw = std::env::var("HOSTNAME")
        .ok()
        .or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_default();
    let os = std::env::consts::OS;
    if raw.is_empty() {
        os.to_string()
    } else {
        format!("{raw} ({os})")
    }
}

fn login(host: Option<String>, no_browser: bool) -> Res<()> {
    let api = Api::new(host.as_deref(), false)?;
    let start = api.send(
        "POST",
        "/api/v1/cli/device",
        Some(&json!({ "client": host_name() })),
    )?;
    let code = start["user_code"]
        .as_str()
        .ok_or("the server did not send a code")?;
    let device = start["device_code"]
        .as_str()
        .ok_or("the server did not send a code")?
        .to_string();
    let url = start["verification_uri_complete"]
        .as_str()
        .unwrap_or_default();
    let interval = start["interval"].as_u64().unwrap_or(5).max(1);
    let expires = start["expires_in"].as_u64().unwrap_or(600);
    eprintln!("! First copy your one-time code: {code}");
    if no_browser || !open_in_browser(url) {
        eprintln!("Open {url} in your browser and approve the sign-in.");
    } else {
        eprintln!("Opening {url} in your browser...");
    }

    let mut wait = interval;
    let mut waited = 0;
    loop {
        std::thread::sleep(Duration::from_secs(wait));
        waited += wait;
        let (status, body) = api.call(
            "POST",
            "/api/v1/cli/token",
            Some(&json!({ "device_code": device })),
        )?;
        match (status, body["error"].as_str()) {
            (200, _) => {
                let token = body["token"]
                    .as_str()
                    .ok_or("the server did not send a token")?;
                let path = store::write_token(token)?;
                let mut cfg = store::load();
                cfg.host = Some(api.host.clone());
                store::save(&cfg)?;
                eprintln!("✓ Authentication complete.");
                eprintln!(
                    "✓ Logged in as {} on {} (token stored in {}, readable only by you)",
                    text(&body["email"]),
                    api.host,
                    path.display()
                );
                return Ok(());
            }
            (_, Some("authorization_pending")) => {}
            (_, Some("slow_down")) => wait += 5,
            (_, Some("access_denied")) => {
                return Err("the sign-in was cancelled in the browser".into());
            }
            (_, Some("expired_token")) => {
                return Err("the code expired; run tazko auth login again".into());
            }
            (s, _) => return Err(format!("the server answered {s}").into()),
        }
        if waited > expires {
            return Err("the code expired; run tazko auth login again".into());
        }
    }
}

fn dispatch(cmd: ClientCommand) -> Res<()> {
    match cmd {
        ClientCommand::Auth { action } => match action {
            AuthAction::Login { host, no_browser } => login(host, no_browser),
            AuthAction::Logout => {
                if store::forget_token() {
                    eprintln!(
                        "Signed out on this computer. Revoke the token under Settings, Access tokens if you no longer need it."
                    );
                } else {
                    eprintln!("No token was stored on this computer.");
                }
                Ok(())
            }
            AuthAction::Status { out } => {
                let api = Api::new(out.host.as_deref(), true)?;
                let me = api.get("/api/v1/me")?;
                let source = if std::env::var("TAZKO_TOKEN").is_ok_and(|t| !t.is_empty()) {
                    "TAZKO_TOKEN"
                } else {
                    "the token file"
                };
                let summary = json!({ "host": api.host, "email": me["email"], "name": me["name"], "companies": me["companies"], "token_from": source });
                print(
                    &out,
                    &summary,
                    |v| {
                        vec![
                            vec!["Server".into(), text(&v["host"])],
                            vec!["Signed in as".into(), text(&v["email"])],
                            vec![
                                "Companies".into(),
                                rows(&v["companies"], |c| vec![text(&c["slug"])])
                                    .concat()
                                    .join(", "),
                            ],
                            vec!["Token from".into(), text(&v["token_from"])],
                        ]
                    },
                    &["", ""],
                )
            }
            AuthAction::Token => {
                let token = std::env::var("TAZKO_TOKEN")
                    .ok()
                    .filter(|t| !t.is_empty())
                    .or_else(store::read_token)
                    .ok_or_else(|| Fail::SignIn("not signed in; run tazko auth login".into()))?;
                println!("{token}");
                Ok(())
            }
        },
        ClientCommand::Project { action } => match action {
            ProjectAction::List { out } => {
                let api = Api::new(out.host.as_deref(), true)?;
                let v = api.get(&format!("{}/my-projects", base(&api, &out)?))?;
                print(
                    &out,
                    &v,
                    |v| {
                        rows(&v["projects"], |p| {
                            vec![
                                text(&p["id"]),
                                text(&p["name"]),
                                text(&p["role"]),
                                text(&p["boards"]),
                            ]
                        })
                    },
                    &["id", "name", "role", "boards"],
                )
            }
            ProjectAction::Members { project, out } => {
                let api = Api::new(out.host.as_deref(), true)?;
                let v = api.get(&format!("{}/projects/{project}/members", base(&api, &out)?))?;
                print(
                    &out,
                    &v,
                    |v| {
                        rows(&v["members"], |m| {
                            vec![text(&m["name"]), text(&m["email"]), text(&m["role"])]
                        })
                    },
                    &["name", "email", "role"],
                )
            }
        },
        ClientCommand::Board { action } => match action {
            BoardAction::List { project, out } => {
                let api = Api::new(out.host.as_deref(), true)?;
                let list = Value::Array(boards(&api, &base(&api, &out)?, project)?);
                print(
                    &out,
                    &list,
                    |v| {
                        rows(v, |b| {
                            vec![
                                text(&b["id"]),
                                text(&b["name"]),
                                text(&b["project"]["name"]),
                            ]
                        })
                    },
                    &["id", "name", "project"],
                )
            }
            BoardAction::View { board, web, out } => {
                let api = Api::new(out.host.as_deref(), true)?;
                let base = base(&api, &out)?;
                let id = board_id(&api, &base, &board)?;
                if web {
                    let url = format!(
                        "{}/c/{}/b/{id}",
                        api.host,
                        api.company(out.company.as_deref())?
                    );
                    if !open_in_browser(&url) {
                        println!("{url}");
                    }
                    return Ok(());
                }

                let v = api.get(&format!("{base}/boards/{id}"))?;
                print(
                    &out,
                    &v,
                    |v| {
                        v["lists"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .flat_map(|l| {
                                rows(&l["cards"], |c| {
                                    vec![
                                        format!("{id}/{}", text(&c["id"])),
                                        text(&l["title"]),
                                        text(&c["title"]),
                                        date(&c["due_at"]),
                                    ]
                                })
                            })
                            .collect()
                    },
                    &["card", "list", "title", "due"],
                )
            }
        },
        ClientCommand::Card { action } => card(action),
        ClientCommand::My {
            action: MyAction::Cards { out },
        } => {
            let api = Api::new(out.host.as_deref(), true)?;
            let company = api.company(out.company.as_deref())?;
            let v = api.get(&format!("/api/v1/me/cards?company={company}"))?;
            print(
                &out,
                &v,
                |v| {
                    rows(&v["cards"], |c| {
                        vec![
                            format!("{}/{}", text(&c["board"]["id"]), text(&c["id"])),
                            text(&c["title"]),
                            text(&c["board"]["name"]),
                            date(&c["due_at"]),
                            if c["done"] == true {
                                "done".into()
                            } else {
                                String::new()
                            },
                        ]
                    })
                },
                &["card", "title", "board", "due", ""],
            )
        }
        ClientCommand::Search { query, out } => {
            let api = Api::new(out.host.as_deref(), true)?;
            let q: String = form_urlencoded::byte_serialize(query.as_bytes()).collect();
            let v = api.get(&format!("{}/search?q={q}", base(&api, &out)?))?;
            print(
                &out,
                &v,
                |v| {
                    rows(&v["cards"], |c| {
                        vec![
                            format!("{}/{}", text(&c["board"]["id"]), text(&c["id"])),
                            text(&c["title"]),
                            text(&c["board"]["name"]),
                            text(&c["list"]),
                        ]
                    })
                },
                &["card", "title", "board", "list"],
            )
        }
        ClientCommand::Api {
            method,
            path,
            data,
            out,
        } => {
            let api = Api::new(out.host.as_deref(), true)?;
            let method = method.to_ascii_uppercase();
            if !["GET", "POST", "PUT", "PATCH", "DELETE"].contains(&method.as_str()) {
                return Err(format!("{method} is not an HTTP method").into());
            }

            let path = if path.starts_with("/api/") {
                path
            } else {
                format!(
                    "/api/v1{}",
                    if path.starts_with('/') {
                        path
                    } else {
                        format!("/{path}")
                    }
                )
            };
            let body = data
                .map(|d| {
                    serde_json::from_str::<Value>(&d)
                        .map_err(|e| format!("--data is not JSON: {e}"))
                })
                .transpose()?;
            let v = api.send(&method, &path, body.as_ref())?;
            let out = Out {
                json: out.json.or(Some(String::new())),
                ..out
            };
            print(&out, &v, |_| vec![], &[])
        }
        ClientCommand::Config { action } => {
            let mut cfg = store::load();
            match action {
                ConfigAction::Get => {
                    println!("host     {}", cfg.host.unwrap_or_default());
                    println!("company  {}", cfg.company.unwrap_or_default());
                }
                ConfigAction::Set { key, value } => {
                    match key.as_str() {
                        "host" => cfg.host = Some(normalise_host(&value)?),
                        "company" if crate::db::valid_slug(&value) => cfg.company = Some(value),
                        "company" => return Err(format!("{value:?} is not a company slug").into()),
                        _ => {
                            return Err(
                                format!("{key} is not a setting; use host or company").into()
                            );
                        }
                    }
                    store::save(&cfg)?;
                }
            }
            Ok(())
        }
    }
}

fn card(action: CardAction) -> Res<()> {
    match action {
        CardAction::List { board, list, out } => {
            let api = Api::new(out.host.as_deref(), true)?;
            let base = base(&api, &out)?;
            let id = board_id(&api, &base, &board)?;
            let v = api.get(&format!("{base}/boards/{id}"))?;
            let only = list.as_deref().map(|l| list_id(&v, Some(l))).transpose()?;
            let cards: Vec<Value> = v["lists"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|l| only.is_none_or(|o| l["id"].as_i64() == Some(o)))
                .flat_map(|l| {
                    l["cards"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(move |mut c| {
                            c["list"] = json!({ "id": l["id"], "title": l["title"] });
                            c
                        })
                })
                .collect();
            let cards = Value::Array(cards);
            print(
                &out,
                &cards,
                |v| {
                    rows(v, |c| {
                        vec![
                            format!("{id}/{}", text(&c["id"])),
                            text(&c["list"]["title"]),
                            text(&c["title"]),
                            date(&c["due_at"]),
                        ]
                    })
                },
                &["card", "list", "title", "due"],
            )
        }
        CardAction::View { card, web, out } => {
            let api = Api::new(out.host.as_deref(), true)?;
            let (b, c) = card_ref(&card)?;
            if web {
                let url = format!(
                    "{}/c/{}/b/{b}/c/{c}",
                    api.host,
                    api.company(out.company.as_deref())?
                );
                if !open_in_browser(&url) {
                    println!("{url}");
                }
                return Ok(());
            }

            let v = api.get(&format!("{}/boards/{b}/cards/{c}", base(&api, &out)?))?;
            let card = if v["card"].is_object() {
                v["card"].clone()
            } else {
                v.clone()
            };
            print(
                &out,
                &v,
                |_| {
                    vec![
                        vec!["Card".into(), format!("{b}/{c}")],
                        vec!["Title".into(), text(&card["title"])],
                        vec!["List".into(), text(&card["list"]["title"])],
                        vec!["Due".into(), date(&card["due_at"])],
                        vec!["Done".into(), text(&card["done"])],
                        vec![
                            "Description".into(),
                            text(&card["description"])
                                .lines()
                                .next()
                                .unwrap_or("")
                                .to_string(),
                        ],
                    ]
                },
                &["", ""],
            )
        }
        CardAction::Create {
            board,
            list,
            title,
            out,
        } => {
            let api = Api::new(out.host.as_deref(), true)?;
            let base = base(&api, &out)?;
            let id = board_id(&api, &base, &board)?;
            let b = api.get(&format!("{base}/boards/{id}"))?;
            let list = list_id(&b, list.as_deref())?;
            let v = api.send(
                "POST",
                &format!("{base}/boards/{id}/cards"),
                Some(&json!({ "list_id": list, "title": title })),
            )?;
            print(
                &out,
                &v,
                |v| {
                    rows(&v["cards"], |c| {
                        vec![format!("{id}/{}", text(&c["id"])), text(&c["title"])]
                    })
                },
                &["card", "title"],
            )
        }
        CardAction::Move { card, to, out } => {
            let api = Api::new(out.host.as_deref(), true)?;
            let base = base(&api, &out)?;
            let (b, c) = card_ref(&card)?;
            let board = api.get(&format!("{base}/boards/{b}"))?;
            let list = list_id(&board, Some(&to))?;
            let v = api.send(
                "POST",
                &format!("{base}/boards/{b}/cards/{c}/move"),
                Some(&json!({ "list_id": list })),
            )?;
            done_line(&out, &v, &format!("Moved {b}/{c} to {to}"))
        }
        CardAction::Done { card, undo, out } => {
            let api = Api::new(out.host.as_deref(), true)?;
            let (b, c) = card_ref(&card)?;
            let body = json!({ "done": !undo });
            let v = api.send(
                "PATCH",
                &format!("{}/boards/{b}/cards/{c}", base(&api, &out)?),
                Some(&body),
            )?;
            done_line(
                &out,
                &v,
                &format!("{b}/{c} marked {}", if undo { "not done" } else { "done" }),
            )
        }
        CardAction::Comment { card, body, out } => {
            let api = Api::new(out.host.as_deref(), true)?;
            let (b, c) = card_ref(&card)?;
            let v = api.send(
                "POST",
                &format!("{}/boards/{b}/cards/{c}/comments", base(&api, &out)?),
                Some(&json!({ "body": body })),
            )?;
            done_line(&out, &v, &format!("Commented on {b}/{c}"))
        }
        CardAction::Archive { card, out } => {
            let api = Api::new(out.host.as_deref(), true)?;
            let (b, c) = card_ref(&card)?;
            let v = api.send(
                "POST",
                &format!("{}/boards/{b}/cards/{c}/archive", base(&api, &out)?),
                Some(&json!({})),
            )?;
            done_line(&out, &v, &format!("Archived {b}/{c}"))
        }
    }
}

fn done_line(out: &Out, v: &Value, message: &str) -> Res<()> {
    if out.json.is_some() || out.jq.is_some() {
        return print(out, v, |_| vec![], &[]);
    }

    println!("{message}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_need_https_except_on_this_machine() {
        assert_eq!(
            normalise_host("tazko.example.com/").unwrap(),
            "https://tazko.example.com"
        );
        assert_eq!(
            normalise_host("http://localhost:8791").unwrap(),
            "http://localhost:8791"
        );
        assert!(normalise_host("http://tazko.example.com").is_err());
        assert!(normalise_host("https://user@evil.example").is_err());
        assert!(normalise_host("http://localhost.evil.example").is_err());
    }

    #[test]
    fn card_references_and_field_picking() {
        assert_eq!(card_ref("3/142").ok(), Some((3, 142)));
        assert_eq!(card_ref("b3:c142").ok(), Some((3, 142)));
        assert!(card_ref("142").is_err());
        let v = json!({ "cards": [{ "id": 1, "title": "A", "due_at": null }] });
        assert_eq!(
            pick(&v, &["id", "title"]),
            json!([{ "id": 1, "title": "A" }])
        );
    }
}
