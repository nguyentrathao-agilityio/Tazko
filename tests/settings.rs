use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tazko::config::{self, DataLayout};

const CLIENT_SECRET: &str = "local-client-secret-for-tests";

fn tazko() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tazko"));
    for (k, _) in std::env::vars() {
        if k.starts_with("TAZKO_") {
            cmd.env_remove(k);
        }
    }
    cmd
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn write_env(dir: &Path, port: u16) -> std::path::PathBuf {
    let path = dir.join("tazko.env");
    fs::write(
        &path,
        format!(
            "TAZKO_HOST=127.0.0.1\nTAZKO_PORT={port}\nTAZKO_PUBLIC_URL=http://localhost:{port}\nTAZKO_DATA_DIR={}\nTAZKO_GOOGLE_CLIENT_ID=client.example\nTAZKO_GOOGLE_CLIENT_SECRET={CLIENT_SECRET}\nTAZKO_SECRET=\n",
            dir.join("data").display()
        ),
    )
    .unwrap();
    path
}

fn start(env: &Path) -> (Child, String) {
    let mut child = tazko()
        .args(["serve", "--env-file"])
        .arg(env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    (child, line)
}

fn stop(mut child: Child) -> String {
    child.kill().unwrap();
    child.wait().unwrap();
    let mut err = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .unwrap();
    err
}

fn wait_for(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "server did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn missing_required_values_stop_start_up_with_a_clear_message() {
    let dir = tempfile::tempdir().unwrap();
    let env = dir.path().join("tazko.env");
    fs::write(&env, "TAZKO_DATA_DIR=./data\nTAZKO_GOOGLE_CLIENT_SECRET=\n").unwrap();
    let out = tazko()
        .args(["serve", "--env-file"])
        .arg(&env)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("the settings are not complete"), "{err}");
    for key in [
        "TAZKO_PUBLIC_URL",
        "TAZKO_GOOGLE_CLIENT_ID",
        "TAZKO_GOOGLE_CLIENT_SECRET",
    ] {
        assert!(err.contains(key), "{key} missing from: {err}");
    }
    assert!(!err.contains("panicked"));
    assert!(!dir.path().join("data").exists());
}

#[test]
fn unreadable_settings_file_is_reported() {
    let out = tazko()
        .args(["serve", "--env-file", "/nonexistent/tazko.env"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("cannot read settings file")
    );
}

#[test]
fn start_up_creates_the_data_layout_and_a_private_secret_once() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let env = write_env(dir.path(), port);

    let (child, banner) = start(&env);
    wait_for(port);
    let err = stop(child);

    let data = dir.path().join("data");
    assert!(data.join("control.db").is_file());
    assert!(data.join("orgs").is_dir());
    assert!(data.join("trash").is_dir());
    let secret_path = data.join("secret");
    let mode = fs::metadata(&secret_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let secret = fs::read_to_string(&secret_path).unwrap();
    assert_eq!(secret.len(), 64);

    for text in [&banner, &err] {
        assert!(!text.contains(&secret), "secret printed: {text}");
        assert!(
            !text.contains(CLIENT_SECRET),
            "client secret printed: {text}"
        );
    }
    assert!(
        banner.contains(&format!("listening on http://127.0.0.1:{port}")),
        "{banner}"
    );

    let (child, _) = start(&env);
    wait_for(port);
    stop(child);
    assert_eq!(fs::read_to_string(&secret_path).unwrap(), secret);
}

#[test]
fn a_secret_file_readable_by_others_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let env = write_env(dir.path(), port);
    let data = dir.path().join("data");
    DataLayout::new(&data).create().unwrap();
    fs::write(data.join("secret"), "x".repeat(64)).unwrap();
    fs::set_permissions(data.join("secret"), fs::Permissions::from_mode(0o644)).unwrap();

    let out = tazko()
        .args(["serve", "--env-file"])
        .arg(&env)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("chmod 600"), "{err}");
    assert!(!err.contains(&"x".repeat(64)));
}

#[test]
fn tazko_secret_setting_replaces_the_secret_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut map = config::parse_env(&fs::read_to_string(write_env(dir.path(), 1)).unwrap());
    map.insert("TAZKO_SECRET".into(), "k".repeat(40));
    let settings = config::from_map(&map).unwrap();
    let layout = DataLayout::new(dir.path().join("data"));
    layout.create().unwrap();
    assert_eq!(
        config::server_secret(&settings, &layout).unwrap().expose(),
        "k".repeat(40)
    );
    assert!(!layout.secret_file.exists());
}

#[test]
fn creating_the_layout_twice_keeps_existing_data() {
    let dir = tempfile::tempdir().unwrap();
    let layout = DataLayout::new(dir.path().join("data"));
    layout.create().unwrap();
    fs::write(layout.orgs.join("keep.txt"), "x").unwrap();
    layout.create().unwrap();
    assert!(layout.orgs.join("keep.txt").exists());
    let mode = fs::metadata(&layout.root).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
}
