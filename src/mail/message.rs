use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::admin::validate;
use crate::session::random_token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mailbox {
    pub name: Option<String>,
    pub addr: String,
}

impl Mailbox {
    pub fn parse(raw: &str) -> Option<Mailbox> {
        let raw = raw.trim();
        match raw.rsplit_once('<') {
            Some((name, rest)) => {
                let addr = validate::email(rest.strip_suffix('>')?)?;
                let name = clean(name.trim().trim_matches('"'));
                Some(Mailbox {
                    name: (!name.is_empty()).then_some(name),
                    addr,
                })
            }
            None => Some(Mailbox {
                name: None,
                addr: validate::email(raw)?,
            }),
        }
    }

    pub fn domain(&self) -> &str {
        self.addr.rsplit_once('@').map_or("localhost", |(_, d)| d)
    }

    pub fn header(&self) -> String {
        match &self.name {
            Some(name) => format!("{} <{}>", encode_word(name), self.addr),
            None => self.addr.clone(),
        }
    }
}

pub fn clean(text: &str) -> String {
    clean_to(text, 200)
}

pub fn name(text: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = clean_to(&text.replace("://", " "), 200).chars().collect();
    for (i, c) in chars.iter().enumerate() {
        out.push(*c);
        let joins = |j: Option<&char>| j.is_some_and(|c| c.is_alphanumeric());
        if *c == '.'
            && joins(i.checked_sub(1).and_then(|p| chars.get(p)))
            && joins(chars.get(i + 1))
        {
            out.push(' ');
        }
    }
    out.trim_end()
        .chars()
        .take(60)
        .collect::<String>()
        .trim_end()
        .to_string()
}

fn clean_to(text: &str, limit: usize) -> String {
    let mut out = String::new();
    for word in text.split(|c: char| c.is_whitespace() || c.is_control()) {
        if word.is_empty() {
            continue;
        }

        if !out.is_empty() {
            out.push(' ');
        }

        out.push_str(word);
    }
    out.chars().take(limit).collect()
}

fn plain(text: &str) -> bool {
    text.chars().all(|c| c.is_ascii_graphic() || c == ' ') && !text.contains("=?")
}

fn encode_text(text: &str) -> String {
    if plain(text) {
        text.to_string()
    } else {
        encoded(text)
    }
}

fn encode_word(text: &str) -> String {
    if plain(text) && !text.contains(['"', '\\', '(', ')', '<', '>', ',', ';', ':', '@', '[', ']'])
    {
        return text.to_string();
    }

    encoded(text)
}

fn encoded(text: &str) -> String {
    let mut words = vec![];
    let mut chunk = String::new();
    for c in text.chars() {
        if chunk.len() + c.len_utf8() > 39 {
            words.push(std::mem::take(&mut chunk));
        }

        chunk.push(c);
    }
    words.push(chunk);
    words
        .iter()
        .map(|w| format!("=?UTF-8?B?{}?=", STANDARD.encode(w)))
        .collect::<Vec<_>>()
        .join("\r\n ")
}

fn base64_lines(body: &str) -> String {
    let encoded = STANDARD.encode(body);
    encoded
        .as_bytes()
        .chunks(76)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join("\r\n")
}

pub fn rfc2822_date(unix: i64) -> String {
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    let (year, month, day) = crate::cards::repeat::civil_from_days(days);
    format!(
        "{}, {:02} {} {} {:02}:{:02}:{:02} +0000",
        DAYS[days.rem_euclid(7) as usize],
        day,
        MONTHS[(month - 1) as usize],
        year,
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

pub struct Message<'a> {
    pub from: &'a Mailbox,
    pub to: &'a str,
    pub subject: &'a str,
    pub text: &'a str,
    pub html: &'a str,
    pub date: i64,
}

impl Message<'_> {
    pub fn render(&self) -> Option<String> {
        let to = validate::email(self.to)?;
        let boundary = format!("tazko-{}", &random_token()[..24]);
        let id = format!("<{}@{}>", &random_token()[..32], self.from.domain());
        let mut out = String::new();
        for (name, value) in [
            ("From", self.from.header()),
            ("To", to),
            ("Subject", encode_text(&clean(self.subject))),
            ("Date", rfc2822_date(self.date)),
            ("Message-ID", id),
            ("MIME-Version", "1.0".into()),
            ("Auto-Submitted", "auto-generated".into()),
            (
                "Content-Type",
                format!("multipart/alternative; boundary=\"{boundary}\""),
            ),
        ] {
            out.push_str(&format!("{name}: {value}\r\n"));
        }
        out.push_str("\r\n");
        for (kind, body) in [("text/plain", self.text), ("text/html", self.html)] {
            out.push_str(&format!(
                "--{boundary}\r\nContent-Type: {kind}; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n",
                base64_lines(body)
            ));
        }
        out.push_str(&format!("--{boundary}--\r\n"));
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn senders_parse_with_or_without_a_name() {
        assert_eq!(
            Mailbox::parse("Tazko <No-Reply@Example.com>"),
            Some(Mailbox {
                name: Some("Tazko".into()),
                addr: "no-reply@example.com".into()
            })
        );
        assert_eq!(Mailbox::parse("a@example.com").unwrap().name, None);
        assert!(Mailbox::parse("Tazko <bad>").is_none());
        assert!(Mailbox::parse("x@example.com\r\nBcc: y@example.com").is_none());
    }

    #[test]
    fn header_values_cannot_start_new_headers() {
        let from = Mailbox::parse("Tazko <no-reply@example.com>").unwrap();
        let msg = Message {
            from: &from,
            to: "mai@example.com",
            subject: "Hi\r\nBcc: evil@example.com\nX: y",
            text: "body",
            html: "<p>body</p>",
            date: 0,
        }
        .render()
        .unwrap();
        let head = msg.split("\r\n\r\n").next().unwrap();
        assert!(!head.contains("\r\nBcc:") && !head.contains("\nX:"));
        assert!(head.contains("Subject: Hi Bcc: evil@example.com X: y"));
        assert!(
            Message {
                to: "a@b.com\r\nBcc: c@d.com",
                ..Message {
                    from: &from,
                    to: "",
                    subject: "",
                    text: "",
                    html: "",
                    date: 0
                }
            }
            .render()
            .is_none()
        );
    }

    #[test]
    fn names_cannot_look_like_links() {
        assert_eq!(
            name("Verify at pay-tazko-login.com"),
            "Verify at pay-tazko-login. com"
        );
        assert_eq!(name("https://evil.example/x"), "https evil. example/x");
        assert_eq!(name("Acme Inc."), "Acme Inc.");
        assert_eq!(name(&"x".repeat(100)).len(), 60);
    }

    #[test]
    fn long_encoded_headers_are_folded() {
        let long = encoded(&"ố".repeat(60));
        assert!(long.split("\r\n ").all(|line| line.len() <= 76));
        assert!(long.contains("\r\n "));
    }

    #[test]
    fn non_ascii_subjects_are_encoded_and_dates_formatted() {
        assert_eq!(
            encode_word("Mời bạn"),
            format!("=?UTF-8?B?{}?=", STANDARD.encode("Mời bạn"))
        );
        assert_eq!(
            encode_word("Tazko: invite"),
            format!("=?UTF-8?B?{}?=", STANDARD.encode("Tazko: invite"))
        );
        assert_eq!(rfc2822_date(0), "Thu, 01 Jan 1970 00:00:00 +0000");
        assert_eq!(
            rfc2822_date(1_763_600_000),
            "Thu, 20 Nov 2025 00:53:20 +0000"
        );
    }
}
