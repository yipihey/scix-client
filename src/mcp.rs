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
            "resources/read" => handle_resource_read(&id, &request["params"]),
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

fn handle_resource_read(id: &Value, params: &Value) -> Value {
    let uri = params["uri"].as_str().unwrap_or("");
    let content = match uri {
        "scix://fields" => FIELDS_REFERENCE.to_string(),
        "scix://syntax" => SYNTAX_REFERENCE.to_string(),
        _ => {
            return json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32602, "message": format!("Unknown resource: {}", uri) }
            });
        }
    };

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "contents": [{
                "uri": uri,
                "mimeType": "text/plain",
                "text": content
            }]
        }
    })
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

    let fl = fields.unwrap_or(crate::parse::DEFAULT_SEARCH_FIELDS);
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
            serde_json::to_string_pretty(&libs).map_err(|e| SciXError::Parse(e.to_string()))
        }
        "get" => {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'id' required for get".into()))?;
            let lib = client.get_library(id).await?;
            serde_json::to_string_pretty(&lib).map_err(|e| SciXError::Parse(e.to_string()))
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
            let id = args["id"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'id' required for update_permissions".into()))?;
            let email = args["email"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'email' required for update_permissions".into()))?;
            let permission = args["permission"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'permission' required for update_permissions".into()))?;
            client.update_permissions(id, email, permission).await?;
            Ok(format!("Permissions updated for {} on library {}", email, id))
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
            let bibcode = args["bibcode"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'bibcode' required for get_notes".into()))?;
            let note = client.get_annotation(library_id, bibcode).await?;
            Ok(note)
        }
        "add_note" | "edit_note" => {
            let bibcode = args["bibcode"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'bibcode' required for add_note/edit_note".into()))?;
            let content = args["content"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'content' required for add_note/edit_note".into()))?;
            client.set_annotation(library_id, bibcode, content).await?;
            Ok(format!("Note saved for {}", bibcode))
        }
        "delete_note" => {
            let bibcode = args["bibcode"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'bibcode' required for delete_note".into()))?;
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
            let query = args["query"]
                .as_str()
                .ok_or_else(|| SciXError::InvalidQuery("'query' required for add_by_query".into()))?;
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

    let query = format!("identifier:{}", bibcode);
    let results = client
        .search_with_options(&query, RICH_FIELDS, None, 1, 0)
        .await?;

    if results.papers.is_empty() {
        return Err(SciXError::NotFound(format!("Paper not found: {}", bibcode)));
    }

    let paper = &results.papers[0];
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", paper.title));

    let authors_str = if paper.authors.len() > 10 {
        let first_five: Vec<_> = paper.authors[..5].iter().map(|a| a.name.as_str()).collect();
        format!("{} ... and {} more", first_five.join("; "), paper.authors.len() - 5)
    } else {
        paper.authors.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join("; ")
    };
    out.push_str(&format!("**Authors:** {}\n", authors_str));
    out.push_str(&format!("**Year:** {}\n", paper.year.map(|y| y.to_string()).unwrap_or_default()));

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
        out.push_str(&format!("**Properties:** {}\n", paper.properties.join(", ")));
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

    Ok(out)
}

// --- Formatting helpers ---

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
        out.push_str(&format!(
            "*Use start={} to see more results*\n",
            shown
        ));
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
