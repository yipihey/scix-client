//! Search and discovery endpoints.
//!
//! Covers: search, bigquery, references, citations, similar, coreads.

use crate::client::SciXClient;
use crate::error::Result;
use crate::parse::{parse_search_response, DEFAULT_SEARCH_FIELDS};
use crate::types::{SearchResponse, Sort};

impl SciXClient {
    /// Search the SciX database.
    ///
    /// Uses ADS query syntax: `author:"Einstein" year:1905`, `title:"dark matter"`, etc.
    pub async fn search(&self, query: &str, rows: u32) -> Result<SearchResponse> {
        self.search_with_options(query, DEFAULT_SEARCH_FIELDS, None, rows, 0)
            .await
    }

    /// Search with full control over fields, sort, and pagination.
    pub async fn search_with_options(
        &self,
        query: &str,
        fields: &str,
        sort: Option<&Sort>,
        rows: u32,
        start: u32,
    ) -> Result<SearchResponse> {
        let rows_str = rows.to_string();
        let start_str = start.to_string();
        let sort_str = sort
            .map(|s| s.to_string())
            .unwrap_or_else(|| "date desc".to_string());

        let params = vec![
            ("q", query),
            ("fl", fields),
            ("rows", &rows_str),
            ("start", &start_str),
            ("sort", &sort_str),
        ];

        let body = self.get("/search/query", &params).await?;
        parse_search_response(&body)
    }

    /// Bigquery: search within a set of known bibcodes.
    ///
    /// Useful for filtering a large set of papers by additional criteria.
    pub async fn bigquery(
        &self,
        bibcodes: &[&str],
        query: Option<&str>,
        fields: Option<&str>,
        sort: Option<&Sort>,
        rows: Option<u32>,
    ) -> Result<SearchResponse> {
        let q = query.unwrap_or("*:*");
        let fl = fields.unwrap_or(DEFAULT_SEARCH_FIELDS);
        let sort_str = sort
            .map(|s| s.to_string())
            .unwrap_or_else(|| "date desc".to_string());
        let rows_val = rows.unwrap_or(bibcodes.len() as u32);

        let bibcode_list = bibcodes.join("\n");
        let body = serde_json::json!({
            "bibcodes": bibcode_list.split('\n').collect::<Vec<_>>(),
            "query": format!("q={}&fl={}&rows={}&sort={}", q, fl, rows_val, sort_str),
        });

        // Bigquery uses POST to /search/bigquery
        let response_body = self.post_json("/search/bigquery", &body).await?;
        parse_search_response(&response_body)
    }

    /// Fetch papers referenced by the given paper.
    pub async fn references(&self, bibcode: &str, rows: u32) -> Result<SearchResponse> {
        let query = format!("references(bibcode:{})", bibcode);
        self.search(&query, rows).await
    }

    /// Fetch papers that cite the given paper.
    pub async fn citations(&self, bibcode: &str, rows: u32) -> Result<SearchResponse> {
        let query = format!("citations(bibcode:{})", bibcode);
        self.search(&query, rows).await
    }

    /// Fetch papers similar to the given paper (content-based).
    pub async fn similar(&self, bibcode: &str, rows: u32) -> Result<SearchResponse> {
        let query = format!("similar(bibcode:{})", bibcode);
        self.search(&query, rows).await
    }

    /// Fetch co-reads (trending papers read by the same audience).
    pub async fn coreads(&self, bibcode: &str, rows: u32) -> Result<SearchResponse> {
        let query = format!("trending(bibcode:{})", bibcode);
        self.search(&query, rows).await
    }
}
