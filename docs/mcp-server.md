# MCP Server Guide

[Back to README](../README.md)

`scix serve` speaks [MCP](https://modelcontextprotocol.io/) (Model Context Protocol) over stdio, giving AI assistants direct access to the SciX/NASA ADS database. It's the same `scix` binary — no separate install.

## What Your AI Agent Can Do with SciX

Once connected, you can ask your AI assistant things like:

- **"Find all refereed papers by Perlmutter on supernovae from 1995-2000 and export the top 10 as BibTeX"** — combines `scix_search` with field queries and `scix_export`
- **"Here are the 15 papers in my introduction. Use the citation helper to find important papers I might be missing."** — uses `scix_citation_helper` to find co-cited papers not yet in your bibliography
- **"Create a library called 'Thesis Chapter 3', search for papers by Weinberg on the cosmological constant, and add them all"** — chains `scix_library` create with `scix_library_documents` add_by_query
- **"Find recent open-access papers about the Crab Nebula discussing X-ray emission"** — combines `scix_object_search` with property filters
- **"What's the h-index for this set of papers? Show me the citation history."** — uses `scix_metrics` for indicators and histograms

## Claude Desktop: the `.mcpb` extension

Every release ships `scix-vX.Y.Z.mcpb`, a [Claude Desktop extension bundle](https://www.anthropic.com/engineering/desktop-extensions) containing a universal macOS binary and a Windows binary. Download it from the [latest release](https://github.com/yipihey/scix-client/releases/latest), double-click it, and paste your API token into the settings panel Claude Desktop shows. The token is stored in the OS keychain. Nothing else to configure.

## Installing the `scix` binary

For every other host, install `scix` once:

```bash
# One-line installer (prebuilt binary, no Rust toolchain)
curl -sSf https://raw.githubusercontent.com/yipihey/scix-client/main/install.sh | sh

# Python — the wheel installs the same `scix` command
pip install scix-client        # or: pipx install scix-client / uv tool install scix-client

# Rust
cargo install scix-client      # compiles locally
cargo binstall scix-client     # prebuilt binary

# From source
cargo build --release && cp target/release/scix ~/.local/bin/
```

Prebuilt binaries cover Linux (x86_64, aarch64), macOS (Intel, Apple silicon), and Windows (x86_64).

## The API token

Get a free token at <https://ui.adsabs.harvard.edu/user/settings/token>. `scix` looks for it in this order:

1. `SCIX_API_TOKEN` environment variable
2. `ADS_API_TOKEN` environment variable
3. The token file: `~/.config/scix/token` (`$XDG_CONFIG_HOME/scix/token` if set; `%APPDATA%\scix\token` on Windows; `SCIX_CONFIG_DIR` overrides the directory)

```bash
scix login            # prompt for the token, validate it, write the token file (mode 0600)
scix login --token …  # non-interactive
```

GUI editors launch MCP servers without your shell environment, so the token file is what makes a bare `scix serve` work from Claude Desktop, Cursor, VS Code, or an install badge. You can still put `SCIX_API_TOKEN` in a host's `env` block instead.

`scix serve` never refuses to start. Without a token it still answers `initialize` and `tools/list` (so the host shows it as connected) and returns a clear error, with the token URL, from the first tool call. The token is re-read on each call, so running `scix login` takes effect without restarting the host.

## Automatic Setup (Recommended)

```bash
scix setup
```

This will:
1. Find your token (environment or token file) or prompt for it, offering to open the token page in your browser
2. Validate the token against the SciX API and save it to the token file
3. Detect which editors you have installed
4. Write the correct MCP config for each one, using the absolute path of the `scix` binary

```
scix setup                     # Full interactive wizard
scix setup claude-code         # Configure only Claude Code   (user scope: all projects)
scix setup claude-desktop      # Configure only Claude Desktop
scix setup cursor              # Configure only Cursor
scix setup zed                 # Configure only Zed
scix setup gemini-cli          # Configure only Gemini CLI
scix setup codex-cli           # Configure only Codex CLI (OpenAI)
scix setup windsurf            # Configure only Windsurf

Flags:
  --skip-validation   Don't test the API token
  -y, --yes           Non-interactive (use env token, configure all detected editors)
```

Check the result any time with `scix doctor`.

## Manual Setup by Editor

All examples assume you ran `scix login`. Otherwise add `"env": { "SCIX_API_TOKEN": "your-token-here" }` to the entry. GUI editors do not inherit your shell `PATH`; when in doubt use the full path printed by `which scix`.

### Claude Code (CLI)

```bash
claude mcp add --scope user scix -- scix serve
```

`--scope user` registers the server for every project. The default (`local`) scope ties it to the directory you ran the command in. User-scope servers live in `~/.claude.json`; project-scope ones in a `.mcp.json` you can commit:

```json
{ "mcpServers": { "scix": { "command": "scix", "args": ["serve"] } } }
```

### Claude Desktop

Prefer the `.mcpb` above. Otherwise edit `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```json
{
  "mcpServers": {
    "scix": {
      "command": "/full/path/to/scix",
      "args": ["serve"],
      "env": { "SCIX_API_TOKEN": "your-token-here" }
    }
  }
}
```

### Cursor

Click the **Add to Cursor** badge in the README, or add to `~/.cursor/mcp.json` (global) or `.cursor/mcp.json` (project):

```json
{ "mcpServers": { "scix": { "command": "scix", "args": ["serve"] } } }
```

### VS Code (GitHub Copilot)

Click the **Install in VS Code** badge in the README, or add to `.vscode/mcp.json`:

```json
{ "servers": { "scix": { "type": "stdio", "command": "scix", "args": ["serve"] } } }
```

### Zed

In Zed's `settings.json`:

```json
{
  "context_servers": {
    "scix": { "command": "scix", "args": ["serve"], "env": {} }
  }
}
```

(Older Zed releases used a nested `"command": { "path": … }` form; current Zed migrates such entries automatically.)

### Gemini CLI

Add to `~/.gemini/settings.json`:

```json
{ "mcpServers": { "scix": { "command": "scix", "args": ["serve"] } } }
```

### Codex CLI

Add to `~/.codex/config.toml`:

```toml
[mcp_servers.scix]
command = "scix"
args = ["serve"]
```

### Windsurf

Add to `~/.codeium/windsurf/mcp_config.json`:

```json
{ "mcpServers": { "scix": { "command": "scix", "args": ["serve"] } } }
```

### No install: `uvx`

Any host can run the PyPI package on demand if [uv](https://docs.astral.sh/uv/) is installed:

```json
{ "command": "uvx", "args": ["scix-client", "serve"], "env": { "SCIX_API_TOKEN": "your-token-here" } }
```

### Verify It Works

```bash
# Should print three lines: initialize result (with "instructions"), ping result, tool list
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"ping"}
{"jsonrpc":"2.0","id":3,"method":"tools/list"}' | scix serve
```

## Protocol Notes

- Transport: stdio, newline-delimited JSON-RPC 2.0.
- Protocol revisions `2024-11-05`, `2025-03-26`, and `2025-06-18` are accepted and echoed back; unknown requests get `2025-06-18`.
- `initialize` returns an `instructions` string with query-syntax guidance for the model; `ping` is supported.
- Capabilities: `tools` and `resources`. All tools carry annotations (`readOnlyHint`, `destructiveHint`, …).
- Also listed in the [official MCP registry](https://registry.modelcontextprotocol.io/) as `io.github.yipihey/scix`.

## Available Tools

12 tools are exposed over MCP:

| Tool | Description | Read-only |
|------|-------------|-----------|
| `scix_search` | Full-text search with SciX query syntax | Yes |
| `scix_get_paper` | Detailed metadata for a single paper (abstract, affiliations, keywords, links) | Yes |
| `scix_bigquery` | Search within a set of known bibcodes | Yes |
| `scix_export` | Export in 17 citation formats (BibTeX, RIS, AASTeX, ...) | Yes |
| `scix_metrics` | h-index, g-index, citation counts, indicators | Yes |
| `scix_citation_helper` | Find co-cited papers you might be missing | Yes |
| `scix_network` | Author collaboration & paper citation networks | Yes |
| `scix_object_search` | Resolve object names (M31, NGC 1234) via SIMBAD/NED | Yes |
| `scix_resolve_reference` | Convert free-text citations to bibcodes | Yes |
| `scix_resolve_links` | Resolve full-text, data, and reference links | Yes |
| `scix_library` | Create/list/edit/delete libraries + permissions & transfer | No |
| `scix_library_documents` | Add/remove papers, notes, set operations, add by query | No |

## Tool Parameter Details

### scix_search

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | ADS query string (e.g., `author:"Einstein" year:1905`) |
| `rows` | integer | No | Max results (default 10) |
| `start` | integer | No | Starting index for pagination (default 0) |
| `sort` | string | No | Sort order (e.g., `date desc`, `citation_count desc`) |
| `fields` | string | No | Comma-separated fields to return |

### scix_get_paper

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `bibcode` | string | Yes | Paper bibcode |

Returns title, authors, year, publication, abstract, DOI, arXiv ID, citation count, properties, and links.

### scix_bigquery

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `bibcodes` | array[string] | Yes | Bibcodes to search within |
| `query` | string | No | Additional query filter |

### scix_export

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `bibcodes` | array[string] | Yes | Bibcodes to export |
| `format` | string | No | Export format (default: `bibtex`). Options: bibtex, bibtexabs, aastex, mnras, ris, ieee, csl, endnote, medlars, icarus, soph, dcxml, refxml, refabsxml, votable, rss, custom |

### scix_metrics

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `bibcodes` | array[string] | Yes | Bibcodes to get metrics for |

### scix_library

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `action` | string | Yes | One of: `list`, `get`, `create`, `edit`, `delete`, `permissions`, `update_permissions`, `transfer` |
| `id` | string | Varies | Library ID (for get/edit/delete/permissions/update_permissions/transfer) |
| `name` | string | Varies | Library name (for create/edit) |
| `description` | string | No | Library description (for create/edit) |
| `public` | boolean | No | Public visibility (for create/edit) |
| `email` | string | Varies | Collaborator email (for update_permissions/transfer) |
| `permission` | string | Varies | Permission level: `owner`, `admin`, `write`, `read` (for update_permissions) |

### scix_library_documents

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `action` | string | Yes | One of: `add`, `remove`, `get_notes`, `add_note`, `edit_note`, `delete_note`, `union`, `intersection`, `difference`, `copy`, `empty`, `add_by_query` |
| `library_id` | string | Yes | Library ID |
| `bibcodes` | array[string] | Varies | Bibcodes to add/remove |
| `bibcode` | string | Varies | Single bibcode (for note operations) |
| `content` | string | Varies | Note content (for add_note/edit_note) |
| `libraries` | array[string] | Varies | Source library IDs (for union/intersection/difference/copy) |
| `query` | string | Varies | Search query (for add_by_query) |
| `rows` | integer | No | Max documents to add by query (default 50) |

### scix_citation_helper

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `bibcodes` | array[string] | Yes | Bibcodes for co-citation analysis |

### scix_network

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `bibcodes` | array[string] | Yes | Bibcodes for network analysis |
| `type` | string | No | `author` (default) or `paper` |

### scix_object_search

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `objects` | array[string] | Yes | Object names (e.g., M31, NGC 1234, Crab Nebula) |

### scix_resolve_reference

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `references` | array[string] | Yes | Free-text reference strings |

### scix_resolve_links

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `bibcode` | string | Yes | Paper bibcode |
| `link_type` | string | No | Specific link type: `esource`, `data`, `citation`, `reference`, `coreads` |

## MCP Resources

Two read-only resources are available:

| URI | Content |
|-----|---------|
| `scix://fields` | Searchable and returnable field names |
| `scix://syntax` | Query syntax quick reference |

## Tips for Best Results

- **Use `scix_get_paper`** when you need the abstract, affiliations, or full metadata for a single paper — it returns richer fields than `scix_search`.
- **Paginate with `start`** — if a search returns 500 results, use `start=10`, `start=20`, etc. to page through them.
- **Use `scix_citation_helper`** to find bibliography gaps — give it the bibcodes from your paper's reference list and it returns frequently co-cited papers you haven't included.
- **Chain tools** — search for papers, then export them, then add them to a library, all in one conversation.
- **Use functional operators** in search queries: `citations(bibcode:X)` to find papers citing X, `references(bibcode:X)` for its reference list, `similar(bibcode:X)` for related work.
- **Filter by properties** — add `property:refereed` for peer-reviewed only, `property:openaccess` for freely available papers.
