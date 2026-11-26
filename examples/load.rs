use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

struct Opts {
    url: String,
    company: String,
    session: String,
    csrf: String,
    users: usize,
    seconds: u64,
    cards: usize,
}

fn opts() -> Opts {
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let known = ["url", "company", "users", "seconds", "cards"];
    assert!(
        args.len().is_multiple_of(2),
        "options come in pairs, like --users 20"
    );
    for pair in args.chunks(2) {
        let key = pair[0].strip_prefix("--").unwrap_or("");
        assert!(
            known.contains(&key),
            "unknown option {}; use --url, --company, --users, --seconds or --cards",
            pair[0]
        );
        map.insert(key.to_string(), pair[1].clone());
    }
    let get = |k: &str, d: &str| map.get(k).cloned().unwrap_or_else(|| d.to_string());
    Opts {
        url: get("url", "http://127.0.0.1:8796"),
        company: get("company", "tazko-demo"),
        session: std::env::var("LOAD_SESSION").expect("LOAD_SESSION"),
        csrf: std::env::var("LOAD_CSRF").expect("LOAD_CSRF"),
        users: get("users", "20").parse().expect("users"),
        seconds: get("seconds", "30").parse().expect("seconds"),
        cards: get("cards", "0").parse().expect("cards"),
    }
}

struct Client {
    agent: ureq::Agent,
    base: String,
    cookie: String,
    csrf: String,
}

impl Client {
    fn new(o: &Opts) -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .new_agent();
        Client {
            agent,
            base: format!("{}/api/v1/c/{}", o.url, o.company),
            cookie: format!("tazko_session={}", o.session),
            csrf: o.csrf.clone(),
        }
    }

    fn get(&self, path: &str) -> (u16, Value) {
        let res = self
            .agent
            .get(&format!("{}{path}", self.base))
            .header("cookie", &self.cookie)
            .call();
        read(res)
    }

    fn post(&self, path: &str, body: Value) -> (u16, Value) {
        let res = self
            .agent
            .post(&format!("{}{path}", self.base))
            .header("cookie", &self.cookie)
            .header("x-csrf-token", &self.csrf)
            .send_json(body);
        read(res)
    }
}

fn read(res: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> (u16, Value) {
    match res {
        Ok(mut r) => {
            let code = r.status().as_u16();
            let body = r.body_mut().read_json::<Value>().unwrap_or(Value::Null);
            (code, body)
        }
        Err(_) => (0, Value::Null),
    }
}

struct Target {
    board: i64,
    lists: Vec<i64>,
    cards: Vec<i64>,
}

fn discover(c: &Client) -> Vec<Target> {
    let (code, projects) = c.get("/my-projects");
    assert!(
        (200..300).contains(&code),
        "listing projects answered {code}: {projects}; check the session"
    );
    let mut out = vec![];
    for p in projects["projects"].as_array().into_iter().flatten() {
        let id = p["id"].as_i64().unwrap_or_default();
        let (_, boards) = c.get(&format!("/projects/{id}/boards"));
        for b in boards["boards"].as_array().into_iter().flatten() {
            let board = b["id"].as_i64().unwrap_or_default();
            let (_, view) = c.get(&format!("/boards/{board}"));
            let mut lists = vec![];
            let mut cards = vec![];
            for l in view["lists"].as_array().into_iter().flatten() {
                lists.push(l["id"].as_i64().unwrap_or_default());
                for card in l["cards"].as_array().into_iter().flatten() {
                    cards.push(card["id"].as_i64().unwrap_or_default());
                }
            }
            if !cards.is_empty() {
                out.push(Target {
                    board,
                    lists,
                    cards,
                });
            }
        }
    }
    out
}

const OPS: [(&str, u32); 5] = [
    ("board", 55),
    ("card", 20),
    ("search", 10),
    ("comment", 10),
    ("move", 5),
];

fn pick(n: u64) -> &'static str {
    let mut r = (n % 100) as u32;
    for (op, w) in OPS {
        if r < w {
            return op;
        }
        r -= w;
    }
    "board"
}

