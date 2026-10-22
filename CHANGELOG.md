# Changelog

## 0.4.1 — 2026-10-22

### Bug Fixes
- MCP registry listing: description trimmed to the registry's 100-character limit
- PyPI package now includes the README as its long description (also required for the registry's PyPI ownership check)

## 0.4.0 — 2026-10-21

Onboarding release: the goal is that a new user gets a working MCP server in one step.

### Install
- `cli` is now a default feature, so a plain `cargo install scix-client` installs the `scix` binary (it previously compiled and installed nothing)
- Claude Desktop extension bundle: every release ships `scix-vX.Y.Z.mcpb` (universal macOS + Windows) — double-click, paste token, done
- One-line installer `install.sh` for prebuilt binaries (Linux x86_64/aarch64, macOS)
- Linux aarch64 prebuilt binaries
- PyPI wheels are `abi3` (one wheel per platform for all Python ≥ 3.8) and install the `scix` and `scix-client` console scripts, so `pip install scix-client` and `uvx scix-client serve` give you the MCP server
- `cargo binstall` fixed on Windows (zip override)
- Listed in the official MCP registry as `io.github.yipihey/scix`
- "Add to Cursor" and "Install in VS Code" badges

### Token handling
- New token file `~/.config/scix/token` (`%APPDATA%\scix\token` on Windows; `SCIX_CONFIG_DIR` override), written by `scix setup` and the new `scix login`; checked after `SCIX_API_TOKEN` / `ADS_API_TOKEN` by `SciXClient::from_env()` in Rust and Python too
- `scix serve` no longer exits when no token is found: it answers `initialize`/`tools/list` and returns a clear `isError` result with the token URL from tool calls, re-checking for a token on each call

### MCP server
- `ping` supported; `initialize` negotiates protocol versions 2024-11-05 / 2025-03-26 / 2025-06-18 and returns `instructions` with query guidance
- Sharper `scix_search` and `scix_resolve_reference` descriptions (field queries vs complete citations)

### Setup wizard
- Claude Code is registered at user scope (`claude mcp add --scope user`); it was previously local to the directory the wizard ran in
- Without the `claude` CLI the wizard writes `~/.claude.json`, not `~/.claude/settings.json`
- Zed entries use the current flat `command`/`args`/`env` schema
- New editors: Gemini CLI, Codex CLI (TOML), Windsurf; offers to open the token page in a browser
- New `scix doctor` diagnostic and `scix login`

### Bug fixes
- `scix_resolve_reference` / `resolve_references` sent a text/plain body; the endpoint wants JSON and answered "no information received" for every input

### Internal
- CLI moved into the library (`scix_client::cli`) so the binary and the Python console script share it

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
