//! Full-text retrieval.
//!
//! ADS is authoritative for metadata and the citation graph but does not serve
//! licensed article bodies. This module closes the "reading" gap by combining
//! ADS metadata with open-access full text fetched from arXiv (the publisher's
//! native HTML, falling back to ar5iv), giving agents a Paperclip-style ability
//! to actually read a paper — not just cite it.

use crate::client::SciXClient;
use crate::error::{Result, SciXError};
use crate::types::{FullText, Paper, Section};

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
        let paper = self.fetch_paper_for_fulltext(bibcode).await?;

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
            section_titles: Vec::new(),
        };

        if let Some(arxiv) = &paper.arxiv_id {
            if let Some((html, text, source)) = self.fetch_arxiv_html(arxiv).await {
                let (clipped, truncated) = clip_chars(&text, max_chars);
                full.truncated = truncated;
                full.body = Some(clipped);
                full.body_source = Some(source);
                full.section_titles = split_html_sections(&html)
                    .into_iter()
                    .map(|s| s.title)
                    .collect();
            }
        }

        Ok(full)
    }

    /// Retrieve the full text of a paper split into sections.
    ///
    /// When the open-access HTML has no heading structure, the whole body is
    /// returned as a single "Document" section. Errors if the paper has no
    /// arXiv preprint or no HTML rendering is retrievable.
    pub async fn fulltext_sections(&self, bibcode: &str) -> Result<Vec<Section>> {
        let paper = self.fetch_paper_for_fulltext(bibcode).await?;
        let arxiv = paper.arxiv_id.ok_or_else(|| {
            SciXError::NotFound(format!(
                "No arXiv preprint for {}; full text not retrievable",
                bibcode
            ))
        })?;

        let (html, text, _source) = self.fetch_arxiv_html(&arxiv).await.ok_or_else(|| {
            SciXError::NotFound(format!(
                "No open-access HTML rendering retrievable for {}",
                bibcode
            ))
        })?;

        let sections = split_html_sections(&html);
        if sections.is_empty() {
            return Ok(vec![Section {
                title: "Document".to_string(),
                text,
            }]);
        }
        Ok(sections)
    }

    /// Retrieve a single section of a paper's full text.
    ///
    /// `selector` is either a 1-based section index ("3") or a case-insensitive
    /// substring of the section title ("method").
    pub async fn fulltext_section(&self, bibcode: &str, selector: &str) -> Result<Section> {
        let sections = self.fulltext_sections(bibcode).await?;
        select_section(&sections, selector).cloned().ok_or_else(|| {
            let titles: Vec<String> = sections
                .iter()
                .enumerate()
                .map(|(i, s)| format!("{}. {}", i + 1, s.title))
                .collect();
            SciXError::NotFound(format!(
                "No section matching '{}'. Available sections:\n{}",
                selector,
                titles.join("\n")
            ))
        })
    }

    /// Fetch a single paper with the fields needed for full-text assembly.
    pub(crate) async fn fetch_paper_for_fulltext(&self, bibcode: &str) -> Result<Paper> {
        let query = format!("identifier:{}", bibcode);
        let results = self
            .search_with_options(&query, FULLTEXT_FIELDS, None, 1, 0)
            .await?;

        results
            .papers
            .into_iter()
            .next()
            .ok_or_else(|| SciXError::NotFound(format!("Paper not found: {}", bibcode)))
    }

    /// Try arXiv's native HTML rendering, then ar5iv, returning the raw HTML,
    /// extracted text, and a human-readable source label on the first that
    /// yields real content.
    pub(crate) async fn fetch_arxiv_html(
        &self,
        arxiv_id: &str,
    ) -> Option<(String, String, String)> {
        let id = arxiv_id.trim_start_matches("arXiv:");
        let candidates = [
            (format!("https://arxiv.org/html/{}", id), "arXiv HTML"),
            (format!("https://ar5iv.org/abs/{}", id), "ar5iv HTML"),
        ];

        for (url, label) in candidates {
            if let Some(html) = self.fetch_url(&url).await {
                let text = strip_html(&html);
                // Guard against "no HTML available" stub pages.
                if text.len() > 500 {
                    return Some((html, text, format!("{} ({})", label, url)));
                }
            }
        }
        None
    }

    /// Fetch a URL body. Uses the shared HTTP client directly — this is an
    /// external resource, so it bypasses the ADS base URL, auth header, and
    /// rate limiter.
    async fn fetch_url(&self, url: &str) -> Option<String> {
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

        response.text().await.ok()
    }
}

/// Select a section by 1-based index or case-insensitive title substring.
pub fn select_section<'a>(sections: &'a [Section], selector: &str) -> Option<&'a Section> {
    if let Ok(idx) = selector.trim().parse::<usize>() {
        return (idx >= 1).then(|| sections.get(idx - 1)).flatten();
    }
    let needle = selector.to_lowercase();
    sections
        .iter()
        .find(|s| s.title.to_lowercase().contains(&needle))
}

