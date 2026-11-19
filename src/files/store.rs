use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn valid_hash(h: &str) -> bool {
    h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

pub fn files_dir(org_dir: &Path) -> PathBuf {
    org_dir.join("files")
}

pub fn path_of(org_dir: &Path, hash: &str) -> Option<PathBuf> {
    valid_hash(hash).then(|| files_dir(org_dir).join(&hash[..2]).join(hash))
}

pub fn put(org_dir: &Path, hash: &str, bytes: &[u8]) -> io::Result<bool> {
    let path = path_of(org_dir, hash)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad content hash"))?;
    if path.is_file() {
        return Ok(false);
    }

    let folder = path.parent().expect("hash path has a folder");
    fs::create_dir_all(folder)?;
    let mut tmp = tempfile::NamedTempFile::new_in(folder)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(&path).map_err(|e| e.error)?;
    Ok(true)
}

pub fn remove(org_dir: &Path, hash: &str) -> io::Result<()> {
    let Some(path) = path_of(org_dir, hash) else {
        return Ok(());
    };
    match fs::remove_file(&path) {
        Ok(()) => {
            if let Some(folder) = path.parent() {
                let _ = fs::remove_dir(folder);
            }
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_bytes_are_stored_once_under_their_hash() {
        let dir = tempfile::tempdir().unwrap();
        let h = hash(b"hello");
        assert_eq!(
            h,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert!(put(dir.path(), &h, b"hello").unwrap());
        assert!(!put(dir.path(), &h, b"hello").unwrap());
        let p = dir.path().join("files/2c").join(&h);
        assert_eq!(fs::read(&p).unwrap(), b"hello");
        remove(dir.path(), &h).unwrap();
        assert!(!p.exists());
    }

    #[test]
    fn only_real_hashes_become_paths() {
        let dir = tempfile::tempdir().unwrap();
        assert!(path_of(dir.path(), "../../control.db").is_none());
        assert!(path_of(dir.path(), &"A".repeat(64)).is_none());
        assert!(path_of(dir.path(), &format!("{}/", "a".repeat(63))).is_none());
        assert!(put(dir.path(), "..", b"x").is_err());
    }
}
