use std::io::{self, Read, Write};
use std::net::{IpAddr, TcpStream, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, StreamOwned};

use super::message::Mailbox;
use crate::config::Smtp;

const TIMEOUT: Duration = Duration::from_secs(30);
const LINE_LIMIT: usize = 4096;
const REPLY_LINES: usize = 100;
const SESSION: Duration = Duration::from_secs(120);

#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    Temporary(String),
    Permanent(String),
}

trait Stream: Read + Write {}
impl<T: Read + Write> Stream for T {}

struct Conn {
    stream: Box<dyn Stream>,
    buf: Vec<u8>,
    deadline: Instant,
}

fn io_failure(e: io::Error) -> Failure {
    Failure::Temporary(format!("connection problem: {}", e.kind()))
}

impl Conn {
    fn line(&mut self) -> Result<String, Failure> {
        loop {
            if Instant::now() > self.deadline {
                return Err(Failure::Temporary("the mail server took too long".into()));
            }

            if let Some(end) = self.buf.windows(2).position(|w| w == b"\r\n") {
                let line: Vec<u8> = self.buf.drain(..end + 2).take(end).collect();
                return Ok(String::from_utf8_lossy(&line).into_owned());
            }

            if self.buf.len() > LINE_LIMIT {
                return Err(Failure::Temporary(
                    "the mail server sent a line that is too long".into(),
                ));
            }

            let mut chunk = [0u8; 1024];
            let n = self.stream.read(&mut chunk).map_err(io_failure)?;
            if n == 0 {
                return Err(Failure::Temporary(
                    "the mail server closed the connection".into(),
                ));
            }

            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    fn reply(&mut self) -> Result<(u16, Vec<String>), Failure> {
        let mut lines = vec![];
        loop {
            let line = self.line()?;
            let code = line.get(..3).and_then(|c| c.parse::<u16>().ok());
            let Some(code) = code else {
                return Err(Failure::Temporary(
                    "the mail server sent an unreadable reply".into(),
                ));
            };
            let last = line.as_bytes().get(3) != Some(&b'-');
            lines.push(line.get(4..).unwrap_or("").to_string());
            if lines.len() > REPLY_LINES {
                return Err(Failure::Temporary(
                    "the mail server sent too long a reply".into(),
                ));
            }

            if last {
                return Ok((code, lines));
            }
        }
    }

    fn expect(&mut self, step: &str, ok: &[u16]) -> Result<Vec<String>, Failure> {
        let (code, lines) = self.reply()?;
        if ok.contains(&code) {
            return Ok(lines);
        }

        let text: String = format!("{step}: {code} {}", lines.join(" "))
            .chars()
            .take(300)
            .collect();
        let about_recipient = matches!(step, "recipient" | "message");
        Err(if code >= 500 && about_recipient {
            Failure::Permanent(text)
        } else {
            Failure::Temporary(text)
        })
    }

    fn send(&mut self, line: &str) -> Result<(), Failure> {
        self.stream.write_all(line.as_bytes()).map_err(io_failure)?;
        self.stream.write_all(b"\r\n").map_err(io_failure)?;
        self.stream.flush().map_err(io_failure)
    }

    fn command(&mut self, step: &str, line: &str, ok: &[u16]) -> Result<Vec<String>, Failure> {
        self.send(line)?;
        self.expect(step, ok)
    }
}

fn tls_config() -> Result<Arc<ClientConfig>, Failure> {
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    if let Some(config) = CONFIG.get() {
        return Ok(config.clone());
    }

    let _ = rustls::crypto::ring::default_provider().install_default();
    let config =
        <ClientConfig as rustls_platform_verifier::ConfigVerifierExt>::with_platform_verifier()
            .map_err(|_| {
                Failure::Temporary("could not load the system's trusted certificates".into())
            })?;
    Ok(CONFIG.get_or_init(|| Arc::new(config)).clone())
}

fn wrap(tcp: TcpStream, host: &str) -> Result<Box<dyn Stream>, Failure> {
    let name = ServerName::try_from(host.to_string())
        .map_err(|_| Failure::Temporary("the mail server name is not valid for TLS".into()))?;
    let conn = ClientConnection::new(tls_config()?, name)
        .map_err(|e| Failure::Temporary(format!("TLS setup failed: {e}")))?;
    Ok(Box::new(StreamOwned::new(conn, tcp)))
}

fn loopback(host: &str) -> bool {
    host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

fn connect(host: &str, port: u16) -> Result<TcpStream, Failure> {
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|_| Failure::Temporary(format!("could not find the mail server {host}")))?;
    let mut last = Failure::Temporary(format!("could not find the mail server {host}"));
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, TIMEOUT) {
            Ok(tcp) => {
                tcp.set_read_timeout(Some(TIMEOUT)).map_err(io_failure)?;
                tcp.set_write_timeout(Some(TIMEOUT)).map_err(io_failure)?;
                return Ok(tcp);
            }
            Err(e) => last = io_failure(e),
        }
    }
    Err(last)
}

