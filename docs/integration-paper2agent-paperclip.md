# Integrating with Paper2Agent and Paperclip

This note records why scix-client is taking the direction it is, and what is
built versus planned. It is the design rationale behind the `scix_fulltext`
tool and the `scix://paper/{bibcode}/...` resource tree added in 0.4.0.

## The landscape

Two projects from James Zou's group at Stanford are often mentioned together
but are quite different in shape:

- **Paper2Agent** ([arXiv:2509.06917](https://arxiv.org/abs/2509.06917),
  [github.com/jmiao24/Paper2Agent](https://github.com/jmiao24/Paper2Agent)) is a
  *factory*: a multi-agent pipeline that takes a research paper's code
  repository and automatically builds a validated **MCP server** from its
  tutorials and methods. It is not a literature service; it produces MCP
  servers.
- **Paperclip** ([gxl.ai](https://gxl.ai/blog/paperclip/),
  [github.com/GXL-ai/paperclip](https://github.com/GXL-ai/paperclip), from Zou &
  Christine Lemke's company GXL) is an *agent-native literature client*: a CLI +
  **hosted** MCP server over 8M+ full-text papers (arXiv, PubMed Central,
  bioRxiv, medRxiv) and 150M abstracts. Signature ideas: document-as-directory
  virtual filesystem (`/papers/<id>/...`), Unix-shaped primitives (`search`,
  `grep`, `map`, `reduce`, `--from`), and "paper repos" versioned like git. The
  corpus/index is a proprietary GXL-hosted backend; the open-source repo is the
  client.

scix-client, by contrast, is an open Rust library/CLI/Python/MCP client over
**NASA ADS / SciX** — authoritative for astronomy metadata, the curated citation
graph, author disambiguation, libraries, exports, metrics, and object
resolution. Its historical gap was *reading*: it could resolve links to full
text but not retrieve or analyze the text itself.

## Decision: interoperate and borrow, don't fork Paperclip

A "Paperclip+" fork was considered and rejected:

1. **The value isn't forkable.** Paperclip's moat is its hosted 8M-paper index,
   which is not in the open-source repo. A fork yields the CLI shell with no
   corpus.
2. **It would discard scix's strengths.** The ADS citation graph, libraries,
   exports, metrics, and object resolution have no Paperclip equivalent, and the
   single-source-Rust → (crate + CLI + PyO3 + MCP) deployment model is a real
   advantage. A Python fork throws all of that away.
3. **Branding/licensing friction.** Paperclip is a commercial GXL product.

The systems are **complementary, not substitutable**. ADS is the authoritative,
curated source for astrophysics; Paperclip is broad full-text with strong agent
ergonomics. The strategy is therefore to (a) adopt Paperclip's best *interaction
ideas*, (b) close scix's full-text gap, and (c) position scix to interoperate
with both Paperclip (run side by side) and Paper2Agent (as a literature backbone
its generated agents can call).

## Roadmap

### Phase 1 — full-text reading + document-as-directory  ✅ (0.4.0)

- `scix_fulltext` tool / `scix fulltext` CLI / `client.fulltext()` Python:
  abstract + open-access body from arXiv HTML (ar5iv fallback), with access
  links and truncation control. Licensing-safe: it only pulls open-access
  renderings and otherwise returns links.
- `scix://paper/{bibcode}/{part}` resource tree (`metadata`, `abstract`,
  `fulltext`, `references`, `citations`, `links`), advertised via
  `resources/templates/list` — Paperclip's document-as-directory model, mapped
  onto ADS.
- `SCIX_API_URL` override for forward-compatible host configuration.

### Phase 2 — batch/agent ergonomics  ✅ (0.5.0)

- `scix_grep` tool / `scix grep` CLI / `client.grep()` Python: fan a regex
  search across the full text of many papers in one call — Paperclip's `grep`
  primitive on top of ADS. One bigquery resolves metadata for the whole set;
  open-access bodies are fetched concurrently (bounded at 4); matches come back
  as context snippets with section attribution, falling back to the abstract
  when no body is retrievable. scix returns structured chunks; the agent does
  the reasoning.
- `--from`-equivalent chaining: `scix_grep` accepts a `query` instead of
  explicit bibcodes, fanning the extraction across a search's result set
  (`grep_query` in the library).
- Section-addressable retrieval: full text is split on HTML headings;
  `scix_fulltext` takes a `section` selector (index or title substring), and
  the resource tree gains `scix://paper/{bibcode}/sections` and
  `scix://paper/{bibcode}/sections/{selector}`.
- Still open for a later phase: figure/caption access where open-access HTML
  exposes it, and a generic `map` helper for arbitrary tool fan-out.

### Phase 3 — interop & Paper2Agent alignment  (planned)

- Document a recommended **multi-MCP** setup: scix-client (authoritative astro
  metadata + citation graph + exports) alongside Paperclip (broad full text).
- Align tool conventions with Paper2Agent (single-purpose tools, embedded
  prompts/resources, validation tests) and position scix-client as the
  literature/citation backbone that Paper2Agent-generated astronomy agents call
  for context.

## On "fully supporting NASA ADS / SciX"

scix-client already targets the ADS API directly and reads
`SCIX_API_TOKEN` / `ADS_API_TOKEN`. SciX is NASA's rebrand of ADS on the same
API, so both are covered today. The only forward-looking change needed was
making the API host configurable (`SCIX_API_URL`, shipped in 0.4.0) in case SciX
migrates endpoints.
