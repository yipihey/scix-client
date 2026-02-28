# Query Syntax & Export Formats

[Back to README](../README.md)

## Query Syntax

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

## Boolean Operators

```
author:"Hawking" AND title:"black hole"
author:"Einstein" OR author:"Bohr"
author:"Einstein" NOT title:"relativity"
(author:"Einstein" OR author:"Bohr") AND year:[1920 TO 1930]
```

## Functional Operators

| Operator | Description |
|----------|-------------|
| `citations(bibcode:X)` | Papers that cite X |
| `references(bibcode:X)` | Papers referenced by X |
| `similar(bibcode:X)` | Content-similar papers |
| `trending(bibcode:X)` | Trending co-reads |
| `reviews(bibcode:X)` | Review articles |

## Wildcards

```
author:"Eins*"    # Prefix matching
title:galax?      # Single character wildcard
```

## Sort Options

| Sort | Description |
|------|-------------|
| `date desc` | Newest first (default) |
| `citation_count desc` | Most cited first |
| `score desc` | Best match first |
| `read_count desc` | Most read first |

## Export Formats

17 formats are supported across all interfaces (CLI `--format`, Rust `ExportFormat`, Python `scix_client.ExportFormat`, MCP `scix_export`):

| Format | CLI flag | Description |
|--------|----------|-------------|
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
