//! `scix setup` — one-command MCP server configuration for AI editors.
//!
//! Detects installed editors (Claude Code, Claude Desktop, Cursor, Zed),
//! prompts for an API token, validates it, and writes the correct MCP
//! config for each editor.

use crate::error::{Result, SciXError};
use crate::SciXClient;
use std::path::PathBuf;

/// Supported AI editors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum EditorTarget {
    ClaudeCode,
    ClaudeDesktop,
    Cursor,
    Zed,
}

impl std::fmt::Display for EditorTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClaudeCode => write!(f, "Claude Code"),
            Self::ClaudeDesktop => write!(f, "Claude Desktop"),
            Self::Cursor => write!(f, "Cursor"),
            Self::Zed => write!(f, "Zed"),
        }
    }
}

struct DetectedEditor {
    target: EditorTarget,
    config_path: Option<PathBuf>,
    use_cli: bool,
}

#[derive(Debug)]
enum ConfigResult {
    Configured,
    Skipped,
    Failed(String),
}

impl std::fmt::Display for ConfigResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Configured => write!(f, "done"),
            Self::Skipped => write!(f, "skipped"),
            Self::Failed(msg) => write!(f, "FAILED ({})", msg),
        }
    }
}

/// Mask a token for display: show first 4 and last 4 chars.
fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        return "****".to_string();
    }
    format!("{}...{}", &token[..4], &token[token.len() - 4..])
}

/// Resolve the API token from env vars or interactive prompt.
fn resolve_token(yes: bool) -> Result<String> {
    // Check env vars first.
    if let Ok(token) = std::env::var("SCIX_API_TOKEN") {
        if !token.is_empty() {
            println!("  Found SCIX_API_TOKEN in environment.");
            println!("  Token: {}", mask_token(&token));
            return Ok(token);
        }
    }
    if let Ok(token) = std::env::var("ADS_API_TOKEN") {
        if !token.is_empty() {
            println!("  Found ADS_API_TOKEN in environment.");
            println!("  Token: {}", mask_token(&token));
            return Ok(token);
        }
    }

    if yes {
        return Err(SciXError::Config(
            "No SCIX_API_TOKEN or ADS_API_TOKEN found in environment (required with --yes)"
                .to_string(),
        ));
    }

    // Interactive prompt.
    println!("  No API token found in environment.");
    println!("  Get a free token at: https://ui.adsabs.harvard.edu/user/settings/token");
    println!();

    let token: String = dialoguer::Password::new()
        .with_prompt("  Enter your SciX/ADS API token")
        .interact()
        .map_err(|e| SciXError::Config(format!("Failed to read token: {}", e)))?;

    if token.is_empty() {
        return Err(SciXError::Config("Token cannot be empty".to_string()));
    }

    println!("  Token: {}", mask_token(&token));
    Ok(token)
}

/// Validate the token by running a simple search.
async fn validate_token(token: &str) -> Result<()> {
    let client = SciXClient::new(token);
    match client.search("star", 1).await {
        Ok(_) => {
            println!("  Validating... OK");
            Ok(())
        }
        Err(SciXError::AuthRequired) => Err(SciXError::Config(
            "Token validation failed: unauthorized (401). Check your token.".to_string(),
        )),
        Err(SciXError::Api { status: 401, .. }) => Err(SciXError::Config(
            "Token validation failed: unauthorized (401). Check your token.".to_string(),
        )),
        Err(e) => {
            println!("  Validating... WARNING: could not reach API ({})", e);
            println!("  Continuing anyway — token will be saved.");
            Ok(())
        }
    }
}

/// Get the absolute path to the current scix binary.
fn locate_binary() -> Result<PathBuf> {
    std::env::current_exe().map_err(|e| SciXError::Config(format!("Cannot locate binary: {}", e)))
}

