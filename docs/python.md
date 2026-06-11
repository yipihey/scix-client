# Python API Reference

[Back to README](../README.md)

The Python module is auto-generated from the Rust crate via [PyO3](https://pyo3.rs/) + [maturin](https://www.maturin.rs/). All types, fields, and methods are derived directly from the Rust source — no separate Python code to maintain.

```bash
# From PyPI (recommended)
pip install scix-client

# From source (requires Rust toolchain)
pip install maturin
maturin develop
```

Requires Python 3.8+.

## Basic Search

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

## Query Builder

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

## Export Citations

```python
# BibTeX (default)
bibtex = client.export_bibtex(["2023ApJ...123..456A", "1998AJ....116.1009R"])
print(bibtex)

# Other formats
ris = client.export(["2023ApJ...123..456A"], format=scix_client.ExportFormat.Ris)
```

## References, Citations, and Metrics

```python
refs = client.references("2023ApJ...123..456A", rows=50)
cites = client.citations("2023ApJ...123..456A", rows=50)
similar = client.similar("2023ApJ...123..456A")

metrics = client.metrics(["2023ApJ...123..456A"])
if metrics.indicators:
    print(f"h-index: {metrics.indicators.h}")
    print(f"g-index: {metrics.indicators.g}")
```

## Libraries

```python
# List your libraries
for lib in client.list_libraries():
    print(f"{lib.name}: {lib.num_documents} papers")

# Create a library
lib = client.create_library("My Reading List", description="Papers to read")

# Add / remove documents
client.add_documents(lib.id, ["2023ApJ...123..456A"])
client.remove_documents(lib.id, ["2023ApJ...123..456A"])
```

## Library Permissions (v0.2.0)

```python
# Get permissions for a library
perms = client.get_permissions("library-id")

# Grant a collaborator access (owner, admin, write, read)
client.update_permissions("library-id", "colleague@example.edu", "write")

# Transfer ownership
client.transfer_library("library-id", "new-owner@example.edu")
```

## Annotations / Notes (v0.2.0)

```python
# Set a note on a paper in a library
client.set_annotation("library-id", "2023ApJ...123..456A", "Key result in Fig. 3")

# Read it back
note = client.get_annotation("library-id", "2023ApJ...123..456A")

# Delete
client.delete_annotation("library-id", "2023ApJ...123..456A")
```

## Set Operations (v0.2.0)

```python
# Union: merge papers from source libraries into the target
client.library_operation("target-id", "union", source_library_ids=["source-1", "source-2"])

# Intersection, difference, copy
client.library_operation("target-id", "intersection", source_library_ids=["source-id"])

# Empty a library
client.library_operation("target-id", "empty")
```

## Add Documents by Query (v0.2.0)

```python
# Search and add matching papers to a library in one call
added = client.add_documents_by_query("library-id", 'author:"Weinberg" title:"cosmological constant"', rows=20)
print(f"Added {added} papers")
```

## Reference and Object Resolution

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

## Full Text and Grep (v0.4.0 / v0.5.0)

```python
# Read a paper: abstract + open-access body (fetched from arXiv)
ft = client.fulltext("2016PhRvL.116f1102A")
print(ft.abstract_text)
print(ft.section_titles)           # ["1 Introduction", "2 Methods", ...]
if ft.body:
    print(ft.body[:2000], ft.body_source)
else:
    print([(s.label, s.url) for s in ft.sources])  # access links fallback

# Section-addressable retrieval
sections = client.fulltext_sections("2016PhRvL.116f1102A")
methods = client.fulltext_section("2016PhRvL.116f1102A", "method")

# Grep a regex across many papers at once (case-insensitive by default)
results = client.grep(r"H_?0\s*=", query='title:"hubble constant" year:2020-2024', rows=20)
for r in results:
    print(r.bibcode, r.searched)   # "fulltext", "abstract", "none", or "not_found"
    for m in r.matches:
        print(f"  ({m.section}) {m.snippet}")

# Or with explicit bibcodes
results = client.grep("dark energy", bibcodes=["2016PhRvL.116f1102A"], max_matches=3)
```

## Sort Control

```python
sort = scix_client.Sort.citation_count_desc()
results = client.search_with_options("dark matter", sort=sort, rows=20)
```

## Available Types

All types are auto-exposed with read-only field access:

| Python class | Key fields |
|-------------|------------|
| `SciXClient` | `search()`, `export()`, `metrics()`, `get_permissions()`, `library_operation()`, ... |
| `QueryBuilder` | `author()`, `title()`, `year()`, `build()`, ... |
| `Paper` | `bibcode`, `title`, `authors`, `year`, `doi`, `arxiv_id`, ... |
| `Author` | `name`, `family_name`, `given_name`, `display_name()` |
| `SearchResponse` | `papers`, `num_found` |
| `ExportFormat` | `BibTeX`, `Ris`, `AasTex`, ... (17 formats) |
| `Metrics` | `basic_stats`, `citation_stats`, `indicators` |
| `Indicators` | `h`, `g`, `i10`, `i100`, `m`, `tori`, `riq`, `read10` |
| `Sort` | `field`, `direction` |
| `Library` | `id`, `name`, `description`, `num_documents` |
| `FullText` | `bibcode`, `body`, `body_source`, `section_titles`, `sources`, `truncated` |
| `Section` | `title`, `text` |
| `GrepResult` | `bibcode`, `title`, `year`, `searched`, `source`, `matches` |
| `GrepMatch` | `section`, `snippet` |

## Complete Method Reference

### Search & Discovery

| Method | Signature |
|--------|-----------|
| `search` | `(query: str, rows: int = 10) -> SearchResponse` |
| `search_with_options` | `(query: str, sort: Sort = None, rows: int = 10) -> SearchResponse` |
| `references` | `(bibcode: str, rows: int = 25) -> SearchResponse` |
| `citations` | `(bibcode: str, rows: int = 25) -> SearchResponse` |
| `similar` | `(bibcode: str, rows: int = 10) -> SearchResponse` |

### Full Text & Grep

| Method | Signature |
|--------|-----------|
| `fulltext` | `(bibcode: str, max_chars: int = 40000) -> FullText` |
| `fulltext_sections` | `(bibcode: str) -> list[Section]` |
| `fulltext_section` | `(bibcode: str, selector: str) -> Section` |
| `grep` | `(pattern: str, bibcodes: list[str] = None, query: str = None, rows: int = 10, case_sensitive: bool = False, max_matches: int = 5, context_chars: int = 120) -> list[GrepResult]` |

### Export & Metrics

| Method | Signature |
|--------|-----------|
| `export` | `(bibcodes: list[str], format: ExportFormat = BibTeX) -> str` |
| `export_bibtex` | `(bibcodes: list[str]) -> str` |
| `metrics` | `(bibcodes: list[str]) -> Metrics` |

### Libraries

| Method | Signature |
|--------|-----------|
| `list_libraries` | `() -> list[Library]` |
| `create_library` | `(name: str, description: str = "") -> Library` |
| `add_documents` | `(library_id: str, bibcodes: list[str]) -> None` |
| `remove_documents` | `(library_id: str, bibcodes: list[str]) -> None` |
| `get_permissions` | `(library_id: str) -> dict` |
| `update_permissions` | `(library_id: str, email: str, permission: str) -> None` |
| `transfer_library` | `(library_id: str, email: str) -> None` |
| `get_annotation` | `(library_id: str, bibcode: str) -> str` |
| `set_annotation` | `(library_id: str, bibcode: str, content: str) -> None` |
| `delete_annotation` | `(library_id: str, bibcode: str) -> None` |
| `library_operation` | `(library_id: str, action: str, source_library_ids: list[str] = None) -> dict` |
| `add_documents_by_query` | `(library_id: str, query: str, rows: int = None) -> int` |

### Resolution

| Method | Signature |
|--------|-----------|
| `resolve_references` | `(references: list[str]) -> list[ResolvedReference]` |
| `resolve_objects` | `(objects: list[str]) -> dict` |
