use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};

pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 8791;
pub const DEFAULT_MAX_UPLOAD_MB: u32 = 10;
pub const SYSTEM_ENV_FILE: &str = "/etc/tazko/tazko.env";
pub const LOCAL_ENV_FILE: &str = ".env.local";
const MIN_SECRET_LEN: usize = 32;

const REQUIRED: [&str; 4] = [
    "TAZKO_PUBLIC_URL",
    "TAZKO_DATA_DIR",
    "TAZKO_GOOGLE_CLIENT_ID",
    "TAZKO_GOOGLE_CLIENT_SECRET",
];

#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[hidden]")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Smtp {
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    pub password: Option<Secret>,
    pub from: crate::mail::message::Mailbox,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub host: IpAddr,
    pub port: u16,
    pub public_url: String,
    pub data_dir: PathBuf,
    pub google_client_id: String,
    pub google_client_secret: Secret,
    pub secret: Option<Secret>,
    pub max_upload_mb: u32,
    pub smtp: Option<Smtp>,
    pub webhooks_allow_local: bool,
    pub source: Option<PathBuf>,
}

#[derive(Debug)]
pub enum ConfigError {
    Read(PathBuf, io::Error),
    Invalid(Vec<String>),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Read(path, e) => {
                write!(f, "cannot read settings file {}: {e}", path.display())
            }
            ConfigError::Invalid(problems) => {
                writeln!(f, "the settings are not complete:")?;
                for p in problems {
                    writeln!(f, "  - {p}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ConfigError {}

pub fn parse_env(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        let value = ['"', '\'']
            .iter()
            .find_map(|q| value.strip_prefix(*q).and_then(|v| v.strip_suffix(*q)))
            .unwrap_or(value);
        map.insert(key.trim().to_string(), value.to_string());
    }

    map
}

pub fn find_env_file(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }

    if let Some(p) = std::env::var_os("TAZKO_ENV_FILE").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(p));
    }

    [LOCAL_ENV_FILE, SYSTEM_ENV_FILE]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

pub fn load(explicit: Option<&Path>) -> Result<Settings, ConfigError> {
    let source = find_env_file(explicit);
    let mut map = match &source {
        Some(path) => {
            let text = fs::read_to_string(path).map_err(|e| ConfigError::Read(path.clone(), e))?;
            parse_env(&text)
        }
        None => BTreeMap::new(),
    };
    for (k, v) in std::env::vars() {
        if k.starts_with("TAZKO_") && k != "TAZKO_ENV_FILE" {
            map.insert(k, v);
        }
    }

    let mut settings = from_map(&map)?;
    settings.source = source;
    Ok(settings)
}

pub fn from_map(map: &BTreeMap<String, String>) -> Result<Settings, ConfigError> {
    let get = |k: &str| map.get(k).map(|v| v.trim()).filter(|v| !v.is_empty());
    let mut problems = Vec::new();

    for key in REQUIRED {
        if get(key).is_none() {
            problems.push(format!("{key} is missing or empty"));
        }
    }

    let host = match get("TAZKO_HOST").unwrap_or(DEFAULT_HOST).parse::<IpAddr>() {
        Ok(h) => h,
        Err(_) => {
            problems.push("TAZKO_HOST must be an IP address such as 127.0.0.1".into());
            DEFAULT_HOST.parse().unwrap()
        }
    };
    let port = number(get("TAZKO_PORT"), DEFAULT_PORT, "TAZKO_PORT", &mut problems);
    let max_upload_mb = number(
        get("TAZKO_MAX_UPLOAD_MB"),
        DEFAULT_MAX_UPLOAD_MB,
        "TAZKO_MAX_UPLOAD_MB",
        &mut problems,
    );
    if max_upload_mb == 0 {
        problems.push("TAZKO_MAX_UPLOAD_MB must be at least 1".into());
    }

    let public_url = get("TAZKO_PUBLIC_URL")
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string();
    if !public_url.is_empty()
        && !(public_url.starts_with("http://") || public_url.starts_with("https://"))
    {
        problems.push("TAZKO_PUBLIC_URL must start with http:// or https://".into());
    }

    let secret = get("TAZKO_SECRET").map(Secret::new);
    if secret
        .as_ref()
        .is_some_and(|s| s.expose().len() < MIN_SECRET_LEN)
    {
        problems.push(format!(
            "TAZKO_SECRET must be at least {MIN_SECRET_LEN} characters, or empty to use the secret file"
        ));
    }

    let smtp = smtp(&get, &mut problems);

    if !problems.is_empty() {
        return Err(ConfigError::Invalid(problems));
    }

    Ok(Settings {
        host,
        port,
        public_url,
        data_dir: PathBuf::from(get("TAZKO_DATA_DIR").unwrap()),
        google_client_id: get("TAZKO_GOOGLE_CLIENT_ID").unwrap().to_string(),
        google_client_secret: Secret::new(get("TAZKO_GOOGLE_CLIENT_SECRET").unwrap()),
        secret,
        max_upload_mb,
        smtp,
        webhooks_allow_local: matches!(
            get("TAZKO_WEBHOOKS_ALLOW_LOCAL"),
            Some("1" | "true" | "yes")
        ),
        source: None,
    })
}

fn number<T: std::str::FromStr>(
    value: Option<&str>,
    default: T,
    key: &str,
    problems: &mut Vec<String>,
) -> T {
    match value {
        None => default,
        Some(v) => v.parse().unwrap_or_else(|_| {
            problems.push(format!("{key} must be a whole number"));
            default
        }),
    }
}

fn smtp<'a>(get: &impl Fn(&str) -> Option<&'a str>, problems: &mut Vec<String>) -> Option<Smtp> {
    let keys = [
        "TAZKO_SMTP_HOST",
        "TAZKO_SMTP_PORT",
        "TAZKO_SMTP_USER",
        "TAZKO_SMTP_PASSWORD",
        "TAZKO_MAIL_FROM",
    ];
    if keys.iter().all(|k| get(k).is_none()) {
        return None;
    }

    let mut missing = false;
    for key in ["TAZKO_SMTP_HOST", "TAZKO_MAIL_FROM"] {
        if get(key).is_none() {
            problems.push(format!("{key} is needed when email is set up"));
            missing = true;
        }
    }

    let port = number(get("TAZKO_SMTP_PORT"), 587u16, "TAZKO_SMTP_PORT", problems);
    if get("TAZKO_SMTP_USER").is_some() != get("TAZKO_SMTP_PASSWORD").is_some() {
        problems.push("TAZKO_SMTP_USER and TAZKO_SMTP_PASSWORD must be set together".into());
        missing = true;
    }

    let from = get("TAZKO_MAIL_FROM").and_then(crate::mail::message::Mailbox::parse);
    if get("TAZKO_MAIL_FROM").is_some() && from.is_none() {
        problems
            .push("TAZKO_MAIL_FROM must be an address like Tazko <no-reply@example.com>".into());
    }

    let (false, Some(from)) = (missing, from) else {
        return None;
    };

    Some(Smtp {
        host: get("TAZKO_SMTP_HOST").unwrap().to_string(),
        port,
        user: get("TAZKO_SMTP_USER").map(str::to_string),
        password: get("TAZKO_SMTP_PASSWORD").map(Secret::new),
        from,
    })
}

