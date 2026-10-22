//! The `scix` command-line interface.
//!
//! Lives in the library (behind the `cli` feature) so the same code backs the
//! `scix` binary and the `scix` console script shipped in the Python wheel.

use crate::{ExportFormat, SciXClient, Sort, SortDirection};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "scix", about = "SciX / NASA ADS API client", version)]
struct Cli {
    /// API token (overrides SCIX_API_TOKEN / ADS_API_TOKEN env var)
    #[arg(long, global = true)]
    token: Option<String>,

    /// Output format
    #[arg(long, global = true, default_value = "table")]
    output: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    Table,
    Json,
}

#[derive(Subcommand)]
enum Commands {
    /// Search the SciX database
    Search {
        /// Search query (SciX/ADS syntax)
        query: String,
        /// Maximum results to return
        #[arg(short, long, default_value = "10")]
        rows: u32,
        /// Sort order (e.g., "date desc", "citation_count desc")
        #[arg(short, long)]
        sort: Option<String>,
        /// Fields to return (comma-separated)
        #[arg(short, long)]
        fields: Option<String>,
    },
    /// Export papers in citation format
    Export {
        /// Bibcodes to export
        bibcodes: Vec<String>,
        /// Export format
        #[arg(short, long, default_value = "bibtex")]
        format: String,
    },
    /// Show papers referenced by a paper
    Refs {
        /// Bibcode
        bibcode: String,
        #[arg(short, long, default_value = "25")]
        rows: u32,
    },
    /// Show papers that cite a paper
    Cites {
        /// Bibcode
        bibcode: String,
        #[arg(short, long, default_value = "25")]
        rows: u32,
    },
    /// Show papers similar to a paper
    Similar {
        /// Bibcode
        bibcode: String,
        #[arg(short, long, default_value = "10")]
        rows: u32,
    },
    /// Get citation metrics for papers
    Metrics {
        /// Bibcodes
        bibcodes: Vec<String>,
    },
    /// Resolve free-text references to bibcodes
    Resolve {
        /// Reference strings
        references: Vec<String>,
    },
    /// Resolve astronomical object names
    Objects {
        /// Object names (M31, NGC 1234, etc.)
        objects: Vec<String>,
    },
    /// Resolve links for a paper
    Links {
        /// Bibcode
        bibcode: String,
        /// Link type (esource, data, citation, reference, coreads)
        #[arg(short, long)]
        link_type: Option<String>,
    },
    /// Manage SciX libraries
    Libraries {
        #[command(subcommand)]
        action: LibraryAction,
    },
    /// Start MCP server (stdio)
    Serve,
    /// Save your SciX/ADS API token so `scix serve` works from any editor
    Login {
        /// Token to save (default: prompt, or read SCIX_API_TOKEN)
        #[arg(long)]
        token: Option<String>,
        /// Don't test the API token
        #[arg(long)]
        skip_validation: bool,
    },
    /// Set up the SciX MCP server for your AI editor
    Setup {
        /// Configure only this editor (default: all detected)
        #[arg(value_enum)]
        editor: Option<crate::setup::EditorTarget>,
        /// Don't test the API token
        #[arg(long)]
        skip_validation: bool,
        /// Non-interactive (use env token, configure all detected editors)
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Diagnose installation, token, API connectivity, and editor detection
    Doctor,
}

#[derive(Subcommand)]
enum LibraryAction {
    /// List all libraries
    List,
    /// Get a library
    Get { id: String },
    /// Create a new library
    Create {
        name: String,
        #[arg(short, long, default_value = "")]
        description: String,
        #[arg(long)]
        public: bool,
    },
    /// Delete a library
    Delete { id: String },
    /// Show permissions for a library
    Permissions { id: String },
    /// Grant a collaborator access to a library
    Grant {
        /// Library ID
        id: String,
        /// Collaborator email
        email: String,
        /// Permission level (owner, admin, write, read)
        permission: String,
    },
    /// Transfer ownership of a library
    Transfer {
        /// Library ID
        id: String,
        /// New owner email
        email: String,
    },
    /// Manage notes on papers in a library
    Notes {
        #[command(subcommand)]
        action: NotesAction,
    },
    /// Perform set operations on a library
    Ops {
        /// Library ID
        id: String,
        /// Operation: union, intersection, difference, copy, empty
        action: String,
        /// Source library IDs
        #[arg(long)]
        source: Vec<String>,
    },
}

#[derive(Subcommand)]
enum NotesAction {
    /// Get a note on a paper
    Get {
        /// Library ID
        library_id: String,
        /// Bibcode
        bibcode: String,
    },
    /// Set a note on a paper
    Set {
        /// Library ID
        library_id: String,
        /// Bibcode
        bibcode: String,
        /// Note content
        content: String,
    },
    /// Delete a note on a paper
    Delete {
        /// Library ID
        library_id: String,
        /// Bibcode
        bibcode: String,
    },
}

fn make_client(token: Option<String>) -> crate::error::Result<SciXClient> {
    match token {
        Some(t) => Ok(SciXClient::new(t)),
        None => SciXClient::from_env(),
    }
}

fn parse_sort(s: &str) -> Sort {
    let parts: Vec<&str> = s.split_whitespace().collect();
    let field = parts.first().copied().unwrap_or("date");
    let dir = match parts.get(1).copied() {
        Some("asc") => SortDirection::Asc,
        _ => SortDirection::Desc,
    };
    Sort::new(field, dir)
}

fn print_papers_table(papers: &[crate::Paper]) {
    use comfy_table::{ContentArrangement, Table};

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["Bibcode", "Year", "First Author", "Title", "Cites"]);

