//! Public types for the SciX client.
//!
//! These types are self-contained with no external dependencies beyond serde,
//! making this crate publishable to crates.io independently.
//!
//! When the `python` feature is enabled, all types are exposed to Python
//! via PyO3 with automatic field access (`get_all`).

use serde::{Deserialize, Serialize};

/// A paper (document) from ADS search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct Paper {
    /// ADS bibcode (primary identifier).
    pub bibcode: String,
    /// Paper title.
    pub title: String,
    /// List of authors.
    pub authors: Vec<Author>,
    /// Publication year.
    pub year: Option<u16>,
    /// Journal/publication name.
    pub publication: Option<String>,
    /// Abstract text.
    pub abstract_text: Option<String>,
    /// DOI (first, if multiple).
    pub doi: Option<String>,
    /// arXiv ID (extracted from identifiers).
    pub arxiv_id: Option<String>,
    /// Raw identifier list from ADS.
    pub identifiers: Vec<String>,
    /// Electronic source flags (EPRINT_PDF, PUB_PDF, etc.).
    pub esources: Vec<String>,
    /// Number of citations.
    pub citation_count: Option<u32>,
    /// Document type (article, inproceedings, etc.).
    pub doctype: Option<String>,
    /// Property flags (OPENACCESS, REFEREED, etc.).
    pub properties: Vec<String>,
    /// Constructed PDF links, ordered by priority.
    pub pdf_links: Vec<PdfLink>,
    /// ADS URL for this paper.
    pub url: String,
}

/// An author of a paper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct Author {
    /// Raw name as returned by ADS ("Last, First M.").
    pub name: String,
    /// Family (last) name.
    pub family_name: String,
    /// Given (first) name and initials.
    pub given_name: Option<String>,
}

impl Author {
    /// Parse an author name in ADS format ("Last, First M.").
    pub fn from_ads_format(name: &str) -> Self {
        let parts: Vec<&str> = name.splitn(2, ',').collect();
        if parts.len() == 2 {
            Author {
                name: name.to_string(),
                family_name: parts[0].trim().to_string(),
                given_name: Some(parts[1].trim().to_string()),
            }
        } else {
            let words: Vec<&str> = name.split_whitespace().collect();
            if words.len() > 1 {
                Author {
                    name: name.to_string(),
                    family_name: words.last().unwrap().to_string(),
                    given_name: Some(words[..words.len() - 1].join(" ")),
                }
            } else {
                Author {
                    name: name.to_string(),
                    family_name: name.to_string(),
                    given_name: None,
                }
            }
        }
    }

    /// Format as "First M. Last" for display.
    pub fn display_name(&self) -> String {
        match &self.given_name {
            Some(given) => format!("{} {}", given, self.family_name),
            None => self.family_name.clone(),
        }
    }

    /// Format as "Last, First M." for BibTeX.
    pub fn bibtex_name(&self) -> String {
        match &self.given_name {
            Some(given) => format!("{}, {}", self.family_name, given),
            None => self.family_name.clone(),
        }
    }
}

/// A link to a PDF of a paper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct PdfLink {
    /// URL to the PDF or landing page.
    pub url: String,
    /// Type of link source.
    pub link_type: PdfLinkType,
    /// Human-readable label.
    pub label: String,
}

/// Source type for a PDF link.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "python", pyo3::pyclass(eq))]
pub enum PdfLinkType {
    ArXiv,
    Publisher,
    AdsScan,
    Direct,
}

impl PdfLink {
    /// Build PDF links from ADS esources, DOI, arXiv ID, and bibcode.
    ///
    /// Priority: arXiv PDF > DOI/publisher > ADS scans > fallbacks.
    pub fn from_esources(
        esources: &[String],
        doi: Option<&str>,
        arxiv_id: Option<&str>,
        bibcode: &str,
    ) -> Vec<PdfLink> {
        let mut links = Vec::new();
        let mut has_preprint = false;
        let mut has_publisher = false;

        for esource in esources {
            let upper = esource.to_uppercase();

            if upper == "EPRINT_PDF" {
                if let Some(arxiv) = arxiv_id {
                    links.push(PdfLink {
                        url: format!("https://arxiv.org/pdf/{}.pdf", arxiv),
                        link_type: PdfLinkType::ArXiv,
                        label: "arXiv PDF".to_string(),
                    });
                    has_preprint = true;
                }
            } else if upper == "PUB_PDF" || upper == "PUB_HTML" {
                if let Some(d) = doi {
                    if !d.is_empty() {
                        links.push(PdfLink {
                            url: format!("https://doi.org/{}", d),
                            link_type: PdfLinkType::Publisher,
                            label: "Publisher".to_string(),
                        });
                        has_publisher = true;
                    }
                }
            } else if upper == "ADS_PDF" || upper == "ADS_SCAN" {
                links.push(PdfLink {
                    url: format!(
                        "https://articles.adsabs.harvard.edu/pdf/{}",
                        bibcode
                    ),
                    link_type: PdfLinkType::AdsScan,
                    label: "ADS Scan".to_string(),
                });
            }
        }

        // Fallback links
        if !has_preprint {
            if let Some(arxiv) = arxiv_id {
                links.push(PdfLink {
                    url: format!("https://arxiv.org/pdf/{}.pdf", arxiv),
                    link_type: PdfLinkType::ArXiv,
                    label: "arXiv PDF".to_string(),
                });
            }
        }

        if !has_publisher {
            if let Some(d) = doi {
                if !d.is_empty() {
                    links.push(PdfLink {
                        url: format!("https://doi.org/{}", d),
                        link_type: PdfLinkType::Publisher,
                        label: "Publisher".to_string(),
                    });
                }
            }
        }

        links
    }
}

