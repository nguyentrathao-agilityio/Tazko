use std::sync::Arc;

use crate::auth::google::{Endpoints, Google};
use crate::config::{Secret, Settings};
use crate::db::Databases;

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub dbs: Arc<Databases>,
    pub secret: Arc<Secret>,
    pub google: Arc<Google>,
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
        }
    }
}
