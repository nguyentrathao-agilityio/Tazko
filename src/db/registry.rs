use std::collections::HashMap;
use std::sync::Mutex;

use super::database::Database;
use super::migrate::{CONTROL, TENANT};
use super::{DbError, DbResult};
use crate::config::DataLayout;

pub struct Databases {
    layout: DataLayout,
    control: Database,
    tenants: Mutex<HashMap<String, Database>>,
}

pub fn valid_slug(slug: &str) -> bool {
    (1..=40).contains(&slug.len())
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

impl Databases {
    pub fn open(layout: DataLayout) -> DbResult<Self> {
        let control = Database::open(&layout.control_db, &CONTROL)?;
        Ok(Self {
            layout,
            control,
            tenants: Mutex::new(HashMap::new()),
        })
    }

    pub fn layout(&self) -> &DataLayout {
        &self.layout
    }

    pub fn control(&self) -> &Database {
        &self.control
    }

    pub fn tenant(&self, slug: &str) -> DbResult<Database> {
        self.open_tenant(slug, false)
    }

    pub fn create_tenant(&self, slug: &str) -> DbResult<Database> {
        self.open_tenant(slug, true)
    }

    fn open_tenant(&self, slug: &str, create: bool) -> DbResult<Database> {
        if !valid_slug(slug) {
            return Err(DbError::BadSlug(slug.to_string()));
        }

        let mut open = self.tenants.lock().unwrap();
        if let Some(db) = open.get(slug) {
            return Ok(db.clone());
        }

        let dir = self.layout.orgs.join(slug);
        let file = dir.join("tazko.db");
        if create {
            std::fs::create_dir_all(dir.join("files"))?;
        } else if !file.is_file() {
            return Err(DbError::Missing(slug.to_string()));
        }

        let db = Database::open(&file, &TENANT)?;
        open.insert(slug.to_string(), db.clone());
        Ok(db)
    }

    pub fn is_open(&self, slug: &str) -> bool {
        self.tenants.lock().unwrap().contains_key(slug)
    }

    pub fn forget(&self, slug: &str) {
        if let Some(db) = self.tenants.lock().unwrap().remove(slug) {
            db.close();
        }
    }

    pub fn close(&self) {
        for (_, db) in self.tenants.lock().unwrap().drain() {
            db.close();
        }
        self.control.close();
    }
}
