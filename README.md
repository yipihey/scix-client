# scix-client

**scix-client** gives your AI assistant direct access to the [SciX](https://scixplorer.org/) / [NASA ADS](https://ui.adsabs.harvard.edu/) database. Search the literature, build bibliographies, analyze citation networks, manage personal libraries, resolve object names via SIMBAD/NED, and export references in BibTeX, AASTeX, or 15 other formats — all from natural language.

It also works standalone: as a CLI tool, a Python library, or a Rust crate.

<!-- mcp-name: io.github.yipihey/scix -->

**Research workflows:** literature reviews, bibliography management, citation analysis, reference export, object resolution, paper details, library collaboration, annotation/notes

| Mode | What it does | Guide |
|------|-------------|-------|
| **MCP server** (`scix serve`) | Expose SciX tools to Claude, Cursor, VS Code, Zed, Gemini CLI, Codex CLI, Windsurf | [docs/mcp-server.md](docs/mcp-server.md) |
| **CLI** (`scix`) | Command-line tool for your terminal | [docs/cli.md](docs/cli.md) |
| **Python library** (`scix_client`) | Native Python module — `pip install scix-client` | [docs/python.md](docs/python.md) |
| **Rust library** (`scix_client`) | Async Rust crate — `cargo add scix-client` | [docs/rust.md](docs/rust.md) |

One binary (`scix`) does everything. Python bindings are auto-generated from the Rust types — zero extra maintenance.

You need a free SciX / ADS API token from <https://ui.adsabs.harvard.edu/user/settings/token>. Every path below asks for it once and stores it for you.

---

## Claude Desktop: one-click install

1. Download **`scix-vX.Y.Z.mcpb`** from the [latest release](https://github.com/yipihey/scix-client/releases/latest) (macOS universal + Windows in one file).
2. Double-click it (or drag it onto Claude Desktop → Settings → Extensions).
3. Paste your API token when asked. Done — no terminal, no PATH, no JSON.

---

## Everything else: install `scix`, then `scix setup`

Pick any one installer:

```bash
# One-line installer (prebuilt binary, no Rust toolchain)
curl -sSf https://raw.githubusercontent.com/yipihey/scix-client/main/install.sh | sh

# Python (also gives you the scix_client module)
pip install scix-client        # or: pipx install scix-client / uv tool install scix-client

# Rust
cargo install scix-client      # compiles locally
cargo binstall scix-client     # prebuilt binary via cargo-binstall
```

Then:

```bash
scix setup
```

The wizard asks for your token (offering to open the token page), validates it, saves it to `~/.config/scix/token`, and registers the server with every editor it finds: Claude Code, Claude Desktop, Cursor, Zed, Gemini CLI, Codex CLI, Windsurf. Run it again any time; it is idempotent.

- `scix setup claude-code` — configure only one editor
- `scix setup --yes` — non-interactive (uses `SCIX_API_TOKEN`, configures all detected editors)
- `scix login` — just save the token, touch no editor configs
- `scix doctor` — check binary, PATH, token, API connectivity, and detected editors

### Install badges

After `scix login`, any MCP host can run plain `scix serve` with no environment setup:

[![Add to Cursor](https://cursor.com/deeplink/mcp-install-dark.svg)](https://cursor.com/en/install-mcp?name=scix&config=eyJjb21tYW5kIjoic2NpeCIsImFyZ3MiOlsic2VydmUiXX0=)
[![Install in VS Code](https://img.shields.io/badge/VS_Code-Install_scix-0098FF?style=for-the-badge&logo=visualstudiocode&logoColor=white)](https://vscode.dev/redirect/mcp/install?name=scix&config=%7B%22command%22%3A%22scix%22%2C%22args%22%3A%5B%22serve%22%5D%7D)

GUI editors do not see your shell `PATH`. If a badge-installed server cannot find `scix`, run `scix setup cursor` (or whichever editor) once; it writes the absolute path.

### No install at all

With [uv](https://docs.astral.sh/uv/) present, this works as the command in any MCP config:

```json
{ "command": "uvx", "args": ["scix-client", "serve"], "env": { "SCIX_API_TOKEN": "your-token" } }
```

### Manual setup

<details>
<summary>Claude Code</summary>

```bash
claude mcp add --scope user scix -- scix serve
```

(`--scope user` makes it available in every project; the default scope is the current directory only.)
</details>

<details>
<summary>Claude Desktop (manual, instead of the .mcpb)</summary>

Add to `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows), using the full path from `which scix`:

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
</details>

See [docs/mcp-server.md](docs/mcp-server.md) for Cursor, VS Code, Zed, Gemini CLI, Codex CLI, Windsurf, the full tool reference, and example workflows.

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

client = scix_client.SciXClient()   # token from SCIX_API_TOKEN or ~/.config/scix/token
results = client.search('author:"Einstein" year:1905', rows=5)
for paper in results.papers:
    print(f"{paper.title} ({paper.year}) — {paper.citation_count} citations")
```

The wheel also installs the `scix` command, so `pip install scix-client && scix setup` is a complete MCP setup. See [docs/python.md](docs/python.md) for the full API reference.

---

## Quick Start: Rust

```toml
[dependencies]
scix-client = { version = "0.4", default-features = false }   # library only, no CLI deps
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

## Token lookup order

`scix` (and the Python/Rust clients' `from_env`) look for the API token in this order:

1. `SCIX_API_TOKEN` environment variable
2. `ADS_API_TOKEN` environment variable
3. `~/.config/scix/token` (`%APPDATA%\scix\token` on Windows; override the directory with `SCIX_CONFIG_DIR`), written by `scix login` / `scix setup`

## Rate Limiting

The SciX API allows 5,000 requests/day and 5 requests/second. `scix-client` handles rate limiting automatically — a token-bucket rate limiter enforces 5 req/s locally, and API rate-limit headers are respected.

## License

MIT
