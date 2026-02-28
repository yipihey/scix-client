# scix-client

A Rust client for the [SciX](https://scixplorer.org/) (formerly [NASA ADS](https://ui.adsabs.harvard.edu/)) API.

Four ways to use it:

| Mode | What it does |
|------|-------------|
| **Rust library** (`scix_client`) | Async Rust crate — add to your `Cargo.toml` |
| **Python library** (`scix_client`) | Native Python module — `pip install .` via maturin |
| **CLI** (`scix`) | Command-line tool for your terminal |
| **MCP server** (`scix serve`) | Expose SciX tools to Claude, Cursor, Zed, etc. |

One binary (`scix`) does everything. The MCP server is `scix serve`.
Python bindings are auto-generated from the Rust types — zero extra maintenance.

## Prerequisites

You need a SciX / ADS API token. Get one (free) at:
<https://ui.adsabs.harvard.edu/user/settings/token>

Then export it:

```bash
export SCIX_API_TOKEN="your-token-here"
# or, for backwards compatibility:
export ADS_API_TOKEN="your-token-here"
```

---

## Installation

### CLI / MCP binary

```bash
# From crates.io (recommended)
cargo install scix-client --features cli

# Pre-built binary (via cargo-binstall — no compile needed)
cargo binstall scix-client

# From source
cargo build --features cli --release
cp target/release/scix ~/.local/bin/
```

### As a Rust dependency

```toml
[dependencies]
scix-client = "0.1"
```

### Python

```bash
# From PyPI (recommended)
pip install scix-client

# From source (requires Rust toolchain)
pip install maturin
maturin develop          # install into current virtualenv for development
```

Requires Python 3.8+.

---

## MCP Server Setup

`scix serve` speaks [MCP](https://modelcontextprotocol.io/) (Model Context Protocol) over stdio, giving AI assistants direct access to the SciX API. It's the same `scix` binary — no separate install needed.

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

### Verify it works

```bash
# Should print the tool list as JSON
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | SCIX_API_TOKEN=your-token scix serve
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `scix_search` | Full-text search with SciX query syntax |
| `scix_bigquery` | Search within a set of known bibcodes |
| `scix_export` | Export in 17 citation formats (BibTeX, RIS, AASTeX, ...) |
| `scix_metrics` | h-index, g-index, citation counts, indicators |
| `scix_library` | Create/list/edit/delete personal SciX libraries |
| `scix_library_documents` | Add/remove papers from libraries |
| `scix_citation_helper` | Find co-cited papers you might be missing |
| `scix_network` | Author collaboration & paper citation networks |
| `scix_object_search` | Resolve object names (M31, NGC 1234) via SIMBAD/NED |
| `scix_resolve_reference` | Convert free-text citations to bibcodes |
| `scix_resolve_links` | Resolve full-text, data, and reference links |

### MCP Resources

| URI | Content |
|-----|---------|
| `scix://fields` | Searchable and returnable field names |
| `scix://syntax` | Query syntax quick reference |

---

## CLI Examples

All examples assume `SCIX_API_TOKEN` (or `ADS_API_TOKEN`) is set in your environment.

### Searching

```bash
# Basic search
scix search "dark matter"

# Search by author
scix search 'author:"Einstein"'

# Author + year range
scix search 'author:"Weinberg" year:[1965 TO 1975]'

# First-author search, most cited first
scix search 'first_author:"Perlmutter" supernova' --sort "citation_count desc"

# Title search
scix search 'title:"cosmological constant problem"'

# Abstract search
scix search 'abs:"gravitational waves" year:2016'

# Combine fields with boolean operators
scix search 'author:"Hawking" AND title:"black hole" AND year:[1970 TO 1980]'

# Refereed papers only
scix search 'author:"Witten" property:refereed' --rows 20

# Papers about an astronomical object
scix search 'object:"Crab Nebula" year:[2020 TO 2025]'

# Search by journal (bibstem)
scix search 'bibstem:ApJ year:2024 title:"exoplanet atmosphere"'

# Search by DOI
scix search 'doi:"10.1103/PhysRevLett.116.061102"'

# Search by arXiv ID
scix search 'identifier:arXiv:1602.03837'

# Search by ORCID
scix search 'orcid:0000-0002-1825-0097'

# Open access papers only
scix search 'title:"machine learning" AND property:openaccess year:2024'

# Get more results
scix search "galaxy clusters weak lensing" --rows 50

# Output as JSON (for scripting)
scix search 'author:"Planck Collaboration" year:2018' --output json

# Custom fields
scix search 'author:"Einstein" year:1905' --fields "bibcode,title,citation_count"
```

### Exporting citations

```bash
# BibTeX (default)
scix export 2023ApJ...123..456A

# Multiple papers
scix export 2023ApJ...123..456A 2024MNRAS.789..012B 1998AJ....116.1009R

# Different formats
scix export 2023ApJ...123..456A --format bibtex
scix export 2023ApJ...123..456A --format aastex
scix export 2023ApJ...123..456A --format mnras
scix export 2023ApJ...123..456A --format ris
scix export 2023ApJ...123..456A --format ieee
scix export 2023ApJ...123..456A --format endnote

# Save to file
scix export 2023ApJ...123..456A 2024MNRAS.789..012B --format bibtex > refs.bib

# Pipe a search into an export (with jq)
scix search 'author:"Einstein" year:1905' --output json \
  | jq -r '.papers[].bibcode' \
  | xargs scix export --format bibtex
```

### References and citations

```bash
# Papers referenced by a paper
scix refs 2023ApJ...123..456A

# Papers that cite a paper
scix cites 2023ApJ...123..456A

# Show more results
scix refs 1998AJ....116.1009R --rows 100

# Similar papers (content-based)
scix similar 2023ApJ...123..456A

# JSON output for further processing
scix cites 2023ApJ...123..456A --output json | jq '.papers | length'
```

### Citation metrics

```bash
# Metrics for one paper
scix metrics 2023ApJ...123..456A

# Metrics for a set of papers (h-index, g-index, etc.)
scix metrics 2023ApJ...123..456A 2024MNRAS.789..012B 1998AJ....116.1009R
```

Sample output:
```json
{
  "basic_stats": {
    "total": { "number_of_papers": 3, "total_citations": 5821 }
  },
  "indicators": {
    "h": 3, "g": 3, "i10": 3, "tori": 142.7
  }
}
```

### Resolving references

```bash
# Free-text reference to bibcode
scix resolve "Einstein 1905 Annalen der Physik 17 891"

# Multiple references
scix resolve \
  "Perlmutter et al. 1999 ApJ 517 565" \
  "Riess et al. 1998 AJ 116 1009"

# JSON output
scix resolve "Weinberg 1989 Rev Mod Phys 61 1" --output json
```

### Astronomical objects

```bash
# Find papers about an object
scix objects "M31"

# Multiple objects
scix objects "M31" "NGC 1234" "Crab Nebula"
```

### Link resolution

```bash
# All links for a paper (full-text, data, etc.)
scix links 2023ApJ...123..456A

# Specific link type
scix links 2023ApJ...123..456A --link-type esource
scix links 2023ApJ...123..456A --link-type data
```

### Library management

```bash
# List your libraries
scix libraries list

# Get library details (includes bibcodes)
scix libraries get abc123def

# Create a library
scix libraries create "My Reading List" --description "Papers to read this week"

# Create a public library
scix libraries create "Dark Energy Review" --description "Key papers" --public

# Delete a library
scix libraries delete abc123def

# JSON output
scix libraries list --output json
```

### MCP server

```bash
# Start MCP server (reads JSON-RPC from stdin, writes to stdout)
scix serve
```

This is the same entry point used by Claude, Cursor, Zed, etc. (see MCP Server Setup above).

---

## Library Usage (Rust)

### Basic search

```rust
use scix_client::SciXClient;

#[tokio::main]
async fn main() -> scix_client::error::Result<()> {
    let client = SciXClient::from_env()?;

    let results = client.search("author:\"Einstein\" year:1905", 10).await?;
    for paper in &results.papers {
        println!("{} ({}) — {} citations",
            paper.title,
            paper.year.unwrap_or(0),
            paper.citation_count.unwrap_or(0),
        );
    }
    Ok(())
}
```

### Query builder

```rust
use scix_client::{SciXClient, QueryBuilder};

let query = QueryBuilder::new()
    .first_author("Weinberg")
    .and()
    .title("cosmological constant")
    .and()
    .property("refereed")
    .build();
// → first_author:"Weinberg" AND title:"cosmological constant" AND property:refereed

let results = client.search(&query, 20).await?;
```

### Export BibTeX

```rust
let bibtex = client.export_bibtex(&["2023ApJ...123..456A", "1998AJ....116.1009R"]).await?;
println!("{}", bibtex);

// Other formats
use scix_client::ExportFormat;
let ris = client.export(&["2023ApJ...123..456A"], ExportFormat::Ris, None).await?;
```

### References and citations

```rust
let refs = client.references("2023ApJ...123..456A", 50).await?;
let cites = client.citations("2023ApJ...123..456A", 50).await?;
let similar = client.similar("2023ApJ...123..456A", 10).await?;
```

### Metrics

```rust
let metrics = client.metrics(&["2023ApJ...123..456A"]).await?;
if let Some(indicators) = &metrics.indicators {
    println!("h-index: {:?}", indicators.h);
}
```

### Custom base URL (for testing)

```rust
let client = SciXClient::new("my-token")
    .with_base_url("https://api.scixplorer.org/v1");
```

---

## Library Usage (Python)

The Python module is auto-generated from the Rust crate via [PyO3](https://pyo3.rs/) + [maturin](https://www.maturin.rs/). All types, fields, and methods are derived directly from the Rust source — no separate Python code to maintain.

### Basic search

```python
import scix_client

# Create client (reads SCIX_API_TOKEN or ADS_API_TOKEN env var)
client = scix_client.SciXClient()
# or: client = scix_client.SciXClient("your-token")

results = client.search('author:"Einstein" year:1905', rows=10)
for paper in results.papers:
    print(f"{paper.title} ({paper.year}) — {paper.citation_count} citations")
    for author in paper.authors:
        print(f"  {author.display_name()}")

print(f"Total: {results.num_found} papers found")
```

### Query builder

```python
q = scix_client.QueryBuilder()
q.author("Weinberg")
q.and_()
q.title("cosmological constant")
q.and_()
q.property("refereed")
results = client.search(q.build(), rows=20)

# Static constructors
q = scix_client.QueryBuilder.citations_of("2023ApJ...123..456A")
results = client.search(q.build(), rows=100)
```

### Export citations

```python
# BibTeX (default)
bibtex = client.export_bibtex(["2023ApJ...123..456A", "1998AJ....116.1009R"])
print(bibtex)

# Other formats
ris = client.export(["2023ApJ...123..456A"], format=scix_client.ExportFormat.Ris)
```

### References, citations, and metrics

```python
refs = client.references("2023ApJ...123..456A", rows=50)
cites = client.citations("2023ApJ...123..456A", rows=50)
similar = client.similar("2023ApJ...123..456A")

metrics = client.metrics(["2023ApJ...123..456A"])
if metrics.indicators:
    print(f"h-index: {metrics.indicators.h}")
    print(f"g-index: {metrics.indicators.g}")
```

### Libraries

```python
# List your libraries
for lib in client.list_libraries():
    print(f"{lib.name}: {lib.num_documents} papers")

# Create a library
lib = client.create_library("My Reading List", description="Papers to read")
client.add_documents(lib.id, ["2023ApJ...123..456A"])
```

### Reference and object resolution

```python
# Resolve free-text references
resolved = client.resolve_references([
    "Einstein 1905 Annalen der Physik 17 891",
    "Perlmutter et al. 1999 ApJ 517 565",
])
for ref in resolved:
    print(f"{ref.reference} → {ref.bibcode}")

# Resolve astronomical objects (returns dict)
objects = client.resolve_objects(["M31", "Crab Nebula"])
```

### Sort control

```python
sort = scix_client.Sort.citation_count_desc()
results = client.search_with_options("dark matter", sort=sort, rows=20)
```

### Available types

All types are auto-exposed with read-only field access:

| Python class | Key fields |
|-------------|------------|
| `SciXClient` | `search()`, `export()`, `metrics()`, ... |
| `QueryBuilder` | `author()`, `title()`, `year()`, `build()`, ... |
| `Paper` | `bibcode`, `title`, `authors`, `year`, `doi`, `arxiv_id`, ... |
| `Author` | `name`, `family_name`, `given_name`, `display_name()` |
| `SearchResponse` | `papers`, `num_found` |
| `ExportFormat` | `BibTeX`, `Ris`, `AasTex`, ... (17 formats) |
| `Metrics` | `basic_stats`, `citation_stats`, `indicators` |
| `Indicators` | `h`, `g`, `i10`, `i100`, `m`, `tori`, `riq`, `read10` |
| `Sort` | `field`, `direction` |
| `Library` | `id`, `name`, `description`, `num_documents` |

---

## Query Syntax Quick Reference

| Pattern | Meaning |
|---------|---------|
| `author:"Einstein"` | Author search |
| `first_author:"Einstein"` | First author only |
| `title:"dark matter"` | Title words |
| `abs:"gravitational waves"` | Abstract words |
| `full:"spectroscopy"` | Full text |
| `year:2023` | Exact year |
| `year:[2020 TO 2023]` | Year range |
| `bibcode:2023ApJ...` | Bibcode |
| `doi:"10.1234/..."` | DOI |
| `identifier:arXiv:2301.12345` | arXiv ID |
| `bibstem:ApJ` | Journal abbreviation |
| `object:"M31"` | Astronomical object |
| `orcid:0000-0002-...` | ORCID identifier |
| `property:refereed` | Refereed papers |
| `property:openaccess` | Open access |
| `doctype:article` | Document type |

**Boolean operators:** `AND`, `OR`, `NOT`, parentheses for grouping
**Functional operators:** `citations(bibcode:X)`, `references(bibcode:X)`, `similar(bibcode:X)`, `trending(bibcode:X)`
**Wildcards:** `author:"Eins*"`, `title:galax?`

**Sort options:** `date desc` (default), `citation_count desc`, `score desc`, `read_count desc`

---

## Export Formats

| Format | Flag | Description |
|--------|------|-------------|
| `bibtex` | `--format bibtex` | BibTeX (default) |
| `bibtexabs` | `--format bibtexabs` | BibTeX with abstracts |
| `aastex` | `--format aastex` | AAS journals (ApJ, AJ, etc.) |
| `icarus` | `--format icarus` | Icarus journal |
| `mnras` | `--format mnras` | MNRAS journal |
| `soph` | `--format soph` | Solar Physics journal |
| `ris` | `--format ris` | RIS (Reference Manager) |
| `endnote` | `--format endnote` | EndNote |
| `medlars` | `--format medlars` | MEDLARS/PubMed |
| `ieee` | `--format ieee` | IEEE |
| `csl` | `--format csl` | CSL-JSON |
| `dcxml` | `--format dcxml` | Dublin Core XML |
| `refxml` | `--format refxml` | Reference XML |
| `refabsxml` | `--format refabsxml` | Ref + Abstract XML |
| `votable` | `--format votable` | VOTable |
| `rss` | `--format rss` | RSS feed |
| `custom` | `--format custom` | Custom format |

---

## Rate Limiting

The SciX API allows 5,000 requests/day and 5 requests/second. `scix-client` handles rate limiting automatically:

- A token-bucket rate limiter enforces 5 req/s locally
- Rate limit headers (`x-ratelimit-remaining`, `x-ratelimit-reset`) are respected
- If rate-limited (HTTP 429), the error includes the retry-after duration

---

## Architecture

```
┌──────────────────────────────────────────────┐
│  scix binary                                 │  ← Single binary
│  ┌────────────┐ ┌────────────┐               │
│  │ CLI (clap) │ │ MCP server │               │  scix search … / scix serve
│  └────────────┘ └────────────┘               │
├──────────────────────────────────────────────┤
│  Python bindings (PyO3)                      │  ← import scix_client
│  ┌───────────────┐ ┌────────────────────┐    │
│  │ PySciXClient  │ │ PyQueryBuilder     │    │  Auto-generated from
│  │ (sync wrapper)│ │ (mutation wrapper) │    │  Rust types via #[pyclass]
│  └───────────────┘ └────────────────────┘    │
├──────────────────────────────────────────────┤
│  scix_client Rust library                    │  ← Async Rust API
│  ┌───────────┐ ┌──────────────┐              │
│  │ SciXClient│ │ QueryBuilder │              │
│  └───────────┘ └──────────────┘              │
│  ┌──────────┐ ┌──────────────┐               │
│  │ Parser   │ │ Rate Limiter │               │
│  └──────────┘ └──────────────┘               │
├──────────────────────────────────────────────┤
│  reqwest + tokio                             │  ← HTTP + async runtime
└──────────────────────────────────────────────┘
         │
         ▼
   SciX API (api.adsabs.harvard.edu/v1)
```

## License

MIT
