//! Network visualization endpoints.
//!
//! Returns author collaboration networks and paper citation/reference clusters.

use crate::client::SciXClient;
use crate::error::{SciXError, Result};

impl SciXClient {
    /// Get author collaboration network for a set of papers.
    pub async fn author_network(&self, bibcodes: &[&str]) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "bibcodes": bibcodes,
            "types": ["author"],
        });

        let response_body = self.post_json("/vis/author-network", &body).await?;
        serde_json::from_str(&response_body)
            .map_err(|e| SciXError::Parse(format!("Invalid network response: {}", e)))
    }

    /// Get paper citation/reference network for a set of papers.
    pub async fn paper_network(&self, bibcodes: &[&str]) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "bibcodes": bibcodes,
            "types": ["paper"],
        });

        let response_body = self.post_json("/vis/paper-network", &body).await?;
        serde_json::from_str(&response_body)
            .map_err(|e| SciXError::Parse(format!("Invalid network response: {}", e)))
    }

    /// Get co-citation suggestions: papers frequently cited alongside the given set
    /// but not yet included.
    pub async fn citation_helper(&self, bibcodes: &[&str]) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "bibcodes": bibcodes,
        });

        let response_body = self.post_json("/citation_helper", &body).await?;
        serde_json::from_str(&response_body)
            .map_err(|e| SciXError::Parse(format!("Invalid citation helper response: {}", e)))
    }
}
