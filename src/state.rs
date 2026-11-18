use std::sync::Arc;

use crate::config::Settings;
use crate::db::Databases;

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub dbs: Arc<Databases>,
}

impl AppState {
    pub fn new(settings: Settings, dbs: Databases) -> Self {
        Self {
            settings: Arc::new(settings),
            dbs: Arc::new(dbs),
        }
    }
}
