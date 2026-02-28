//! CLI binary for the SciX client.
//!
//! Usage: scix search "dark matter" --rows 10

#[cfg(feature = "cli")]
mod cli {
    use scix_client::{SciXClient, ExportFormat, Sort, SortDirection};
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
        use comfy_table::{Table, ContentArrangement};

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
        let client = make_client(cli.token)?;

        match cli.command {
            Commands::Search {
                query,
                rows,
                sort,
                fields,
            } => {
                let sort_val = sort.as_deref().map(parse_sort);
                let fields_str = fields.as_deref().unwrap_or(scix_client::parse::DEFAULT_SEARCH_FIELDS);
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

            Commands::Links {
                bibcode,
                link_type,
            } => {
                let result = client
                    .resolve_links(&bibcode, link_type.as_deref())
                    .await?;
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
                            use comfy_table::{Table, ContentArrangement};
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
                    let lib = client.create_library(&name, &description, public, None).await?;
                    println!("Created library: {} ({})", lib.name, lib.id);
                }
                LibraryAction::Delete { id } => {
                    client.delete_library(&id).await?;
                    println!("Deleted library: {}", id);
                }
            },

            Commands::Serve => {
                scix_client::mcp::run_server(client).await?;
            }
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
