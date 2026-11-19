use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use tokio::sync::watch;

#[derive(Debug, Clone, Serialize)]
pub struct Viewer {
    pub id: i64,
    pub name: String,
    pub email: String,
}

struct Room {
    viewers: HashMap<u64, Viewer>,
    changed: watch::Sender<u64>,
}

#[derive(Default)]
pub struct Hub {
    rooms: Mutex<HashMap<(String, i64), Room>>,
    next: AtomicU64,
}

impl Hub {
    pub fn join(&self, company: &str, board: i64, viewer: Viewer) -> (u64, watch::Receiver<u64>) {
        let conn = self.next.fetch_add(1, Ordering::Relaxed);
        let mut rooms = self.rooms.lock().unwrap();
        let room = rooms
            .entry((company.to_string(), board))
            .or_insert_with(|| Room {
                viewers: HashMap::new(),
                changed: watch::channel(0).0,
            });
        room.viewers.insert(conn, viewer);
        let rx = room.changed.subscribe();
        room.changed.send_modify(|n| *n += 1);
        (conn, rx)
    }

    pub fn leave(&self, company: &str, board: i64, conn: u64) {
        let mut rooms = self.rooms.lock().unwrap();
        let key = (company.to_string(), board);
        let empty = match rooms.get_mut(&key) {
            Some(room) => {
                room.viewers.remove(&conn);
                room.changed.send_modify(|n| *n += 1);
                room.viewers.is_empty()
            }
            None => false,
        };
        if empty {
            rooms.remove(&key);
        }
    }

    pub fn viewers(&self, company: &str, board: i64) -> Vec<Viewer> {
        let rooms = self.rooms.lock().unwrap();
        let mut seen = HashMap::new();
        if let Some(room) = rooms.get(&(company.to_string(), board)) {
            for v in room.viewers.values() {
                seen.entry(v.id).or_insert_with(|| v.clone());
            }
        }
        let mut list: Vec<Viewer> = seen.into_values().collect();
        list.sort_by_cached_key(|v| (v.name.to_lowercase(), v.id));
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(id: i64, name: &str) -> Viewer {
        Viewer {
            id,
            name: name.into(),
            email: format!("{id}@x.example"),
        }
    }

    #[test]
    fn presence_counts_each_person_once_per_board() {
        let hub = Hub::default();
        let (a, mut rx) = hub.join("acme", 1, v(1, "Mai"));
        let (b, _) = hub.join("acme", 1, v(1, "Mai"));
        hub.join("acme", 1, v(2, "Jo"));
        hub.join("acme", 2, v(3, "Ana"));
        hub.join("other", 1, v(4, "Kim"));
        assert!(rx.has_changed().unwrap());
        rx.mark_unchanged();
        let names: Vec<String> = hub.viewers("acme", 1).into_iter().map(|v| v.name).collect();
        assert_eq!(names, vec!["Jo", "Mai"]);
        hub.leave("acme", 1, a);
        assert!(rx.has_changed().unwrap());
        assert_eq!(hub.viewers("acme", 1).len(), 2);
        hub.leave("acme", 1, b);
        assert_eq!(hub.viewers("acme", 1).len(), 1);
    }
}
