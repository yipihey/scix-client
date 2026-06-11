//! MCP (Model Context Protocol) server implementation.
//!
//! Implements the JSON-RPC 2.0 protocol over stdio, exposing SciX API tools
//! for AI agent integration.

use crate::client::SciXClient;
use crate::error::SciXError;
use crate::types::ExportFormat;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

/// Rich fields for single-paper detail view.
const RICH_FIELDS: &str = "bibcode,title,author,year,pub,abstract,doi,identifier,doctype,esources,citation_count,property,read_count,volume,page,keyword,aff";

/// Run the MCP server over stdin/stdout.
pub async fn run_server(client: SciXClient) -> crate::error::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| SciXError::Config(format!("stdin error: {}", e)))?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {}", e) }
                });
                writeln!(stdout.lock(), "{}", error_response)
                    .map_err(|e| SciXError::Config(format!("stdout error: {}", e)))?;
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request["method"].as_str().unwrap_or("");

        let response = match method {
            "initialize" => handle_initialize(&id),
            "tools/list" => handle_tools_list(&id),
            "tools/call" => handle_tool_call(&client, &id, &request["params"]).await,
            "resources/list" => handle_resources_list(&id),
            "resources/templates/list" => handle_resource_templates_list(&id),
            "resources/read" => handle_resource_read(&client, &id, &request["params"]).await,
            "notifications/initialized" | "notifications/cancelled" => continue,
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {}", method) }
            }),
        };

        writeln!(stdout.lock(), "{}", response)
            .map_err(|e| SciXError::Config(format!("stdout error: {}", e)))?;
        stdout
            .lock()
            .flush()
            .map_err(|e| SciXError::Config(format!("stdout flush error: {}", e)))?;
    }

    Ok(())
}

fn handle_initialize(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {}
            },
            "serverInfo": {
                "name": "scix-mcp",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

fn handle_tools_list(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": tool_definitions()
        }
    })
}

fn handle_resources_list(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "resources": [
                {
                    "uri": "scix://fields",
                    "name": "SciX Searchable Fields",
                    "description": "List of searchable and returnable fields in ADS",
                    "mimeType": "text/plain"
                },
                {
                    "uri": "scix://syntax",
                    "name": "SciX Query Syntax",
                    "description": "Guide to ADS query syntax",
                    "mimeType": "text/plain"
                }
            ]
        }
    })
}

fn handle_resource_templates_list(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "resourceTemplates": [
                {
                    "uriTemplate": "scix://paper/{bibcode}/{part}",
                    "name": "SciX Paper (document-as-directory)",
                    "description": "Navigate a paper like a directory. {part} is one of: metadata, abstract, fulltext, references, citations, links, sections, sections/{selector}. Example: scix://paper/2016PhRvL.116f1102A/sections/2",
                    "mimeType": "text/markdown"
                }
            ]
        }
    })
}

async fn handle_resource_read(client: &SciXClient, id: &Value, params: &Value) -> Value {
    let uri = params["uri"].as_str().unwrap_or("");
    match read_resource(client, uri).await {
        Ok((mime, content)) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "contents": [{
                    "uri": uri,
                    "mimeType": mime,
                    "text": content
                }]
            }
        }),
        Err(e) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32602, "message": e.to_string() }
        }),
    }
}

/// Resolve a `scix://` resource URI to a `(mime_type, content)` pair.
///
/// Supports the static `scix://fields` and `scix://syntax` references plus the
/// `scix://paper/{bibcode}/{part}` document-as-directory layout.
async fn read_resource(client: &SciXClient, uri: &str) -> Result<(String, String), SciXError> {
    match uri {
        "scix://fields" => Ok(("text/plain".into(), FIELDS_REFERENCE.to_string())),
        "scix://syntax" => Ok(("text/plain".into(), SYNTAX_REFERENCE.to_string())),
        _ => {
            let (bibcode, part) = parse_paper_uri(uri)
                .ok_or_else(|| SciXError::NotFound(format!("Unknown resource: {}", uri)))?;

            let content = match part {
                "metadata" => format_paper_detail(&fetch_paper_detail(client, bibcode).await?),
                "abstract" => {
                    let paper = fetch_paper_detail(client, bibcode).await?;
                    paper
                        .abstract_text
                        .unwrap_or_else(|| "No abstract available.".to_string())
                }
                "fulltext" => {
                    let ft = client
                        .fulltext(bibcode, crate::fulltext::DEFAULT_MAX_CHARS)
                        .await?;
                    format_fulltext(&ft)
                }
                "references" => {
                    let results = client.references(bibcode, 50).await?;
                    format_search_results(&results, 0)
                }
                "citations" => {
                    let results = client.citations(bibcode, 50).await?;
                    format_search_results(&results, 0)
                }
                "links" => {
                    let links = client.resolve_links(bibcode, None).await?;
                    serde_json::to_string_pretty(&links)
                        .map_err(|e| SciXError::Parse(e.to_string()))?
                }
                "sections" => {
                    let sections = client.fulltext_sections(bibcode).await?;
                    format_section_list(bibcode, &sections)
                }
                other => {
                    if let Some(selector) = other.strip_prefix("sections/") {
                        let section = client.fulltext_section(bibcode, selector).await?;
                        format!("## {}\n\n{}\n", section.title, section.text)
                    } else {
                        return Err(SciXError::InvalidQuery(format!(
                            "Unknown paper part '{}'. Use: metadata, abstract, fulltext, references, citations, links, sections, sections/{{selector}}",
                            other
                        )));
                    }
                }
            };

            Ok(("text/markdown".into(), content))
        }
    }
}

/// Parse a `scix://paper/{bibcode}/{part}` URI into its bibcode and part.
///
/// Bibcodes contain no `/`, so the first segment is the bibcode; the part may
/// itself contain `/` (e.g., `sections/2`).
fn parse_paper_uri(uri: &str) -> Option<(&str, &str)> {
    let rest = uri.strip_prefix("scix://paper/")?;
    let (bibcode, part) = rest.split_once('/')?;
    if bibcode.is_empty() || part.is_empty() || part.ends_with('/') {
        return None;
    }
    Some((bibcode, part))
}

