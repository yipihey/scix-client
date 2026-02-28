//! Link resolution endpoint.
//!
//! Resolves links for a paper (full-text, datasets, citations, references).

use crate::client::SciXClient;
use crate::error::{Result, SciXError};

impl SciXClient {
    /// Resolve links for a paper.
    ///
    /// `link_type` can be: "esource", "data", "citation", "reference", "coreads",
    /// or None for all link types.
    pub async fn resolve_links(
        &self,
        bibcode: &str,
        link_type: Option<&str>,
    ) -> Result<serde_json::Value> {
        let path = match link_type {
            Some(lt) => format!("/resolver/{}/{}", bibcode, lt),
            None => format!("/resolver/{}", bibcode),
        };

        let response_body = self.get(&path, &[]).await?;
        serde_json::from_str(&response_body)
            .map_err(|e| SciXError::Parse(format!("Invalid links response: {}", e)))
    }
}
