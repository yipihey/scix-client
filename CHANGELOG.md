# Changelog

## 0.5.0 — 2026-06-11

### Grep across papers (Paper2Agent / Paperclip integration, phase 2)
- New `scix_grep` MCP tool, `scix grep` CLI command, and Python `client.grep()` — fan a regex search across the full text of many papers in one call, returning context snippets with section attribution. Searches the open-access body (from arXiv) when retrievable, falling back to the abstract; results report what was searched.
- Query chaining: pass a search `query` instead of explicit bibcodes to grep directly over a result set (`grep_query` in the library) — the `--from` equivalent of Paperclip's pipeline model.
- Metadata for the whole set is resolved in a single bigquery request; body fetches run concurrently (bounded at 4).
- New `GrepResult`/`GrepMatch` types and `GrepOptions` (case sensitivity, per-paper match budget, context width).

### Section-addressable full text
- Full text is now split into sections on HTML headings: `FullText.section_titles`, Python `client.fulltext_sections()` / `client.fulltext_section()`, a `section` parameter on `scix_fulltext` and `scix fulltext --section` (1-based index or case-insensitive title substring).
- New resources: `scix://paper/{bibcode}/sections` (list) and `scix://paper/{bibcode}/sections/{selector}` (one section).

### Dependencies
- Added `regex` for grep pattern matching.

## 0.4.0 — 2026-06-11

### Full-text reading (Paper2Agent / Paperclip integration, phase 1)
- New `scix_fulltext` MCP tool, `scix fulltext` CLI command, and Python `client.fulltext()` method — retrieve a paper's abstract plus its open-access body text, fetched from arXiv's native HTML (falling back to ar5iv). Closes the "read, don't just cite" gap relative to Paperclip while keeping ADS as the authoritative metadata/citation source.
- New `FullText` type exposing `body`, `body_source`, `sources` (access links), `open_access`, and `truncated`.

### Document-as-directory resources
- Papers are now navigable as a resource tree via `resources/templates/list`: `scix://paper/{bibcode}/{part}` where `part` is one of `metadata`, `abstract`, `fulltext`, `references`, `citations`, `links`.

### Configuration
- `from_env()` now honors a `SCIX_API_URL` environment variable to override the API base URL, easing any future SciX endpoint migration.

### Notes
- See `docs/integration-paper2agent-paperclip.md` for the integration strategy and roadmap.

## 0.3.1 — 2026-03-05

### Enhancements
- Library `get` and `list` actions now return full paper details (title, authors, year) instead of bare bibcodes

### Bug Fixes
- Ensure `bibcode` is always included in search result fields, even when custom fields are requested

## 0.3.0 — 2026-02-28

### Setup Wizard
- New `scix setup` command — one-command MCP server configuration
- Auto-detects installed editors (Claude Code, Claude Desktop, Cursor, Zed)
- Prompts for API token, validates against the SciX API
- Writes correct MCP config for each detected editor
- Supports `--yes` for non-interactive mode and `--skip-validation`
- Idempotent — safe to run multiple times

## 0.2.0 — 2026-02-28

### Library Management
- Permissions: get, update, and grant collaborator access (owner/admin/write/read)
- Transfer library ownership
- Annotations/notes on individual papers in libraries
- Set operations: union, intersection, difference, copy, empty
- Add documents to a library by search query

### MCP Server
- New `scix_get_paper` tool for detailed single-paper metadata
- Tool annotations on all 12 tools (readOnlyHint, destructiveHint, etc.)
- Search pagination via `start` parameter
- Expanded `scix_library`: permissions, update_permissions, transfer actions
- Expanded `scix_library_documents`: notes, set operations, add_by_query

### CLI
- New subcommands: permissions, grant, transfer, notes, ops

### Python
- 8 new methods: get_permissions, update_permissions, transfer_library, get/set/delete_annotation, library_operation, add_documents_by_query

### Documentation
- Restructured: README as landing page; detailed guides in docs/

## 0.1.0 — 2026-02-27

Initial release. Rust client for the SciX (NASA ADS) API.

- Async Rust library with 11 API endpoints
- CLI tool (`scix`) with table and JSON output
- MCP server (`scix serve`) with 11 tools and 2 resources
- Python bindings via PyO3 (`import scix_client`)
- Token-bucket rate limiter (5 req/s, respects API headers)
- Query builder with fluent API