async fn handle_tool_call(client: &SciXClient, id: &Value, params: &Value) -> Value {
    let tool_name = params["name"].as_str().unwrap_or("");
    let args = &params["arguments"];

    let result = match tool_name {
        "scix_search" => tool_search(client, args).await,
        "scix_bigquery" => tool_bigquery(client, args).await,
        "scix_export" => tool_export(client, args).await,
        "scix_metrics" => tool_metrics(client, args).await,
        "scix_library" => tool_library(client, args).await,
        "scix_library_documents" => tool_library_documents(client, args).await,
        "scix_citation_helper" => tool_citation_helper(client, args).await,
        "scix_network" => tool_network(client, args).await,
        "scix_object_search" => tool_object_search(client, args).await,
        "scix_resolve_reference" => tool_resolve_reference(client, args).await,
        "scix_resolve_links" => tool_resolve_links(client, args).await,
        "scix_get_paper" => tool_get_paper(client, args).await,
        "scix_fulltext" => tool_fulltext(client, args).await,
        "scix_grep" => tool_grep(client, args).await,
        _ => Err(SciXError::Config(format!("Unknown tool: {}", tool_name))),
    };

    match result {
        Ok(content) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": content }]
            }
        }),
        Err(e) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                "isError": true
            }
        }),
    }
}

// --- Tool implementations ---

async fn tool_search(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let query = args["query"]
        .as_str()
        .ok_or_else(|| SciXError::InvalidQuery("'query' parameter required".into()))?;
    let rows = args["rows"].as_u64().unwrap_or(10) as u32;
    let start = args["start"].as_u64().unwrap_or(0) as u32;
    let sort = args["sort"].as_str();
    let fields = args["fields"].as_str();

    let sort_val = sort.map(|s| {
        let parts: Vec<&str> = s.split_whitespace().collect();
        crate::types::Sort::new(
            parts.first().copied().unwrap_or("date"),
            if parts.get(1) == Some(&"asc") {
                crate::types::SortDirection::Asc
            } else {
                crate::types::SortDirection::Desc
            },
        )
    });

    let fl_owned: String;
    let fl = match fields {
        None => crate::parse::DEFAULT_SEARCH_FIELDS,
        Some(f) if f.split(',').any(|s| s.trim() == "bibcode") => f,
        Some(f) => {
            fl_owned = format!("bibcode,{}", f);
            fl_owned.as_str()
        }
    };
    let results = client
        .search_with_options(query, fl, sort_val.as_ref(), rows, start)
        .await?;

    Ok(format_search_results(&results, start))
}

async fn tool_bigquery(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let bibcodes: Vec<&str> = args["bibcodes"]
        .as_array()
        .ok_or_else(|| SciXError::InvalidQuery("'bibcodes' array required".into()))?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    let query = args["query"].as_str();
    let results = client.bigquery(&bibcodes, query, None, None, None).await?;
    Ok(format_search_results(&results, 0))
}

async fn tool_export(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let bibcodes: Vec<&str> = args["bibcodes"]
        .as_array()
        .ok_or_else(|| SciXError::InvalidQuery("'bibcodes' array required".into()))?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    let format_str = args["format"].as_str().unwrap_or("bibtex");
    let format = ExportFormat::from_str_loose(format_str).unwrap_or(ExportFormat::BibTeX);

    client.export(&bibcodes, format, None).await
}

async fn tool_metrics(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let bibcodes: Vec<&str> = args["bibcodes"]
        .as_array()
        .ok_or_else(|| SciXError::InvalidQuery("'bibcodes' array required".into()))?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    let metrics = client.metrics(&bibcodes).await?;
    serde_json::to_string_pretty(&metrics).map_err(|e| SciXError::Parse(e.to_string()))
}