/// Check if `claude` CLI is available on PATH.
fn claude_cli_available() -> bool {
    std::process::Command::new("which")
        .arg("claude")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Detect which editors are installed.
fn detect_editors(filter: Option<EditorTarget>) -> Vec<DetectedEditor> {
    let mut editors = Vec::new();

    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return editors,
    };

    // Claude Code: prefer CLI, fallback to config file.
    if filter.is_none() || filter == Some(EditorTarget::ClaudeCode) {
        let has_cli = claude_cli_available();
        let claude_dir = home.join(".claude");
        if has_cli || claude_dir.exists() {
            editors.push(DetectedEditor {
                target: EditorTarget::ClaudeCode,
                config_path: if has_cli {
                    None
                } else {
                    Some(claude_dir.join("settings.json"))
                },
                use_cli: has_cli,
            });
        }
    }

    // Claude Desktop
    if filter.is_none() || filter == Some(EditorTarget::ClaudeDesktop) {
        let config_dir = if cfg!(target_os = "macos") {
            Some(home.join("Library/Application Support/Claude"))
        } else if cfg!(target_os = "windows") {
            std::env::var("APPDATA")
                .ok()
                .map(|p| PathBuf::from(p).join("Claude"))
        } else {
            None
        };

        if let Some(dir) = config_dir {
            if dir.exists() {
                editors.push(DetectedEditor {
                    target: EditorTarget::ClaudeDesktop,
                    config_path: Some(dir.join("claude_desktop_config.json")),
                    use_cli: false,
                });
            }
        }
    }

    // Cursor
    if filter.is_none() || filter == Some(EditorTarget::Cursor) {
        let cursor_dir = home.join(".cursor");
        if cursor_dir.exists() {
            editors.push(DetectedEditor {
                target: EditorTarget::Cursor,
                config_path: Some(cursor_dir.join("mcp.json")),
                use_cli: false,
            });
        }
    }

    // Zed
    if filter.is_none() || filter == Some(EditorTarget::Zed) {
        let zed_dir = home.join(".config/zed");
        if zed_dir.exists() {
            editors.push(DetectedEditor {
                target: EditorTarget::Zed,
                config_path: Some(zed_dir.join("settings.json")),
                use_cli: false,
            });
        }
    }

    editors
}

/// Build the MCP server entry for standard editors (Claude Code/Desktop, Cursor).
fn standard_mcp_entry(binary: &str, token: &str) -> serde_json::Value {
    serde_json::json!({
        "command": binary,
        "args": ["serve"],
        "env": {
            "SCIX_API_TOKEN": token
        }
    })
}

/// Build the MCP server entry for Zed (different schema).
fn zed_mcp_entry(binary: &str, token: &str) -> serde_json::Value {
    serde_json::json!({
        "command": {
            "path": binary,
            "args": ["serve"],
            "env": {
                "SCIX_API_TOKEN": token
            }
        }
    })
}

/// Update a JSON config file, inserting the scix entry under the given section key.
/// Returns Ok(true) if the entry already existed.
fn update_json_config(
    path: &PathBuf,
    section_key: &str,
    entry: serde_json::Value,
    yes: bool,
) -> std::result::Result<ConfigResult, String> {
    // Read existing file or start fresh.
    let content = if path.exists() {
        std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?
    } else {
        "{}".to_string()
    };

    // Parse JSON.
    let mut root: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => {
            // File has comments (e.g. JSONC) or is otherwise unparseable.
            // Print the snippet for manual paste.
            let snippet = serde_json::json!({ section_key: { "scix": entry } });
            return Err(format!(
                "Could not parse {} (may contain comments). Add manually:\n{}",
                path.display(),
                serde_json::to_string_pretty(&snippet).unwrap()
            ));
        }
    };

    // Ensure the root is an object.
    {
        let obj = root
            .as_object_mut()
            .ok_or_else(|| format!("{} is not a JSON object", path.display()))?;

        // Ensure section exists.
        if !obj.contains_key(section_key) {
            obj.insert(
                section_key.to_string(),
                serde_json::Value::Object(serde_json::Map::new()),
            );
        }

        let section = obj
            .get_mut(section_key)
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| format!("\"{}\" in {} is not an object", section_key, path.display()))?;

        // Check if already configured.
        if section.contains_key("scix") && !yes {
            let overwrite = dialoguer::Confirm::new()
                .with_prompt(format!(
                    "  scix is already configured in {}. Overwrite?",
                    path.display()
                ))
                .default(false)
                .interact()
                .unwrap_or(false);
            if !overwrite {
                return Ok(ConfigResult::Skipped);
            }
        }

        section.insert("scix".to_string(), entry);
    }

    // Write back (mutable borrows are dropped).
    let output = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    // Create parent dir if needed.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create {}: {}", parent.display(), e))?;
    }

    std::fs::write(path, output.as_bytes())
        .map_err(|e| format!("Cannot write {}: {}", path.display(), e))?;

    Ok(ConfigResult::Configured)
}

