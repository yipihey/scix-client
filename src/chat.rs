//! `scix chat` — built-in REPL that drives any tool-use-capable LLM against
//! the scix tool set.
//!
//! Auto-detects a provider from the environment (Anthropic, OpenAI, Gemini,
//! Ollama) or accepts an explicit `--provider`. Tool definitions and dispatch
//! are reused from the MCP server, so the same tools work everywhere.

use crate::error::{Result, SciXError};
use crate::mcp::{dispatch_tool_call, tool_definitions_value};
use crate::SciXClient;
use reqwest::Client as HttpClient;
use serde_json::{json, Value};
use std::time::Duration;

const SYSTEM_PROMPT: &str = r#"You are a research assistant with access to the SciX / NASA ADS astronomy literature database via a set of tools (search, export, metrics, library management, object name resolution, paper details, and citation/reference network analysis).

When a user asks about scientific papers, authors, citations, libraries, or astronomical objects, prefer using the tools to fetch live data rather than relying on your training. Be concise. When showing search results, include bibcodes so the user can reference them later. When the user asks for citations or references in a specific format (BibTeX, AASTeX, MNRAS, etc.), use the scix_export tool."#;

const DEFAULT_MAX_TURNS: u32 = 25;

/// Provider selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Provider {
    Anthropic,
    Openai,
    Gemini,
    Ollama,
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anthropic => write!(f, "anthropic"),
            Self::Openai => write!(f, "openai"),
            Self::Gemini => write!(f, "gemini"),
            Self::Ollama => write!(f, "ollama"),
        }
    }
}

impl Provider {
    fn default_model(&self) -> &'static str {
        match self {
            Self::Anthropic => "claude-sonnet-4-6",
            Self::Openai => "gpt-5",
            Self::Gemini => "gemini-2.5-pro",
            Self::Ollama => "llama3.1:8b",
        }
    }
}

/// Auto-detect a provider from environment variables.
fn detect_provider() -> Option<Provider> {
    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        return Some(Provider::Anthropic);
    }
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return Some(Provider::Openai);
    }
    if std::env::var("GEMINI_API_KEY").is_ok() || std::env::var("GOOGLE_API_KEY").is_ok() {
        return Some(Provider::Gemini);
    }
    if std::env::var("OLLAMA_HOST").is_ok() {
        return Some(Provider::Ollama);
    }
    None
}

/// One assistant turn: any text output plus any tool calls the model issued.
#[derive(Default)]
struct AssistantTurn {
    text: String,
    tool_calls: Vec<ToolCall>,
}

#[derive(Clone)]
struct ToolCall {
    id: String,
    name: String,
    arguments: Value,
}

/// Canonical conversation message (Anthropic-shaped; each provider converts
/// from this on the way out).
#[derive(Clone)]
struct Message {
    role: Role,
    blocks: Vec<Block>,
}

#[derive(Clone, PartialEq)]
enum Role {
    User,
    Assistant,
}

#[derive(Clone)]
enum Block {
    Text(String),
    ToolUse(ToolCall),
    ToolResult {
        id: String,
        content: String,
        is_error: bool,
    },
}

// --- Tool-schema translation ---------------------------------------------

fn tools_for_anthropic() -> Value {
    let defs = tool_definitions_value();
    let mut out = Vec::new();
    for t in defs.as_array().cloned().unwrap_or_default() {
        out.push(json!({
            "name": t["name"],
            "description": t["description"],
            "input_schema": t["inputSchema"],
        }));
    }
    Value::Array(out)
}

fn tools_for_openai() -> Value {
    let defs = tool_definitions_value();
    let mut out = Vec::new();
    for t in defs.as_array().cloned().unwrap_or_default() {
        out.push(json!({
            "type": "function",
            "function": {
                "name": t["name"],
                "description": t["description"],
                "parameters": t["inputSchema"],
            }
        }));
    }
    Value::Array(out)
}

fn tools_for_gemini() -> Value {
    let defs = tool_definitions_value();
    let mut decls = Vec::new();
    for t in defs.as_array().cloned().unwrap_or_default() {
        decls.push(json!({
            "name": t["name"],
            "description": t["description"],
            "parameters": t["inputSchema"],
        }));
    }
    json!([{ "functionDeclarations": decls }])
}

