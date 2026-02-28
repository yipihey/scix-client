# Rust API Reference

[Back to README](../README.md)

`scix-client` is an async Rust crate built on `reqwest` + `tokio`.

```toml
[dependencies]
scix-client = "0.2"
```

## Basic Search

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

## Query Builder

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

## Export BibTeX

```rust
let bibtex = client.export_bibtex(&["2023ApJ...123..456A", "1998AJ....116.1009R"]).await?;
println!("{}", bibtex);

// Other formats
use scix_client::ExportFormat;
let ris = client.export(&["2023ApJ...123..456A"], ExportFormat::Ris, None).await?;
```

## References and Citations

```rust
let refs = client.references("2023ApJ...123..456A", 50).await?;
let cites = client.citations("2023ApJ...123..456A", 50).await?;
let similar = client.similar("2023ApJ...123..456A", 10).await?;
```

## Metrics

```rust
let metrics = client.metrics(&["2023ApJ...123..456A"]).await?;
if let Some(indicators) = &metrics.indicators {
    println!("h-index: {:?}", indicators.h);
}
```

## Libraries

```rust
// List
let libs = client.list_libraries().await?;

// Create
let lib = client.create_library("My Papers", "Reading list", false, None).await?;

// Add documents
client.add_documents(&lib.id, &["2023ApJ...123..456A"]).await?;

// Remove documents
client.remove_documents(&lib.id, &["2023ApJ...123..456A"]).await?;

// Edit metadata
client.edit_library(&lib.id, Some("New Name"), None, Some(true)).await?;

// Delete
client.delete_library(&lib.id).await?;
```

## Library Permissions (v0.2.0)

```rust
// Get permissions
let perms = client.get_permissions("library-id").await?;

// Grant collaborator access (owner, admin, write, read)
client.update_permissions("library-id", "colleague@example.edu", "write").await?;

// Transfer ownership
client.transfer_library("library-id", "new-owner@example.edu").await?;
```

## Annotations / Notes (v0.2.0)

```rust
// Set a note on a paper in a library
client.set_annotation("library-id", "2023ApJ...123..456A", "Key result in Fig. 3").await?;

// Read it back
let note = client.get_annotation("library-id", "2023ApJ...123..456A").await?;

// Delete
client.delete_annotation("library-id", "2023ApJ...123..456A").await?;
```

## Set Operations (v0.2.0)

```rust
// Union: merge papers from source libraries into the target
client.library_operation("target-id", "union", Some(&["source-id-1", "source-id-2"])).await?;

// Intersection, difference, copy work the same way
client.library_operation("target-id", "intersection", Some(&["source-id"])).await?;

// Empty a library (no source needed)
client.library_operation("target-id", "empty", None).await?;
```

## Add Documents by Query (v0.2.0)

```rust
// Search and add matching papers to a library in one call
let added = client.add_documents_by_query("library-id", "author:\"Weinberg\" title:\"cosmological constant\"", Some(20)).await?;
println!("Added {} papers", added);
```

## Custom Base URL

```rust
let client = SciXClient::new("my-token")
    .with_base_url("https://api.scixplorer.org/v1");
```

## Error Handling

All methods return `scix_client::error::Result<T>`. The error type `SciXError` has these variants:

| Variant | Meaning |
|---------|---------|
| `Http(reqwest::Error)` | Network/timeout failure |
| `Api { status, message }` | SciX API returned an error (e.g., 403, 500) |
| `AuthRequired` | No API token set |
| `RateLimited { retry_after }` | HTTP 429 — try again later |
| `Parse(String)` | Failed to parse API response |
| `InvalidQuery(String)` | Bad query syntax |
| `NotFound(String)` | HTTP 404 |
| `Config(String)` | Configuration error |
| `Json(serde_json::Error)` | JSON serialization error |

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
