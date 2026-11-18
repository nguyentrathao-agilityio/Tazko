mod common;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use axum::Router;
use axum::body::Body;
use axum::extract::{Form, State};
use axum::http::{Request, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use common::{TestApp, body_json, cookie_pair, location, set_cookies};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::traits::PublicKeyParts;
use rusqlite::params;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tazko::auth::google::Endpoints;
use tazko::auth::{CSRF_HEADER, safe_next};
use tazko::session;

const CLIENT_ID: &str = "client.example";
const CLIENT_SECRET: &str = "test-client-secret";

struct Key {
    der: Vec<u8>,
    n: String,
    e: String,
}

fn keys() -> &'static (Key, Key) {
    static KEYS: OnceLock<(Key, Key)> = OnceLock::new();
    KEYS.get_or_init(|| {
        let make = || {
            let k = rsa::RsaPrivateKey::new(&mut rand::thread_rng(), 2048).unwrap();
            Key {
                der: k.to_pkcs1_der().unwrap().as_bytes().to_vec(),
                n: URL_SAFE_NO_PAD.encode(k.n().to_bytes_be()),
                e: URL_SAFE_NO_PAD.encode(k.e().to_bytes_be()),
            }
        };
        (make(), make())
    })
}

#[derive(Default)]
struct Mock {
    codes: Mutex<HashMap<String, (String, String)>>,
    jwks_fetches: AtomicUsize,
}

async fn jwks(State(m): State<Arc<Mock>>) -> Response {
    m.jwks_fetches.fetch_add(1, Ordering::SeqCst);
    let k = &keys().0;
    (
        [(header::CACHE_CONTROL, "public, max-age=3600")],
        Json(json!({ "keys": [{ "kid": "good", "kty": "RSA", "alg": "RS256", "use": "sig", "n": k.n, "e": k.e }] })),
    )
        .into_response()
}

async fn token(State(m): State<Arc<Mock>>, Form(f): Form<HashMap<String, String>>) -> Response {
    let ok = f.get("grant_type").map(String::as_str) == Some("authorization_code")
        && f.get("client_id").map(String::as_str) == Some(CLIENT_ID)
        && f.get("client_secret").map(String::as_str) == Some(CLIENT_SECRET)
        && f.get("redirect_uri").map(String::as_str)
            == Some("http://localhost:8791/auth/google/callback");
    let Some((challenge, id_token)) = f
        .get("code")
        .and_then(|c| m.codes.lock().unwrap().remove(c))
    else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_grant" })),
        )
            .into_response();
    };
    let verifier = f.get("code_verifier").cloned().unwrap_or_default();
    if !ok || URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) != challenge {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_grant" })),
        )
            .into_response();
    }
    Json(json!({ "id_token": id_token, "access_token": "unused", "token_type": "Bearer" }))
        .into_response()
}

struct Harness {
    app: TestApp,
    mock: Arc<Mock>,
}

async fn harness() -> Harness {
    let mock = Arc::new(Mock::default());
    let router = Router::new()
        .route("/jwks", get(jwks))
        .route("/token", post(token))
        .with_state(mock.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let app = TestApp::with_google(Endpoints {
        auth_url: "https://accounts.example/o/auth".into(),
        token_url: format!("{base}/token"),
        jwks_url: format!("{base}/jwks"),
        issuers: vec!["https://accounts.google.com".into()],
    });
    Harness { app, mock }
}

fn claims(sub: &str, email: &str, hd: Option<&str>) -> Value {
    json!({
        "iss": "https://accounts.google.com",
        "aud": CLIENT_ID,
        "sub": sub,
        "email": email,
        "email_verified": true,
        "name": "Test Person",
        "hd": hd,
        "iat": session::now(),
        "exp": session::now() + 3600,
    })
}

fn sign(claims: &Value, kid: &str, key: &Key) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.into());
    jsonwebtoken::encode(&header, claims, &EncodingKey::from_rsa_der(&key.der)).unwrap()
}

fn query_of(url: &str) -> HashMap<String, String> {
    let q = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    form_urlencoded::parse(q.as_bytes()).into_owned().collect()
}

struct Started {
    oauth_cookie: String,
    query: HashMap<String, String>,
}

