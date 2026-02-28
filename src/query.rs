//! Type-safe SciX query builder.
//!
//! Helps construct ADS query strings with proper escaping and syntax.
//!
//! # Example
//!
//! ```
//! use scix_client::QueryBuilder;
//!
//! let query = QueryBuilder::new()
//!     .author("Einstein")
//!     .and()
//!     .year_range(1905, 1910)
//!     .build();
//! assert_eq!(query, "author:\"Einstein\" AND year:[1905 TO 1910]");
//! ```

/// Builder for constructing ADS query strings.
#[derive(Debug, Clone, Default)]
pub struct QueryBuilder {
    parts: Vec<String>,
}

impl QueryBuilder {
    /// Create a new empty query builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an author search term.
    pub fn author(mut self, name: &str) -> Self {
        self.parts.push(format!("author:\"{}\"", name));
        self
    }

    /// Add a first-author search term.
    pub fn first_author(mut self, name: &str) -> Self {
        self.parts.push(format!("first_author:\"{}\"", name));
        self
    }

    /// Add a title search term.
    pub fn title(mut self, text: &str) -> Self {
        self.parts.push(format!("title:\"{}\"", text));
        self
    }

    /// Add an abstract search term.
    pub fn abstract_contains(mut self, text: &str) -> Self {
        self.parts.push(format!("abs:\"{}\"", text));
        self
    }

    /// Add an exact year filter.
    pub fn year(mut self, year: u16) -> Self {
        self.parts.push(format!("year:{}", year));
        self
    }

    /// Add a year range filter (inclusive).
    pub fn year_range(mut self, from: u16, to: u16) -> Self {
        self.parts.push(format!("year:[{} TO {}]", from, to));
        self
    }

    /// Add a bibcode filter.
    pub fn bibcode(mut self, bibcode: &str) -> Self {
        self.parts.push(format!("bibcode:{}", bibcode));
        self
    }

    /// Add a DOI filter.
    pub fn doi(mut self, doi: &str) -> Self {
        self.parts.push(format!("doi:\"{}\"", doi));
        self
    }

    /// Add an arXiv ID filter.
    pub fn arxiv(mut self, arxiv_id: &str) -> Self {
        self.parts.push(format!("identifier:arXiv:{}", arxiv_id));
        self
    }

    /// Add an astronomical object filter.
    pub fn object(mut self, name: &str) -> Self {
        self.parts.push(format!("object:\"{}\"", name));
        self
    }

    /// Add a bibstem (journal abbreviation) filter.
    pub fn bibstem(mut self, stem: &str) -> Self {
        self.parts.push(format!("bibstem:{}", stem));
        self
    }

    /// Add a property filter (e.g., "refereed", "openaccess").
    pub fn property(mut self, prop: &str) -> Self {
        self.parts.push(format!("property:{}", prop));
        self
    }

    /// Add a doctype filter (e.g., "article", "inproceedings").
    pub fn doctype(mut self, dtype: &str) -> Self {
        self.parts.push(format!("doctype:{}", dtype));
        self
    }

    /// Add an ORCID filter.
    pub fn orcid(mut self, orcid: &str) -> Self {
        self.parts.push(format!("orcid:{}", orcid));
        self
    }

    /// Add an AND operator.
    pub fn and(mut self) -> Self {
        self.parts.push("AND".to_string());
        self
    }

    /// Add an OR operator.
    pub fn or(mut self) -> Self {
        self.parts.push("OR".to_string());
        self
    }

    /// Add a NOT operator (exclude the following term).
    pub fn exclude(mut self) -> Self {
        self.parts.push("NOT".to_string());
        self
    }

    /// Add a raw query fragment (no escaping).
    pub fn raw(mut self, query: &str) -> Self {
        self.parts.push(query.to_string());
        self
    }

    /// Build a citations-of query.
    pub fn citations_of(bibcode: &str) -> Self {
        let mut qb = Self::new();
        qb.parts
            .push(format!("citations(bibcode:{})", bibcode));
        qb
    }

    /// Build a references-of query.
    pub fn references_of(bibcode: &str) -> Self {
        let mut qb = Self::new();
        qb.parts
            .push(format!("references(bibcode:{})", bibcode));
        qb
    }

    /// Build a similar-to query.
    pub fn similar_to(bibcode: &str) -> Self {
        let mut qb = Self::new();
        qb.parts.push(format!("similar(bibcode:{})", bibcode));
        qb
    }

    /// Build a trending query.
    pub fn trending(bibcode: &str) -> Self {
        let mut qb = Self::new();
        qb.parts.push(format!("trending(bibcode:{})", bibcode));
        qb
    }

    /// Build the final query string.
    pub fn build(self) -> String {
        self.parts.join(" ")
    }
}

impl std::fmt::Display for QueryBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.parts.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_author_query() {
        let q = QueryBuilder::new().author("Einstein").build();
        assert_eq!(q, "author:\"Einstein\"");
    }

    #[test]
    fn test_combined_query() {
        let q = QueryBuilder::new()
            .author("Einstein")
            .and()
            .year_range(1905, 1910)
            .build();
        assert_eq!(q, "author:\"Einstein\" AND year:[1905 TO 1910]");
    }

    #[test]
    fn test_complex_query() {
        let q = QueryBuilder::new()
            .first_author("Weinberg")
            .and()
            .title("cosmological constant")
            .and()
            .property("refereed")
            .build();
        assert_eq!(
            q,
            "first_author:\"Weinberg\" AND title:\"cosmological constant\" AND property:refereed"
        );
    }

    #[test]
    fn test_citations_of() {
        let q = QueryBuilder::citations_of("2023ApJ...123..456A").build();
        assert_eq!(q, "citations(bibcode:2023ApJ...123..456A)");
    }

    #[test]
    fn test_raw_fragment() {
        let q = QueryBuilder::new()
            .raw("full_text:\"galaxy cluster\"")
            .and()
            .year(2023)
            .build();
        assert_eq!(q, "full_text:\"galaxy cluster\" AND year:2023");
    }

    #[test]
    fn test_display_trait() {
        let q = QueryBuilder::new().author("Hawking").and().year(1974);
        assert_eq!(format!("{}", q), "author:\"Hawking\" AND year:1974");
    }
}
