use std::fmt::Write;

pub const MAX_INPUT: usize = 100_000;

const ALLOWED: &[&str] = &[
    "p",
    "br",
    "b",
    "strong",
    "i",
    "em",
    "u",
    "s",
    "strike",
    "del",
    "ul",
    "ol",
    "li",
    "h2",
    "h3",
    "blockquote",
    "code",
    "pre",
    "a",
    "div",
    "span",
];

const DROP_CONTENT: &[&str] = &[
    "script",
    "style",
    "template",
    "iframe",
    "noscript",
    "textarea",
    "title",
    "xmp",
    "object",
    "embed",
    "svg",
    "math",
    "frameset",
    "noembed",
    "plaintext",
    "head",
];

fn escape_text(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest[1..]
            .find(|c: char| c == ';' || c == '&' || c.is_whitespace())
            .map(|e| e + 1);
        let decoded = end
            .filter(|e| rest.as_bytes().get(*e) == Some(&b';'))
            .and_then(|e| {
                let name = &rest[1..e];
                let ch = match name {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" | "#39" => Some('\''),
                    "nbsp" => Some('\u{a0}'),
                    n if n.starts_with("#x") || n.starts_with("#X") => {
                        u32::from_str_radix(&n[2..], 16)
                            .ok()
                            .and_then(char::from_u32)
                    }
                    n if n.starts_with('#') => n[1..].parse::<u32>().ok().and_then(char::from_u32),
                    _ => None,
                };
                ch.map(|c| (c, e + 1))
            });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn safe_href(raw: &str) -> Option<String> {
    let decoded = decode_entities(raw);
    let cleaned: String = decoded
        .chars()
        .filter(|c| !c.is_control() && !c.is_whitespace())
        .collect();
    let lower = cleaned.to_ascii_lowercase();
    let ok = lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("mailto:");
    (ok && cleaned.len() <= 2000).then_some(cleaned)
}

struct Tag {
    name: String,
    closing: bool,
    self_closing: bool,
    href: Option<String>,
    end: usize,
}

fn parse_tag(s: &str, start: usize) -> Option<Tag> {
    let b = s.as_bytes();
    let mut i = start + 1;
    let closing = b.get(i) == Some(&b'/');
    if closing {
        i += 1;
    }

    let name_start = i;
    while i < b.len() && b[i].is_ascii_alphanumeric() {
        i += 1;
    }
    if i == name_start || !b[name_start].is_ascii_alphabetic() {
        return None;
    }

    let name = s[name_start..i].to_ascii_lowercase();
    let mut href = None;
    let mut self_closing = false;
    loop {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            if b[i] == b'/' {
                self_closing = true;
            }
            i += 1;
        }
        if i >= b.len() {
            return None;
        }

        if b[i] == b'>' {
            return Some(Tag {
                name,
                closing,
                self_closing,
                href,
                end: i + 1,
            });
        }

        self_closing = false;
        let an_start = i;
        while i < b.len()
            && !b[i].is_ascii_whitespace()
            && b[i] != b'='
            && b[i] != b'>'
            && b[i] != b'/'
        {
            i += 1;
        }
        let attr = s[an_start..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = None;
        if b.get(i) == Some(&b'=') {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            match b.get(i) {
                Some(&q) if q == b'"' || q == b'\'' => {
                    let close = s[i + 1..].find(q as char)? + i + 1;
                    value = Some(&s[i + 1..close]);
                    i = close + 1;
                }
                Some(_) => {
                    let v_start = i;
                    while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                        i += 1;
                    }
                    value = Some(&s[v_start..i]);
                }
                None => return None,
            }
        }
        if attr == "href" && name == "a" {
            href = value.and_then(safe_href);
        }
    }
}

fn skip_dropped(s: &str, name: &str, from: usize) -> usize {
    let lower = s[from..].to_ascii_lowercase();
    let needle = format!("</{name}");
    match lower.find(&needle) {
        Some(pos) => {
            let after = from + pos;
            s[after..].find('>').map_or(s.len(), |e| after + e + 1)
        }
        None => s.len(),
    }
}