#[derive(Debug, Clone)]
pub struct DataLayout {
    pub root: PathBuf,
    pub control_db: PathBuf,
    pub orgs: PathBuf,
    pub trash: PathBuf,
    pub secret_file: PathBuf,
}

impl DataLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            control_db: root.join("control.db"),
            orgs: root.join("orgs"),
            trash: root.join("trash"),
            secret_file: root.join("secret"),
            root,
        }
    }

    pub fn create(&self) -> io::Result<()> {
        for dir in [&self.root, &self.orgs, &self.trash] {
            create_private_dir(dir)?;
        }

        Ok(())
    }
}

fn create_private_dir(dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }

    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}

pub fn server_secret(settings: &Settings, layout: &DataLayout) -> io::Result<Secret> {
    if let Some(s) = &settings.secret {
        return Ok(s.clone());
    }

    let path = &layout.secret_file;
    match read_secret_file(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == io::ErrorKind::NotFound => create_secret_file(path),
        Err(e) => Err(e),
    }
}

fn read_secret_file(path: &Path) -> io::Result<Secret> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)?.permissions().mode();
        if mode & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "{} must be readable by its owner only (chmod 600)",
                    path.display()
                ),
            ));
        }
    }

    let value = fs::read_to_string(path)?.trim().to_string();
    if value.len() < MIN_SECRET_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} is shorter than {MIN_SECRET_LEN} characters",
                path.display()
            ),
        ));
    }

    Ok(Secret::new(value))
}

fn create_secret_file(path: &Path) -> io::Result<Secret> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    let value = crate::session::hex(&bytes);

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = match options.open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return read_secret_file(path),
        Err(e) => return Err(e),
    };
    file.write_all(value.as_bytes())?;
    file.sync_all()?;
    Ok(Secret::new(value))
}

