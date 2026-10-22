//! The SciX / NASA ADS API client.

use crate::error::{Result, SciXError};
use crate::rate_limit::RateLimiter;
use reqwest::Client;
use std::path::PathBuf;
use std::time::Duration;

/// Where the saved API token lives.
///
/// Writes `scix/token` under the platform config directory:
/// `%APPDATA%` on Windows, otherwise `$XDG_CONFIG_HOME` or `~/.config`.
/// The GUI editors that launch `scix serve` do not inherit a shell
/// environment, so a saved token is what lets a bare `scix serve` work
/// from Claude Desktop, Cursor, or an install badge.
pub fn token_file_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("SCIX_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("token"));
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("scix").join("token"))
}

/// Where a resolved token came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSource {
    /// `SCIX_API_TOKEN` or `ADS_API_TOKEN` environment variable.
    Env(&'static str),
    /// The token file written by `scix setup` / `scix login`.
    File(PathBuf),
}

impl std::fmt::Display for TokenSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Env(name) => write!(f, "{} environment variable", name),
            Self::File(path) => write!(f, "{}", path.display()),
        }
    }
}

/// Find the API token without constructing a client, reporting where it was found.
///
/// Order: `SCIX_API_TOKEN`, `ADS_API_TOKEN`, then the token file.
pub fn resolve_token_with_source() -> Option<(String, TokenSource)> {
    for name in ["SCIX_API_TOKEN", "ADS_API_TOKEN"] {
        if let Ok(token) = std::env::var(name) {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return Some((token, TokenSource::Env(name)));
            }
        }
    }
    let path = token_file_path()?;
    let token = std::fs::read_to_string(&path).ok()?;
    let token = token.trim().to_string();
    if token.is_empty() {
        return None;
    }
    Some((token, TokenSource::File(path)))
}

/// Find the API token without constructing a client.
pub fn resolve_token() -> Option<String> {
    resolve_token_with_source().map(|(t, _)| t)
}

/// Save a token to the token file (mode 0600 on Unix). Returns the path written.
pub fn save_token(token: &str) -> std::io::Result<PathBuf> {
    let path = token_file_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "cannot determine a config directory (HOME/APPDATA unset)",
        )
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, format!("{}\n", token.trim()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(path)
}

/// Async client for the SciX (NASA ADS) API.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> scix_client::error::Result<()> {
/// let client = scix_client::SciXClient::from_env()?;
/// let results = client.search("author:\"Einstein\" year:1905", 10).await?;
/// for paper in &results.papers {
///     println!("{} ({})", paper.title, paper.bibcode);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct SciXClient {
    pub(crate) http: Client,
    pub(crate) api_token: String,
    pub(crate) base_url: String,
    pub(crate) rate_limiter: RateLimiter,
}

impl SciXClient {
    /// Create a new client with the given API token.
    pub fn new(api_token: impl Into<String>) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http,
            api_token: api_token.into(),
            base_url: "https://api.adsabs.harvard.edu/v1".to_string(),
            rate_limiter: RateLimiter::new(5.0),
        }
    }

    /// Create a client from the ambient token: `SCIX_API_TOKEN`, then
    /// `ADS_API_TOKEN`, then the token file written by `scix setup` /
    /// `scix login` (see [`token_file_path`]).
    pub fn from_env() -> Result<Self> {
        let token = resolve_token().ok_or(SciXError::AuthRequired)?;
        Ok(Self::new(token))
    }

    /// Override the base URL (useful for testing).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Override the rate limit (requests per second).
    pub fn with_rate_limit(mut self, per_second: f64) -> Self {
        self.rate_limiter = RateLimiter::new(per_second);
        self
    }

    /// Make an authenticated GET request to the SciX API.
    pub(crate) async fn get(&self, path: &str, params: &[(&str, &str)]) -> Result<String> {
        self.rate_limiter.acquire().await;

        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("User-Agent", "scix-client/0.1.0")
            .query(params)
            .send()
            .await?;

        self.rate_limiter
            .update_from_headers(response.headers())
            .await;
        handle_response(response).await
    }

    /// Make an authenticated POST request with a JSON body.
    pub(crate) async fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<String> {
        self.rate_limiter.acquire().await;

        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("User-Agent", "scix-client/0.1.0")
            // `/reference/text` content-negotiates: without this it answers in
            // plain "score bibcode -- refstring" lines, which no caller parses.
            // Every other endpoint here returns JSON regardless, so asking for
            // it explicitly is correct everywhere and load-bearing in one place.
            .header("Accept", "application/json")
            .json(body)
            .send()
            .await?;

        self.rate_limiter
            .update_from_headers(response.headers())
            .await;
        handle_response(response).await
    }

    /// Make an authenticated PUT request with a JSON body.
    pub(crate) async fn put_json(&self, path: &str, body: &serde_json::Value) -> Result<String> {
        self.rate_limiter.acquire().await;

        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("User-Agent", "scix-client/0.1.0")
            .json(body)
            .send()
            .await?;

        self.rate_limiter
            .update_from_headers(response.headers())
            .await;
        handle_response(response).await
    }

    /// Make an authenticated DELETE request.
    pub(crate) async fn delete(&self, path: &str) -> Result<String> {
        self.rate_limiter.acquire().await;

        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("User-Agent", "scix-client/0.1.0")
            .send()
            .await?;

        self.rate_limiter
            .update_from_headers(response.headers())
            .await;
        handle_response(response).await
    }
}

/// Handle the HTTP response, mapping status codes to errors.
async fn handle_response(response: reqwest::Response) -> Result<String> {
    let status = response.status().as_u16();

    match status {
        200..=299 => Ok(response.text().await?),
        401 => Err(SciXError::AuthRequired),
        404 => Err(SciXError::NotFound("Resource not found".to_string())),
        429 => {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .map(Duration::from_secs);
            Err(SciXError::RateLimited { retry_after })
        }
        _ => {
            let body = response.text().await.unwrap_or_default();
            Err(SciXError::Api {
                status,
                message: body,
            })
        }
    }
}
