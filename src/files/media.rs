use std::io::SeekFrom;
use std::path::Path;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use tokio::io::AsyncSeekExt;

use crate::error::ApiError;

#[derive(Debug, PartialEq, Eq)]
pub enum Wanted {
    All,
    Part(u64, u64),
    Unsatisfiable,
}

pub fn wanted(range: Option<&str>, size: u64) -> Wanted {
    let Some(spec) = range.and_then(|r| r.trim().strip_prefix("bytes=")) else {
        return Wanted::All;
    };
    if spec.contains(',') {
        return Wanted::All;
    }

    let Some((from, to)) = spec.trim().split_once('-') else {
        return Wanted::All;
    };
    let (from, to) = (from.trim(), to.trim());
    let part = if from.is_empty() {
        match to.parse::<u64>() {
            Err(_) => return Wanted::All,
            Ok(0) => return Wanted::Unsatisfiable,
            Ok(n) => (size.saturating_sub(n), size.saturating_sub(1)),
        }
    } else {
        let Ok(start) = from.parse::<u64>() else {
            return Wanted::All;
        };
        let end = match to {
            "" => size.saturating_sub(1),
            t => match t.parse::<u64>() {
                Ok(e) => e.min(size.saturating_sub(1)),
                Err(_) => return Wanted::All,
            },
        };
        (start, end)
    };
    if size == 0 || part.0 >= size || part.0 > part.1 {
        return Wanted::Unsatisfiable;
    }

    Wanted::Part(part.0, part.1)
}

pub async fn response(
    path: &Path,
    hash: &str,
    mime: &str,
    name: &str,
    inline: bool,
    headers: &HeaderMap,
) -> Result<Response, ApiError> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|_| ApiError::NotFound)?;
    let size = file
        .metadata()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .len();
    let etag = format!("\"{hash}\"");
    let fresh = headers
        .get(header::IF_RANGE)
        .and_then(|v| v.to_str().ok())
        .is_none_or(|v| v == etag);
    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    let want = if fresh {
        wanted(range, size)
    } else {
        Wanted::All
    };

    let (status, start, len) = match want {
        Wanted::All => (StatusCode::OK, 0, size),
        Wanted::Part(a, b) => (StatusCode::PARTIAL_CONTENT, a, b - a + 1),
        Wanted::Unsatisfiable => {
            let mut res = Response::new(Body::empty());
            *res.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
            if let Ok(v) = HeaderValue::from_str(&format!("bytes */{size}")) {
                res.headers_mut().insert(header::CONTENT_RANGE, v);
            }
            return Ok(res);
        }
    };
    file.seek(SeekFrom::Start(start))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut res = Response::new(super::file_body(file, len));
    *res.status_mut() = status;
    let h = res.headers_mut();
    super::safe_file_headers(h, mime, name, inline);
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    if let Ok(v) = HeaderValue::from_str(&etag) {
        h.insert(header::ETAG, v);
    }
    if status == StatusCode::PARTIAL_CONTENT
        && let Ok(v) = HeaderValue::from_str(&format!("bytes {start}-{}/{size}", start + len - 1))
    {
        h.insert(header::CONTENT_RANGE, v);
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_follow_the_http_rules() {
        assert_eq!(wanted(None, 100), Wanted::All);
        assert_eq!(wanted(Some("bytes=0-"), 100), Wanted::Part(0, 99));
        assert_eq!(wanted(Some("bytes=10-19"), 100), Wanted::Part(10, 19));
        assert_eq!(wanted(Some("bytes=90-500"), 100), Wanted::Part(90, 99));
        assert_eq!(wanted(Some("bytes=-10"), 100), Wanted::Part(90, 99));
        assert_eq!(wanted(Some("bytes=-500"), 100), Wanted::Part(0, 99));
        assert_eq!(wanted(Some("bytes=100-"), 100), Wanted::Unsatisfiable);
        assert_eq!(wanted(Some("bytes=20-10"), 100), Wanted::Unsatisfiable);
        assert_eq!(wanted(Some("bytes=-0"), 100), Wanted::Unsatisfiable);
        assert_eq!(wanted(Some("bytes=0-1,5-6"), 100), Wanted::All);
        assert_eq!(wanted(Some("items=0-1"), 100), Wanted::All);
        assert_eq!(wanted(Some("bytes=x-1"), 100), Wanted::All);
    }
}