/// Split HTML into sections at `<h1>`–`<h6>` headings.
///
/// Content before the first heading (title block, authors, abstract in arXiv
/// HTML) becomes a "Front matter" section when non-empty. Returns an empty list
/// when the document has no headings.
pub fn split_html_sections(html: &str) -> Vec<Section> {
    let lower = html.to_ascii_lowercase();
    let mut sections = Vec::new();
    let mut cursor = 0;
    let mut current_title: Option<String> = None;

    while let Some(rel) = find_heading_open(&lower[cursor..]) {
        let open_at = cursor + rel;
        let level = lower.as_bytes()[open_at + 2] - b'0';

        // Heading inner HTML runs from past the opening tag's '>' to '</hN>'.
        let tag_end = match html[open_at..].find('>') {
            Some(e) => open_at + e + 1,
            None => break,
        };
        let close_tag = format!("</h{}>", level);
        let (title_html, after_heading) = match lower[tag_end..].find(&close_tag) {
            Some(c) => (&html[tag_end..tag_end + c], tag_end + c + close_tag.len()),
            None => (&html[tag_end..tag_end], tag_end),
        };

        let body_text = strip_html(&html[cursor..open_at]);
        push_section(&mut sections, current_title.take(), body_text);

        let title = strip_html(title_html);
        current_title = Some(if title.is_empty() {
            "(untitled section)".to_string()
        } else {
            title
        });
        cursor = after_heading;
    }

    if current_title.is_none() {
        // No headings at all.
        return sections;
    }

    let body_text = strip_html(&html[cursor..]);
    push_section(&mut sections, current_title, body_text);
    sections
}

/// Append a pending section. Untitled leading content becomes "Front matter";
/// sections with neither title nor text are dropped.
fn push_section(sections: &mut Vec<Section>, title: Option<String>, text: String) {
    match title {
        Some(title) => sections.push(Section { title, text }),
        None => {
            if !text.is_empty() {
                sections.push(Section {
                    title: "Front matter".to_string(),
                    text,
                });
            }
        }
    }
}

/// Find the byte offset of the next `<hN` heading open tag (N in 1..=6,
/// followed by `>` or whitespace) in already-lowercased HTML.
fn find_heading_open(lower: &str) -> Option<usize> {
    let bytes = lower.as_bytes();
    let mut i = 0;
    while let Some(rel) = lower[i..].find("<h") {
        let at = i + rel;
        if at + 3 < bytes.len()
            && (b'1'..=b'6').contains(&bytes[at + 2])
            && (bytes[at + 3] == b'>' || bytes[at + 3].is_ascii_whitespace())
        {
            return Some(at);
        }
        i = at + 2;
    }
    None
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

    #[test]
    fn split_sections_basic() {
        let html = "<title>Paper</title><p>Authors here</p>\
                    <h2>1 Introduction</h2><p>Intro text.</p>\
                    <h2>2 Methods</h2><p>Methods text.</p>";
        let sections = split_html_sections(html);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].title, "Front matter");
        assert!(sections[0].text.contains("Authors here"));
        assert_eq!(sections[1].title, "1 Introduction");
        assert_eq!(sections[1].text, "Intro text.");
        assert_eq!(sections[2].title, "2 Methods");
        assert_eq!(sections[2].text, "Methods text.");
    }

    #[test]
    fn split_sections_heading_attributes_and_inline_markup() {
        let html = r#"<h2 class="ltx_title">1 <span>Intro</span>duction</h2><p>Body</p>"#;
        let sections = split_html_sections(html);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "1 Introduction");
        assert_eq!(sections[0].text, "Body");
    }

    #[test]
    fn split_sections_no_headings_returns_empty() {
        assert!(split_html_sections("<p>Just a paragraph</p>").is_empty());
    }

    #[test]
    fn split_sections_ignores_non_heading_h_tags() {
        // <header> and <html> start with "<h" but are not headings.
        let html = "<html><header>top</header><h3>Real</h3><p>x</p></html>";
        let sections = split_html_sections(html);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[1].title, "Real");
    }

    #[test]
    fn select_section_by_index_and_substring() {
        let sections = vec![
            Section {
                title: "1 Introduction".into(),
                text: "a".into(),
            },
            Section {
                title: "2 Data and Methods".into(),
                text: "b".into(),
            },
        ];
        assert_eq!(
            select_section(&sections, "2").unwrap().title,
            "2 Data and Methods"
        );
        assert_eq!(
            select_section(&sections, "method").unwrap().title,
            "2 Data and Methods"
        );
        assert_eq!(
            select_section(&sections, "INTRO").unwrap().title,
            "1 Introduction"
        );
        assert!(select_section(&sections, "0").is_none());
        assert!(select_section(&sections, "3").is_none());
        assert!(select_section(&sections, "conclusion").is_none());
    }
}
