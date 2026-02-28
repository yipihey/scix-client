# CLI Reference

[Back to README](../README.md)

The `scix` binary provides command-line access to the SciX/NASA ADS API. Install it with:

```bash
cargo install scix-client --features cli
# or pre-built:
cargo binstall scix-client
```

All commands require `SCIX_API_TOKEN` (or `ADS_API_TOKEN`) in your environment.

## Searching

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

## Exporting Citations

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

See [query-syntax.md](query-syntax.md) for the full list of 17 export formats.

## References and Citations

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

## Citation Metrics

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

## Resolving References

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

## Astronomical Objects

```bash
# Find papers about an object
scix objects "M31"

# Multiple objects
scix objects "M31" "NGC 1234" "Crab Nebula"
```

## Link Resolution

```bash
# All links for a paper (full-text, data, etc.)
scix links 2023ApJ...123..456A

# Specific link type
scix links 2023ApJ...123..456A --link-type esource
scix links 2023ApJ...123..456A --link-type data
```

## Library Management

### Basic Operations

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

### Permissions (v0.2.0)

```bash
# Show who has access to a library
scix libraries permissions abc123def

# Grant a collaborator read/write/admin/owner access
scix libraries grant abc123def colleague@example.edu write

# Transfer ownership
scix libraries transfer abc123def new-owner@example.edu
```

### Notes (v0.2.0)

```bash
# Get a note on a paper
scix libraries notes get abc123def 2023ApJ...123..456A

# Set a note
scix libraries notes set abc123def 2023ApJ...123..456A "Key result in Fig. 3"

# Delete a note
scix libraries notes delete abc123def 2023ApJ...123..456A
```

### Set Operations (v0.2.0)

```bash
# Union: merge papers from source libraries into target
scix libraries ops abc123def union --source source1 --source source2

# Intersection: keep only papers present in all
scix libraries ops abc123def intersection --source source1

# Difference: remove papers found in source libraries
scix libraries ops abc123def difference --source source1

# Copy: copy papers from source library
scix libraries ops abc123def copy --source source1

# Empty: remove all papers
scix libraries ops abc123def empty
```

## MCP Server

```bash
# Start MCP server (reads JSON-RPC from stdin, writes to stdout)
scix serve
```

See [mcp-server.md](mcp-server.md) for full setup instructions.
