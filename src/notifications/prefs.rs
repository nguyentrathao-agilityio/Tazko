use std::collections::HashMap;

use rusqlite::Connection;
use serde_json::{Map, Value};

pub const KINDS: [&str; 4] = ["mention", "assigned", "card_added", "due"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frequency {
    Instant,
    Hourly,
    Daily,
    Never,
}

#[derive(Clone, Debug)]
pub struct Prefs {
    app: [bool; 4],
    email: [bool; 4],
    pub frequency: Frequency,
    pub reminder: i64,
    pub daily_hour: i64,
}

fn index(kind: &str) -> Option<usize> {
    KINDS.iter().position(|k| *k == kind)
}

impl Prefs {
    pub fn from_settings(s: &Map<String, Value>) -> Prefs {
        let flag =
            |key: String, default: bool| s.get(&key).and_then(Value::as_bool).unwrap_or(default);
        let app = KINDS.map(|k| k == "due" || flag(format!("notify_{k}_app"), true));
        let email = KINDS.map(|k| flag(format!("notify_{k}_email"), k != "card_added"));
        let frequency = match s.get("email_frequency").and_then(Value::as_str) {
            Some("hourly") => Frequency::Hourly,
            Some("daily") => Frequency::Daily,
            Some("never") => Frequency::Never,
            _ => Frequency::Instant,
        };
        let reminder = match s.get("due_reminder").and_then(Value::as_str) {
            Some("at") => 0,
            Some("1h") => 3600,
            Some("2d") => 2 * 86_400,
            _ => 86_400,
        };
        let daily_hour = s.get("daily_hour_utc").and_then(Value::as_i64).unwrap_or(1);
        Prefs {
            app,
            email,
            frequency,
            reminder,
            daily_hour,
        }
    }

    pub fn load(c: &Connection, user: i64) -> rusqlite::Result<Prefs> {
        Ok(Prefs::from_settings(&crate::settings::load(c, user)?))
    }

    pub fn in_app(&self, kind: &str) -> bool {
        index(kind).is_none_or(|i| self.app[i])
    }

    pub fn by_email(&self, kind: &str) -> bool {
        self.frequency != Frequency::Never && index(kind).is_some_and(|i| self.email[i])
    }

    pub fn hidden(&self) -> Vec<&'static str> {
        KINDS.iter().copied().filter(|k| !self.in_app(k)).collect()
    }
}

pub fn hidden_clause(hidden: &[&'static str]) -> String {
    let known: Vec<String> = hidden
        .iter()
        .filter(|k| KINDS.contains(k))
        .map(|k| format!("'{k}'"))
        .collect();
    if known.is_empty() {
        String::new()
    } else {
        format!(" AND n.kind NOT IN ({})", known.join(", "))
    }
}

pub fn reminders(c: &Connection, users: &[i64]) -> rusqlite::Result<HashMap<i64, i64>> {
    let mut out = HashMap::new();
    for user in users {
        out.insert(*user, Prefs::load(c, *user)?.reminder);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_keep_everything_in_the_app_and_skip_card_added_emails() {
        let p = Prefs::from_settings(&crate::settings::defaults());
        assert!(KINDS.iter().all(|k| p.in_app(k)));
        assert!(p.by_email("mention") && !p.by_email("card_added"));
        assert_eq!((p.frequency, p.reminder), (Frequency::Instant, 86_400));
    }

    #[test]
    fn due_reminders_always_show_in_the_app() {
        let mut s = crate::settings::defaults();
        s.insert("notify_mention_app".into(), json!(false));
        s.insert("email_frequency".into(), json!("never"));
        let p = Prefs::from_settings(&s);
        assert_eq!(p.hidden(), ["mention"]);
        assert!(p.in_app("due") && !p.by_email("due"));
        assert_eq!(hidden_clause(&p.hidden()), " AND n.kind NOT IN ('mention')");
    }
}
