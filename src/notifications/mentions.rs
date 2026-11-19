use rusqlite::Connection;

pub struct Person {
    pub id: i64,
    pub handles: Vec<String>,
}

pub fn handles(name: &str, email: &str) -> Vec<String> {
    let mut out = Vec::new();
    let name = name.trim().to_lowercase();
    if name.chars().count() >= 2 {
        out.push(name);
    }

    let email = email.trim().to_lowercase();
    if let Some((local, _)) = email.split_once('@') {
        if local.chars().count() >= 2 {
            out.push(local.to_string());
        }

        out.push(email.clone());
    }
    out
}

pub fn people(c: &Connection) -> rusqlite::Result<Vec<Person>> {
    let mut stmt = c.prepare("SELECT id, name, email FROM people")?;
    stmt.query_map([], |r| {
        let name: String = r.get(1)?;
        let email: String = r.get(2)?;
        Ok(Person {
            id: r.get(0)?,
            handles: handles(&name, &email),
        })
    })?
    .collect()
}

fn ends_word(rest: &str) -> bool {
    match rest.chars().next() {
        None => true,
        Some(ch) => !(ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '@'),
    }
}

pub fn find(people: &[Person], text: &str) -> Vec<i64> {
    let lower = text.to_lowercase();
    let mut found = Vec::new();
    for (at, _) in lower.match_indices('@') {
        if lower[..at]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_alphanumeric() || ch == '.' || ch == '_')
        {
            continue;
        }

        let after = &lower[at + 1..];
        let hits: Vec<(i64, usize)> = people
            .iter()
            .flat_map(|p| p.handles.iter().map(move |h| (p.id, h)))
            .filter(|(_, h)| after.starts_with(h.as_str()) && ends_word(&after[h.len()..]))
            .map(|(id, h)| (id, h.len()))
            .collect();
        let Some(longest) = hits.iter().map(|(_, n)| *n).max() else {
            continue;
        };
        let mut ids: Vec<i64> = hits
            .iter()
            .filter(|(_, n)| *n == longest)
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        if let [id] = ids[..]
            && !found.contains(&id)
        {
            found.push(id);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: i64, name: &str, email: &str) -> Person {
        Person {
            id,
            handles: handles(name, email),
        }
    }

    #[test]
    fn mentions_match_full_names_and_emails_at_word_edges() {
        let people = [
            p(1, "Mai Tran", "mai@a.example"),
            p(2, "Mai", "mai.le@a.example"),
            p(3, "Trung Ngo", "trung@a.example"),
            p(4, "", "x@a.example"),
        ];
        assert_eq!(find(&people, "@Mai Tran can you check?"), [1]);
        assert_eq!(find(&people, "thanks @mai.le, and @TRUNG NGO."), [2, 3]);
        assert!(find(&people, "@mai could be either person").is_empty());
        assert_eq!(find(&people, "ping @mai.le@a.example"), [2]);
        assert_eq!(find(&people, "@trung @trung"), [3]);
        assert!(find(&people, "mail@trung.example @trungx @x").is_empty());
        assert_eq!(find(&people, "(@Mai Tran)"), [1]);
    }

    #[test]
    fn accented_names_are_matched() {
        let people = [p(5, "Nguyễn Văn Đức", "duc@a.example")];
        assert_eq!(find(&people, "nhờ @Nguyễn Văn Đức xem"), [5]);
    }
}
