//! SciX API response parsing.
//!
//! Ported from imbib-core/src/sources/ads.rs — this is the canonical implementation.

use crate::error::SciXError;
use crate::types::{Author, Paper, PdfLink, SearchResponse};
use serde::Deserialize;

/// Default fields requested in search queries.
pub const DEFAULT_SEARCH_FIELDS: &str =
    "bibcode,title,author,year,pub,abstract,doi,identifier,doctype,esources,citation_count,property";

/// ADS API search response wrapper.
#[derive(Debug, Deserialize)]
pub(crate) struct AdsApiResponse {
    pub response: AdsApiResponseBody,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AdsApiResponseBody {
    pub docs: Vec<AdsApiDocument>,
    #[serde(rename = "numFound")]
    pub num_found: Option<u64>,
}

/// Custom deserializer for year field that accepts both string and integer.
fn deserialize_year_option<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct YearVisitor;

    impl<'de> Visitor<'de> for YearVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string, integer, or null")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(YearValueVisitor).map(Some)
        }
    }

    struct YearValueVisitor;

    impl<'de> Visitor<'de> for YearValueVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or integer")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v.to_string())
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v)
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v.to_string())
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v.to_string())
        }
    }

    deserializer.deserialize_option(YearVisitor)
}

/// A single document from ADS API responses.
#[derive(Debug, Deserialize)]
pub(crate) struct AdsApiDocument {
    pub bibcode: String,
    pub title: Option<Vec<String>>,
    pub author: Option<Vec<String>>,
    #[serde(deserialize_with = "deserialize_year_option", default)]
    pub year: Option<String>,
    #[serde(rename = "pub")]
    pub publication: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub doi: Option<Vec<String>>,
    pub identifier: Option<Vec<String>>,
    pub doctype: Option<String>,
    pub esources: Option<Vec<String>>,
    pub citation_count: Option<i32>,
    #[serde(rename = "reference")]
    #[allow(dead_code)]
    pub references: Option<Vec<String>>,
    pub property: Option<Vec<String>>,
}

/// ADS BibTeX export response.
#[derive(Debug, Deserialize)]
pub(crate) struct AdsExportResponse {
    pub export: String,
}

/// Parse an ADS search/query JSON response into a [`SearchResponse`].
pub fn parse_search_response(json: &str) -> crate::error::Result<SearchResponse> {
    let response: AdsApiResponse =
        serde_json::from_str(json).map_err(|e| SciXError::Parse(format!("Invalid ADS JSON: {}", e)))?;

    let papers = response
        .response
        .docs
        .into_iter()
        .filter_map(document_to_paper)
        .collect();

    Ok(SearchResponse {
        num_found: response.response.num_found.unwrap_or(0),
        papers,
    })
}

/// Parse an ADS BibTeX export JSON response.
pub fn parse_export_response(json: &str) -> crate::error::Result<String> {
    let response: AdsExportResponse =
        serde_json::from_str(json).map_err(|e| SciXError::Parse(format!("Invalid export response: {}", e)))?;
    Ok(response.export)
}

/// Convert an ADS API document to a [`Paper`].
fn document_to_paper(doc: AdsApiDocument) -> Option<Paper> {
    let bibcode = doc.bibcode;
    let title = doc
        .title
        .and_then(|t| t.into_iter().next())
        .unwrap_or_default();

    if title.is_empty() {
        return None;
    }

    let authors: Vec<Author> = doc
        .author
        .unwrap_or_default()
        .into_iter()
        .map(|name| Author::from_ads_format(&name))
        .collect();

    let year = doc.year.as_ref().and_then(|y| y.parse().ok());
    let doi = doc.doi.and_then(|d| d.into_iter().next());
    let identifiers = doc.identifier.unwrap_or_default();
    let arxiv_id = extract_arxiv_id(&identifiers);
    let esources = doc.esources.unwrap_or_default();

    let pdf_links = PdfLink::from_esources(
        &esources,
        doi.as_deref(),
        arxiv_id.as_deref(),
        &bibcode,
    );

    let url = format!("https://scixplorer.org/abs/{}", bibcode);

    Some(Paper {
        title,
        authors,
        year,
        publication: doc.publication,
        abstract_text: doc.abstract_text,
        doi,
        arxiv_id,
        identifiers,
        esources,
        citation_count: doc.citation_count.map(|c| c.max(0) as u32),
        doctype: doc.doctype,
        properties: doc.property.unwrap_or_default(),
        pdf_links,
        url,
        bibcode,
    })
}