pub fn clean_html(input: &str) -> String {
    let input = if input.len() > MAX_INPUT {
        let mut cut = MAX_INPUT;
        while !input.is_char_boundary(cut) {
            cut -= 1;
        }
        &input[..cut]
    } else {
        input
    };
    let mut out = String::with_capacity(input.len());
    let mut stack: Vec<String> = Vec::new();
    let mut text_start = 0;
    let mut i = 0;
    let flush = |out: &mut String, from: usize, to: usize| {
        if from < to {
            escape_text(out, &decode_entities(&input[from..to]));
        }
    };
    while let Some(off) = input[i..].find('<') {
        let at = i + off;
        if input[at..].starts_with("<!--") {
            flush(&mut out, text_start, at);
            let end = input[at + 4..]
                .find("-->")
                .map_or(input.len(), |e| at + 4 + e + 3);
            i = end;
            text_start = end;
            continue;
        }

        if input[at..].starts_with("<!") || input[at..].starts_with("<?") {
            flush(&mut out, text_start, at);
            let end = input[at..].find('>').map_or(input.len(), |e| at + e + 1);
            i = end;
            text_start = end;
            continue;
        }

        let Some(tag) = parse_tag(input, at) else {
            i = at + 1;
            continue;
        };
        flush(&mut out, text_start, at);
        i = tag.end;
        text_start = tag.end;
        if DROP_CONTENT.contains(&tag.name.as_str()) {
            if !tag.closing && !tag.self_closing {
                i = skip_dropped(input, &tag.name, tag.end);
                text_start = i;
            }
            continue;
        }

        if !ALLOWED.contains(&tag.name.as_str()) {
            continue;
        }

        if tag.name == "br" {
            if !tag.closing {
                out.push_str("<br>");
            }
            continue;
        }

        if tag.closing {
            if let Some(pos) = stack.iter().rposition(|t| *t == tag.name) {
                for t in stack.drain(pos..).rev() {
                    let _ = write!(out, "</{t}>");
                }
            }
            continue;
        }

        if stack.len() >= 64 {
            continue;
        }

        if tag.name == "a" {
            match &tag.href {
                Some(h) => {
                    out.push_str("<a href=\"");
                    escape_text(&mut out, h);
                    out.push_str("\" target=\"_blank\" rel=\"noopener noreferrer nofollow\">");
                }
                None => out.push_str("<a>"),
            }
        } else {
            let _ = write!(out, "<{}>", tag.name);
        }
        stack.push(tag.name);
    }
    flush(&mut out, text_start, input.len());
    for t in stack.into_iter().rev() {
        let _ = write!(out, "</{t}>");
    }
    out
}

pub fn markdown_to_html(md: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let mut raw = String::new();
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    html::push_html(&mut raw, Parser::new_ext(md, opts));
    let raw = raw
        .replace("<h1>", "<h2>")
        .replace("</h1>", "</h2>")
        .replace("<h4>", "<h3>")
        .replace("</h4>", "</h3>");
    clean_html(&raw)
}