fn has(caps: &[String], word: &str) -> bool {
    caps.iter().any(|c| {
        c.split_whitespace()
            .next()
            .is_some_and(|w| w.eq_ignore_ascii_case(word))
    })
}

fn auth_methods(caps: &[String]) -> Vec<String> {
    caps.iter()
        .filter(|c| c.get(..4).is_some_and(|w| w.eq_ignore_ascii_case("AUTH")))
        .flat_map(|c| {
            c.get(4..)
                .unwrap_or("")
                .split_whitespace()
                .map(|m| m.to_ascii_uppercase())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn dot_stuffed(data: &str) -> String {
    let mut out = String::with_capacity(data.len() + 64);
    for line in data.split("\r\n") {
        if line.starts_with('.') {
            out.push('.');
        }

        out.push_str(line);
        out.push_str("\r\n");
    }
    out
}

pub fn send(cfg: &Smtp, to: &str, data: &str) -> Result<(), Failure> {
    let from: &Mailbox = &cfg.from;
    if to.contains(['\r', '\n', '<', '>']) {
        return Err(Failure::Permanent(
            "the recipient address contains characters that are not allowed".into(),
        ));
    }

    let tcp = connect(&cfg.host, cfg.port)?;
    let implicit = cfg.port == 465;
    let stream: Box<dyn Stream> = if implicit {
        wrap(tcp.try_clone().map_err(io_failure)?, &cfg.host)?
    } else {
        Box::new(tcp.try_clone().map_err(io_failure)?)
    };
    let deadline = Instant::now() + SESSION;
    let mut conn = Conn {
        stream,
        buf: vec![],
        deadline,
    };
    conn.expect("greeting", &[220])?;
    let helo = format!("EHLO {}", from.domain());
    let mut caps = conn.command("EHLO", &helo, &[250])?;
    if !implicit {
        if has(&caps, "STARTTLS") {
            conn.command("STARTTLS", "STARTTLS", &[220])?;
            conn = Conn {
                stream: wrap(tcp, &cfg.host)?,
                buf: vec![],
                deadline,
            };
            caps = conn.command("EHLO", &helo, &[250])?;
        } else if !loopback(&cfg.host) {
            return Err(Failure::Temporary(
                "the mail server does not offer STARTTLS, so nothing was sent".into(),
            ));
        }
    }

    if let (Some(user), Some(password)) = (&cfg.user, &cfg.password) {
        let methods = auth_methods(&caps);
        if methods.iter().any(|m| m == "PLAIN") {
            let token = STANDARD.encode(format!("\0{user}\0{}", password.expose()));
            conn.command("sign-in", &format!("AUTH PLAIN {token}"), &[235])?;
        } else if methods.iter().any(|m| m == "LOGIN") {
            conn.command("sign-in", "AUTH LOGIN", &[334])?;
            conn.command("sign-in", &STANDARD.encode(user), &[334])?;
            conn.command("sign-in", &STANDARD.encode(password.expose()), &[235])?;
        } else {
            return Err(Failure::Temporary(
                "the mail server offers no sign-in method Tazko supports (PLAIN or LOGIN)".into(),
            ));
        }
    }

    conn.command("sender", &format!("MAIL FROM:<{}>", from.addr), &[250])?;
    conn.command("recipient", &format!("RCPT TO:<{to}>"), &[250, 251])?;
    conn.command("message", "DATA", &[354])?;
    conn.stream
        .write_all(dot_stuffed(data.trim_end_matches("\r\n")).as_bytes())
        .map_err(io_failure)?;
    conn.command("message", ".", &[250])?;
    let _ = conn.send("QUIT");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_starting_with_a_dot_are_doubled() {
        assert_eq!(dot_stuffed("a\r\n.b\r\n..c"), "a\r\n..b\r\n...c\r\n");
    }

    #[test]
    fn capabilities_are_read_case_insensitively() {
        let caps = vec![
            "mail.example.com".to_string(),
            "starttls".into(),
            "AUTH login PLAIN".into(),
        ];
        assert!(has(&caps, "STARTTLS"));
        assert_eq!(auth_methods(&caps), ["LOGIN", "PLAIN"]);
        assert!(auth_methods(&["abcé x".to_string()]).is_empty());
        assert!(loopback("127.0.0.1") && loopback("localhost") && !loopback("smtp.example.com"));
    }
}
