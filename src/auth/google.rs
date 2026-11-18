use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;

pub const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
pub const ISSUERS: [&str; 2] = ["https://accounts.google.com", "accounts.google.com"];

const DEFAULT_KEY_AGE: Duration = Duration::from_secs(60 * 60);
const MAX_KEY_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const REFETCH_GAP: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct Endpoints {
    pub auth_url: String,
    pub token_url: String,
    pub jwks_url: String,
    pub issuers: Vec<String>,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            auth_url: AUTH_URL.into(),
            token_url: TOKEN_URL.into(),
            jwks_url: JWKS_URL.into(),
            issuers: ISSUERS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: Option<String>,
    #[serde(default, deserialize_with = "flexible_bool")]
    pub email_verified: bool,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub hd: Option<String>,
    pub nonce: Option<String>,
}

fn flexible_bool<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum B {
        Bool(bool),
        Text(String),
    }
    Ok(match B::deserialize(d)? {
        B::Bool(b) => b,
        B::Text(s) => s == "true",
    })
}

#[derive(Deserialize)]
struct TokenResponse {
    id_token: String,
}

#[derive(Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Deserialize)]
struct Jwk {
    kid: String,
    kty: String,
    n: Option<String>,
    e: Option<String>,
}

struct KeyCache {
    keys: HashMap<String, DecodingKey>,
    fetched: Instant,
    max_age: Duration,
}

pub struct Google {
    pub endpoints: Endpoints,
    agent: ureq::Agent,
    keys: RwLock<Option<KeyCache>>,
}

#[derive(Debug)]
pub enum GoogleError {
    Exchange(String),
    Keys(String),
    Token(String),
}

impl std::fmt::Display for GoogleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GoogleError::Exchange(m) => write!(f, "code exchange failed: {m}"),
            GoogleError::Keys(m) => write!(f, "could not load Google signing keys: {m}"),
            GoogleError::Token(m) => write!(f, "ID token rejected: {m}"),
        }
    }
}

pub struct Exchange<'a> {
    pub code: &'a str,
    pub client_id: &'a str,
    pub client_secret: &'a str,
    pub redirect_uri: &'a str,
    pub code_verifier: &'a str,
}

impl Google {
    pub fn new(endpoints: Endpoints) -> Self {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let agent = ureq::Agent::config_builder()
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            .timeout_global(Some(Duration::from_secs(10)))
            .http_status_as_error(true)
            .build()
            .new_agent();
        Self {
            endpoints,
            agent,
            keys: RwLock::new(None),
        }
    }

    pub fn exchange_code(&self, x: &Exchange<'_>) -> Result<String, GoogleError> {
        let res = self.agent.post(&self.endpoints.token_url).send_form([
            ("grant_type", "authorization_code"),
            ("code", x.code),
            ("client_id", x.client_id),
            ("client_secret", x.client_secret),
            ("redirect_uri", x.redirect_uri),
            ("code_verifier", x.code_verifier),
        ]);
        let mut res = res.map_err(|e| GoogleError::Exchange(e.to_string()))?;
        let body: TokenResponse = res
            .body_mut()
            .read_json()
            .map_err(|e| GoogleError::Exchange(e.to_string()))?;
        Ok(body.id_token)
    }

    pub fn verify_id_token(
        &self,
        token: &str,
        client_id: &str,
        nonce: &str,
    ) -> Result<Claims, GoogleError> {
        let header = decode_header(token).map_err(|e| GoogleError::Token(e.to_string()))?;
        if header.alg != Algorithm::RS256 {
            return Err(GoogleError::Token("unexpected algorithm".into()));
        }

        let kid = header
            .kid
            .ok_or_else(|| GoogleError::Token("no key id".into()))?;
        let key = self.key(&kid)?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[client_id]);
        validation.set_issuer(&self.endpoints.issuers);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.leeway = 60;
        let data = decode::<Claims>(token, &key, &validation)
            .map_err(|e| GoogleError::Token(e.to_string()))?;
        if data.claims.nonce.as_deref() != Some(nonce) {
            return Err(GoogleError::Token("nonce does not match".into()));
        }

        Ok(data.claims)
    }

    fn key(&self, kid: &str) -> Result<DecodingKey, GoogleError> {
        let stale = {
            let cache = self.keys.read().unwrap();
            match cache.as_ref() {
                Some(c) if c.fetched.elapsed() < c.max_age => {
                    if let Some(k) = c.keys.get(kid) {
                        return Ok(k.clone());
                    }
                    c.fetched.elapsed() >= REFETCH_GAP
                }
                _ => true,
            }
        };

        if stale {
            self.refresh_keys()?;
        }

        self.keys
            .read()
            .unwrap()
            .as_ref()
            .and_then(|c| c.keys.get(kid).cloned())
            .ok_or_else(|| GoogleError::Token("unknown signing key".into()))
    }

    fn refresh_keys(&self) -> Result<(), GoogleError> {
        let mut res = self
            .agent
            .get(&self.endpoints.jwks_url)
            .call()
            .map_err(|e| GoogleError::Keys(e.to_string()))?;
        let max_age = res
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            .and_then(max_age)
            .unwrap_or(DEFAULT_KEY_AGE)
            .min(MAX_KEY_AGE);
        let jwks: Jwks = res
            .body_mut()
            .read_json()
            .map_err(|e| GoogleError::Keys(e.to_string()))?;
        let mut keys = HashMap::new();
        for k in jwks.keys.into_iter().filter(|k| k.kty == "RSA") {
            if let (Some(n), Some(e)) = (k.n, k.e)
                && let Ok(key) = DecodingKey::from_rsa_components(&n, &e)
            {
                keys.insert(k.kid, key);
            }
        }

        *self.keys.write().unwrap() = Some(KeyCache {
            keys,
            fetched: Instant::now(),
            max_age,
        });
        Ok(())
    }
}

pub fn max_age(cache_control: &str) -> Option<Duration> {
    cache_control
        .split(',')
        .filter_map(|p| p.trim().strip_prefix("max-age="))
        .find_map(|v| v.trim().parse().ok())
        .map(Duration::from_secs)
}