async fn tool_library(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let action = args["action"]
        .as_str()
        .ok_or_else(|| SciXError::InvalidQuery("'action' parameter required".into()))?;

    match action {
        "list" => {
            let libs = client.list_libraries().await?;
            Ok(format_library_list(&libs))
        }
        "get" => {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'id' required for get".into()))?;
            let lib = client.get_library(id).await?;

            let mut out = format!(
                "Library: {}\nDocuments: {}\n",
                lib.metadata.name, lib.metadata.num_documents
            );
            if !lib.metadata.description.is_empty() {
                out.push_str(&format!("Description: {}\n", lib.metadata.description));
            }
            out.push('\n');

            if lib.documents.is_empty() {
                out.push_str("No documents in this library.\n");
            } else {
                // Fetch paper details via bigquery
                let bibcode_refs: Vec<&str> = lib.documents.iter().map(|s| s.as_str()).collect();
                let results = client
                    .bigquery(&bibcode_refs, None, None, None, None)
                    .await?;
                out.push_str(&format_search_results(&results, 0));
            }

            Ok(out)
        }
        "create" => {
            let name = args["name"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'name' required for create".into()))?;
            let description = args["description"].as_str().unwrap_or("");
            let public = args["public"].as_bool().unwrap_or(false);
            let lib = client
                .create_library(name, description, public, None)
                .await?;
            serde_json::to_string_pretty(&lib).map_err(|e| SciXError::Parse(e.to_string()))
        }
        "delete" => {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'id' required for delete".into()))?;
            client.delete_library(id).await?;
            Ok(format!("Library {} deleted", id))
        }
        "edit" => {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'id' required for edit".into()))?;
            let name = args["name"].as_str();
            let description = args["description"].as_str();
            let public = args["public"].as_bool();
            client.edit_library(id, name, description, public).await?;
            Ok(format!("Library {} updated", id))
        }
        "permissions" => {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'id' required for permissions".into()))?;
            let perms = client.get_permissions(id).await?;
            serde_json::to_string_pretty(&perms).map_err(|e| SciXError::Parse(e.to_string()))
        }
        "update_permissions" => {
            let id = args["id"].as_str().ok_or_else(|| {
                SciXError::InvalidQuery("'id' required for update_permissions".into())
            })?;
            let email = args["email"].as_str().ok_or_else(|| {
                SciXError::InvalidQuery("'email' required for update_permissions".into())
            })?;
            let permission = args["permission"].as_str().ok_or_else(|| {
                SciXError::InvalidQuery("'permission' required for update_permissions".into())
            })?;
            client.update_permissions(id, email, permission).await?;
            Ok(format!(
                "Permissions updated for {} on library {}",
                email, id
            ))
        }
        "transfer" => {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'id' required for transfer".into()))?;
            let email = args["email"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'email' required for transfer".into()))?;
            client.transfer_library(id, email).await?;
            Ok(format!("Library {} transferred to {}", id, email))
        }
        _ => Err(SciXError::InvalidQuery(format!(
            "Unknown library action: {}",
            action
        ))),
    }
}

async fn tool_library_documents(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let action = args["action"]
        .as_str()
        .ok_or_else(|| SciXError::InvalidQuery("'action' parameter required".into()))?;
    let library_id = args["library_id"]
        .as_str()
        .ok_or_else(|| SciXError::InvalidQuery("'library_id' required".into()))?;

    match action {
        "add" => {
            let bibcodes: Vec<&str> = args["bibcodes"]
                .as_array()
                .ok_or_else(|| SciXError::InvalidQuery("'bibcodes' array required".into()))?
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            client.add_documents(library_id, &bibcodes).await?;
            Ok(format!("Added {} documents", bibcodes.len()))
        }
        "remove" => {
            let bibcodes: Vec<&str> = args["bibcodes"]
                .as_array()
                .ok_or_else(|| SciXError::InvalidQuery("'bibcodes' array required".into()))?
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            client.remove_documents(library_id, &bibcodes).await?;
            Ok(format!("Removed {} documents", bibcodes.len()))
        }
        "get_notes" => {
            let bibcode = args["bibcode"].as_str().ok_or_else(|| {
                SciXError::InvalidQuery("'bibcode' required for get_notes".into())
            })?;
            let note = client.get_annotation(library_id, bibcode).await?;
            Ok(note)
        }
        "add_note" | "edit_note" => {
            let bibcode = args["bibcode"].as_str().ok_or_else(|| {
                SciXError::InvalidQuery("'bibcode' required for add_note/edit_note".into())
            })?;
            let content = args["content"].as_str().ok_or_else(|| {
                SciXError::InvalidQuery("'content' required for add_note/edit_note".into())
            })?;
            client.set_annotation(library_id, bibcode, content).await?;
            Ok(format!("Note saved for {}", bibcode))
        }
        "delete_note" => {
            let bibcode = args["bibcode"].as_str().ok_or_else(|| {
                SciXError::InvalidQuery("'bibcode' required for delete_note".into())
            })?;
            client.delete_annotation(library_id, bibcode).await?;
            Ok(format!("Note deleted for {}", bibcode))
        }
        "union" | "intersection" | "difference" | "copy" | "empty" => {
            let source_ids: Option<Vec<&str>> = args["libraries"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect());
            let source_refs: Option<Vec<&str>> = source_ids;
            let source_slice: Option<&[&str]> = source_refs.as_deref();
            let result = client
                .library_operation(library_id, action, source_slice)
                .await?;
            serde_json::to_string_pretty(&result).map_err(|e| SciXError::Parse(e.to_string()))
        }
        "add_by_query" => {
            let query = args["query"].as_str().ok_or_else(|| {
                SciXError::InvalidQuery("'query' required for add_by_query".into())
            })?;
            let rows = args["rows"].as_u64().map(|r| r as u32);
            let count = client
                .add_documents_by_query(library_id, query, rows)
                .await?;
            Ok(format!("Added {} documents by query", count))
        }
        _ => Err(SciXError::InvalidQuery(format!(
            "Unknown document action: {}",
            action
        ))),
    }
}

async fn tool_citation_helper(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let bibcodes: Vec<&str> = args["bibcodes"]
        .as_array()
        .ok_or_else(|| SciXError::InvalidQuery("'bibcodes' array required".into()))?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    let result = client.citation_helper(&bibcodes).await?;
    serde_json::to_string_pretty(&result).map_err(|e| SciXError::Parse(e.to_string()))
}

async fn tool_network(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let bibcodes: Vec<&str> = args["bibcodes"]
        .as_array()
        .ok_or_else(|| SciXError::InvalidQuery("'bibcodes' array required".into()))?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    let network_type = args["type"].as_str().unwrap_or("author");
    let result = match network_type {
        "paper" => client.paper_network(&bibcodes).await?,
        _ => client.author_network(&bibcodes).await?,
    };

    serde_json::to_string_pretty(&result).map_err(|e| SciXError::Parse(e.to_string()))
}

async fn tool_object_search(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let objects: Vec<&str> = args["objects"]
        .as_array()
        .ok_or_else(|| SciXError::InvalidQuery("'objects' array required".into()))?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    let result = client.resolve_objects(&objects).await?;
    serde_json::to_string_pretty(&result).map_err(|e| SciXError::Parse(e.to_string()))
}

async fn tool_resolve_reference(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let references: Vec<&str> = args["references"]
        .as_array()
        .ok_or_else(|| SciXError::InvalidQuery("'references' array required".into()))?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    let result = client.resolve_references(&references).await?;
    serde_json::to_string_pretty(&result).map_err(|e| SciXError::Parse(e.to_string()))
}

async fn tool_resolve_links(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let bibcode = args["bibcode"]
        .as_str()
        .ok_or_else(|| SciXError::InvalidQuery("'bibcode' required".into()))?;
    let link_type = args["link_type"].as_str();

    let result = client.resolve_links(bibcode, link_type).await?;
    serde_json::to_string_pretty(&result).map_err(|e| SciXError::Parse(e.to_string()))
}

async fn tool_get_paper(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let bibcode = args["bibcode"]
        .as_str()
        .ok_or_else(|| SciXError::InvalidQuery("'bibcode' required".into()))?;

    let paper = fetch_paper_detail(client, bibcode).await?;
    Ok(format_paper_detail(&paper))
}

async fn tool_fulltext(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let bibcode = args["bibcode"]
        .as_str()
        .ok_or_else(|| SciXError::InvalidQuery("'bibcode' required".into()))?;
    let max_chars = args["max_chars"]
        .as_u64()
        .map(|n| n as usize)
        .unwrap_or(crate::fulltext::DEFAULT_MAX_CHARS);

    if let Some(selector) = args["section"].as_str() {
        let section = client.fulltext_section(bibcode, selector).await?;
        return Ok(format!("## {}\n\n{}\n", section.title, section.text));
    }

    let ft = client.fulltext(bibcode, max_chars).await?;
    Ok(format_fulltext(&ft))
}

async fn tool_grep(client: &SciXClient, args: &Value) -> Result<String, SciXError> {
    let pattern = args["pattern"]
        .as_str()
        .ok_or_else(|| SciXError::InvalidQuery("'pattern' required".into()))?;

    let mut opts = crate::batch::GrepOptions::default();
    if let Some(cs) = args["case_sensitive"].as_bool() {
        opts.case_sensitive = cs;
    }
    if let Some(m) = args["max_matches"].as_u64() {
        opts.max_matches_per_paper = m as usize;
    }
    if let Some(c) = args["context_chars"].as_u64() {
        opts.context_chars = c as usize;
    }

    let results = if let Some(arr) = args["bibcodes"].as_array() {
        let bibcodes: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
        if bibcodes.is_empty() {
            return Err(SciXError::InvalidQuery("'bibcodes' array is empty".into()));
        }
        client.grep(&bibcodes, pattern, &opts).await?
    } else if let Some(query) = args["query"].as_str() {
        let rows = args["rows"].as_u64().unwrap_or(10).min(50) as u32;
        client.grep_query(query, rows, pattern, &opts).await?
    } else {
        return Err(SciXError::InvalidQuery(
            "Provide either 'bibcodes' or 'query'".into(),
        ));
    };

    Ok(format_grep_results(pattern, &results))
}

/// Fetch a single paper with the rich field set, erroring if not found.
async fn fetch_paper_detail(
    client: &SciXClient,
    bibcode: &str,
) -> Result<crate::types::Paper, SciXError> {
    let query = format!("identifier:{}", bibcode);
    let results = client
        .search_with_options(&query, RICH_FIELDS, None, 1, 0)
        .await?;

    results
        .papers
        .into_iter()
        .next()
        .ok_or_else(|| SciXError::NotFound(format!("Paper not found: {}", bibcode)))
}

/// Format a paper's rich metadata as Markdown.
fn format_paper_detail(paper: &crate::types::Paper) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", paper.title));

    let authors_str = if paper.authors.len() > 10 {
        let first_five: Vec<_> = paper.authors[..5].iter().map(|a| a.name.as_str()).collect();
        format!(
            "{} ... and {} more",
            first_five.join("; "),
            paper.authors.len() - 5
        )
    } else {
        paper
            .authors
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    };
    out.push_str(&format!("**Authors:** {}\n", authors_str));
    out.push_str(&format!(
        "**Year:** {}\n",
        paper.year.map(|y| y.to_string()).unwrap_or_default()
    ));

    if let Some(pub_name) = &paper.publication {
        out.push_str(&format!("**Publication:** {}\n", pub_name));
    }
    if let Some(doctype) = &paper.doctype {
        out.push_str(&format!("**Type:** {}\n", doctype));
    }
    out.push_str(&format!("**Bibcode:** {}\n", paper.bibcode));
    if let Some(doi) = &paper.doi {
        out.push_str(&format!("**DOI:** {}\n", doi));
    }
    if let Some(arxiv) = &paper.arxiv_id {
        out.push_str(&format!("**arXiv:** {}\n", arxiv));
    }
    if let Some(cites) = paper.citation_count {
        out.push_str(&format!("**Citations:** {}\n", cites));
    }
    if !paper.properties.is_empty() {
        out.push_str(&format!(
            "**Properties:** {}\n",
            paper.properties.join(", ")
        ));
    }

    if let Some(abstract_text) = &paper.abstract_text {
        out.push_str(&format!("\n**Abstract:**\n{}\n", abstract_text));
    }

    if !paper.pdf_links.is_empty() {
        out.push_str("\n**Links:**\n");
        for link in &paper.pdf_links {
            out.push_str(&format!("- [{}]({})\n", link.label, link.url));
        }
    }

    out.push_str(&format!("\n**ADS:** {}\n", paper.url));

    out
}

/// Format a [`FullText`] result as Markdown: metadata, abstract, body (or a
/// fallback note), and access links.
fn format_fulltext(ft: &crate::types::FullText) -> String {
    let mut out = format!("# {}\n\n", ft.title);
    out.push_str(&format!("**Bibcode:** {}\n", ft.bibcode));
    if let Some(arxiv) = &ft.arxiv_id {
        out.push_str(&format!("**arXiv:** {}\n", arxiv));
    }
    out.push_str(&format!(
        "**Open access:** {}\n",
        if ft.open_access { "yes" } else { "unknown/no" }
    ));

    if let Some(abstract_text) = &ft.abstract_text {
        out.push_str(&format!("\n## Abstract\n{}\n", abstract_text));
    }

    match &ft.body {
        Some(body) => {
            let source = ft.body_source.as_deref().unwrap_or("open-access source");
            out.push_str(&format!(
                "\n## Full text\n_Source: {}_\n\n{}\n",
                source, body
            ));
            if ft.truncated {
                out.push_str("\n_[Full text truncated. Request a larger max_chars for more.]_\n");
            }
        }
        None => {
            out.push_str(
                "\n## Full text\n_No open-access body could be retrieved inline. Use the access links below._\n",
            );
        }
    }

    if !ft.section_titles.is_empty() {
        out.push_str("\n## Sections\n");
        for (i, title) in ft.section_titles.iter().enumerate() {
            out.push_str(&format!("{}. {}\n", i + 1, title));
        }
        out.push_str(
            "\n_Retrieve one section with the `section` parameter (index or title substring)._\n",
        );
    }

    if !ft.sources.is_empty() {
        out.push_str("\n## Sources\n");
        for link in &ft.sources {
            out.push_str(&format!("- [{}]({})\n", link.label, link.url));
        }
    }

    out
}

/// Format a paper's section list as Markdown.
fn format_section_list(bibcode: &str, sections: &[crate::types::Section]) -> String {
    let mut out = format!("Sections of {}:\n\n", bibcode);
    for (i, section) in sections.iter().enumerate() {
        out.push_str(&format!(
            "{}. {} ({} chars)\n",
            i + 1,
            section.title,
            section.text.chars().count()
        ));
    }
    out.push_str(&format!(
        "\n_Read one with scix://paper/{}/sections/{{index or title substring}}_\n",
        bibcode
    ));
    out
}

/// Format grep results across papers as Markdown.
fn format_grep_results(pattern: &str, results: &[crate::types::GrepResult]) -> String {
    let with_matches = results.iter().filter(|r| !r.matches.is_empty()).count();
    let mut out = format!(
        "Grep '{}' across {} papers — {} with matches:\n\n",
        pattern,
        results.len(),
        with_matches
    );

    for r in results {
        let year = r.year.map(|y| y.to_string()).unwrap_or_default();
        match r.searched.as_str() {
            "not_found" => {
                out.push_str(&format!("## {} — not found in ADS\n\n", r.bibcode));
                continue;
            }
            "none" => {
                out.push_str(&format!(
                    "## {} — {} ({})\nNo searchable text available.\n\n",
                    r.bibcode, r.title, year
                ));
                continue;
            }
            _ => {}
        }

        out.push_str(&format!(
            "## {} — {} ({}) [searched: {}]\n",
            r.bibcode, r.title, year, r.searched
        ));
        if r.matches.is_empty() {
            out.push_str("No matches.\n\n");
            continue;
        }
        for m in &r.matches {
            match &m.section {
                Some(section) => out.push_str(&format!("- ({}) {}\n", section, m.snippet)),
                None => out.push_str(&format!("- {}\n", m.snippet)),
            }
        }
        out.push('\n');
    }

    out
}

// --- Formatting helpers ---

fn format_library_list(libs: &[crate::types::Library]) -> String {
    if libs.is_empty() {
        return "No libraries found.".to_string();
    }
    let mut out = format!("Found {} libraries:\n\n", libs.len());
    for lib in libs {
        out.push_str(&format!(
            "- {} ({} documents)\n  ID: {}\n",
            lib.name, lib.num_documents, lib.id
        ));
        if !lib.description.is_empty() {
            out.push_str(&format!("  Description: {}\n", lib.description));
        }
        out.push('\n');
    }
    out
}

fn format_search_results(results: &crate::types::SearchResponse, start: u32) -> String {
    let mut out = format!("Found {} results:\n\n", results.num_found);
    for (i, paper) in results.papers.iter().enumerate() {
        let authors_str = if paper.authors.len() > 3 {
            format!("{} et al.", paper.authors[0].family_name)
        } else {
            paper
                .authors
                .iter()
                .map(|a| a.family_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };

        out.push_str(&format!(
            "{}. {} ({})\n   {}\n   Bibcode: {}\n",
            start as usize + i + 1,
            paper.title,
            paper.year.map(|y| y.to_string()).unwrap_or_default(),
            authors_str,
            paper.bibcode,
        ));

        if let Some(doi) = &paper.doi {
            out.push_str(&format!("   DOI: {}\n", doi));
        }
        if let Some(cites) = paper.citation_count {
            out.push_str(&format!("   Citations: {}\n", cites));
        }
        out.push('\n');
    }

    let shown = start as u64 + results.papers.len() as u64;
    if results.num_found > shown {
        out.push_str(&format!("*Use start={} to see more results*\n", shown));
    }

    out
}

// --- Tool definitions ---

fn tool_definitions() -> Value {
    json!([
        {
            "name": "scix_search",
            "description": "Search the SciX / NASA ADS database. Supports field queries (author, title, abstract, year, etc.), boolean operators, and functional operators (citations(), references(), similar()).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "ADS query string (e.g., 'author:\"Einstein\" year:1905')" },
                    "rows": { "type": "integer", "description": "Max results (default 10)", "default": 10 },
                    "start": { "type": "integer", "description": "Starting index for pagination (default 0)", "default": 0 },
                    "sort": { "type": "string", "description": "Sort order (e.g., 'date desc', 'citation_count desc')" },
                    "fields": { "type": "string", "description": "Comma-separated fields to return" }
                },
                "required": ["query"]
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            }
        },
        {
            "name": "scix_bigquery",
            "description": "Search within a set of known bibcodes. Useful for filtering a collection of papers.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "bibcodes": { "type": "array", "items": { "type": "string" }, "description": "List of bibcodes to search within" },
                    "query": { "type": "string", "description": "Optional additional query filter" }
                },
                "required": ["bibcodes"]
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            }
        },
        {
            "name": "scix_export",
            "description": "Export papers in citation formats (bibtex, ris, aastex, mnras, ieee, csl, etc.).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "bibcodes": { "type": "array", "items": { "type": "string" }, "description": "Bibcodes to export" },
                    "format": { "type": "string", "description": "Export format (bibtex, ris, aastex, mnras, ieee, csl, etc.)", "default": "bibtex" }
                },
                "required": ["bibcodes"]
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            }
        },
        {
            "name": "scix_metrics",
            "description": "Get citation metrics (h-index, g-index, citation counts) for a set of papers.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "bibcodes": { "type": "array", "items": { "type": "string" }, "description": "Bibcodes to get metrics for" }
                },
                "required": ["bibcodes"]
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            }
        },
        {
            "name": "scix_library",
            "description": "Manage SciX personal libraries (list, get, create, edit, delete, permissions, transfer).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "get", "create", "edit", "delete", "permissions", "update_permissions", "transfer"] },
                    "id": { "type": "string", "description": "Library ID (for get/edit/delete/permissions/update_permissions/transfer)" },
                    "name": { "type": "string", "description": "Library name (for create/edit)" },
                    "description": { "type": "string", "description": "Library description (for create/edit)" },
                    "public": { "type": "boolean", "description": "Public visibility (for create/edit)" },
                    "email": { "type": "string", "description": "Collaborator email (for update_permissions/transfer)" },
                    "permission": { "type": "string", "description": "Permission level: owner, admin, write, read (for update_permissions)", "enum": ["owner", "admin", "write", "read"] }
                },
                "required": ["action"]
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": false,
                "openWorldHint": true
            }
        },
        {
            "name": "scix_library_documents",
            "description": "Manage documents in a SciX library: add/remove bibcodes, notes, set operations (union/intersection/difference/copy/empty), or add by search query.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["add", "remove", "get_notes", "add_note", "edit_note", "delete_note", "union", "intersection", "difference", "copy", "empty", "add_by_query"] },
                    "library_id": { "type": "string", "description": "Library ID" },
                    "bibcodes": { "type": "array", "items": { "type": "string" }, "description": "Bibcodes to add/remove" },
                    "bibcode": { "type": "string", "description": "Single bibcode (for note operations)" },
                    "content": { "type": "string", "description": "Note content (for add_note/edit_note)" },
                    "libraries": { "type": "array", "items": { "type": "string" }, "description": "Source library IDs (for set operations: union/intersection/difference/copy)" },
                    "query": { "type": "string", "description": "Search query (for add_by_query)" },
                    "rows": { "type": "integer", "description": "Max documents to add by query (default 50)" }
                },
                "required": ["action", "library_id"]
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": false,
                "openWorldHint": true
            }
        },
        {
            "name": "scix_citation_helper",
            "description": "Find papers frequently co-cited with the given set but not yet included.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "bibcodes": { "type": "array", "items": { "type": "string" }, "description": "Bibcodes for co-citation analysis" }
                },
                "required": ["bibcodes"]
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            }
        },
        {
            "name": "scix_network",
            "description": "Get author collaboration or paper citation network data.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "bibcodes": { "type": "array", "items": { "type": "string" }, "description": "Bibcodes for network analysis" },
                    "type": { "type": "string", "enum": ["author", "paper"], "description": "Network type", "default": "author" }
                },
                "required": ["bibcodes"]
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            }
        },
        {
            "name": "scix_object_search",
            "description": "Resolve astronomical object names (M31, NGC 1234, Crab Nebula) via SIMBAD/NED.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "objects": { "type": "array", "items": { "type": "string" }, "description": "Object names to resolve" }
                },
                "required": ["objects"]
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            }
        },
        {
            "name": "scix_resolve_reference",
            "description": "Resolve free-text references to bibcodes (e.g., 'Einstein 1905 Annalen der Physik 17 891').",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "references": { "type": "array", "items": { "type": "string" }, "description": "Free-text reference strings" }
                },
                "required": ["references"]
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            }
        },
        {
            "name": "scix_resolve_links",
            "description": "Resolve links for a paper (full-text, datasets, citations, references).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "bibcode": { "type": "string", "description": "Paper bibcode" },
                    "link_type": { "type": "string", "enum": ["esource", "data", "citation", "reference", "coreads"], "description": "Specific link type (optional)" }
                },
                "required": ["bibcode"]
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            }
        },
        {
            "name": "scix_get_paper",
            "description": "Get detailed metadata for a single paper by bibcode, including abstract, affiliations, keywords, and links.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "bibcode": { "type": "string", "description": "Paper bibcode" }
                },
                "required": ["bibcode"]
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            }
        },
        {
            "name": "scix_fulltext",
            "description": "Retrieve the full text of a paper for reading. Returns the abstract plus the open-access body (fetched from arXiv when available), along with access links and a section list. Pass 'section' to retrieve a single section instead. Use this when you need to read or analyze a paper's contents, not just its metadata.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "bibcode": { "type": "string", "description": "Paper bibcode" },
                    "max_chars": { "type": "integer", "description": "Maximum body characters to return before truncating (default 40000, 0 = unlimited)", "default": 40000 },
                    "section": { "type": "string", "description": "Retrieve only this section: a 1-based index ('3') or a case-insensitive title substring ('method')" }
                },
                "required": ["bibcode"]
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            }
        },
        {
            "name": "scix_grep",
            "description": "Search a regex pattern across the full text of multiple papers at once. Give either explicit bibcodes or a search query to fan out over its results. Returns context snippets per paper with section attribution. Searches the open-access body (from arXiv) when available, falling back to the abstract. Ideal for extracting specific values, methods, or statements from a set of papers.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regex pattern (case-insensitive by default)" },
                    "bibcodes": { "type": "array", "items": { "type": "string" }, "description": "Bibcodes to search (alternative to 'query')" },
                    "query": { "type": "string", "description": "ADS search query whose results are grepped (alternative to 'bibcodes')" },
                    "rows": { "type": "integer", "description": "Max papers when using 'query' (default 10, max 50)", "default": 10 },
                    "case_sensitive": { "type": "boolean", "description": "Match case-sensitively (default false)", "default": false },
                    "max_matches": { "type": "integer", "description": "Max matches per paper (default 5)", "default": 5 },
                    "context_chars": { "type": "integer", "description": "Characters of context around each match (default 120)", "default": 120 }
                },
                "required": ["pattern"]
            },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            }
        }
    ])
}