pub fn plain_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode_entities(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_danger(s: &str) -> bool {
        let mut rest = s;
        while let Some(i) = rest.find('<') {
            let end = match rest[i..].find('>') {
                Some(e) => i + e,
                None => return true,
            };
            let tag = &rest[i + 1..end];
            let name = tag.trim_start_matches('/').split(' ').next().unwrap_or("");
            if !ALLOWED.contains(&name) {
                return true;
            }

            if tag.contains(' ') {
                let ok = tag.starts_with("a href=\"")
                    && tag.ends_with("\" target=\"_blank\" rel=\"noopener noreferrer nofollow\"")
                    && {
                        let href = &tag[8..tag.find("\" target").unwrap_or(8)];
                        let l = href.to_ascii_lowercase();
                        (l.starts_with("https://")
                            || l.starts_with("http://")
                            || l.starts_with("mailto:"))
                            && !href.contains('"')
                    };
                if !ok {
                    return true;
                }
            }
            rest = &rest[end + 1..];
        }
        false
    }

    #[test]
    fn keeps_the_editor_formatting() {
        let html = "<p><b>Bold</b> and <i>it</i> <s>gone</s></p><ul><li>one</li></ul><h3>Title</h3><blockquote>q</blockquote><p><code>x</code></p><a href=\"https://example.com/a?b=1&amp;c=2\">link</a>";
        assert_eq!(
            clean_html(html),
            "<p><b>Bold</b> and <i>it</i> <s>gone</s></p><ul><li>one</li></ul><h3>Title</h3><blockquote>q</blockquote><p><code>x</code></p><a href=\"https://example.com/a?b=1&amp;c=2\" target=\"_blank\" rel=\"noopener noreferrer nofollow\">link</a>"
        );
    }

    #[test]
    fn strips_scripts_and_event_handlers() {
        let attacks = [
            "<script>alert(1)</script>hello",
            "<SCRIPT SRC=//x.example/x.js></SCRIPT>",
            "<img src=x onerror=alert(1)>",
            "<p onclick=\"alert(1)\">click</p>",
            "<a href=\"javascript:alert(1)\">x</a>",
            "<a href=\"jav&#x09;ascript:alert(1)\">x</a>",
            "<a href=\" JaVaScRiPt:alert(1)\">x</a>",
            "<a href=\"data:text/html;base64,PHNjcmlwdD4=\">x</a>",
            "<svg onload=alert(1)><circle/></svg>",
            "<iframe src=\"https://x.example\"></iframe>",
            "<style>body{display:none}</style>",
            "<p style=\"background:url(javascript:alert(1))\">x</p>",
            "<b onmouseover=alert(1)>x</b>",
            "<<script>script>alert(1)<</script>/script>",
            "<scr<script>ipt>alert(1)</script>",
            "<!--<script>alert(1)</script>-->",
            "<div><textarea><script>alert(1)</script></textarea></div>",
            "<a href=https://ok.example onclick=alert(1)>x</a>",
            "&lt;script&gt;alert(1)&lt;/script&gt;",
            "<math><mi xlink:href=\"javascript:alert(1)\">x</mi></math>",
            "<p>unterminated <b onclick=x",
        ];
        for a in attacks {
            let out = clean_html(a);
            assert!(!has_danger(&out), "{a} -> {out}");
            assert!(!out.contains("<script"), "{a} -> {out}");
        }
        assert_eq!(
            clean_html("<p onclick=\"alert(1)\">click</p>"),
            "<p>click</p>"
        );
        assert_eq!(
            clean_html("<a href=\"javascript:alert(1)\">x</a>"),
            "<a>x</a>"
        );
        assert_eq!(clean_html("&lt;script&gt;"), "&lt;script&gt;");
    }

    #[test]
    fn closes_what_it_opens() {
        assert_eq!(clean_html("<b><i>x</b>y"), "<b><i>x</i></b>y");
        assert_eq!(clean_html("</p>text<ul><li>a"), "text<ul><li>a</li></ul>");
    }

    #[test]
    fn markdown_becomes_clean_html() {
        assert_eq!(
            markdown_to_html("# Plan\n\n**Bold** and [site](https://example.com)\n\n- a\n- b"),
            "<h2>Plan</h2>\n<p><b>Bold</b> and <a href=\"https://example.com\" target=\"_blank\" rel=\"noopener noreferrer nofollow\">site</a></p>\n<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n"
                .replace("<b>", "<strong>")
                .replace("</b>", "</strong>")
        );
        let out = markdown_to_html(
            "[x](javascript:alert(1)) <script>alert(1)</script> <img src=x onerror=alert(1)>",
        );
        assert!(!has_danger(&out), "{out}");
    }

    #[test]
    fn plain_text_drops_tags() {
        assert_eq!(
            plain_text("<p>Hello <b>you</b></p><p>&amp; me</p>"),
            "Hello you & me"
        );
    }
}
