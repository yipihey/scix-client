//! `scix doctor` — diagnostic checks for installation and connectivity.
//!
//! Prints the version, the resolved binary path, environment-token status,
//! a live API health check, and which AI editors were detected.

use crate::error::{Result, SciXError};
use crate::SciXClient;
use std::path::PathBuf;

/// Result of a single check, with a short status line.
enum CheckStatus {
    Ok(String),
    Warn(String),
    Fail(String),
}

impl CheckStatus {
    fn render(&self) -> String {
        match self {
            Self::Ok(msg) => format!("[ ok ]  {}", msg),
            Self::Warn(msg) => format!("[warn]  {}", msg),
            Self::Fail(msg) => format!("[fail]  {}", msg),
        }
    }
    fn is_fail(&self) -> bool {
        matches!(self, Self::Fail(_))
    }
}

fn locate_binary() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

fn check_binary() -> CheckStatus {
    match locate_binary() {
        Some(p) => CheckStatus::Ok(format!(
            "scix binary v{} at {}",
            env!("CARGO_PKG_VERSION"),
            p.display()
        )),
        None => CheckStatus::Warn("cannot resolve current executable path".to_string()),
    }
}

fn check_token() -> (CheckStatus, Option<String>) {
    if let Ok(t) = std::env::var("SCIX_API_TOKEN") {
        if !t.is_empty() {
            return (
                CheckStatus::Ok("SCIX_API_TOKEN is set".to_string()),
                Some(t),
            );
        }
    }
    if let Ok(t) = std::env::var("ADS_API_TOKEN") {
        if !t.is_empty() {
            return (CheckStatus::Ok("ADS_API_TOKEN is set".to_string()), Some(t));
        }
    }
    (
        CheckStatus::Fail(
            "no SCIX_API_TOKEN or ADS_API_TOKEN in environment — run `scix setup`".to_string(),
        ),
        None,
    )
}

async fn check_api(token: &str) -> CheckStatus {
    let client = SciXClient::new(token);
    match client.search("star", 1).await {
        Ok(r) => CheckStatus::Ok(format!(
            "SciX API reachable ({} results for probe query)",
            r.num_found
        )),
        Err(SciXError::AuthRequired) | Err(SciXError::Api { status: 401, .. }) => {
            CheckStatus::Fail("API rejected token (401 unauthorized)".to_string())
        }
        Err(SciXError::RateLimited { .. }) => {
            CheckStatus::Warn("API rate-limited the probe; token is otherwise valid".to_string())
        }
        Err(e) => CheckStatus::Fail(format!("API call failed: {}", e)),
    }
}

fn check_editors() -> CheckStatus {
    let detected = crate::setup::detect_editors_public(None);
    if detected.is_empty() {
        CheckStatus::Warn(
            "no supported MCP hosts detected (Claude Code/Desktop, Cursor, Zed, Gemini CLI, Codex CLI, Windsurf)"
                .to_string(),
        )
    } else {
        let names: Vec<String> = detected.iter().map(|n| n.to_string()).collect();
        CheckStatus::Ok(format!("MCP hosts detected: {}", names.join(", ")))
    }
}

fn check_path() -> CheckStatus {
    let Some(exe) = locate_binary() else {
        return CheckStatus::Warn("cannot check PATH".to_string());
    };
    let Ok(path) = std::env::var("PATH") else {
        return CheckStatus::Warn("PATH not set".to_string());
    };
    let parent = match exe.parent() {
        Some(p) => p.to_path_buf(),
        None => return CheckStatus::Warn("binary has no parent directory".to_string()),
    };
    let sep = if cfg!(windows) { ';' } else { ':' };
    let parent_path = parent.as_path();
    let on_path = path
        .split(sep)
        .any(|dir| std::path::Path::new(dir) == parent_path);
    if on_path {
        CheckStatus::Ok(format!("{} is on PATH", parent.display()))
    } else {
        CheckStatus::Warn(format!(
            "binary directory {} is not on PATH",
            parent.display()
        ))
    }
}

/// Run the full diagnostic check. Exits with code 1 if any check failed.
pub async fn run_doctor() -> Result<()> {
    println!();
    println!("scix doctor — environment diagnostic");
    println!("====================================");
    println!();

    let mut checks: Vec<CheckStatus> = Vec::new();

    checks.push(check_binary());
    checks.push(check_path());

    let (token_check, token) = check_token();
    let have_token = matches!(token_check, CheckStatus::Ok(_));
    checks.push(token_check);

    if let Some(t) = token {
        checks.push(check_api(&t).await);
    }

    checks.push(check_editors());

    for c in &checks {
        println!("  {}", c.render());
    }
    println!();

    let any_fail = checks.iter().any(|c| c.is_fail());
    if any_fail {
        println!("Some checks failed. Next steps:");
        if !have_token {
            println!("  • Run `scix setup` to acquire and store an API token.");
        }
        std::process::exit(1);
    } else {
        println!("All checks passed.");
    }
    Ok(())
}