// --- Reference content ---

const FIELDS_REFERENCE: &str = r#"SciX Searchable Fields
======================

Common search fields:
  author       - Author name (e.g., author:"Einstein, A.")
  first_author - First author only
  title        - Title words
  abs          - Abstract words
  year         - Publication year (e.g., year:2023 or year:[2020 TO 2023])
  bibcode      - ADS bibcode
  doi          - Digital Object Identifier
  identifier   - Any identifier (DOI, arXiv, bibcode)
  bibstem      - Journal abbreviation (e.g., bibstem:ApJ)
  object       - Astronomical object name
  orcid        - Author ORCID
  keyword      - Keywords
  full         - Full text search
  property     - Paper properties (refereed, openaccess, etc.)
  doctype      - Document type (article, inproceedings, etc.)

Common returnable fields:
  bibcode, title, author, year, pub, abstract, doi, identifier,
  doctype, esources, citation_count, reference, property, aff,
  orcid_pub, keyword, volume, page, read_count
"#;

const SYNTAX_REFERENCE: &str = r#"SciX Query Syntax Guide
=======================

Field queries:
  author:"Einstein"           - Author search
  title:"dark matter"         - Title search
  year:2023                   - Exact year
  year:[2020 TO 2023]         - Year range

Boolean operators:
  term1 AND term2             - Both terms
  term1 OR term2              - Either term
  NOT term                    - Exclude term
  (term1 OR term2) AND term3  - Grouping

