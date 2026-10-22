# scix-client

**scix-client** gives your AI assistant direct access to the [SciX](https://scixplorer.org/) / [NASA ADS](https://ui.adsabs.harvard.edu/) database. Search the literature, build bibliographies, analyze citation networks, manage personal libraries, resolve object names via SIMBAD/NED, and export references in BibTeX, AASTeX, or 15 other formats — all from natural language.

It also works standalone: as a CLI tool, a Python library, or a Rust crate.

**Research workflows:** literature reviews, bibliography management, citation analysis, reference export, object resolution, paper details, library collaboration, annotation/notes

| Mode | What it does | Guide |
|------|-------------|-------|
| **MCP server** (`scix serve`) | Expose SciX tools to Claude, Cursor, Zed, Gemini CLI, Codex CLI, Windsurf | [docs/mcp-server.md](docs/mcp-server.md) |
| **Chat REPL** (`scix chat`) | Built-in tool-use loop against any model (Anthropic, OpenAI, Gemini, Ollama) — no MCP host needed | below |
| **CLI** (`scix`) | Command-line tool for your terminal | [docs/cli.md](docs/cli.md) |
| **Python library** (`scix_client`) | Native Python module — `pip install scix-client` | [docs/python.md](docs/python.md) |
| **Rust library** (`scix_client`) | Async Rust crate — `cargo add scix-client` | [docs/rust.md](docs/rust.md) |

One binary (`scix`) does everything. Python bindings are auto-generated from the Rust types — zero extra maintenance.

## Install

```bash
# One-line installer (no Rust toolchain required)
curl -sSf https://raw.githubusercontent.com/yipihey/scix-client/main/install.sh | sh

# From PyPI (also gives you the Python module)
pip install scix-client          # or: uvx scix-client, pipx install scix-client

# From crates.io (compiles locally)
cargo install scix-client --features cli
# or, prebuilt binary via cargo-binstall:
cargo binstall scix-client
```

Verify your setup at any time with:

```bash
scix doctor
```

## Prerequisites

You need a SciX / ADS API token. Get one (free) at:
<https://ui.adsabs.harvard.edu/user/settings/token>

```bash
export SCIX_API_TOKEN="your-token-here"
# or, for backwards compatibility:
export ADS_API_TOKEN="your-token-here"
```

---

## Quick Start: MCP

Give your AI assistant access to the entire SciX database. (Install with any method from [Install](#install) above.)

### Automatic setup (recommended)

```bash
scix setup
```

This detects your installed editors (Claude Code, Claude Desktop, Cursor, Zed, Gemini CLI, Codex CLI, Windsurf), prompts for your API token (offering to open the token page in your browser), validates it, and writes the correct config for each one. Run it once and you're done.

Options:
- `scix setup claude-code` — configure only one editor
- `scix setup --yes` — non-interactive (uses env token, configures all detected editors)
- `scix setup --skip-validation` — skip token validation

### Manual setup

<details>
<summary>Claude Code</summary>

```bash
claude mcp add scix -- /path/to/scix serve
```
</details>

<details>
<summary>Claude Desktop</summary>

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "scix": {
      "command": "/path/to/scix",
      "args": ["serve"],
      "env": { "SCIX_API_TOKEN": "your-token-here" }
    }
  }
}
```
</details>

See [docs/mcp-server.md](docs/mcp-server.md) for Cursor, Zed, full tool reference, and example workflows.

---

## Quick Start: Chat REPL

Drive scix from any tool-use-capable LLM, with no MCP host required. Set one provider's API key and go.

```bash
export ANTHROPIC_API_KEY="sk-ant-..."     # or OPENAI_API_KEY, GEMINI_API_KEY, OLLAMA_HOST
export SCIX_API_TOKEN="your-ads-token"
scix chat                                  # auto-detects provider

# Or pick explicitly:
scix chat --provider openai --model gpt-5
scix chat --provider gemini --model gemini-2.5-pro
scix chat --provider ollama --model llama3.1:70b
```

Inside the REPL, just ask questions. The model uses the same SciX tools the MCP server exposes (search, export, metrics, libraries, citation/reference networks, object name resolution, paper details).

---

## Quick Start: CLI

```bash
scix search 'first_author:"Perlmutter" supernova' --sort "citation_count desc"
scix export 1999ApJ...517..565P --format bibtex
scix cites 1999ApJ...517..565P --rows 50
scix libraries create "Thesis Papers" --description "Key references"
```

See [docs/cli.md](docs/cli.md) for all commands including library permissions, notes, and set operations.

---

## Quick Start: Python

```bash
pip install scix-client
```

```python
import scix_client

client = scix_client.SciXClient()
results = client.search('author:"Einstein" year:1905', rows=5)
for paper in results.papers:
    print(f"{paper.title} ({paper.year}) — {paper.citation_count} citations")
```

See [docs/python.md](docs/python.md) for the full API reference including v0.2.0 library management methods.

---

## Quick Start: Rust

```toml
[dependencies]
scix-client = "0.3"
```

```rust
use scix_client::SciXClient;

#[tokio::main]
async fn main() -> scix_client::error::Result<()> {
    let client = SciXClient::from_env()?;
    let results = client.search("author:\"Einstein\" year:1905", 5).await?;
    for paper in &results.papers {
        println!("{} — {} citations", paper.title, paper.citation_count.unwrap_or(0));
    }
    Ok(())
}
```

See [docs/rust.md](docs/rust.md) for the full API, error handling, and architecture.

---

## Query Syntax Quick Reference

| Pattern | Meaning |
|---------|---------|
| `author:"Einstein"` | Author search |
| `first_author:"Einstein"` | First author only |
| `title:"dark matter"` | Title words |
| `abs:"gravitational waves"` | Abstract words |
| `year:2023` or `year:[2020 TO 2023]` | Year / year range |
| `bibcode:2023ApJ...` | Bibcode |
| `doi:"10.1234/..."` | DOI |
| `object:"M31"` | Astronomical object |
| `property:refereed` | Refereed papers |
| `property:openaccess` | Open access |

**Boolean:** `AND`, `OR`, `NOT`, parentheses
**Functional:** `citations(bibcode:X)`, `references(bibcode:X)`, `similar(bibcode:X)`
**Sort:** `date desc` (default), `citation_count desc`, `score desc`, `read_count desc`

See [docs/query-syntax.md](docs/query-syntax.md) for the full reference including all 17 export formats.

---

## Rate Limiting

The SciX API allows 5,000 requests/day and 5 requests/second. `scix-client` handles rate limiting automatically — a token-bucket rate limiter enforces 5 req/s locally, and API rate-limit headers are respected.

## License

MIT
