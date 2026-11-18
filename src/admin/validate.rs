use crate::db::valid_slug;

pub fn email(raw: &str) -> Option<String> {
    let e = raw.trim().to_lowercase();
    let (local, domain) = e.split_once('@')?;
    let ok = !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains('@')
        && e.len() <= 254
        && !e.chars().any(|c| {
            c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | ',' | ';' | '"')
        });
    ok.then_some(e)
}

pub fn slug_from(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }

    let mut out: String = out.trim_end_matches('-').chars().take(40).collect();
    while out.ends_with('-') {
        out.pop();
    }

    out
}

pub fn slug(raw: Option<&str>, name: &str) -> Option<String> {
    let s = match raw.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => s.to_string(),
        None => slug_from(name),
    };
    valid_slug(&s).then_some(s)
}

pub fn name(raw: &str) -> Option<String> {
    let n = raw.trim();
    (!n.is_empty() && n.chars().count() <= 80 && !n.chars().any(char::is_control))
        .then(|| n.to_string())
}

pub const GB: i64 = 1024 * 1024 * 1024;

pub fn storage_gb(raw: Option<i64>) -> Option<i64> {
    match raw {
        None => Some(2),
        Some(g) if (1..=10_000).contains(&g) => Some(g),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails() {
        assert_eq!(
            email(" Mai@Example.COM ").as_deref(),
            Some("mai@example.com")
        );
        for bad in [
            "",
            "a",
            "a@b",
            "@b.com",
            "a@.com",
            "a b@c.com",
            "a@b.com,c@d.com",
            "a@b@c.com",
        ] {
            assert_eq!(email(bad), None, "{bad}");
        }
    }

    #[test]
    fn slugs() {
        assert_eq!(slug_from("Acme Studio!"), "acme-studio");
        assert_eq!(slug_from("  --Hà Nội Team--  "), "h-n-i-team");
        assert_eq!(slug(Some("acme"), "x").as_deref(), Some("acme"));
        assert_eq!(slug(Some("../x"), "x"), None);
        assert_eq!(slug(None, "!!!"), None);
        assert_eq!(slug_from(&"a".repeat(60)).len(), 40);
    }
}
