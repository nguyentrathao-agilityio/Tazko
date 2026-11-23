use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNT: AtomicU64 = AtomicU64::new(0);
const MESSAGE_LIMIT: usize = 5 * 1024 * 1024;

pub fn serve(listener: TcpListener, dir: PathBuf) {
    for stream in listener.incoming().flatten() {
        let dir = dir.clone();
        std::thread::spawn(move || {
            let _ = session(stream, &dir);
        });
    }
}

fn session(stream: TcpStream, dir: &Path) -> std::io::Result<()> {
    let mut out = stream.try_clone()?;
    let mut lines = BufReader::new(stream);
    let mut to = vec![];
    out.write_all(b"220 tazko-dev-mail ready\r\n")?;
    let mut line = String::new();
    loop {
        line.clear();
        if lines.read_line(&mut line)? == 0 {
            return Ok(());
        }

        let verb = line
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        let reply: &[u8] = match verb.as_str() {
            "EHLO" | "HELO" => b"250-tazko-dev-mail\r\n250-AUTH PLAIN LOGIN\r\n250 8BITMIME\r\n",
            "AUTH" => b"235 accepted\r\n",
            "MAIL" | "RSET" => {
                to.clear();
                b"250 ok\r\n"
            }
            "NOOP" => b"250 ok\r\n",
            "RCPT" => {
                to.push(line.trim().to_string());
                b"250 ok\r\n"
            }
            "DATA" => {
                out.write_all(b"354 send it\r\n")?;
                let mut data = String::new();
                loop {
                    line.clear();
                    if lines.read_line(&mut line)? == 0 {
                        return Ok(());
                    }

                    if line == ".\r\n" || line == ".\n" {
                        break;
                    }

                    if data.len() < MESSAGE_LIMIT {
                        data.push_str(line.strip_prefix('.').unwrap_or(&line));
                    }
                }
                let n = COUNT.fetch_add(1, Ordering::Relaxed);
                let name = format!("{}-{n}.eml", crate::session::now());
                std::fs::create_dir_all(dir)?;
                std::fs::write(
                    dir.join(name),
                    format!("X-Stub-Recipients: {}\r\n{data}", to.join(" ")),
                )?;
                b"250 saved\r\n"
            }
            "QUIT" => {
                out.write_all(b"221 bye\r\n")?;
                return Ok(());
            }
            _ => b"502 not supported\r\n",
        };
        out.write_all(reply)?;
    }
}
