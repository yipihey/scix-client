//! Astronomical object resolution endpoint.
//!
//! Resolves object names (M31, NGC 1234, Crab Nebula, etc.) to bibcodes
//! using SIMBAD/NED integration.

use crate::client::SciXClient;
use crate::error::{SciXError, Result};

impl SciXClient {
    /// Resolve astronomical object names to associated bibcodes.
    ///
    /// Uses SIMBAD/NED databases to find papers about the given objects.
    pub async fn resolve_objects(
        &self,
        objects: &[&str],
    ) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "query": objects.iter()
                .map(|o| format!("object:\"{}\"", o))
                .collect::<Vec<_>>(),
        });

        let response_body = self.post_json("/objects", &body).await?;
        serde_json::from_str(&response_body)
            .map_err(|e| SciXError::Parse(format!("Invalid objects response: {}", e)))
    }
}