Functional operators:
  citations(bibcode:XXX)      - Papers citing XXX
  references(bibcode:XXX)     - Papers referenced by XXX
  similar(bibcode:XXX)        - Content-similar papers
  trending(bibcode:XXX)       - Trending co-reads
  reviews(bibcode:XXX)        - Review articles

Wildcards:
  author:"Eins*"              - Prefix matching
  title:galax?                - Single character wildcard

Properties:
  property:refereed           - Refereed papers only
  property:openaccess         - Open access papers
  property:nonarticle         - Non-article documents

Sort options:
  date desc                   - Newest first (default)
  citation_count desc         - Most cited first
  score desc                  - Best match first
  read_count desc             - Most read first
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Author, Library, Paper, PdfLink, SearchResponse};

    fn make_paper(bibcode: &str, title: &str, authors: &[&str], year: u16) -> Paper {
        Paper {
            bibcode: bibcode.to_string(),
            title: title.to_string(),
            authors: authors
                .iter()
                .map(|name| Author::from_ads_format(name))
                .collect(),
            year: Some(year),
            publication: None,
            citation_count: None,
            doi: None,
            arxiv_id: None,
            abstract_text: None,
            doctype: None,
            identifiers: vec![],
            esources: vec![],
            pdf_links: vec![],
            properties: vec![],
            url: String::new(),
        }
    }

    fn make_library(id: &str, name: &str, num_documents: u32, description: &str) -> Library {
        Library {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            num_documents,
            public: false,
            owner: "test@example.com".to_string(),
            date_created: "2024-01-01".to_string(),
            date_last_modified: "2024-01-01".to_string(),
        }
    }

    #[test]
    fn test_format_search_results_basic() {
        let results = SearchResponse {
            papers: vec![
                make_paper(
                    "2016PhRvL.116f1102A",
                    "Observation of Gravitational Waves",
                    &["Abbott, B. P.", "Einstein, A."],
                    2016,
                ),
                make_paper(
                    "1998AJ....116.1009R",
                    "Observational Evidence from Supernovae",
                    &["Riess, A. G."],
                    1998,
                ),
            ],
            num_found: 2,
        };

        let output = format_search_results(&results, 0);
        assert!(output.contains("Found 2 results:"));
        assert!(output.contains("1. Observation of Gravitational Waves (2016)"));
        assert!(output.contains("Abbott, Einstein"));
        assert!(output.contains("Bibcode: 2016PhRvL.116f1102A"));
        assert!(output.contains("2. Observational Evidence from Supernovae (1998)"));
        assert!(output.contains("Riess"));
        assert!(output.contains("Bibcode: 1998AJ....116.1009R"));
    }

    #[test]
    fn test_format_search_results_with_citations_and_doi() {
        let mut paper = make_paper(
            "2016PhRvL.116f1102A",
            "Gravitational Waves",
            &["Abbott, B. P."],
            2016,
        );
        paper.citation_count = Some(5000);
        paper.doi = Some("10.1103/PhysRevLett.116.061102".to_string());

        let results = SearchResponse {
            papers: vec![paper],
            num_found: 1,
        };

        let output = format_search_results(&results, 0);
        assert!(output.contains("Citations: 5000"));
        assert!(output.contains("DOI: 10.1103/PhysRevLett.116.061102"));
    }

    #[test]
    fn test_format_search_results_many_authors_uses_et_al() {
        let paper = make_paper(
            "2020ApJ...000..000X",
            "Many Authors Paper",
            &["First, A.", "Second, B.", "Third, C.", "Fourth, D."],
            2020,
        );
        let results = SearchResponse {
            papers: vec![paper],
            num_found: 1,
        };

        let output = format_search_results(&results, 0);
        assert!(output.contains("First et al."));
        assert!(!output.contains("Second"));
    }

    #[test]
    fn test_format_search_results_pagination_hint() {
        let results = SearchResponse {
            papers: vec![make_paper("2020X...", "Paper", &["Auth, A."], 2020)],
            num_found: 100,
        };

        let output = format_search_results(&results, 0);
        assert!(output.contains("Use start=1 to see more results"));
    }

    #[test]
    fn test_format_search_results_with_start_offset() {
        let results = SearchResponse {
            papers: vec![make_paper("2020X...", "Paper", &["Auth, A."], 2020)],
            num_found: 50,
        };

        let output = format_search_results(&results, 10);
        assert!(output.contains("11. Paper (2020)"));
        assert!(output.contains("Use start=11 to see more results"));
    }

    #[test]
    fn test_format_search_results_empty() {
        let results = SearchResponse {
            papers: vec![],
            num_found: 0,
        };

        let output = format_search_results(&results, 0);
        assert!(output.contains("Found 0 results:"));
    }

    #[test]
    fn test_format_library_list_basic() {
        let libs = vec![
            make_library("abc123", "Cosmology", 22, "Dark matter papers"),
            make_library("def456", "Exoplanets", 15, ""),
        ];

        let output = format_library_list(&libs);
        assert!(output.contains("Found 2 libraries:"));
        assert!(output.contains("- Cosmology (22 documents)"));
        assert!(output.contains("ID: abc123"));
        assert!(output.contains("Description: Dark matter papers"));
        assert!(output.contains("- Exoplanets (15 documents)"));
        assert!(output.contains("ID: def456"));
        // Empty description should not appear
        assert!(!output.contains("Description: \n"));
    }

    #[test]
    fn test_format_library_list_empty() {
        let output = format_library_list(&[]);
        assert_eq!(output, "No libraries found.");
    }

    #[test]
    fn test_parse_paper_uri_valid() {
        assert_eq!(
            parse_paper_uri("scix://paper/2016PhRvL.116f1102A/abstract"),
            Some(("2016PhRvL.116f1102A", "abstract"))
        );
        assert_eq!(
            parse_paper_uri("scix://paper/2016PhRvL.116f1102A/fulltext"),
            Some(("2016PhRvL.116f1102A", "fulltext"))
        );
        assert_eq!(
            parse_paper_uri("scix://paper/2016PhRvL.116f1102A/sections"),
            Some(("2016PhRvL.116f1102A", "sections"))
        );
        assert_eq!(
            parse_paper_uri("scix://paper/2016PhRvL.116f1102A/sections/2"),
            Some(("2016PhRvL.116f1102A", "sections/2"))
        );
        assert_eq!(
            parse_paper_uri("scix://paper/2016PhRvL.116f1102A/sections/data analysis"),
            Some(("2016PhRvL.116f1102A", "sections/data analysis"))
        );
    }

    #[test]
    fn test_parse_paper_uri_rejects_non_paper() {
        assert_eq!(parse_paper_uri("scix://fields"), None);
        assert_eq!(parse_paper_uri("scix://paper/onlybibcode"), None);
        assert_eq!(parse_paper_uri("scix://paper//abstract"), None);
        assert_eq!(parse_paper_uri("scix://paper/bibcode/"), None);
        assert_eq!(parse_paper_uri("scix://paper/bibcode/sections/"), None);
    }

    #[test]
    fn test_format_fulltext_with_body() {
        let ft = crate::types::FullText {
            bibcode: "2016PhRvL.116f1102A".to_string(),
            title: "Observation of Gravitational Waves".to_string(),
            abstract_text: Some("We report the observation...".to_string()),
            arxiv_id: Some("1602.03837".to_string()),
            open_access: true,
            sources: vec![PdfLink {
                url: "https://arxiv.org/pdf/1602.03837.pdf".to_string(),
                link_type: crate::types::PdfLinkType::ArXiv,
                label: "arXiv PDF".to_string(),
            }],
            body: Some("Introduction. The body text.".to_string()),
            body_source: Some("arXiv HTML (https://arxiv.org/html/1602.03837)".to_string()),
            truncated: true,
            section_titles: vec!["1 Introduction".to_string(), "2 Methods".to_string()],
        };

        let out = format_fulltext(&ft);
        assert!(out.contains("# Observation of Gravitational Waves"));
        assert!(out.contains("**arXiv:** 1602.03837"));
        assert!(out.contains("## Abstract"));
        assert!(out.contains("## Full text"));
        assert!(out.contains("The body text."));
        assert!(out.contains("truncated"));
        assert!(out.contains("arXiv PDF"));
        assert!(out.contains("## Sections"));
        assert!(out.contains("1. 1 Introduction"));
        assert!(out.contains("2. 2 Methods"));
    }

    #[test]
    fn test_format_fulltext_no_body_falls_back_to_links() {
        let ft = crate::types::FullText {
            bibcode: "2016PhRvL.116f1102A".to_string(),
            title: "A Paper".to_string(),
            abstract_text: None,
            arxiv_id: None,
            open_access: false,
            sources: vec![PdfLink {
                url: "https://doi.org/10.1/x".to_string(),
                link_type: crate::types::PdfLinkType::Publisher,
                label: "Publisher".to_string(),
            }],
            body: None,
            body_source: None,
            truncated: false,
            section_titles: Vec::new(),
        };

        let out = format_fulltext(&ft);
        assert!(out.contains("No open-access body"));
        assert!(out.contains("Publisher"));
        assert!(!out.contains("## Sections"));
    }

    #[test]
    fn test_format_grep_results() {
        use crate::types::{GrepMatch, GrepResult};

        let results = vec![
            GrepResult {
                bibcode: "2016PhRvL.116f1102A".to_string(),
                title: "GW Paper".to_string(),
                year: Some(2016),
                searched: "fulltext".to_string(),
                source: Some("arXiv HTML".to_string()),
                matches: vec![GrepMatch {
                    section: Some("3 Results".to_string()),
                    snippet: "…the Hubble constant is 73…".to_string(),
                }],
            },
            GrepResult {
                bibcode: "1998AJ....116.1009R".to_string(),
                title: "SN Paper".to_string(),
                year: Some(1998),
                searched: "abstract".to_string(),
                source: None,
                matches: vec![],
            },
            GrepResult {
                bibcode: "BADBIBCODE".to_string(),
                title: String::new(),
                year: None,
                searched: "not_found".to_string(),
                source: None,
                matches: vec![],
            },
        ];

        let out = format_grep_results("hubble", &results);
        assert!(out.contains("across 3 papers — 1 with matches"));
        assert!(out.contains("GW Paper (2016) [searched: fulltext]"));
        assert!(out.contains("- (3 Results) …the Hubble constant is 73…"));
        assert!(out.contains("SN Paper (1998) [searched: abstract]"));
        assert!(out.contains("No matches."));
        assert!(out.contains("BADBIBCODE — not found in ADS"));
    }

    #[test]
    fn test_format_section_list() {
        use crate::types::Section;

        let sections = vec![
            Section {
                title: "1 Introduction".to_string(),
                text: "abc".to_string(),
            },
            Section {
                title: "2 Methods".to_string(),
                text: "defgh".to_string(),
            },
        ];

        let out = format_section_list("2016PhRvL.116f1102A", &sections);
        assert!(out.contains("Sections of 2016PhRvL.116f1102A"));
        assert!(out.contains("1. 1 Introduction (3 chars)"));
        assert!(out.contains("2. 2 Methods (5 chars)"));
    }

    #[test]
    fn test_format_library_list_single() {
        let libs = vec![make_library("id1", "My Library", 5, "Test")];
        let output = format_library_list(&libs);
        assert!(output.contains("Found 1 libraries:"));
        assert!(output.contains("- My Library (5 documents)"));
        assert!(output.contains("ID: id1"));
        assert!(output.contains("Description: Test"));
    }
}
