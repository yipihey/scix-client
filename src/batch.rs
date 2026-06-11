//! Batch operations across paper sets.
//!
//! Paperclip-style agent ergonomics: fan a text search (grep) across the full
//! text of many papers at once, either from an explicit bibcode list or chained
//! onto a search query. Metadata comes from a single ADS bigquery request;
//! open-access bodies are fetched concurrently (bounded) from arXiv.

use crate::client::SciXClient;
use crate::error::{Result, SciXError};
use crate::fulltext::{split_html_sections, FULLTEXT_FIELDS};
use crate::types::{GrepMatch, GrepResult, Paper, Section};
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use tokio::task::JoinSet;

/// Maximum concurrent full-text fetches during a grep fan-out.
const GREP_CONCURRENCY: usize = 4;

/// Options for [`SciXClient::grep`].
#[derive(Debug, Clone, Copy)]
pub struct GrepOptions {
    /// Match case-sensitively (default false).
    pub case_sensitive: bool,
    /// Maximum matches reported per paper (default 5).
    pub max_matches_per_paper: usize,
    /// Approximate characters of context on each side of a match (default 120).
    pub context_chars: usize,
}

impl Default for GrepOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            max_matches_per_paper: 5,
            context_chars: 120,
        }
    }
}

impl SciXClient {
    /// Grep a regex pattern across the full text of a set of papers.
    ///
    /// For each bibcode: searches the open-access body (fetched from arXiv)
    /// when available, falling back to the abstract. Results preserve the input
    /// order; unresolvable bibcodes are reported with `searched: "not_found"`.
    pub async fn grep(
        &self,
        bibcodes: &[&str],
        pattern: &str,
        opts: &GrepOptions,
    ) -> Result<Vec<GrepResult>> {
        let regex = build_regex(pattern, opts.case_sensitive)?;

        // One bigquery call resolves metadata for the whole set.
        let results = self
            .bigquery(
                bibcodes,
                None,
                Some(FULLTEXT_FIELDS),
                None,
                Some(bibcodes.len() as u32),
            )
            .await?;

        let mut papers: HashMap<String, Paper> = results
            .papers
            .into_iter()
            .map(|p| (p.bibcode.clone(), p))
            .collect();

        // Fan out body fetches with bounded concurrency, preserving input order.
        let mut out: Vec<Option<GrepResult>> = (0..bibcodes.len()).map(|_| None).collect();
        let mut set: JoinSet<(usize, GrepResult)> = JoinSet::new();
        let mut pending = bibcodes.iter().enumerate().map(|(i, b)| {
            let paper = papers.remove(*b);
            (i, b.to_string(), paper)
        });

        let spawn_next =
            |set: &mut JoinSet<(usize, GrepResult)>,
             pending: &mut dyn Iterator<Item = (usize, String, Option<Paper>)>| {
                if let Some((i, bibcode, paper)) = pending.next() {
                    let client = self.clone();
                    let regex = regex.clone();
                    let opts = *opts;
                    set.spawn(async move {
                        let result = match paper {
                            Some(p) => grep_paper(&client, p, &regex, &opts).await,
                            None => GrepResult {
                                bibcode,
                                title: String::new(),
                                year: None,
                                searched: "not_found".to_string(),
                                source: None,
                                matches: Vec::new(),
                            },
                        };
                        (i, result)
                    });
                    true
                } else {
                    false
                }
            };

        for _ in 0..GREP_CONCURRENCY {
            if !spawn_next(&mut set, &mut pending) {
                break;
            }
        }
        while let Some(joined) = set.join_next().await {
            let (i, result) =
                joined.map_err(|e| SciXError::Config(format!("grep task failed: {}", e)))?;
            out[i] = Some(result);
            spawn_next(&mut set, &mut pending);
        }

        Ok(out.into_iter().flatten().collect())
    }

    /// Search ADS, then grep the pattern across the resulting papers.
    ///
    /// The `--from` chaining primitive: fan a text extraction across a query's
    /// result set in one call.
    pub async fn grep_query(
        &self,
        query: &str,
        rows: u32,
        pattern: &str,
        opts: &GrepOptions,
    ) -> Result<Vec<GrepResult>> {
        let results = self
            .search_with_options(query, "bibcode,title", None, rows, 0)
            .await?;
        let bibcodes: Vec<String> = results.papers.into_iter().map(|p| p.bibcode).collect();
        let refs: Vec<&str> = bibcodes.iter().map(|s| s.as_str()).collect();
        self.grep(&refs, pattern, opts).await
    }
}

/// Compile the pattern, honoring case sensitivity.
fn build_regex(pattern: &str, case_sensitive: bool) -> Result<Regex> {
    RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .size_limit(1 << 20)
        .build()
        .map_err(|e| SciXError::InvalidQuery(format!("Invalid pattern: {}", e)))
}

