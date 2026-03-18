//! MCP client configuration for **Cursor** (`~/.cursor/mcp.json`) and **Windsurf**
//! (`~/.codeium/windsurf/mcp_config.json`): merge `relay-mcp` (`command` = `relay`, `args` = `["mcp"]`, …).
//! See `docs/TERMINOLOGY.md`. Tool `relay_interactive_feedback` requires non-empty `retell` (this turn's assistant reply).
//! Full install may also update user `PATH`.

use crate::{gui_binary_name, relay_cli_directory};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

fn home_dir() -> Result<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("USERPROFILE not set"))
    } else {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("HOME not set"))
    }
}

/// Absolute path to `relay` and argv prefix `["mcp"]` for MCP stdio server.
pub fn relay_mcp_command_and_args() -> Result<(String, Vec<String>)> {
    let dir = relay_cli_directory()?;
    let exe = dir.join(gui_binary_name());
    if !exe.exists() {
        anyhow::bail!("relay not found at {}", exe.display());
    }
    Ok((exe.to_string_lossy().into_owned(), vec!["mcp".to_string()]))
}

fn relay_mcp_entry() -> Result<Value> {
    let (command, args) = relay_mcp_command_and_args()?;
    Ok(json!({
        "command": command,
        "args": args,
        "timeout": 600,
        "autoApprove": ["relay_interactive_feedback"]
    }))
}

pub fn mcp_config_json_pretty() -> Result<String> {
    let root = json!({
        "mcpServers": {
            "relay-mcp": relay_mcp_entry()?
        }
    });
    Ok(serde_json::to_string_pretty(&root)?)
}

fn has_relay_in_file(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    v.get("mcpServers")
        .and_then(|s| s.get("relay-mcp"))
        .is_some()
}

fn install_relay_mcp_at(path: &Path, invalid_json_hint: &'static str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let entry = relay_mcp_entry()?;
    let mut root: Value = if path.exists() {
        let text = fs::read_to_string(path).context("read mcp config")?;
        serde_json::from_str(&text).context(invalid_json_hint)?
    } else {
        json!({})
    };
    if !root
        .get("mcpServers")
        .map(|v| v.is_object())
        .unwrap_or(false)
    {
        root["mcpServers"] = json!({});
    }
    root["mcpServers"]["relay-mcp"] = entry;
    let out = serde_json::to_string_pretty(&root).context("serialize")?;
    fs::write(path, out).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn uninstall_relay_mcp_at(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let text = fs::read_to_string(path).context("read mcp config")?;
    let mut root: Value = serde_json::from_str(&text).context("parse mcp config")?;
    if let Some(servers) = root.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        servers.remove("relay-mcp");
    }
    fs::write(path, serde_json::to_string_pretty(&root)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

// --- Cursor ~/.cursor/mcp.json ---

pub fn cursor_mcp_json_path() -> Result<PathBuf> {
    Ok(home_dir()?.join(".cursor").join("mcp.json"))
}

pub fn cursor_has_relay_mcp() -> bool {
    cursor_mcp_json_path()
        .ok()
        .map(|p| has_relay_in_file(&p))
        .unwrap_or(false)
}

pub fn install_relay_mcp_cursor() -> Result<()> {
    install_relay_mcp_at(
        &cursor_mcp_json_path()?,
        "Cursor mcp.json is not valid JSON — fix or rename before installing",
    )
}

pub fn uninstall_relay_mcp_cursor() -> Result<()> {
    uninstall_relay_mcp_at(&cursor_mcp_json_path()?)
}

// --- Windsurf ~/.codeium/windsurf/mcp_config.json ---

pub fn windsurf_mcp_json_path() -> Result<PathBuf> {
    Ok(home_dir()?
        .join(".codeium")
        .join("windsurf")
        .join("mcp_config.json"))
}

pub fn windsurf_has_relay_mcp() -> bool {
    windsurf_mcp_json_path()
        .ok()
        .map(|p| has_relay_in_file(&p))
        .unwrap_or(false)
}

pub fn install_relay_mcp_windsurf() -> Result<()> {
    install_relay_mcp_at(
        &windsurf_mcp_json_path()?,
        "Windsurf mcp_config.json is not valid JSON — fix or rename before installing",
    )
}

pub fn uninstall_relay_mcp_windsurf() -> Result<()> {
    uninstall_relay_mcp_at(&windsurf_mcp_json_path()?)
}

/// Permanent PATH (if possible) + Cursor + Windsurf MCP files.
pub fn full_install_integrated() -> Result<serde_json::Value> {
    let (path_action, path_error) = match crate::persist_relay_cli_path() {
        Ok(s) => (s.to_string(), serde_json::Value::Null),
        Err(e) => (
            "skipped".to_string(),
            serde_json::Value::String(e.to_string()),
        ),
    };
    install_relay_mcp_cursor()?;
    install_relay_mcp_windsurf()?;
    Ok(json!({
        "pathAction": path_action,
        "pathError": path_error,
        "mcpInstalled": true
    }))
}

pub fn full_uninstall_integrated() -> Result<()> {
    uninstall_relay_mcp_cursor()?;
    uninstall_relay_mcp_windsurf()?;
    crate::remove_relay_cli_path_persistent()?;
    Ok(())
}
