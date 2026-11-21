use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_host: Option<String>,
}

pub fn dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("TAZKO_CONFIG_DIR") {
        return Some(PathBuf::from(d));
    }

    if let Some(d) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(d).join("tazko"));
    }

    if let Some(d) = std::env::var_os("APPDATA") {
        return Some(PathBuf::from(d).join("tazko"));
    }

    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config").join("tazko"))
}

fn ensure_dir() -> Result<PathBuf, String> {
    let d = dir().ok_or("cannot find a folder for the settings (set HOME or TAZKO_CONFIG_DIR)")?;
    fs::create_dir_all(&d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&d, fs::Permissions::from_mode(0o700));
    }
    Ok(d)
}

pub fn load() -> Config {
    dir()
        .and_then(|d| fs::read_to_string(d.join("config.json")).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(c: &Config) -> Result<(), String> {
    let path = ensure_dir()?.join("config.json");
    let text = serde_json::to_string_pretty(c).map_err(|e| e.to_string())?;
    fs::write(&path, text + "\n").map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn token_path() -> Option<PathBuf> {
    dir().map(|d| d.join("token"))
}

pub fn read_token() -> Option<String> {
    let t = fs::read_to_string(token_path()?).ok()?;
    let t = t.trim().to_string();
    (!t.is_empty()).then_some(t)
}

pub fn write_token(token: &str) -> Result<PathBuf, String> {
    let path = ensure_dir()?.join("token");
    let tmp = path.with_extension("tmp");
    let _ = fs::remove_file(&tmp);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&tmp)
        .map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    file.write_all(token.as_bytes())
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

pub fn forget_token() -> bool {
    token_path().is_some_and(|p| fs::remove_file(p).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_token_file_is_private() {
        let d = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("TAZKO_CONFIG_DIR", d.path()) };
        let path = write_token("tzk_p_secret").unwrap();
        assert_eq!(read_token().as_deref(), Some("tzk_p_secret"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(d.path()).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        write_token("tzk_p_other").unwrap();
        assert_eq!(read_token().as_deref(), Some("tzk_p_other"));
        save(&Config {
            host: Some("https://t.example".into()),
            ..Config::default()
        })
        .unwrap();
        assert_eq!(load().host.as_deref(), Some("https://t.example"));
        assert!(forget_token());
        assert!(read_token().is_none());
        unsafe { std::env::remove_var("TAZKO_CONFIG_DIR") };
    }
}