// --- Provider HTTP calls --------------------------------------------------

async fn complete_anthropic(
    http: &HttpClient,
    api_key: &str,
    model: &str,
    messages: &[Message],
) -> std::result::Result<AssistantTurn, SciXError> {
    let mut native_messages = Vec::new();
    for m in messages {
        let role = match m.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        let mut content = Vec::new();
        for b in &m.blocks {
            match b {
                Block::Text(t) => content.push(json!({"type": "text", "text": t})),
                Block::ToolUse(tc) => content.push(json!({
                    "type": "tool_use",
                    "id": tc.id,
                    "name": tc.name,
                    "input": tc.arguments,
                })),
                Block::ToolResult {
                    id,
                    content: c,
                    is_error,
                } => content.push(json!({
                    "type": "tool_result",
                    "tool_use_id": id,
                    "content": c,
                    "is_error": is_error,
                })),
            }
        }
        native_messages.push(json!({"role": role, "content": content}));
    }

    let body = json!({
        "model": model,
        "max_tokens": 4096,
        "system": SYSTEM_PROMPT,
        "tools": tools_for_anthropic(),
        "messages": native_messages,
    });

    let resp = http
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(SciXError::Http)?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(SciXError::Api {
            status,
            message: format!("Anthropic: {}", text),
        });
    }
    let v: Value = resp.json().await.map_err(SciXError::Http)?;

    let mut turn = AssistantTurn::default();
    if let Some(blocks) = v["content"].as_array() {
        for b in blocks {
            match b["type"].as_str() {
                Some("text") => {
                    if let Some(t) = b["text"].as_str() {
                        turn.text.push_str(t);
                    }
                }
                Some("tool_use") => {
                    turn.tool_calls.push(ToolCall {
                        id: b["id"].as_str().unwrap_or("").to_string(),
                        name: b["name"].as_str().unwrap_or("").to_string(),
                        arguments: b["input"].clone(),
                    });
                }
                _ => {}
            }
        }
    }
    Ok(turn)
}

async fn complete_openai(
    http: &HttpClient,
    api_key: &str,
    base_url: &str,
    model: &str,
    messages: &[Message],
) -> std::result::Result<AssistantTurn, SciXError> {
    let mut native_messages = vec![json!({"role": "system", "content": SYSTEM_PROMPT})];

    for m in messages {
        match m.role {
            Role::User => {
                // OpenAI distinguishes "user" text from "tool" results.
                // Tool results are role:"tool" with tool_call_id.
                let mut text_parts = Vec::new();
                let mut tool_results = Vec::new();
                for b in &m.blocks {
                    match b {
                        Block::Text(t) => text_parts.push(t.clone()),
                        Block::ToolResult { id, content, .. } => {
                            tool_results.push((id.clone(), content.clone()));
                        }
                        Block::ToolUse(_) => {} // never on user role
                    }
                }
                if !text_parts.is_empty() {
                    native_messages.push(json!({"role": "user", "content": text_parts.join("\n")}));
                }
                for (id, content) in tool_results {
                    native_messages.push(json!({
                        "role": "tool",
                        "tool_call_id": id,
                        "content": content,
                    }));
                }
            }
            Role::Assistant => {
                let mut text = String::new();
                let mut tool_calls = Vec::new();
                for b in &m.blocks {
                    match b {
                        Block::Text(t) => text.push_str(t),
                        Block::ToolUse(tc) => tool_calls.push(json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": serde_json::to_string(&tc.arguments).unwrap_or_default(),
                            }
                        })),
                        Block::ToolResult { .. } => {}
                    }
                }
                let mut msg = json!({"role": "assistant"});
                if !text.is_empty() {
                    msg["content"] = json!(text);
                } else {
                    msg["content"] = Value::Null;
                }
                if !tool_calls.is_empty() {
                    msg["tool_calls"] = Value::Array(tool_calls);
                }
                native_messages.push(msg);
            }
        }
    }

    let body = json!({
        "model": model,
        "messages": native_messages,
        "tools": tools_for_openai(),
    });

    let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let mut req = http.post(&endpoint).json(&body);
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }
    let resp = req.send().await.map_err(SciXError::Http)?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(SciXError::Api {
            status,
            message: format!("OpenAI ({}): {}", endpoint, text),
        });
    }
    let v: Value = resp.json().await.map_err(SciXError::Http)?;

    let mut turn = AssistantTurn::default();
    let msg = &v["choices"][0]["message"];
    if let Some(t) = msg["content"].as_str() {
        turn.text.push_str(t);
    }
    if let Some(calls) = msg["tool_calls"].as_array() {
        for c in calls {
            let id = c["id"].as_str().unwrap_or("").to_string();
            let name = c["function"]["name"].as_str().unwrap_or("").to_string();
            let raw_args = c["function"]["arguments"].as_str().unwrap_or("{}");
            let arguments: Value = serde_json::from_str(raw_args).unwrap_or_else(|_| json!({}));
            turn.tool_calls.push(ToolCall {
                id,
                name,
                arguments,
            });
        }
    }
    Ok(turn)
}

