#![allow(dead_code)]

use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use axum::response::Response;
use http_body_util::BodyExt;
use rusqlite::params;
use tazko::config::{self, DataLayout};
use tazko::db::Databases;
use tazko::session;
use tazko::state::AppState;
use tower::ServiceExt;

pub struct TestApp {
    pub dir: tempfile::TempDir,
    pub state: AppState,
}

impl TestApp {
    pub fn new() -> Self {
        Self::with_google(tazko::auth::google::Endpoints::default())
    }

    pub fn with_google(endpoints: tazko::auth::google::Endpoints) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let map = config::parse_env(&format!(
            "TAZKO_PUBLIC_URL=http://localhost:8791\nTAZKO_DATA_DIR={}\nTAZKO_GOOGLE_CLIENT_ID=client.example\nTAZKO_GOOGLE_CLIENT_SECRET=test-client-secret\n",
            data.display()
        ));
        let settings = config::from_map(&map).unwrap();
        let layout = DataLayout::new(&data);
        layout.create().unwrap();
        let dbs = Databases::open(layout).unwrap();
        Self {
            dir,
            state: AppState::with_google(
                settings,
                dbs,
                config::Secret::new("t".repeat(64)),
                endpoints,
            ),
        }
    }

    pub fn router(&self) -> Router {
        tazko::http::router(self.state.clone())
    }

    pub async fn sql<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut rusqlite::Connection) -> rusqlite::Result<T> + Send + 'static,
    ) -> T {
        self.state
            .dbs
            .control()
            .write(move |c| Ok(f(c)?))
            .await
            .unwrap()
    }

    pub async fn user(&self, email: &str) -> i64 {
        let email = email.to_string();
        self.sql(move |c| {
            c.execute(
                "INSERT INTO users (email, name) VALUES (?1, ?1)",
                params![email],
            )?;
            Ok(c.last_insert_rowid())
        })
        .await
    }

    pub async fn company(&self, slug: &str, status: &str) -> i64 {
        let slug_owned = slug.to_string();
        let (slug, status) = (slug.to_string(), status.to_string());
        let status_now = status.clone();
        let id = self
            .sql(move |c| {
                c.execute(
                    "INSERT INTO orgs (slug, name, status) VALUES (?1, upper(?1), ?2)",
                    params![slug, status],
                )?;
                Ok(c.last_insert_rowid())
            })
            .await;
        if status_now != "deleted" {
            self.state.dbs.create_tenant(&slug_owned).unwrap();
            self.state.dbs.forget(&slug_owned);
        }
        id
    }

    pub async fn member(&self, org: i64, user: i64, role: &str) {
        let role = role.to_string();
        self.sql(move |c| {
            c.execute(
                "INSERT INTO org_members (org_id, user_id, role) VALUES (?1, ?2, ?3)",
                params![org, user, role],
            )
        })
        .await;
    }

    pub async fn sign_in(&self, user: i64) -> String {
        let (token, _) = session::create(self.state.dbs.control(), user, None)
            .await
            .unwrap();
        format!("{}={token}", session::COOKIE)
    }

    pub async fn get(&self, path: &str, cookie: Option<&str>) -> Response {
        let mut req = Request::get(path);
        if let Some(c) = cookie {
            req = req.header(header::COOKIE, c);
        }
        self.router()
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    pub async fn send(&self, req: Request<Body>) -> Response {
        self.router().oneshot(req).await.unwrap()
    }
}

pub fn set_cookies(res: &Response) -> Vec<String> {
    res.headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect()
}

pub fn cookie_pair(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap().to_string()
}

pub fn location(res: &Response) -> String {
    res.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string()
}

pub async fn body_text(res: Response) -> String {
    String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap()
}

pub async fn body_json(res: Response) -> serde_json::Value {
    serde_json::from_str(&body_text(res).await).unwrap()
}
