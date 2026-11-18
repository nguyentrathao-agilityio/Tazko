use std::sync::Arc;
use std::time::Instant;

use crate::auth::google::{Endpoints, Google};
use crate::config::{Secret, Settings};
use crate::db::Databases;
use crate::realtime::hub::Hub;

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub dbs: Arc<Databases>,
    pub secret: Arc<Secret>,
    pub google: Arc<Google>,
    pub started: Instant,
    pub live: Arc<Hub>,
}

impl AppState {
    pub fn new(settings: Settings, dbs: Databases, secret: Secret) -> Self {
        Self::with_google(settings, dbs, secret, Endpoints::default())
    }

    pub fn with_google(
        settings: Settings,
        dbs: Databases,
        secret: Secret,
        endpoints: Endpoints,
    ) -> Self {
        Self {
            settings: Arc::new(settings),
            dbs: Arc::new(dbs),
            secret: Arc::new(secret),
            google: Arc::new(Google::new(endpoints)),
            started: Instant::now(),
            live: Arc::new(Hub::default()),
        }
    }
}
