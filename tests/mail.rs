mod common;

use std::path::{Path, PathBuf};

use axum::http::{Method, StatusCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::{TestApp, Who, call, person};
use serde_json::{Value, json};

const C: &str = "/api/v1/c/company-a";

fn stub() -> (u16, PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("outbox");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let serve_dir = out.clone();
    std::thread::spawn(move || tazko::mail::stub::serve(listener, serve_dir));
    (port, out, dir)
}

fn smtp_env(port: u16) -> String {
    format!(
        "TAZKO_SMTP_HOST=127.0.0.1\nTAZKO_SMTP_PORT={port}\nTAZKO_MAIL_FROM=Tazko <no-reply@example.com>\n"
    )
}

fn outbox(dir: &Path) -> Vec<String> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();
    files
        .iter()
        .map(|f| std::fs::read_to_string(f).unwrap())
        .collect()
}

fn text_part(mail: &str) -> String {
    let part = mail
        .split(
            "Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n",
        )
        .nth(1)
        .unwrap();
    let b64: String = part.split("\r\n--").next().unwrap().split("\r\n").collect();
    String::from_utf8(STANDARD.decode(b64).unwrap()).unwrap()
}

async fn req(
    app: &TestApp,
    who: &Who,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let res = call(app, who, method, path, body).await;
    let status = res.status();
    (
        status,
        serde_json::from_str(&common::body_text(res).await).unwrap_or(Value::Null),
    )
}

async fn deliver(app: &TestApp) -> usize {
    tazko::mail::deliver_due(&app.state, tazko::session::now() + 1)
        .await
        .unwrap()
}

struct World {
    app: TestApp,
    boss: Who,
    dev: Who,
    org: i64,
}

async fn world(env: &str) -> World {
    let app = TestApp::with_env(env);
    let org = app.company("company-a", "active").await;
    let boss = person(&app, "boss@a.example").await;
    let dev = person(&app, "dev@a.example").await;
    app.member(org, boss.id, "owner").await;
    app.member(org, dev.id, "member").await;
    World {
        app,
        boss,
        dev,
        org,
    }
}

#[tokio::test]
async fn invitations_are_emailed_when_asked_and_email_is_on() {
    let (port, out, _keep) = stub();
    let w = world(&smtp_env(port)).await;
    let (status, body) = req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/invites"),
        Some(json!({ "email": "new@b.example", "role": "member", "send_email": true })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["emailed"], true);
    assert_eq!(body["share_url"], "http://localhost:8791");
    let (_, quiet) = req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/invites"),
        Some(json!({ "email": "quiet@b.example", "role": "member" })),
    )
    .await;
    assert_eq!(quiet["emailed"], false, "no email unless asked");

    let project = req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/projects"),
        Some(json!({ "name": "Web", "owner_id": w.boss.id })),
    )
    .await
    .1["id"]
        .as_i64()
        .unwrap();
    let (_, p) = req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/projects/{project}/invites"),
        Some(json!({ "email": "guest@c.example", "role": "member", "send_email": true })),
    )
    .await;
    assert_eq!(p["emailed"], true);

    assert_eq!(deliver(&w.app).await, 2);
    let mails = outbox(&out);
    assert_eq!(mails.len(), 2);
    let invite = mails
        .iter()
        .find(|m| m.contains("To: new@b.example"))
        .unwrap();
    assert!(invite.contains("From: Tazko <no-reply@example.com>"));
    assert!(invite.contains("Subject: You are invited to COMPANY-A on Tazko"));
    assert!(text_part(invite).contains("boss@a.example invited you to COMPANY-A on Tazko."));
    assert!(!invite.to_lowercase().contains("img"), "no tracking images");
    let text = text_part(invite);
    assert!(text.contains("Sign in with Google: http://localhost:8791/"));
    let guest = text_part(
        mails
            .iter()
            .find(|m| m.contains("To: guest@c.example"))
            .unwrap(),
    );
    assert!(guest.contains("the Web project in COMPANY-A"));

    w.app
        .sql(|c| {
            for _ in 0..3 {
                c.execute("INSERT INTO email_queue (kind, to_addr, subject, body_text, body_html, status) VALUES ('invite', 'again@b.example', 's', 't', 'h', 'sent')", [])?;
            }
            Ok(())
        })
        .await;
    let (_, limited) = req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/invites"),
        Some(json!({ "email": "again@b.example", "role": "member", "send_email": true })),
    )
    .await;
    assert_eq!(
        (limited["emailed"].clone(), limited["email_limited"].clone()),
        (json!(false), json!(true)),
        "three invitation emails a day per address"
    );
}