    for paper in papers {
        let first_author = paper
            .authors
            .first()
            .map(|a| a.family_name.as_str())
            .unwrap_or("-");
        let year = paper.year.map(|y| y.to_string()).unwrap_or_default();
        let cites = paper
            .citation_count
            .map(|c| c.to_string())
            .unwrap_or_default();
        let title = if paper.title.len() > 60 {
            format!("{}...", &paper.title[..57])
        } else {
            paper.title.clone()
        };

        table.add_row(vec![&paper.bibcode, &year, first_author, &title, &cites]);
    }

    println!("{table}");
}

/// Run the CLI with the process arguments.
pub async fn run() -> crate::error::Result<()> {
    run_from(std::env::args()).await
}

/// Run the CLI with explicit arguments (`args[0]` is the program name).
///
/// This is the entry point shared by the `scix` binary and the `scix`
/// console script installed with the Python package.
pub async fn run_from<I, T>(args: I) -> crate::error::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::parse_from(args);

    // Commands that must work before a token exists.
    match cli.command {
        Commands::Setup {
            editor,
            skip_validation,
            yes,
        } => return crate::setup::run_setup(editor, skip_validation, yes).await,
        Commands::Login {
            token,
            skip_validation,
        } => return crate::setup::run_login(token.or(cli.token), skip_validation).await,
        Commands::Doctor => return crate::doctor::run_doctor().await,
        Commands::Serve => {
            // Never refuse to start: a host that sees the process exit shows
            // only "server disconnected". Without a token the server answers
            // initialize/tools/list and explains itself on the first call.
            let client = match cli.token {
                Some(t) => Some(SciXClient::new(t)),
                None => SciXClient::from_env().ok(),
            };
            return crate::mcp::run_server(client).await;
        }
        _ => {}
    }

    let client = make_client(cli.token)?;

    match cli.command {
        Commands::Search {
            query,
            rows,
            sort,
            fields,
        } => {
            let sort_val = sort.as_deref().map(parse_sort);
            let fields_str = fields
                .as_deref()
                .unwrap_or(crate::parse::DEFAULT_SEARCH_FIELDS);
            let results = client
                .search_with_options(&query, fields_str, sort_val.as_ref(), rows, 0)
                .await?;

            match cli.output {
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&results)?);
                }
                OutputFormat::Table => {
                    println!("Found {} results:", results.num_found);
                    print_papers_table(&results.papers);
                }
            }
        }

        Commands::Export { bibcodes, format } => {
            let fmt = ExportFormat::from_str_loose(&format).unwrap_or(ExportFormat::BibTeX);
            let refs: Vec<&str> = bibcodes.iter().map(|s| s.as_str()).collect();
            let output = client.export(&refs, fmt, None).await?;
            println!("{}", output);
        }

        Commands::Refs { bibcode, rows } => {
            let results = client.references(&bibcode, rows).await?;
            match cli.output {
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&results)?),
                OutputFormat::Table => {
                    println!("References for {}:", bibcode);
                    print_papers_table(&results.papers);
                }
            }
        }

        Commands::Cites { bibcode, rows } => {
            let results = client.citations(&bibcode, rows).await?;
            match cli.output {
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&results)?),
                OutputFormat::Table => {
                    println!("Citations of {}:", bibcode);
                    print_papers_table(&results.papers);
                }
            }
        }

        Commands::Similar { bibcode, rows } => {
            let results = client.similar(&bibcode, rows).await?;
            match cli.output {
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&results)?),
                OutputFormat::Table => {
                    println!("Similar to {}:", bibcode);
                    print_papers_table(&results.papers);
                }
            }
        }

        Commands::Metrics { bibcodes } => {
            let refs: Vec<&str> = bibcodes.iter().map(|s| s.as_str()).collect();
            let metrics = client.metrics(&refs).await?;
            println!("{}", serde_json::to_string_pretty(&metrics)?);
        }

        Commands::Resolve { references } => {
            let refs: Vec<&str> = references.iter().map(|s| s.as_str()).collect();
            let resolved = client.resolve_references(&refs).await?;
            match cli.output {
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&resolved)?),
                OutputFormat::Table => {
                    for r in &resolved {
                        let bib = r.bibcode.as_deref().unwrap_or("(not found)");
                        println!("{} → {}", r.reference, bib);
                    }
                }
            }
        }

        Commands::Objects { objects } => {
            let refs: Vec<&str> = objects.iter().map(|s| s.as_str()).collect();
            let result = client.resolve_objects(&refs).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }

        Commands::Links { bibcode, link_type } => {
            let result = client.resolve_links(&bibcode, link_type.as_deref()).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }

        Commands::Libraries { action } => match action {
            LibraryAction::List => {
                let libs = client.list_libraries().await?;
                match cli.output {
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string_pretty(&libs)?);
                    }
                    OutputFormat::Table => {
                        use comfy_table::{ContentArrangement, Table};
                        let mut table = Table::new();
                        table.set_content_arrangement(ContentArrangement::Dynamic);
                        table.set_header(vec!["ID", "Name", "Documents", "Public"]);
                        for lib in &libs {
                            table.add_row(vec![
                                &lib.id,
                                &lib.name,
                                &lib.num_documents.to_string(),
                                &lib.public.to_string(),
                            ]);
                        }
                        println!("{table}");
                    }
                }
            }
            LibraryAction::Get { id } => {
                let lib = client.get_library(&id).await?;
                println!("{}", serde_json::to_string_pretty(&lib)?);
            }
            LibraryAction::Create {
                name,
                description,
                public,
            } => {
                let lib = client
                    .create_library(&name, &description, public, None)
                    .await?;
                println!("Created library: {} ({})", lib.name, lib.id);
            }
            LibraryAction::Delete { id } => {
                client.delete_library(&id).await?;
                println!("Deleted library: {}", id);
            }
            LibraryAction::Permissions { id } => {
                let perms = client.get_permissions(&id).await?;
                println!("{}", serde_json::to_string_pretty(&perms)?);
            }
            LibraryAction::Grant {
                id,
                email,
                permission,
            } => {
                client.update_permissions(&id, &email, &permission).await?;
                println!(
                    "Granted {} access ({}) to library {}",
                    email, permission, id
                );
            }
            LibraryAction::Transfer { id, email } => {
                client.transfer_library(&id, &email).await?;
                println!("Transferred library {} to {}", id, email);
            }
            LibraryAction::Notes {
                action: notes_action,
            } => match notes_action {
                NotesAction::Get {
                    library_id,
                    bibcode,
                } => {
                    let note = client.get_annotation(&library_id, &bibcode).await?;
                    println!("{}", note);
                }
                NotesAction::Set {
                    library_id,
                    bibcode,
                    content,
                } => {
                    client
                        .set_annotation(&library_id, &bibcode, &content)
                        .await?;
                    println!("Note set for {} in library {}", bibcode, library_id);
                }
                NotesAction::Delete {
                    library_id,
                    bibcode,
                } => {
                    client.delete_annotation(&library_id, &bibcode).await?;
                    println!("Note deleted for {} in library {}", bibcode, library_id);
                }
            },
            LibraryAction::Ops { id, action, source } => {
                let source_refs: Vec<&str> = source.iter().map(|s| s.as_str()).collect();
                let source_slice: Option<&[&str]> = if source_refs.is_empty() {
                    None
                } else {
                    Some(&source_refs)
                };
                let result = client.library_operation(&id, &action, source_slice).await?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        },

        Commands::Serve | Commands::Login { .. } | Commands::Setup { .. } | Commands::Doctor => {
            unreachable!("handled before the client is built")
        }
    }

    Ok(())
}
