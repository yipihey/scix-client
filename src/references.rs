//! Free-text reference resolution endpoint.
//!
//! Converts unstructured citation strings to ADS bibcodes.

use crate::client::SciXClient;
use crate::error::{Result, SciXError};
use crate::types::ResolvedReference;

impl SciXClient {
    /// Resolve free-text references to ADS bibcodes.
    ///
    /// Example: "Einstein 1905 Annalen der Physik 17 891" → bibcode.
    pub async fn resolve_references(&self, references: &[&str]) -> Result<Vec<ResolvedReference>> {
        let text = references.join("\n");
        let response_body = self
            .post_text("/reference/text", "text/plain", &text)
            .await?;

        let parsed: serde_json::Value = serde_json::from_str(&response_body)
            .map_err(|e| SciXError::Parse(format!("Invalid reference response: {}", e)))?;

        let resolved = parsed["resolved"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .zip(references.iter())
            .map(|(entry, reference)| ResolvedReference {
                reference: reference.to_string(),
                bibcode: entry["bibcode"].as_str().map(String::from),
                score: entry["score"].as_str().map(String::from),
            })
            .collect();

        Ok(resolved)
    }
}