/// Configure Claude Code via its CLI.
fn configure_claude_code_cli(binary: &str, token: &str) -> ConfigResult {
    // Remove existing entry first (ignore errors — it may not exist).
    for scope in ["user", "local", "project"] {
        let _ = std::process::Command::new("claude")
            .args(["mcp", "remove", "scix", "--scope", scope])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    let status = std::process::Command::new("claude")
        .args([
            "mcp",
            "add",
            "scix",
            "-e",
            &format!("SCIX_API_TOKEN={}", token),
            "--",
            binary,
            "serve",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => ConfigResult::Configured,
        Ok(s) => ConfigResult::Failed(format!("claude mcp add exited with {}", s)),
        Err(e) => ConfigResult::Failed(format!("failed to run claude CLI: {}", e)),
    }
}

/// Configure a single editor.
fn configure_editor(editor: &DetectedEditor, binary: &str, token: &str, yes: bool) -> ConfigResult {
    if editor.use_cli && editor.target == EditorTarget::ClaudeCode {
        return configure_claude_code_cli(binary, token);
    }

    let path = match &editor.config_path {
        Some(p) => p,
        None => return ConfigResult::Failed("no config path".to_string()),
    };

    let (section_key, entry) = match editor.target {
        EditorTarget::Zed => ("context_servers", zed_mcp_entry(binary, token)),
        _ => ("mcpServers", standard_mcp_entry(binary, token)),
    };

    match update_json_config(path, section_key, entry, yes) {
        Ok(result) => result,
        Err(msg) => {
            eprintln!("  {}", msg);
            ConfigResult::Failed("parse error".to_string())
        }
    }
}

/// Run the setup wizard.
pub async fn run_setup(
    editor: Option<EditorTarget>,
    skip_validation: bool,
    yes: bool,
) -> Result<()> {
    println!();
    println!("scix setup \u{2014} SciX MCP Server Setup");
    println!("====================================");
    println!();

    // 1. Resolve token.
    println!("Checking API token...");
    let token = resolve_token(yes)?;

    // 2. Validate token.
    if !skip_validation {
        validate_token(&token).await?;
    } else {
        println!("  Skipping validation (--skip-validation).");
    }
    println!();

    // 3. Locate binary.
    println!("Locating scix binary...");
    let binary_path = locate_binary()?;
    let binary = binary_path.to_string_lossy().to_string();
    println!("  {}", binary);
    println!();

    // 4. Detect editors.
    println!("Detecting editors...");
    let all_targets = [
        EditorTarget::ClaudeCode,
        EditorTarget::ClaudeDesktop,
        EditorTarget::Cursor,
        EditorTarget::Zed,
    ];

    let detected = detect_editors(editor);
    let detected_targets: Vec<EditorTarget> = detected.iter().map(|e| e.target).collect();

    if editor.is_none() {
        // Show all editors with found/absent status.
        for target in &all_targets {
            if detected_targets.contains(target) {
                println!("  [found]   {}", target);
            } else {
                println!("  [absent]  {}", target);
            }
        }
    } else {
        for target in &detected_targets {
            println!("  [found]   {}", target);
        }
    }

    if detected.is_empty() {
        println!();
        if let Some(target) = editor {
            println!("{} was not detected on this system.", target);
        } else {
            println!("No supported editors detected.");
        }
        println!("See https://github.com/yipihey/scix-client for manual setup instructions.");
        return Ok(());
    }
    println!();

    // 5. Configure each editor.
    println!("Configuring editors...");
    let mut any_configured = false;
    for editor_info in &detected {
        let result = configure_editor(editor_info, &binary, &token, yes);
        let pad = 15 - editor_info.target.to_string().len();
        println!(
            "  {}:{}{}",
            editor_info.target,
            " ".repeat(pad.max(1)),
            result
        );
        if matches!(result, ConfigResult::Configured) {
            any_configured = true;
        }
    }
    println!();

    // 6. Summary.
    if any_configured {
        println!("Setup complete! Your AI assistant can now search the astronomy literature.");
        println!("Try asking: \"Find recent papers about gravitational waves\"");
    } else {
        println!("No editors were configured. Run `scix setup` again or configure manually.");
    }

    Ok(())
}
