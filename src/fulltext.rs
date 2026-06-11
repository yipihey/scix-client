//! Full-text retrieval.
//!
//! ADS is authoritative for metadata and the citation graph but does not serve
//! licensed article bodies. This module closes the "reading" gap by combining
//! ADS metadata with open-access full text fetched from arXiv (the publisher's
//! native HTML, falling back to ar5iv), giving agents a Paperclip-style ability
//! to actually read a paper — not just cite it.

use crate::client::SciXClient;
use crate::error::{Result, SciXError};
use crate::types::FullText;

/// Fields needed to assemble a [`FullText`]: metadata, abstract, sources, and
/// the identifiers/properties used to locate open-access copies.
pub const FULLTEXT_FIELDS: &str =
    "bibcode,title,author,year,pub,abstract,doi,identifier,esources,property,citation_count";

/// Default cap on retrieved body length (characters) before truncation.
pub const DEFAULT_MAX_CHARS: usize = 40_000;

impl SciXClient {
    /// Retrieve metadata, access links, and (when openly available) the full
    /// text body for a paper.
    ///
    /// Body text is sourced from arXiv when an arXiv identifier is present and
    /// an HTML rendering is available; otherwise `body` is `None` and the caller
    /// should fall back to the resolved `sources` links. `max_chars` bounds the
    /// returned body (0 means "no limit").
    pub async fn fulltext(&self, bibcode: &str, max_chars: usize) -> Result<FullText> {
        let query = format!("identifier:{}", bibcode);
        let results = self
            .search_with_options(&query, FULLTEXT_FIELDS, None, 1, 0)
            .await?;

        let paper = results
            .papers
            .into_iter()
            .next()
            .ok_or_else(|| SciXError::NotFound(format!("Paper not found: {}", bibcode)))?;

        let open_access = paper.properties.iter().any(|p| {
            p.eq_ignore_ascii_case("OPENACCESS") || p.eq_ignore_ascii_case("EPRINT_OPENACCESS")
        });

        let mut full = FullText {
            bibcode: paper.bibcode,
            title: paper.title,
            abstract_text: paper.abstract_text,
            arxiv_id: paper.arxiv_id.clone(),
            open_access,
            sources: paper.pdf_links,
            body: None,
            body_source: None,
            truncated: false,
        };

        if let Some(arxiv) = &paper.arxiv_id {
            if let Some((text, source)) = self.fetch_arxiv_body(arxiv).await {
                let (clipped, truncated) = clip_chars(&text, max_chars);
                full.truncated = truncated;
                full.body = Some(clipped);
                full.body_source = Some(source);
            }
        }

        Ok(full)
    }

    /// Try arXiv's native HTML rendering, then ar5iv, returning extracted text
    /// and a human-readable source label on the first that yields real content.
    async fn fetch_arxiv_body(&self, arxiv_id: &str) -> Option<(String, String)> {
        let id = arxiv_id.trim_start_matches("arXiv:");
        let candidates = [
            (format!("https://arxiv.org/html/{}", id), "arXiv HTML"),
            (format!("https://ar5iv.org/abs/{}", id), "ar5iv HTML"),
        ];

        for (url, label) in candidates {
            if let Some(text) = self.fetch_html_text(&url).await {
                // Guard against "no HTML available" stub pages.
                if text.len() > 500 {
                    return Some((text, format!("{} ({})", label, url)));
                }
            }
        }
        None
    }

    /// Fetch a URL and strip it to plain text. Uses the shared HTTP client
    /// directly — this is an external resource, so it bypasses the ADS base URL,
    /// auth header, and rate limiter.
    async fn fetch_html_text(&self, url: &str) -> Option<String> {
        let response = self
            .http
            .get(url)
            .header("User-Agent", "scix-client")
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            return None;
        }

        let html = response.text().await.ok()?;
        Some(strip_html(&html))
    }
}

/// Truncate `s` to at most `max_chars` characters (0 = unlimited), returning the
/// possibly-truncated string and whether truncation occurred.
fn clip_chars(s: &str, max_chars: usize) -> (String, bool) {
    if max_chars == 0 {
        return (s.to_string(), false);
    }
    match s.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => (s[..byte_idx].to_string(), true),
        None => (s.to_string(), false),
    }
}