pub fn startup_message(settings: &Settings) -> String {
    let source = settings.source.as_ref().map_or_else(
        || "environment only".to_string(),
        |p| p.display().to_string(),
    );
    let email = if settings.smtp.is_some() { "on" } else { "off" };
    format!(
        "tazko {} listening on http://{}:{} · public URL {} · data {} · settings {} · email {}",
        env!("CARGO_PKG_VERSION"),
        settings.host,
        settings.port,
        settings.public_url,
        settings.data_dir.display(),
        source,
        email
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> BTreeMap<String, String> {
        parse_env(
            "TAZKO_PUBLIC_URL=http://localhost:8791\nTAZKO_DATA_DIR=./data\nTAZKO_GOOGLE_CLIENT_ID=id.example\nTAZKO_GOOGLE_CLIENT_SECRET=client-secret-value\n",
        )
    }

    fn problems(map: &BTreeMap<String, String>) -> Vec<String> {
        match from_map(map) {
            Err(ConfigError::Invalid(p)) => p,
            other => panic!("expected invalid settings, got {other:?}"),
        }
    }

    #[test]
    fn parses_comments_quotes_and_export() {
        let map = parse_env("# comment\n\nexport A=1\nB = \"two words\"\nC='x'\nD=\nnot a pair\n");
        assert_eq!(map["A"], "1");
        assert_eq!(map["B"], "two words");
        assert_eq!(map["C"], "x");
        assert_eq!(map["D"], "");
        assert_eq!(map.len(), 4);
    }

    #[test]
    fn complete_settings_use_defaults() {
        let s = from_map(&base()).unwrap();
        assert_eq!(s.host.to_string(), "127.0.0.1");
        assert_eq!(s.port, 8791);
        assert_eq!(s.max_upload_mb, 10);
        assert!(s.smtp.is_none());
        assert!(s.secret.is_none());
    }

    #[test]
    fn every_missing_required_value_is_listed() {
        let p = problems(&BTreeMap::new());
        for key in REQUIRED {
            assert!(
                p.iter().any(|m| m.contains(key)),
                "{key} not reported in {p:?}"
            );
        }
    }

    #[test]
    fn empty_required_value_counts_as_missing() {
        let mut map = base();
        map.insert("TAZKO_GOOGLE_CLIENT_ID".into(), "  ".into());
        assert_eq!(
            problems(&map),
            vec!["TAZKO_GOOGLE_CLIENT_ID is missing or empty"]
        );
    }

    #[test]
    fn bad_values_are_reported() {
        let mut map = base();
        map.insert("TAZKO_PORT".into(), "eighty".into());
        map.insert("TAZKO_HOST".into(), "localhost".into());
        map.insert("TAZKO_PUBLIC_URL".into(), "localhost:8791".into());
        map.insert("TAZKO_SECRET".into(), "short".into());
        let p = problems(&map);
        assert_eq!(p.len(), 4, "{p:?}");
    }

    #[test]
    fn partial_email_settings_are_reported() {
        let mut map = base();
        map.insert("TAZKO_SMTP_USER".into(), "mailer".into());
        let p = problems(&map);
        assert!(p.iter().any(|m| m.contains("TAZKO_SMTP_HOST")));
        assert!(p.iter().any(|m| m.contains("TAZKO_MAIL_FROM")));
    }

    #[test]
    fn complete_email_settings_turn_email_on() {
        let mut map = base();
        map.insert("TAZKO_SMTP_HOST".into(), "smtp.example.com".into());
        map.insert("TAZKO_MAIL_FROM".into(), "tazko@example.com".into());
        let smtp = from_map(&map).unwrap().smtp.unwrap();
        assert_eq!(smtp.port, 587);
    }

    #[test]
    fn a_mail_user_needs_a_password() {
        let mut map = base();
        map.insert("TAZKO_SMTP_HOST".into(), "smtp.example.com".into());
        map.insert("TAZKO_MAIL_FROM".into(), "tazko@example.com".into());
        map.insert("TAZKO_SMTP_USER".into(), "mailer".into());
        assert!(
            problems(&map)
                .iter()
                .any(|m| m.contains("TAZKO_SMTP_PASSWORD"))
        );
    }

    #[test]
    fn debug_output_hides_secrets() {
        let mut map = base();
        map.insert("TAZKO_SECRET".into(), "s".repeat(40));
        map.insert("TAZKO_SMTP_HOST".into(), "smtp.example.com".into());
        map.insert("TAZKO_MAIL_FROM".into(), "tazko@example.com".into());
        map.insert("TAZKO_SMTP_USER".into(), "mailer".into());
        map.insert("TAZKO_SMTP_PASSWORD".into(), "mail-password-value".into());
        let s = from_map(&map).unwrap();
        let out = format!("{s:?}");
        assert!(!out.contains("client-secret-value"));
        assert!(!out.contains(&"s".repeat(40)));
        assert!(!out.contains("mail-password-value"));
        assert!(!startup_message(&s).contains("client-secret-value"));
    }

    #[test]
    fn trailing_slash_is_removed_from_public_url() {
        let mut map = base();
        map.insert("TAZKO_PUBLIC_URL".into(), "http://localhost:8791/".into());
        assert_eq!(from_map(&map).unwrap().public_url, "http://localhost:8791");
    }
}