/// Extract arXiv ID from ADS identifier array.
pub fn extract_arxiv_id(identifiers: &[String]) -> Option<String> {
    identifiers.iter().find_map(|id| {
        if let Some(stripped) = id.strip_prefix("arXiv:") {
            Some(stripped.to_string())
        } else if is_bare_new_arxiv_id(id) {
            Some(id.clone())
        } else {
            None
        }
    })
}

/// Check if a string is a bare new-format arXiv ID: YYMM.NNNNN(vN).
///
/// Must NOT match DOIs (10.1086/300151) or bibcodes (1999AJ....117.2063K).
fn is_bare_new_arxiv_id(s: &str) -> bool {
    // Strip optional version suffix (e.g., "v2")
    let base = if let Some(v_pos) = s.rfind('v') {
        if s[v_pos + 1..].chars().all(|c| c.is_ascii_digit()) && v_pos > 0 {
            &s[..v_pos]
        } else {
            s
        }
    } else {
        s
    };

    // Must be exactly DDDD.DDDDD or DDDD.DDDD (4 digits, dot, 4-5 digits)
    let parts: Vec<&str> = base.splitn(2, '.').collect();
    if parts.len() != 2 {
        return false;
    }
    let (prefix, suffix) = (parts[0], parts[1]);
    prefix.len() == 4
        && prefix.chars().all(|c| c.is_ascii_digit())
        && (suffix.len() == 4 || suffix.len() == 5)
        && suffix.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PdfLinkType;

    const SAMPLE_RESPONSE: &str = r#"{
        "response": {
            "docs": [{
                "bibcode": "2023ApJ...123..456A",
                "title": ["A Great Paper About Stars"],
                "author": ["Author, First", "Researcher, Second"],
                "year": "2023",
                "pub": "The Astrophysical Journal",
                "doi": ["10.3847/1234-5678"],
                "identifier": ["arXiv:2301.12345"],
                "esources": ["EPRINT_PDF", "PUB_PDF"]
            }],
            "numFound": 1
        }
    }"#;

    #[test]
    fn test_parse_search_response() {
        let result = parse_search_response(SAMPLE_RESPONSE).unwrap();
        assert_eq!(result.papers.len(), 1);
        assert_eq!(result.num_found, 1);

        let paper = &result.papers[0];
        assert_eq!(paper.title, "A Great Paper About Stars");
        assert_eq!(paper.authors.len(), 2);
        assert_eq!(paper.year, Some(2023));
        assert_eq!(paper.bibcode, "2023ApJ...123..456A");
        assert_eq!(paper.doi, Some("10.3847/1234-5678".to_string()));
        assert_eq!(paper.arxiv_id, Some("2301.12345".to_string()));
    }

    #[test]
    fn test_parse_search_response_with_year_as_int() {
        let json = r#"{
            "response": {
                "docs": [{
                    "bibcode": "2024ApJ...999..001B",
                    "title": ["Paper with Integer Year"],
                    "author": ["Author, Test"],
                    "year": 2024,
                    "pub": "The Astrophysical Journal"
                }],
                "numFound": 1
            }
        }"#;

        let result = parse_search_response(json).unwrap();
        assert_eq!(result.papers.len(), 1);
        assert_eq!(result.papers[0].year, Some(2024));
    }

    #[test]
    fn test_parse_export_response() {
        let json = r#"{"export": "@article{2023ApJ...123..456A,\n  title={A Paper}\n}"}"#;
        let bibtex = parse_export_response(json).unwrap();
        assert!(bibtex.contains("@article"));
    }

    #[test]
    fn test_build_pdf_links() {
        let esources = vec!["EPRINT_PDF".to_string(), "PUB_PDF".to_string()];
        let links = PdfLink::from_esources(
            &esources,
            Some("10.1234/test"),
            Some("2301.12345"),
            "2023ApJ...",
        );

        assert!(links.iter().any(|l| l.url.contains("arxiv.org")));
        assert!(links.iter().any(|l| l.url.contains("doi.org")));
    }

    #[test]
    fn test_build_pdf_links_ads_scan() {
        let esources = vec!["ADS_SCAN".to_string()];
        let links = PdfLink::from_esources(&esources, None, None, "1990ApJ...353..159B");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].link_type, PdfLinkType::AdsScan);
        assert!(links[0].url.contains("articles.adsabs.harvard.edu"));
    }

    #[test]
    fn test_parse_ads_author() {
        let author = Author::from_ads_format("Einstein, Albert");
        assert_eq!(author.family_name, "Einstein");
        assert_eq!(author.given_name, Some("Albert".to_string()));
        assert_eq!(author.display_name(), "Albert Einstein");
        assert_eq!(author.bibtex_name(), "Einstein, Albert");
    }

    #[test]
    fn test_parse_ads_author_single_name() {
        let author = Author::from_ads_format("Collaboration");
        assert_eq!(author.family_name, "Collaboration");
        assert!(author.given_name.is_none());
    }

    #[test]
    fn test_extract_arxiv_id_prefixed() {
        let ids = vec!["arXiv:2301.12345".to_string()];
        assert_eq!(extract_arxiv_id(&ids), Some("2301.12345".to_string()));
    }

    #[test]
    fn test_extract_arxiv_id_bare() {
        let ids = vec!["2301.12345".to_string()];
        assert_eq!(extract_arxiv_id(&ids), Some("2301.12345".to_string()));
    }

    #[test]
    fn test_extract_arxiv_id_old_format() {
        let ids = vec!["arXiv:astro-ph/9901313".to_string()];
        assert_eq!(
            extract_arxiv_id(&ids),
            Some("astro-ph/9901313".to_string())
        );
    }

    #[test]
    fn test_extract_arxiv_id_with_version() {
        let ids = vec!["arXiv:2301.12345v2".to_string()];
        assert_eq!(extract_arxiv_id(&ids), Some("2301.12345v2".to_string()));
    }

    #[test]
    fn test_extract_arxiv_id_doi_not_matched() {
        let ids = vec!["10.1086/300151".to_string()];
        assert_eq!(extract_arxiv_id(&ids), None);
    }

    #[test]
    fn test_extract_arxiv_id_bibcode_not_matched() {
        let ids = vec!["1999AJ....117.2063K".to_string()];
        assert_eq!(extract_arxiv_id(&ids), None);
    }

    #[test]
    fn test_extract_arxiv_id_arxiv_doi_not_matched() {
        let ids = vec!["10.48550/arXiv.astro-ph/9901313".to_string()];
        assert_eq!(extract_arxiv_id(&ids), None);
    }

    #[test]
    fn test_extract_arxiv_id_mixed_identifiers() {
        let ids = vec![
            "1999AJ....117.2063K".to_string(),
            "10.1086/300151".to_string(),
            "arXiv:astro-ph/9901313".to_string(),
            "10.48550/arXiv.astro-ph/9901313".to_string(),
        ];
        assert_eq!(
            extract_arxiv_id(&ids),
            Some("astro-ph/9901313".to_string())
        );
    }

    #[test]
    fn test_extract_arxiv_id_empty() {
        assert_eq!(extract_arxiv_id(&[]), None);
    }

    #[test]
    fn test_document_with_empty_title_filtered() {
        let json = r#"{
            "response": {
                "docs": [
                    {
                        "bibcode": "2023ApJ...123..456A",
                        "title": [""],
                        "author": ["Author, Test"]
                    },
                    {
                        "bibcode": "2023ApJ...789..012B",
                        "title": ["Real Paper"],
                        "author": ["Author, Test"]
                    }
                ],
                "numFound": 2
            }
        }"#;

        let result = parse_search_response(json).unwrap();
        assert_eq!(result.papers.len(), 1);
        assert_eq!(result.papers[0].title, "Real Paper");
    }
}