async fn complete_gemini(
    http: &HttpClient,
    api_key: &str,
    model: &str,
    messages: &[Message],
) -> std::result::Result<AssistantTurn, SciXError> {
    let mut contents = Vec::new();
    for m in messages {
        let role = match m.role {
            Role::User => "user",
            Role::Assistant => "model",
        };
        let mut parts = Vec::new();
        for b in &m.blocks {
            match b {
                Block::Text(t) => parts.push(json!({"text": t})),
                Block::ToolUse(tc) => parts.push(json!({
                    "functionCall": { "name": tc.name, "args": tc.arguments }
                })),
                Block::ToolResult { id: _, content, .. } => parts.push(json!({
                    "functionResponse": {
                        "name": "tool_result",
                        "response": { "content": content }
                    }
                })),
            }
        }
        contents.push(json!({"role": role, "parts": parts}));
    }

    let body = json!({
        "system_instruction": { "parts": [{"text": SYSTEM_PROMPT}] },
        "contents": contents,
        "tools": tools_for_gemini(),
    });

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );
    let resp = http
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(SciXError::Http)?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(SciXError::Api {
            status,
            message: format!("Gemini: {}", text),
        });
    }
    let v: Value = resp.json().await.map_err(SciXError::Http)?;

    let mut turn = AssistantTurn::default();
    if let Some(parts) = v["candidates"][0]["content"]["parts"].as_array() {
        for p in parts {
            if let Some(t) = p["text"].as_str() {
                turn.text.push_str(t);
            }
            if let Some(fc) = p.get("functionCall") {
                let id = format!("call_{}", turn.tool_calls.len());
                turn.tool_calls.push(ToolCall {
                    id,
                    name: fc["name"].as_str().unwrap_or("").to_string(),
                    arguments: fc.get("args").cloned().unwrap_or(json!({})),
                });
            }
        }
    }
    Ok(turn)
}

// --- REPL -----------------------------------------------------------------

#[derive(Clone)]
struct ChatConfig {
    provider: Provider,
    model: String,
    api_key: String,
    base_url: String, // OpenAI / Ollama only
    max_turns: u32,
}

fn build_config(
    provider_arg: Option<Provider>,
    model_arg: Option<String>,
    max_turns: u32,
) -> Result<ChatConfig> {
    let provider = match provider_arg {
        Some(p) => p,
        None => detect_provider().ok_or_else(|| {
            SciXError::Config(
                "No LLM provider detected. Set one of: ANTHROPIC_API_KEY, OPENAI_API_KEY, \
                 GEMINI_API_KEY, OLLAMA_HOST. Or pass --provider."
                    .into(),
            )
        })?,
    };

    let (api_key, base_url) = match provider {
        Provider::Anthropic => (
            std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| SciXError::Config("ANTHROPIC_API_KEY not set".into()))?,
            String::new(),
        ),
        Provider::Openai => (
            std::env::var("OPENAI_API_KEY")
                .map_err(|_| SciXError::Config("OPENAI_API_KEY not set".into()))?,
            std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".into()),
        ),
        Provider::Gemini => (
            std::env::var("GEMINI_API_KEY")
                .or_else(|_| std::env::var("GOOGLE_API_KEY"))
                .map_err(|_| {
                    SciXError::Config("GEMINI_API_KEY (or GOOGLE_API_KEY) not set".into())
                })?,
            String::new(),
        ),
        Provider::Ollama => {
            // Ollama exposes an OpenAI-compatible API at /v1.
            let host =
                std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into());
            let base = if host.contains("/v1") {
                host
            } else {
                format!("{}/v1", host.trim_end_matches('/'))
            };
            ("ollama".to_string(), base)
        }
    };

    Ok(ChatConfig {
        provider,
        model: model_arg.unwrap_or_else(|| provider.default_model().to_string()),
        api_key,
        base_url,
        max_turns,
    })
}