/// Paginated search response from ADS.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct SearchResponse {
    /// Matching papers.
    pub papers: Vec<Paper>,
    /// Total number of results (may be larger than `papers.len()`).
    pub num_found: u64,
}

/// Citation export formats supported by ADS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(eq))]
pub enum ExportFormat {
    BibTeX,
    BibTeXAbs,
    AasTex,
    Icarus,
    Mnras,
    Soph,
    Ris,
    Endnote,
    Medlars,
    Ieee,
    Csl,
    DcXml,
    RefXml,
    RefAbsXml,
    VoTable,
    Rss,
    Custom,
}

impl ExportFormat {
    /// ADS API format string.
    pub fn as_api_str(&self) -> &'static str {
        match self {
            Self::BibTeX => "bibtex",
            Self::BibTeXAbs => "bibtexabs",
            Self::AasTex => "aastex",
            Self::Icarus => "icarus",
            Self::Mnras => "mnras",
            Self::Soph => "soph",
            Self::Ris => "ris",
            Self::Endnote => "endnote",
            Self::Medlars => "medlars",
            Self::Ieee => "ieee",
            Self::Csl => "csl",
            Self::DcXml => "dcxml",
            Self::RefXml => "refxml",
            Self::RefAbsXml => "refabsxml",
            Self::VoTable => "votable",
            Self::Rss => "rss",
            Self::Custom => "custom",
        }
    }

    /// Parse from string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "bibtex" => Some(Self::BibTeX),
            "bibtexabs" => Some(Self::BibTeXAbs),
            "aastex" => Some(Self::AasTex),
            "icarus" => Some(Self::Icarus),
            "mnras" => Some(Self::Mnras),
            "soph" => Some(Self::Soph),
            "ris" => Some(Self::Ris),
            "endnote" => Some(Self::Endnote),
            "medlars" => Some(Self::Medlars),
            "ieee" => Some(Self::Ieee),
            "csl" => Some(Self::Csl),
            "dcxml" => Some(Self::DcXml),
            "refxml" => Some(Self::RefXml),
            "refabsxml" => Some(Self::RefAbsXml),
            "votable" => Some(Self::VoTable),
            "rss" => Some(Self::Rss),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_api_str())
    }
}

/// Citation metrics for a set of papers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct Metrics {
    /// Basic statistics.
    pub basic_stats: Option<BasicStats>,
    /// Citation statistics.
    pub citation_stats: Option<CitationStats>,
    /// Bibliometric indicators.
    pub indicators: Option<Indicators>,
}

/// Basic paper statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct BasicStats {
    #[serde(rename = "refereed")]
    pub refereed: Option<BasicStatsEntry>,
    #[serde(rename = "total")]
    pub total: Option<BasicStatsEntry>,
}

/// Basic statistics entry (for refereed or total).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct BasicStatsEntry {
    pub number_of_papers: Option<u32>,
    pub normalized_paper_count: Option<f64>,
    pub total_citations: Option<u32>,
    pub total_normalized_citations: Option<f64>,
    pub median_refereed_citations: Option<f64>,
    pub mean_refereed_citations: Option<f64>,
}

/// Citation statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct CitationStats {
    #[serde(rename = "refereed")]
    pub refereed: Option<CitationStatsEntry>,
    #[serde(rename = "total")]
    pub total: Option<CitationStatsEntry>,
}

/// Citation statistics entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct CitationStatsEntry {
    pub number_of_citing_papers: Option<u32>,
    pub total_citations: Option<u32>,
    pub number_of_self_citations: Option<u32>,
    pub average_citations: Option<f64>,
    pub normalized_citations: Option<f64>,
}

/// Bibliometric indicators (h-index, g-index, etc.).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct Indicators {
    pub h: Option<u32>,
    pub g: Option<u32>,
    pub i10: Option<u32>,
    pub i100: Option<u32>,
    pub m: Option<f64>,
    pub tori: Option<f64>,
    pub riq: Option<u32>,
    pub read10: Option<f64>,
}

/// An ADS personal library.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct Library {
    pub id: String,
    pub name: String,
    pub description: String,
    pub num_documents: u32,
    pub public: bool,
    pub owner: String,
    pub date_created: String,
    pub date_last_modified: String,
}

/// Detailed library including documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct LibraryDetail {
    pub metadata: Library,
    pub documents: Vec<String>,
}

/// Result of astronomical object resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct ObjectResult {
    pub object: String,
    pub bibcodes: Vec<String>,
}

/// Result of free-text reference resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct ResolvedReference {
    pub reference: String,
    pub bibcode: Option<String>,
    pub score: Option<String>,
}

/// Sort specification for search queries.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct Sort {
    pub field: String,
    pub direction: SortDirection,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "python", pyo3::pyclass(eq))]
pub enum SortDirection {
    Asc,
    Desc,
}

impl Sort {
    pub fn new(field: impl Into<String>, direction: SortDirection) -> Self {
        Self {
            field: field.into(),
            direction,
        }
    }

    pub fn date_desc() -> Self {
        Self::new("date", SortDirection::Desc)
    }

    pub fn citation_count_desc() -> Self {
        Self::new("citation_count", SortDirection::Desc)
    }

    pub fn score_desc() -> Self {
        Self::new("score", SortDirection::Desc)
    }
}

impl std::fmt::Display for Sort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dir = match self.direction {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        };
        write!(f, "{} {}", self.field, dir)
    }
}
