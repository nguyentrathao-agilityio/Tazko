use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, KeyInit, Mac};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::session::now;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Envelope<T> {
    purpose: String,
    exp: i64,
    data: T,
}

fn mac(secret: &str, payload: &str) -> Vec<u8> {
    let mut m =
        <Hmac<Sha256> as KeyInit>::new_from_slice(secret.as_bytes()).expect("any key length");
    m.update(payload.as_bytes());
    m.finalize().into_bytes().to_vec()
}

pub fn seal<T: Serialize>(secret: &str, purpose: &str, ttl_secs: i64, data: T) -> String {
    let env = Envelope {
        purpose: purpose.to_string(),
        exp: now() + ttl_secs,
        data,
    };
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&env).expect("serialisable"));
    let sig = URL_SAFE_NO_PAD.encode(mac(secret, &payload));
    format!("{payload}.{sig}")
}

pub fn open<T: DeserializeOwned>(secret: &str, purpose: &str, value: &str) -> Option<T> {
    let (payload, sig) = value.split_once('.')?;
    let sig = URL_SAFE_NO_PAD.decode(sig).ok()?;
    if !bool::from(mac(secret, payload).ct_eq(&sig)) {
        return None;
    }

    let env: Envelope<T> = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    (env.purpose == purpose && env.exp > now()).then_some(env.data)
}

pub fn read(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.trim().to_string())
}

pub fn set(name: &str, value: &str, path: &str, max_age: i64) -> String {
    format!("{name}={value}; Path={path}; HttpOnly; Secure; SameSite=Lax; Max-Age={max_age}")
}

pub fn clear(name: &str, path: &str) -> String {
    format!("{name}=; Path={path}; HttpOnly; Secure; SameSite=Lax; Max-Age=0")
}