fn grow(c: &Client, targets: &[Target], cards: usize) {
    for n in 0..cards {
        let t = &targets[n % targets.len()];
        let list = t.lists[n % t.lists.len()];
        let (code, _) = c.post(
            &format!("/boards/{}/cards", t.board),
            json!({ "list_id": list, "title": format!("Load test card {n} design review") }),
        );
        assert!(
            (200..300).contains(&code),
            "creating a card answered {code}"
        );
    }
}

fn main() {
    let o = opts();
    let probe = Client::new(&o);
    let first = discover(&probe);
    assert!(
        !first.is_empty(),
        "no boards with cards found; seed the data first"
    );
    grow(&probe, &first, o.cards);
    let targets = Arc::new(discover(&probe));
    let total_cards: usize = targets.iter().map(|t| t.cards.len()).sum();
    eprintln!(
        "load: {} users for {} s on {} boards, {} cards",
        o.users,
        o.seconds,
        targets.len(),
        total_cards
    );

    let seq = Arc::new(AtomicU64::new(0));
    let deadline = Instant::now() + Duration::from_secs(o.seconds);
    let started = Instant::now();
    let workers: Vec<_> = (0..o.users)
        .map(|w| {
            let c = Client::new(&o);
            let targets = targets.clone();
            let seq = seq.clone();
            thread::spawn(move || {
                let mut seen: Vec<(&'static str, u128, bool)> = vec![];
                while Instant::now() < deadline {
                    let n = seq.fetch_add(1, Ordering::Relaxed);
                    let t = &targets[(n as usize + w) % targets.len()];
                    let card = t.cards[(n as usize * 7 + w) % t.cards.len()];
                    let op = pick(n.wrapping_mul(2654435761));
                    let at = Instant::now();
                    let (code, _) = match op {
                        "board" => c.get(&format!("/boards/{}", t.board)),
                        "card" => c.get(&format!("/boards/{}/cards/{card}", t.board)),
                        "search" => c.get("/search?q=design"),
                        "comment" => c.post(
                            &format!("/boards/{}/cards/{card}/comments", t.board),
                            json!({ "body": format!("load test comment {n}") }),
                        ),
                        _ => {
                            let list = t.lists[(n as usize) % t.lists.len()];
                            c.post(
                                &format!("/boards/{}/cards/{card}/move", t.board),
                                json!({ "list_id": list }),
                            )
                        }
                    };
                    seen.push((op, at.elapsed().as_micros(), (200..300).contains(&code)));
                }
                seen
            })
        })
        .collect();
    let mut all: Vec<(&'static str, u128, bool)> = vec![];
    for w in workers {
        all.extend(w.join().expect("worker"));
    }
    let elapsed = started.elapsed().as_secs_f64();

    let mut by: BTreeMap<&str, Vec<u128>> = BTreeMap::new();
    let mut errors: BTreeMap<&str, usize> = BTreeMap::new();
    for (op, us, ok) in &all {
        by.entry(op).or_default().push(*us);
        by.entry("all").or_default().push(*us);
        if !ok {
            *errors.entry(op).or_default() += 1;
            *errors.entry("all").or_default() += 1;
        }
    }
    println!("| Request | Count | Errors | p50 ms | p95 ms | p99 ms | Max ms |");
    println!("| --- | ---: | ---: | ---: | ---: | ---: | ---: |");
    for (op, mut v) in by {
        v.sort_unstable();
        let q = |p: f64| v[((v.len() as f64 - 1.0) * p).round() as usize] as f64 / 1000.0;
        println!(
            "| {op} | {} | {} | {:.1} | {:.1} | {:.1} | {:.1} |",
            v.len(),
            errors.get(op).copied().unwrap_or(0),
            q(0.50),
            q(0.95),
            q(0.99),
            q(1.0)
        );
    }
    println!();
    println!(
        "{} requests in {:.1} s: {:.0} requests a second",
        all.len(),
        elapsed,
        all.len() as f64 / elapsed
    );
}