async fn start(h: &Harness, next: &str) -> Started {
    let res = h
        .app
        .get(&format!("/auth/google/start?next={next}"), None)
        .await;
    assert!(res.status().is_redirection());
    let loc = location(&res);
    let cookie = set_cookies(&res)
        .into_iter()
        .find(|c| c.starts_with("tazko_oauth="))
        .unwrap();
    Started {
        oauth_cookie: cookie_pair(&cookie),
        query: query_of(&loc),
    }
}

async fn finish(h: &Harness, s: &Started, code: &str) -> Response {
    let state = &s.query["state"];
    h.app
        .get(
            &format!("/auth/google/callback?code={code}&state={state}"),
            Some(&s.oauth_cookie),
        )
        .await
}

async fn sign_in_with(h: &Harness, mut c: Value, kid: &str, key: &Key) -> Response {
    let s = start(h, "/after").await;
    if c.get("nonce").is_none() {
        c["nonce"] = Value::String(s.query["nonce"].clone());
    }
    let code = format!("code-{}", session::random_token());
    h.mock.codes.lock().unwrap().insert(
        code.clone(),
        (s.query["code_challenge"].clone(), sign(&c, kid, key)),
    );
    finish(h, &s, &code).await
}

async fn sign_in(h: &Harness, c: Value) -> Response {
    sign_in_with(h, c, "good", &keys().0).await
}

async fn invite(h: &Harness, slug: &str, email: &str, role: &str) -> i64 {
    let org = h.app.company(slug, "active").await;
    let (email, role) = (email.to_string(), role.to_string());
    h.app
        .sql(move |c| {
            c.execute(
                "INSERT INTO invites (org_id, email, company_role) VALUES (?1, ?2, ?3)",
                params![org, email, role],
            )
        })
        .await;
    org
}

fn session_cookie(res: &Response) -> Option<String> {
    set_cookies(res)
        .into_iter()
        .find(|c| c.starts_with("tazko_session=") && !c.contains("Max-Age=0"))
}

