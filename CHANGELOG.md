# Changelog

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
