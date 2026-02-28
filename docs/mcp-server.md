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

## Installation

```bash
# From crates.io (recommended)
cargo install scix-client --features cli

# Pre-built binary (via cargo-binstall — no compile needed)
cargo binstall scix-client

# From source
cargo build --features cli --release
cp target/release/scix ~/.local/bin/
```

You need a SciX/ADS API token (free): <https://ui.adsabs.harvard.edu/user/settings/token>

```bash
export SCIX_API_TOKEN="your-token-here"
```

## Automatic Setup (Recommended)

The easiest way to configure your editors is with the setup wizard:

```bash
scix setup
```

This will:
1. Find or prompt for your API token
2. Validate the token against the SciX API
3. Detect which editors you have installed
4. Write the correct MCP config for each one

Options:
```
scix setup                     # Full interactive wizard
scix setup claude-code         # Configure only Claude Code
scix setup claude-desktop      # Configure only Claude Desktop
scix setup cursor              # Configure only Cursor
scix setup zed                 # Configure only Zed

Flags:
  --skip-validation   Don't test the API token
  -y, --yes           Non-interactive (use env token, configure all detected editors)
```

## Manual Setup by Editor

If you prefer to configure editors manually:

### Claude Code (CLI)

```bash
claude mcp add scix -- /path/to/scix serve
```

Or add manually to `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "scix": {
      "command": "/path/to/scix",
      "args": ["serve"],
      "env": {
        "SCIX_API_TOKEN": "your-token-here"
      }
    }
  }
}
```

### Claude Desktop

Edit `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS)
or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```json
{
  "mcpServers": {
    "scix": {
      "command": "/path/to/scix",
      "args": ["serve"],
      "env": {
        "SCIX_API_TOKEN": "your-token-here"
      }
    }
  }
}
```

### Cursor

In Cursor Settings > MCP, add:

```json
{
  "mcpServers": {
    "scix": {
      "command": "/path/to/scix",
      "args": ["serve"],
      "env": {
        "SCIX_API_TOKEN": "your-token-here"
      }
    }
  }
}
```

### Zed

In Zed settings (`settings.json`):

```json
{
  "context_servers": {
    "scix": {
      "command": {
        "path": "/path/to/scix",
        "args": ["serve"],
        "env": {
          "SCIX_API_TOKEN": "your-token-here"
        }
      }
    }
  }
}
```

### Verify It Works

```bash
# Should print the tool list as JSON
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | SCIX_API_TOKEN=your-token scix serve
```

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
