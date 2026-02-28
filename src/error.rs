//! Error types for the SciX client.

use std::time::Duration;

/// Errors that can occur when interacting with the SciX API.
#[derive(Debug, thiserror::Error)]
pub enum SciXError {
    /// HTTP request failed (network, timeout, etc.)
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// SciX API returned an error status code.
    #[error("API error (HTTP {status}): {message}")]
    Api { status: u16, message: String },

    /// No API token provided.
    #[error("Authentication required: set SCIX_API_TOKEN (or ADS_API_TOKEN) environment variable or pass token to SciXClient::new()")]
    AuthRequired,

    /// Rate limited by SciX API (HTTP 429).
    #[error("Rate limited, retry after {retry_after:?}")]
    RateLimited { retry_after: Option<Duration> },

    /// Failed to parse API response.
    #[error("Failed to parse response: {0}")]
    Parse(String),

    /// Invalid query syntax.
    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    /// Resource not found (HTTP 404).
    #[error("Not found: {0}")]
    NotFound(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// JSON serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Convenience alias for Results using [`SciXError`].
pub type Result<T> = std::result::Result<T, SciXError>;
