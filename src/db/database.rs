use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use rusqlite::Connection;
use tokio::sync::{oneshot, watch};

use super::migrate::{Migrations, migrate, open_connection};
use super::{DbError, DbResult};

pub const READERS: usize = 4;

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

struct Inner {
    path: PathBuf,
    writer: Mutex<Option<mpsc::Sender<Job>>>,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
    readers: Vec<Mutex<Connection>>,
    next: AtomicUsize,
    changed: watch::Sender<u64>,
}

#[derive(Clone)]
pub struct Database {
    inner: Arc<Inner>,
}

impl Database {
    pub fn open(path: &Path, migrations: &Migrations) -> DbResult<Self> {
        let mut conn = open_connection(path)?;
        migrate(&mut conn, migrations)?;

        let readers = (0..READERS)
            .map(|_| {
                let c = open_connection(path)?;
                c.pragma_update(None, "query_only", "ON")?;
                Ok(Mutex::new(c))
            })
            .collect::<DbResult<Vec<_>>>()?;

        let (tx, rx) = mpsc::channel::<Job>();
        let (changed, _) = watch::channel(0u64);
        let notify = changed.clone();
        let name = format!("db-writer:{}", path.display());
        let handle = thread::Builder::new().name(name).spawn(move || {
            let mut conn = conn;
            while let Ok(job) = rx.recv() {
                job(&mut conn);
                notify.send_modify(|n| *n = n.wrapping_add(1));
            }
        })?;

        Ok(Self {
            inner: Arc::new(Inner {
                path: path.to_path_buf(),
                writer: Mutex::new(Some(tx)),
                thread: Mutex::new(Some(handle)),
                readers,
                next: AtomicUsize::new(0),
                changed,
            }),
        })
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.inner.changed.subscribe()
    }

    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    pub async fn write<T, F>(&self, f: F) -> DbResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> DbResult<T> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let job: Job = Box::new(move |conn| {
            let _ = tx.send(f(conn));
        });
        let sender = self
            .inner
            .writer
            .lock()
            .unwrap()
            .clone()
            .ok_or(DbError::Closed)?;
        sender.send(job).map_err(|_| DbError::Closed)?;
        rx.await.map_err(|_| DbError::Closed)?
    }

    pub async fn read<T, F>(&self, f: F) -> DbResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> DbResult<T> + Send + 'static,
    {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let i = inner.next.fetch_add(1, Ordering::Relaxed) % inner.readers.len();
            let conn = inner.readers[i].lock().unwrap();
            f(&conn)
        })
        .await
        .map_err(|_| DbError::Closed)?
    }

    pub fn close(&self) {
        self.inner.writer.lock().unwrap().take();
        if let Some(h) = self.inner.thread.lock().unwrap().take() {
            let _ = h.join();
        }
    }
}
