//! Citation export endpoints.

use crate::client::SciXClient;
use crate::error::Result;
use crate::parse::parse_export_response;
use crate::types::{ExportFormat, Sort};

impl SciXClient {
    /// Export papers in the specified citation format.
    pub async fn export(
        &self,
        bibcodes: &[&str],
        format: ExportFormat,
        sort: Option<&Sort>,
    ) -> Result<String> {
        let mut body = serde_json::json!({
            "bibcode": bibcodes,
        });

        if let Some(sort) = sort {
            body["sort"] = serde_json::Value::String(sort.to_string());
        }

        let path = format!("/export/{}", format.as_api_str());
        let response_body = self.post_json(&path, &body).await?;
        parse_export_response(&response_body)
    }

    /// Convenience: export as BibTeX.
    pub async fn export_bibtex(&self, bibcodes: &[&str]) -> Result<String> {
        self.export(bibcodes, ExportFormat::BibTeX, None).await
    }
}
