//! # scix-client
//!
//! A Rust client for the SciX (formerly NASA ADS) API.
//!
//! Provides:
//! - **Library**: Async API client for search, export, metrics, libraries, and more
//! - **CLI**: `scix` binary for terminal use (includes `scix serve` for MCP)
//!
//! ## Quick Start
//!
//! ```no_run
//! # async fn example() -> scix_client::error::Result<()> {
//! use scix_client::SciXClient;
//!
//! // Create client from SCIX_API_TOKEN (or ADS_API_TOKEN) environment variable
//! let client = SciXClient::from_env()?;
//!
//! // Search for papers
//! let results = client.search("author:\"Einstein\" year:1905", 10).await?;
//! for paper in &results.papers {
//!     println!("{} ({}) - {}", paper.title, paper.year.unwrap_or(0), paper.bibcode);
//! }
//!
//! // Export as BibTeX
//! let bibtex = client.export_bibtex(&["2023ApJ...123..456A"]).await?;
//! println!("{}", bibtex);
//! # Ok(())
//! # }
//! ```
//!
//! ## Query Builder
//!
//! ```
//! use scix_client::QueryBuilder;
//!
//! let query = QueryBuilder::new()
//!     .author("Weinberg")
//!     .and()
//!     .title("cosmological constant")
//!     .and()
//!     .property("refereed")
//!     .build();
//! ```

pub mod client;
pub mod error;
pub mod export;
pub mod libraries;
pub mod links;
pub mod metrics;
pub mod network;
pub mod objects;
pub mod parse;
pub mod query;
pub mod rate_limit;
pub mod references;
pub mod search;
pub mod types;

pub mod mcp;

#[cfg(feature = "cli")]
pub mod setup;

#[cfg(feature = "python")]
pub mod python;

// Re-export key types at the crate root.
pub use client::SciXClient;
pub use error::SciXError;
pub use query::QueryBuilder;
pub use types::*;