/// Grep one paper: full text (with section attribution) when retrievable,
/// otherwise the abstract.
async fn grep_paper(
    client: &SciXClient,
    paper: Paper,
    regex: &Regex,
    opts: &GrepOptions,
) -> GrepResult {
    let mut result = GrepResult {
        bibcode: paper.bibcode,
        title: paper.title,
        year: paper.year,
        searched: "none".to_string(),
        source: None,
        matches: Vec::new(),
    };

    if let Some(arxiv) = &paper.arxiv_id {
        if let Some((html, text, source)) = client.fetch_arxiv_html(arxiv).await {
            result.searched = "fulltext".to_string();
            result.source = Some(source);
            let sections = split_html_sections(&html);
            result.matches = if sections.is_empty() {
                grep_text(
                    &text,
                    None,
                    regex,
                    opts.max_matches_per_paper,
                    opts.context_chars,
                )
            } else {
                grep_sections(&sections, regex, opts)
            };
            return result;
        }
    }

    if let Some(abstract_text) = &paper.abstract_text {
        result.searched = "abstract".to_string();
        result.matches = grep_text(
            abstract_text,
            Some("Abstract"),
            regex,
            opts.max_matches_per_paper,
            opts.context_chars,
        );
    }

    result
}

/// Grep section-by-section, sharing the per-paper match budget across sections.
fn grep_sections(sections: &[Section], regex: &Regex, opts: &GrepOptions) -> Vec<GrepMatch> {
    let mut matches = Vec::new();
    for section in sections {
        let remaining = opts.max_matches_per_paper.saturating_sub(matches.len());
        if remaining == 0 {
            break;
        }
        matches.extend(grep_text(
            &section.text,
            Some(&section.title),
            regex,
            remaining,
            opts.context_chars,
        ));
    }
    matches
}

/// Find up to `max` matches in `text`, returning context snippets.
fn grep_text(
    text: &str,
    section: Option<&str>,
    regex: &Regex,
    max: usize,
    context_chars: usize,
) -> Vec<GrepMatch> {
    regex
        .find_iter(text)
        .take(max)
        .map(|m| GrepMatch {
            section: section.map(String::from),
            snippet: snippet_around(text, m.start(), m.end(), context_chars),
        })
        .collect()
}

/// Extract a single-line context snippet around a match, with ellipses marking
/// clipped edges.
fn snippet_around(text: &str, start: usize, end: usize, context_chars: usize) -> String {
    let from = floor_char_boundary(text, start.saturating_sub(context_chars));
    let to = ceil_char_boundary(text, (end + context_chars).min(text.len()));

    let mut snippet = String::new();
    if from > 0 {
        snippet.push('…');
    }
    snippet.push_str(text[from..to].replace(['\n', '\r'], " ").trim());
    if to < text.len() {
        snippet.push('…');
    }
    snippet
}

fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_char_boundary(s: &str, mut i: usize) -> usize {
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> GrepOptions {
        GrepOptions::default()
    }

    #[test]
    fn grep_text_finds_matches_with_context() {
        let regex = build_regex("hubble constant", false).unwrap();
        let text = "We measure the Hubble constant to be 73 km/s/Mpc using Cepheids.";
        let matches = grep_text(text, Some("Results"), &regex, 5, 20);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].section.as_deref(), Some("Results"));
        assert!(matches[0].snippet.contains("Hubble constant"));
        assert!(matches[0].snippet.contains("73 km/s"));
    }

    #[test]
    fn grep_text_case_sensitivity() {
        let text = "The Hubble constant.";
        let insensitive = build_regex("hubble", false).unwrap();
        assert_eq!(grep_text(text, None, &insensitive, 5, 20).len(), 1);
        let sensitive = build_regex("hubble", true).unwrap();
        assert!(grep_text(text, None, &sensitive, 5, 20).is_empty());
    }

    #[test]
    fn grep_text_respects_max_matches() {
        let regex = build_regex("star", false).unwrap();
        let text = "star star star star star star star";
        assert_eq!(grep_text(text, None, &regex, 3, 5).len(), 3);
    }

    #[test]
    fn grep_sections_shares_budget_and_attributes_sections() {
        let sections = vec![
            Section {
                title: "1 Intro".into(),
                text: "star star star".into(),
            },
            Section {
                title: "2 Methods".into(),
                text: "star star".into(),
            },
        ];
        let regex = build_regex("star", false).unwrap();
        let mut o = opts();
        o.max_matches_per_paper = 4;
        let matches = grep_sections(&sections, &regex, &o);
        assert_eq!(matches.len(), 4);
        assert_eq!(matches[0].section.as_deref(), Some("1 Intro"));
        assert_eq!(matches[3].section.as_deref(), Some("2 Methods"));
    }

    #[test]
    fn snippet_marks_clipped_edges() {
        let text = "aaaaaaaaaa MATCH bbbbbbbbbb";
        let start = text.find("MATCH").unwrap();
        let snippet = snippet_around(text, start, start + 5, 4);
        assert!(snippet.starts_with('…'));
        assert!(snippet.ends_with('…'));
        assert!(snippet.contains("MATCH"));

        // Full-width context has no ellipses.
        let snippet = snippet_around(text, start, start + 5, 100);
        assert!(!snippet.contains('…'));
    }

    #[test]
    fn snippet_safe_on_multibyte_boundaries() {
        let text = "ééééé MATCH ééééé";
        let start = text.find("MATCH").unwrap();
        let snippet = snippet_around(text, start, start + 5, 3);
        assert!(snippet.contains("MATCH"));
    }

    #[test]
    fn invalid_pattern_is_invalid_query_error() {
        let err = build_regex("(unclosed", false).unwrap_err();
        assert!(matches!(err, SciXError::InvalidQuery(_)));
    }
}