#[tokio::test]
async fn the_admin_sees_email_status_and_can_send_a_test() {
    let (port, out, _keep) = stub();
    let w = world(&smtp_env(port)).await;
    let admin = w.boss.id;
    w.app
        .sql(move |c| c.execute("UPDATE users SET is_super_admin = 1 WHERE id = ?1", [admin]))
        .await;
    assert_eq!(
        req(
            &w.app,
            &w.dev,
            Method::POST,
            "/api/v1/admin/email/test",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, body) = req(
        &w.app,
        &w.boss,
        Method::POST,
        "/api/v1/admin/email/test",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(body["to"], "boss@a.example");
    assert_eq!(deliver(&w.app).await, 1);
    assert!(outbox(&out)[0].contains("Subject: Tazko test email"));
    let (_, server) = req(&w.app, &w.boss, Method::GET, "/api/v1/admin/server", None).await;
    assert_eq!(server["email_server"]["host"], "127.0.0.1");
    assert!(server["email_server"].get("password").is_none());
    assert_eq!(server["email_queue"]["sent_24h"], 1);
    for _ in 0..4 {
        req(
            &w.app,
            &w.boss,
            Method::POST,
            "/api/v1/admin/email/test",
            None,
        )
        .await;
    }
    assert_eq!(
        req(
            &w.app,
            &w.boss,
            Method::POST,
            "/api/v1/admin/email/test",
            None
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    let _ = w.org;
}

#[tokio::test]
async fn with_email_off_invites_still_work_and_show_the_address() {
    let w = world("").await;
    let (status, body) = req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/invites"),
        Some(json!({ "email": "new@b.example", "role": "member", "send_email": true })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        (body["emailed"].clone(), body["share_url"].clone()),
        (json!(false), json!("http://localhost:8791"))
    );
    let queued: i64 = w
        .app
        .sql(|c| c.query_row("SELECT count(*) FROM email_queue", [], |r| r.get(0)))
        .await;
    assert_eq!(queued, 0);
    let (me_status, me) = req(&w.app, &w.boss, Method::GET, "/api/v1/me", None).await;
    assert_eq!(
        (me_status, me["email_enabled"].clone()),
        (StatusCode::OK, json!(false))
    );
}

#[tokio::test]
async fn an_unreachable_mail_server_is_retried_later() {
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let w = world(&smtp_env(port)).await;
    req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/invites"),
        Some(json!({ "email": "new@b.example", "role": "member", "send_email": true })),
    )
    .await;
    assert_eq!(deliver(&w.app).await, 1);
    let (status, attempts, error, next): (String, i64, String, i64) = w
        .app
        .sql(|c| {
            c.query_row(
                "SELECT status, attempts, error, next_attempt_at FROM email_queue",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
        })
        .await;
    assert_eq!((status.as_str(), attempts), ("pending", 1));
    assert!(error.contains("connection"), "{error}");
    assert!(next > tazko::session::now() + 30);
    assert_eq!(deliver(&w.app).await, 0, "waits before trying again");
}

fn refusing_server() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader, Write};
        for stream in listener.incoming().flatten() {
            let mut out = stream.try_clone().unwrap();
            let mut lines = BufReader::new(stream);
            let _ = out.write_all(b"220 ready\r\n");
            let mut line = String::new();
            while lines.read_line(&mut line).unwrap_or(0) > 0 {
                let reply: &[u8] = if line.starts_with("EHLO") {
                    b"250-x\r\n250 AUTH PLAIN\r\n"
                } else {
                    b"535 5.7.8 bad credentials\r\n"
                };
                let _ = out.write_all(reply);
                line.clear();
            }
        }
    });
    port
}

#[tokio::test]
async fn a_rejected_sign_in_is_retried_not_given_up() {
    let port = refusing_server();
    let w = world(&format!(
        "{}TAZKO_SMTP_USER=mailer\nTAZKO_SMTP_PASSWORD=wrong\n",
        smtp_env(port)
    ))
    .await;
    for email in ["one@b.example", "two@b.example"] {
        req(
            &w.app,
            &w.boss,
            Method::POST,
            &format!("{C}/invites"),
            Some(json!({ "email": email, "role": "member", "send_email": true })),
        )
        .await;
    }
    assert_eq!(
        deliver(&w.app).await,
        1,
        "a server-wide problem stops the batch"
    );
    let rows: Vec<(String, i64)> = w
        .app
        .sql(|c| {
            c.prepare("SELECT status, attempts FROM email_queue ORDER BY id")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect()
        })
        .await;
    assert_eq!(
        rows,
        [("pending".to_string(), 1), ("pending".to_string(), 0)]
    );
    let admin = w.boss.id;
    w.app
        .sql(move |c| c.execute("UPDATE users SET is_super_admin = 1 WHERE id = ?1", [admin]))
        .await;
    let (_, server) = req(&w.app, &w.boss, Method::GET, "/api/v1/admin/server", None).await;
    assert!(
        server["email_queue"]["last_error"]["error"]
            .as_str()
            .unwrap()
            .contains("535")
    );
}

#[tokio::test]
async fn disabled_and_non_ascii_addresses_get_no_email_and_old_mail_is_cleared() {
    let (port, _out, _keep) = stub();
    let w = world(&smtp_env(port)).await;
    w.app
        .sql(|c| {
            c.execute(
                "INSERT INTO users (email, name, disabled) VALUES ('gone@b.example', 'Gone', 1)",
                [],
            )
        })
        .await;
    let (_, owner_gone) = req(
        &w.app,
        &w.boss,
        Method::POST,
        &format!("{C}/invites"),
        Some(json!({ "email": "gone@b.example", "role": "member", "send_email": true })),
    )
    .await;
    assert_eq!(
        owner_gone["emailed"], false,
        "a disabled account is not told it joins automatically"
    );
    w.app
        .sql(|c| c.execute("INSERT INTO email_queue (kind, to_addr, subject, body_text, body_html, status, created_at) VALUES ('invite', 'old@b.example', 's', 't', 'h', 'sent', 1)", []))
        .await;
    assert_eq!(
        tazko::mail::purge(&w.app.state, tazko::session::now())
            .await
            .unwrap(),
        1
    );
}