async fn refusals(h: &Harness) -> Vec<(String, String)> {
    h.app
        .sql(|c| {
            let mut stmt = c.prepare(
                "SELECT target, detail FROM audit_log WHERE action = 'sign_in_refused' ORDER BY id",
            )?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn start_sends_the_browser_to_google_with_state_nonce_and_pkce() {
    let h = harness().await;
    let res = h.app.get("/auth/google/start?next=/c/acme", None).await;
    let loc = location(&res);
    assert!(loc.starts_with("https://accounts.example/o/auth?"), "{loc}");
    let q = query_of(&loc);
    assert_eq!(q["client_id"], CLIENT_ID);
    assert_eq!(
        q["redirect_uri"],
        "http://localhost:8791/auth/google/callback"
    );
    assert_eq!(q["response_type"], "code");
    assert_eq!(q["scope"], "openid email profile");
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(q["code_challenge"].len(), 43);
    assert_eq!(q["state"].len(), 64);
    assert_eq!(q["nonce"].len(), 64);
    assert_ne!(q["state"], q["nonce"]);

    let cookie = set_cookies(&res)
        .into_iter()
        .find(|c| c.starts_with("tazko_oauth="))
        .unwrap();
    for flag in [
        "HttpOnly",
        "Secure",
        "SameSite=Lax",
        "Path=/auth/google",
        "Max-Age=600",
    ] {
        assert!(cookie.contains(flag), "{flag} missing in {cookie}");
    }
    assert!(
        !cookie.contains(&q["state"]),
        "state must not be readable from the cookie"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn invited_workspace_and_personal_accounts_both_sign_in() {
    let h = harness().await;
    let org = invite(&h, "acme", "Mai@Company.Example", "owner").await;
    h.app
        .sql(move |c| {
            c.execute(
                "INSERT INTO invites (org_id, email, company_role) VALUES (?1, 'jo.lee@gmail.com', 'member')",
                [org],
            )
        })
        .await;

    for (sub, email, hd, role) in [
        (
            "sub-mai",
            "mai@company.example",
            Some("company.example"),
            "owner",
        ),
        ("sub-jo", "jo.lee@gmail.com", None, "member"),
    ] {
        let res = sign_in(&h, claims(sub, email, hd)).await;
        assert!(res.status().is_redirection(), "{email}");
        assert_eq!(location(&res), "/after");
        let cookie = session_cookie(&res).unwrap_or_else(|| panic!("no session for {email}"));
        for flag in [
            "HttpOnly",
            "Secure",
            "SameSite=Lax",
            "Path=/",
            "Max-Age=2592000",
        ] {
            assert!(cookie.contains(flag), "{flag} missing in {cookie}");
        }

        let me = body_json(h.app.get("/api/v1/me", Some(&cookie_pair(&cookie))).await).await;
        assert_eq!(me["email"], email);
        assert_eq!(me["companies"][0]["slug"], "acme");
        assert_eq!(me["companies"][0]["role"], role);
        assert_eq!(me["csrf_token"].as_str().unwrap().len(), 64);
    }

    let (accepted, subs, hd): (i64, i64, Option<String>) = h
        .app
        .sql(|c| {
            c.query_row(
                "SELECT (SELECT count(*) FROM invites WHERE accepted_at IS NOT NULL),
                        (SELECT count(*) FROM users WHERE google_sub IS NOT NULL),
                        (SELECT hd FROM users WHERE email = 'mai@company.example')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
        })
        .await;
    assert_eq!(
        (accepted, subs, hd.as_deref()),
        (2, 2, Some("company.example"))
    );
    assert!(refusals(&h).await.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_uninvited_account_is_refused_and_audit_logged() {
    let h = harness().await;
    invite(&h, "acme", "someone@example.com", "member").await;
    let res = sign_in(&h, claims("sub-x", "stranger@gmail.com", None)).await;
    assert_eq!(location(&res), "/signin/refused");
    assert!(session_cookie(&res).is_none());

    let note = set_cookies(&res)
        .into_iter()
        .find(|c| c.starts_with("tazko_refusal="))
        .unwrap();
    assert!(note.contains("Path=/auth/refusal") && note.contains("HttpOnly"));
    let body = body_json(h.app.get("/auth/refusal", Some(&cookie_pair(&note))).await).await;
    assert_eq!(
        body,
        json!({ "email": "stranger@gmail.com", "reason": "not_invited" })
    );

    let log = refusals(&h).await;
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].0, "stranger@gmail.com");
    assert!(log[0].1.contains("not_invited"));
    let users: i64 = h
        .app
        .sql(|c| c.query_row("SELECT count(*) FROM users", [], |r| r.get(0)))
        .await;
    assert_eq!(users, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_refusal_page_data_needs_the_signed_note() {
    let h = harness().await;
    assert_eq!(
        h.app.get("/auth/refusal", None).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        h.app
            .get("/auth/refusal", Some("tazko_refusal=eyJ4IjoxfQ.AAAA"))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unverified_disabled_revoked_and_mismatched_accounts_are_refused() {
    let h = harness().await;
    let org = invite(&h, "acme", "unverified@example.com", "member").await;

    let mut c = claims("sub-u", "unverified@example.com", None);
    c["email_verified"] = json!(false);
    assert_eq!(location(&sign_in(&h, c).await), "/signin/refused");

    let id = h.app.user("off@example.com").await;
    h.app
        .sql(move |c| c.execute("UPDATE users SET disabled = 1 WHERE id = ?1", [id]))
        .await;
    assert_eq!(
        location(&sign_in(&h, claims("sub-off", "off@example.com", None)).await),
        "/signin/refused"
    );

    h.app
        .sql(move |c| {
            c.execute(
                "INSERT INTO invites (org_id, email, company_role, revoked_at) VALUES (?1, 'revoked@example.com', 'member', unixepoch())",
                [org],
            )
        })
        .await;
    assert_eq!(
        location(&sign_in(&h, claims("sub-r", "revoked@example.com", None)).await),
        "/signin/refused"
    );

    let id = h.app.user("taken@example.com").await;
    h.app
        .sql(move |c| {
            c.execute(
                "UPDATE users SET google_sub = 'sub-original' WHERE id = ?1",
                [id],
            )
        })
        .await;
    assert_eq!(
        location(&sign_in(&h, claims("sub-other", "taken@example.com", None)).await),
        "/signin/refused"
    );

    let reasons: Vec<String> = refusals(&h).await.into_iter().map(|(_, d)| d).collect();
    for (i, want) in [
        "email_not_verified",
        "disabled",
        "not_invited",
        "account_mismatch",
    ]
    .iter()
    .enumerate()
    {
        assert!(reasons[i].contains(want), "{reasons:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn people_are_matched_by_google_id_after_an_email_change() {
    let h = harness().await;
    invite(&h, "acme", "old@example.com", "member").await;
    assert!(session_cookie(&sign_in(&h, claims("sub-1", "old@example.com", None)).await).is_some());
    assert!(session_cookie(&sign_in(&h, claims("sub-1", "new@example.com", None)).await).is_some());
    let emails: Vec<String> = h
        .app
        .sql(|c| {
            let mut s = c.prepare("SELECT email FROM users")?;
            let r = s
                .query_map([], |r| r.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(r)
        })
        .await;
    assert_eq!(emails, vec!["new@example.com"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_admin_added_from_the_command_line_signs_in_by_email() {
    let h = harness().await;
    let id = h.app.user("boss@example.com").await;
    let res = sign_in(&h, claims("sub-boss", "BOSS@example.com", None)).await;
    assert!(session_cookie(&res).is_some());
    let sub: String = h
        .app
        .sql(move |c| {
            c.query_row("SELECT google_sub FROM users WHERE id = ?1", [id], |r| {
                r.get(0)
            })
        })
        .await;
    assert_eq!(sub, "sub-boss");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bad_or_replayed_tokens_never_sign_in() {
    let h = harness().await;
    invite(&h, "acme", "mai@example.com", "member").await;
    let good = || claims("sub-mai", "mai@example.com", None);

    let mut wrong_nonce = good();
    wrong_nonce["nonce"] = json!("not-the-nonce");
    let mut wrong_aud = good();
    wrong_aud["aud"] = json!("someone-else");
    let mut wrong_iss = good();
    wrong_iss["iss"] = json!("https://issuer.example");
    let mut expired = good();
    expired["exp"] = json!(session::now() - 3600);

    for c in [wrong_nonce, wrong_aud, wrong_iss, expired] {
        let res = sign_in(&h, c).await;
        assert_eq!(location(&res), "/signin?error=failed");
        assert!(session_cookie(&res).is_none());
    }

    let res = sign_in_with(&h, good(), "good", &keys().1).await;
    assert_eq!(
        location(&res),
        "/signin?error=failed",
        "signed with another key"
    );
    let res = sign_in_with(&h, good(), "unknown", &keys().1).await;
    assert_eq!(location(&res), "/signin?error=failed", "unknown key id");

    let sessions: i64 = h
        .app
        .sql(|c| c.query_row("SELECT count(*) FROM sessions", [], |r| r.get(0)))
        .await;
    assert_eq!(sessions, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_callback_without_the_matching_state_is_rejected() {
    let h = harness().await;
    let s = start(&h, "/").await;
    let res = h
        .app
        .get(
            "/auth/google/callback?code=abc&state=wrong",
            Some(&s.oauth_cookie),
        )
        .await;
    assert_eq!(location(&res), "/signin?error=expired");

    let res = h
        .app
        .get(
            &format!("/auth/google/callback?code=abc&state={}", s.query["state"]),
            None,
        )
        .await;
    assert_eq!(location(&res), "/signin?error=expired");

    let tampered = format!("{}x", s.oauth_cookie);
    let res = h
        .app
        .get(
            &format!("/auth/google/callback?code=abc&state={}", s.query["state"]),
            Some(&tampered),
        )
        .await;
    assert_eq!(location(&res), "/signin?error=expired");

    let res = h
        .app
        .get(
            "/auth/google/callback?error=access_denied",
            Some(&s.oauth_cookie),
        )
        .await;
    assert_eq!(location(&res), "/signin?error=cancelled");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn google_signing_keys_are_cached() {
    let h = harness().await;
    invite(&h, "acme", "mai@example.com", "member").await;
    for _ in 0..3 {
        assert!(
            session_cookie(&sign_in(&h, claims("sub-mai", "mai@example.com", None)).await)
                .is_some()
        );
    }
    assert_eq!(h.mock.jwks_fetches.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sign_out_needs_the_csrf_token_and_ends_the_session() {
    let h = harness().await;
    invite(&h, "acme", "mai@example.com", "member").await;
    let res = sign_in(&h, claims("sub-mai", "mai@example.com", None)).await;
    let cookie = cookie_pair(&session_cookie(&res).unwrap());
    let csrf = body_json(h.app.get("/api/v1/me", Some(&cookie)).await).await["csrf_token"]
        .as_str()
        .unwrap()
        .to_string();

    let post = |token: Option<&str>| {
        let mut r = Request::post("/auth/signout").header(header::COOKIE, &cookie);
        if let Some(t) = token {
            r = r.header(CSRF_HEADER, t);
        }
        r.body(Body::empty()).unwrap()
    };
    assert_eq!(h.app.send(post(None)).await.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        h.app.send(post(Some("wrong"))).await.status(),
        StatusCode::FORBIDDEN
    );

    let res = h.app.send(post(Some(&csrf))).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert!(
        set_cookies(&res)
            .iter()
            .any(|c| c.starts_with("tazko_session=;") && c.contains("Max-Age=0"))
    );
    assert_eq!(
        h.app.get("/api/v1/me", Some(&cookie)).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[test]
fn only_local_paths_are_followed_after_sign_in() {
    assert_eq!(safe_next(Some("/c/acme/boards")), "/c/acme/boards");
    assert_eq!(safe_next(Some("/c/acme?view=table")), "/c/acme?view=table");
    for bad in [
        "https://evil.example",
        "//evil.example",
        "/\\evil.example",
        "/\t/evil.example",
        "/\n/evil.example",
        "/\r//evil.example",
        "/ /evil.example",
        "/auth/google/start",
        "/x/../auth/signout",
        "/.//evil.example",
        "/x/..//evil.example",
        "/%2e//evil.example",
        "/%2E%2E//evil.example",
        "/x/%2e%2e//evil.example",
        "/x/..",
        "/x/../auth",
        "relative",
        "",
    ] {
        assert_eq!(safe_next(Some(bad)), "/", "{bad:?}");
    }
    assert_eq!(safe_next(None), "/");
    assert_eq!(
        safe_next(Some("/c/acme/./boards/../settings")),
        "/c/acme/settings"
    );
    assert_eq!(safe_next(Some("/c/acme/#top")), "/c/acme/");
}

#[test]
#[ignore = "needs the internet"]
fn google_signing_keys_load_over_tls() {
    let google = tazko::auth::google::Google::new(Endpoints::default());
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("no-such-key".into());
    let token = jsonwebtoken::encode(
        &header,
        &claims("s", "e@example.com", None),
        &EncodingKey::from_rsa_der(&keys().0.der),
    )
    .unwrap();
    let err = google
        .verify_id_token(&token, CLIENT_ID, "n")
        .unwrap_err()
        .to_string();
    assert!(err.contains("unknown signing key"), "{err}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_owner_invites_a_member_who_joins_on_first_sign_in() {
    let h = harness().await;
    let org = h.app.company("acme", "active").await;
    let owner = h.app.user("owner@acme.example").await;
    h.app.member(org, owner, "owner").await;
    let cookie = h.app.sign_in(owner).await;
    let csrf = body_json(h.app.get("/api/v1/me", Some(&cookie)).await).await["csrf_token"]
        .as_str()
        .unwrap()
        .to_string();

    let res = h
        .app
        .send(
            Request::post("/api/v1/c/acme/invites")
                .header(header::COOKIE, &cookie)
                .header(CSRF_HEADER, &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "email": "Newbie@Gmail.com", "role": "member" }).to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    assert_eq!(body_json(res).await["result"], "invited");

    let res = sign_in(&h, claims("sub-newbie", "newbie@gmail.com", None)).await;
    let session = cookie_pair(&session_cookie(&res).expect("the invited member signs in"));
    let me = body_json(h.app.get("/api/v1/me", Some(&session)).await).await;
    assert_eq!(
        me["companies"],
        json!([{ "slug": "acme", "name": "ACME", "role": "member" }])
    );
    assert_eq!(
        h.app.get("/api/v1/c/acme", Some(&session)).await.status(),
        StatusCode::OK
    );

    let settings = h
        .app
        .send(
            Request::get("/api/v1/c/acme/settings")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    let settings = body_json(settings).await;
    assert_eq!(settings["invites"], json!([]));
    assert!(
        settings["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["email"] == "newbie@gmail.com")
    );
}