/// Convert HTML to readable plain text without pulling in an HTML parser.
///
/// Drops `<script>`, `<style>`, and comments; emits newlines around block-level
/// elements; collapses whitespace; and decodes common/numeric entities.
fn strip_html(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let bytes = html.as_bytes();
    let n = bytes.len();
    let mut out = String::with_capacity(n / 2);
    let mut i = 0;

    while i < n {
        if bytes[i] == b'<' {
            if lower[i..].starts_with("<script") {
                i = skip_until(&lower, i, "</script>", n);
                continue;
            }
            if lower[i..].starts_with("<style") {
                i = skip_until(&lower, i, "</style>", n);
                continue;
            }
            if lower[i..].starts_with("<!--") {
                i = skip_until(&lower, i, "-->", n);
                continue;
            }
            match html[i..].find('>') {
                Some(end) => {
                    if is_block_tag(&lower[i..i + end]) {
                        out.push('\n');
                    }
                    i += end + 1;
                }
                None => break,
            }
            continue;
        }

        let start = i;
        while i < n && bytes[i] != b'<' {
            i += 1;
        }
        out.push_str(&html[start..i]);
    }

    decode_entities(&collapse_whitespace(&out))
}

/// Return the index just past the first `needle` at or after `from`, or `n`.
fn skip_until(lower: &str, from: usize, needle: &str, n: usize) -> usize {
    match lower[from..].find(needle) {
        Some(rel) => from + rel + needle.len(),
        None => n,
    }
}

/// Whether a tag (e.g. `<p`, `</div`, `<h2 class=...`) is block-level, so that
/// stripping it should insert a line break.
fn is_block_tag(tag: &str) -> bool {
    let t = tag.trim_start_matches('<').trim_start_matches('/');
    let name: String = t
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    matches!(
        name.as_str(),
        "p" | "div"
            | "br"
            | "li"
            | "ul"
            | "ol"
            | "tr"
            | "table"
            | "section"
            | "article"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "blockquote"
            | "pre"
            | "figure"
            | "figcaption"
            | "header"
            | "footer"
    )
}

/// Collapse horizontal whitespace within lines and runs of blank lines, so the
/// output stays paragraph-structured but compact.
fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank_pending = false;

    for line in s.lines() {
        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            blank_pending = !out.is_empty();
        } else {
            if blank_pending {
                out.push_str("\n\n");
                blank_pending = false;
            } else if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&collapsed);
        }
    }
    out
}

/// Decode HTML entities (named common ones plus numeric `&#NN;` / `&#xNN;`).
fn decode_entities(s: &str) -> String {
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;

    while i < n {
        if bytes[i] == b'&' {
            if let Some(rel) = s[i..].find(';') {
                if rel <= 12 {
                    if let Some(ch) = decode_one_entity(&s[i + 1..i + rel]) {
                        out.push(ch);
                        i += rel + 1;
                        continue;
                    }
                }
            }
            out.push('&');
            i += 1;
        } else {
            let start = i;
            while i < n && bytes[i] != b'&' {
                i += 1;
            }
            out.push_str(&s[start..i]);
        }
    }
    out
}

/// Decode a single entity body (the text between `&` and `;`).
fn decode_one_entity(ent: &str) -> Option<char> {
    match ent {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        "mdash" => Some('\u{2014}'),
        "ndash" => Some('\u{2013}'),
        _ => {
            if let Some(hex) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else if let Some(dec) = ent.strip_prefix('#') {
                dec.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_keeps_text() {
        let html = "<p>Hello <b>world</b></p>";
        assert_eq!(strip_html(html), "Hello world");
    }

    #[test]
    fn drops_script_and_style() {
        let html = "<style>.x{color:red}</style><p>Visible</p><script>alert(1)</script>";
        assert_eq!(strip_html(html), "Visible");
    }

    #[test]
    fn drops_comments() {
        let html = "<p>Keep<!-- secret -->Me</p>";
        assert_eq!(strip_html(html), "KeepMe");
    }

    #[test]
    fn block_tags_create_paragraphs() {
        let html = "<h1>Title</h1><p>Para one</p><p>Para two</p>";
        let text = strip_html(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Para one"));
        assert!(text.contains("Para two"));
        // Distinct blocks should not be glued together.
        assert!(!text.contains("TitlePara"));
        assert!(!text.contains("onePara"));
    }

    #[test]
    fn decodes_named_and_numeric_entities() {
        let html = "<p>a &amp; b &lt; c &#65; &#x42;</p>";
        assert_eq!(strip_html(html), "a & b < c A B");
    }

    #[test]
    fn unknown_entity_is_preserved() {
        assert_eq!(decode_entities("x &notareal; y"), "x &notareal; y");
    }

    #[test]
    fn clip_respects_char_boundaries() {
        let (clipped, truncated) = clip_chars("héllo", 3);
        assert_eq!(clipped, "hél");
        assert!(truncated);
    }

    #[test]
    fn clip_no_truncation_when_short() {
        let (clipped, truncated) = clip_chars("hi", 10);
        assert_eq!(clipped, "hi");
        assert!(!truncated);
    }

    #[test]
    fn clip_zero_means_unlimited() {
        let (clipped, truncated) = clip_chars("hello", 0);
        assert_eq!(clipped, "hello");
        assert!(!truncated);
    }
}
