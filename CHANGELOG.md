# Changelog

## 0.1.0 — 2026-02-27

Initial release. Rust client for the SciX (NASA ADS) API.

- Async Rust library with 11 API endpoints
- CLI tool (`scix`) with table and JSON output
- MCP server (`scix serve`) with 11 tools and 2 resources
- Python bindings via PyO3 (`import scix_client`)
- Token-bucket rate limiter (5 req/s, respects API headers)
- Query builder with fluent API
