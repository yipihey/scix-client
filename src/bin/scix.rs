//! CLI binary for the SciX client.
//!
//! Usage: scix search "dark matter" --rows 10

#[cfg(feature = "cli")]
mod cli {
    use clap::{Parser, Subcommand};
    use scix_client::{ExportFormat, SciXClient, Sort, SortDirection};

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
        /// Retrieve the full text of a paper (abstract + open-access body)
        Fulltext {
            /// Bibcode
            bibcode: String,
            /// Maximum body characters before truncating (0 = unlimited)
            #[arg(short, long, default_value = "40000")]
            max_chars: usize,
            /// Retrieve only this section (1-based index or title substring)
            #[arg(short, long)]
            section: Option<String>,
        },
        /// Search a regex pattern across the full text of multiple papers
        Grep {
            /// Regex pattern (case-insensitive by default)
            pattern: String,
            /// Bibcodes to search (or use --query)
            bibcodes: Vec<String>,
            /// ADS search query whose results are grepped
            #[arg(short, long)]
            query: Option<String>,
            /// Max papers when using --query
            #[arg(short, long, default_value = "10")]
            rows: u32,
            /// Match case-sensitively
            #[arg(long)]
            case_sensitive: bool,
            /// Max matches per paper
            #[arg(long, default_value = "5")]
            max_matches: usize,
            /// Characters of context around each match
            #[arg(long, default_value = "120")]
            context: usize,
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
        /// Set up the SciX MCP server for your AI editor
        Setup {
            /// Configure only this editor (default: all detected)
            #[arg(value_enum)]
            editor: Option<scix_client::setup::EditorTarget>,
            /// Don't test the API token
            #[arg(long)]
            skip_validation: bool,
            /// Non-interactive (use env token, configure all detected editors)
            #[arg(long, short = 'y')]
            yes: bool,
        },
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

    fn make_client(token: Option<String>) -> scix_client::error::Result<SciXClient> {
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

    fn print_papers_table(papers: &[scix_client::Paper]) {
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

    pub async fn run() -> scix_client::error::Result<()> {
        let cli = Cli::parse();

        // Setup doesn't need a pre-built client.
        if let Commands::Setup {
            editor,
            skip_validation,
            yes,
        } = cli.command
        {
            return scix_client::setup::run_setup(editor, skip_validation, yes).await;
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
                    .unwrap_or(scix_client::parse::DEFAULT_SEARCH_FIELDS);
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

            Commands::Fulltext {
                bibcode,
                max_chars,
                section,
            } => {
                if let Some(selector) = section {
                    let sec = client.fulltext_section(&bibcode, &selector).await?;
                    match cli.output {
                        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&sec)?),
                        OutputFormat::Table => println!("{}\n\n{}", sec.title, sec.text),
                    }
                    return Ok(());
                }
                let ft = client.fulltext(&bibcode, max_chars).await?;
                match cli.output {
                    OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&ft)?),
                    OutputFormat::Table => {
                        println!("{}\n", ft.title);
                        if let Some(abs) = &ft.abstract_text {
                            println!("Abstract:\n{}\n", abs);
                        }
                        match &ft.body {
                            Some(body) => {
                                let source =
                                    ft.body_source.as_deref().unwrap_or("open-access source");
                                println!("Full text (source: {}):\n{}", source, body);
                                if ft.truncated {
                                    println!("\n[truncated — raise --max-chars for more]");
                                }
                            }
                            None => {
                                println!("No open-access full text retrievable. Access links:");
                                for link in &ft.sources {
                                    println!("  {} — {}", link.label, link.url);
                                }
                            }
                        }
                        if !ft.section_titles.is_empty() {
                            println!("\nSections (retrieve one with --section):");
                            for (i, title) in ft.section_titles.iter().enumerate() {
                                println!("  {}. {}", i + 1, title);
                            }
                        }
                    }
                }
            }

            Commands::Grep {
                pattern,
                bibcodes,
                query,
                rows,
                case_sensitive,
                max_matches,
                context,
            } => {
                let opts = scix_client::batch::GrepOptions {
                    case_sensitive,
                    max_matches_per_paper: max_matches,
                    context_chars: context,
                };
                let results = if !bibcodes.is_empty() {
                    let refs: Vec<&str> = bibcodes.iter().map(|s| s.as_str()).collect();
                    client.grep(&refs, &pattern, &opts).await?
                } else if let Some(q) = &query {
                    client.grep_query(q, rows, &pattern, &opts).await?
                } else {
                    return Err(scix_client::SciXError::InvalidQuery(
                        "Provide bibcodes or --query".into(),
                    ));
                };

                match cli.output {
                    OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&results)?),
                    OutputFormat::Table => {
                        for r in &results {
                            let year = r.year.map(|y| y.to_string()).unwrap_or_default();
                            println!("{} — {} ({}) [{}]", r.bibcode, r.title, year, r.searched);
                            for m in &r.matches {
                                match &m.section {
                                    Some(s) => println!("  ({}) {}", s, m.snippet),
                                    None => println!("  {}", m.snippet),
                                }
                            }
                            if r.matches.is_empty() {
                                println!("  (no matches)");
                            }
                            println!();
                        }
                    }
                }
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

            Commands::Serve => {
                scix_client::mcp::run_server(client).await?;
            }

            Commands::Setup { .. } => unreachable!(),
        }

        Ok(())
    }
}

#[cfg(feature = "cli")]
#[tokio::main]
async fn main() {
    if let Err(e) = cli::run().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(not(feature = "cli"))]
fn main() {
    eprintln!("This binary requires the 'cli' feature. Build with: cargo build --features cli");
    std::process::exit(1);
}