async fn one_completion(
    http: &HttpClient,
    cfg: &ChatConfig,
    messages: &[Message],
) -> std::result::Result<AssistantTurn, SciXError> {
    match cfg.provider {
        Provider::Anthropic => complete_anthropic(http, &cfg.api_key, &cfg.model, messages).await,
        Provider::Openai | Provider::Ollama => {
            complete_openai(http, &cfg.api_key, &cfg.base_url, &cfg.model, messages).await
        }
        Provider::Gemini => complete_gemini(http, &cfg.api_key, &cfg.model, messages).await,
    }
}

fn pretty_arguments(args: &Value) -> String {
    let s = serde_json::to_string(args).unwrap_or_default();
    if s.len() > 120 {
        format!("{}...", &s[..117])
    } else {
        s
    }
}

/// Public entry point.
pub async fn run_chat(
    provider: Option<Provider>,
    model: Option<String>,
    max_turns: Option<u32>,
    client: SciXClient,
) -> Result<()> {
    let cfg = build_config(provider, model, max_turns.unwrap_or(DEFAULT_MAX_TURNS))?;
    let http = HttpClient::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(SciXError::Http)?;

    println!();
    println!("scix chat — {} ({})", cfg.provider, cfg.model);
    println!("Type your question, blank line to send. Ctrl-D or 'exit' to quit.");
    println!();

    let mut messages: Vec<Message> = Vec::new();

    while let Ok(prompt) = dialoguer::Input::<String>::new()
        .with_prompt(">")
        .allow_empty(false)
        .interact_text()
    {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            continue;
        }
        if matches!(trimmed, "exit" | "quit" | ":q") {
            break;
        }

        messages.push(Message {
            role: Role::User,
            blocks: vec![Block::Text(trimmed.to_string())],
        });

        // Tool-call loop.
        let mut turns_remaining = cfg.max_turns;
        loop {
            if turns_remaining == 0 {
                println!("\n[reached --max-turns limit, ending iteration]");
                break;
            }
            turns_remaining -= 1;

            let turn = match one_completion(&http, &cfg, &messages).await {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("\nerror: {}", e);
                    break;
                }
            };

            if !turn.text.is_empty() {
                println!("\n{}", turn.text);
            }

            if turn.tool_calls.is_empty() {
                // Final assistant turn — record and break.
                messages.push(Message {
                    role: Role::Assistant,
                    blocks: vec![Block::Text(turn.text.clone())],
                });
                break;
            }

            // Record the assistant turn (text + tool uses) verbatim.
            let mut blocks: Vec<Block> = Vec::new();
            if !turn.text.is_empty() {
                blocks.push(Block::Text(turn.text.clone()));
            }
            for tc in &turn.tool_calls {
                blocks.push(Block::ToolUse(tc.clone()));
            }
            messages.push(Message {
                role: Role::Assistant,
                blocks,
            });

            // Execute each tool call and gather results.
            let mut result_blocks: Vec<Block> = Vec::new();
            for tc in &turn.tool_calls {
                println!("  [tool] {}({})", tc.name, pretty_arguments(&tc.arguments));
                let r = dispatch_tool_call(&client, &tc.name, &tc.arguments).await;
                let (content, is_error) = match r {
                    Ok(s) => (s, false),
                    Err(e) => (format!("Error: {}", e), true),
                };
                result_blocks.push(Block::ToolResult {
                    id: tc.id.clone(),
                    content,
                    is_error,
                });
            }
            messages.push(Message {
                role: Role::User,
                blocks: result_blocks,
            });
        }

        println!();
    }

    println!("Bye.");
    Ok(())
}
