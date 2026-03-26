//! MCP client configuration for **Cursor** (`~/.cursor/mcp.json`) and **Windsurf**
//! (`~/.codeium/windsurf/mcp_config.json`): merge `relay-mcp` (`command` = `relay`, `args` = `["mcp"]` or `["mcp", "--exe_in_wsl"]` for WSL, …).
//! See `docs/TERMINOLOGY.md`. Tool `relay_interactive_feedback` requires non-empty `retell` (this turn's assistant reply).
//! Full install may also update user `PATH`.

use crate::{gui_binary_name, relay_cli_directory};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

fn config_path_error_hint() -> String {
    if cfg!(windows) {
        "Cannot get config path (check USERPROFILE). Use one-click install."
    } else {
        "Cannot get config path (check HOME). Use one-click install."
    }
    .to_string()
}

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

/// Absolute path to `relay` and argv for MCP stdio server.
/// Default args: `["mcp"]`; WSL users add `"--exe_in_wsl"` to get `/mnt/...` paths.
pub fn relay_mcp_command_and_args() -> Result<(String, Vec<String>)> {
    let dir = relay_cli_directory()?;
    let exe = dir.join(gui_binary_name());
    if !exe.exists() {
        anyhow::bail!("relay not found at {}", exe.display());
    }
    Ok((exe.to_string_lossy().into_owned(), vec!["mcp".to_string()]))
}

/// `command`, `args`, and `autoApprove` for `relay-mcp` (Cursor / Windsurf one-click install).
/// Default `args`: `["mcp"]`. For WSL-hosted agents with Windows `relay.exe`, use `["mcp", "--exe_in_wsl"]`.
fn relay_mcp_entry() -> Result<Value> {
    let (command, args) = relay_mcp_command_and_args()?;
    Ok(json!({
        "command": command,
        "args": args,
        "autoApprove": ["relay_interactive_feedback"]
    }))
}

/// Merge `mcpServers.relay-mcp`: keep values from the user file, fill only missing keys from [`relay_mcp_entry`].
fn merge_relay_mcp_entry(user: Option<&Value>) -> Result<Value> {
    let defaults = relay_mcp_entry()?;
    let def_obj = defaults
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("relay defaults not object"))?;
    let mut map: serde_json::Map<String, Value> = user
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    for (k, v) in def_obj {
        map.entry(k.clone()).or_insert_with(|| v.clone());
    }
    Ok(Value::Object(map))
}

/// Pretty JSON for Settings → Copy MCP: reads `mcpServers.relay-mcp` from real `~/.cursor/mcp.json` when present,
/// merges missing keys from [`relay_mcp_entry`], and returns **only** `{ "mcpServers": { "relay-mcp": … } }` (no other servers).
pub fn mcp_config_json_pretty() -> Result<String> {
    let path = cursor_mcp_json_path()?;
    let user_relay = if path.exists() {
        let text = fs::read_to_string(&path).unwrap_or_default();
        let root: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({}));
        root.get("mcpServers")
            .and_then(|s| s.get("relay-mcp"))
            .cloned()
    } else {
        None
    };
    let merged = merge_relay_mcp_entry(user_relay.as_ref())?;
    let root = json!({
        "mcpServers": {
            "relay-mcp": merged
        }
    });
    Ok(serde_json::to_string_pretty(&root)?)
}

/// True if config has relay-mcp entry, its `command` path exists, and equals current relay binary path.
fn relay_mcp_configured_and_command_matches(path: &Path) -> bool {
    relay_mcp_reason(path).is_none()
}

/// When relay-mcp is not correctly configured at path, returns reason for the user to fix manually.
fn relay_mcp_reason(path: &Path) -> Option<String> {
    let expected_path = relay_mcp_command_and_args()
        .ok()
        .map(|(s, _)| PathBuf::from(s))?;
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => {
            return Some("Config file missing or unreadable. Use one-click install.".to_string())
        }
    };
    let v: Value = match serde_json::from_str(&text) {
        Ok(x) => x,
        Err(_) => {
            return Some("Config is not valid JSON. Fix or re-run one-click install.".to_string())
        }
    };
    let servers = match v.get("mcpServers").and_then(|s| s.as_object()) {
        Some(s) => s,
        None => return Some("No mcpServers in config. Use one-click install.".to_string()),
    };
    let relay = match servers.get("relay-mcp") {
        Some(r) => r,
        None => return Some("relay-mcp not in config. Use one-click install.".to_string()),
    };
    let command = match relay.get("command").and_then(|c| c.as_str()) {
        Some(c) => c,
        None => return Some("relay-mcp has no command. Re-run one-click install.".to_string()),
    };
    let config_path = PathBuf::from(command);
    if !config_path.exists() {
        return Some(format!(
            "relay-mcp command path does not exist: {} (fix or re-run one-click install).",
            config_path.display()
        ));
    }
    let expected_canon = expected_path.canonicalize().ok()?;
    let config_canon = config_path.canonicalize().ok()?;
    if expected_canon != config_canon {
        return Some(format!(
            "relay-mcp command path does not match current relay (config: {}, current: {}). Re-run one-click install or edit config.",
            config_canon.display(),
            expected_canon.display()
        ));
    }
    None
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
        .map(|p| relay_mcp_configured_and_command_matches(&p))
        .unwrap_or(false)
}

/// When Cursor relay-mcp is not configured, returns reason for the user to fix manually.
pub fn cursor_relay_mcp_reason() -> Option<String> {
    let path = match cursor_mcp_json_path() {
        Ok(p) => p,
        Err(_) => return Some(config_path_error_hint()),
    };
    relay_mcp_reason(&path)
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
        .map(|p| relay_mcp_configured_and_command_matches(&p))
        .unwrap_or(false)
}

/// When Windsurf relay-mcp is not configured, returns reason for the user to fix manually.
pub fn windsurf_relay_mcp_reason() -> Option<String> {
    let path = match windsurf_mcp_json_path() {
        Ok(p) => p,
        Err(_) => return Some(config_path_error_hint()),
    };
    relay_mcp_reason(&path)
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

// --- Cursor rule prompt sync: ~/.cursor/rules/ ---

const CURSOR_RULE_FILENAME: &str = "relay-interactive-feedback.mdc";

pub fn cursor_rules_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".cursor").join("rules"))
}

pub fn cursor_rule_file_path() -> Result<PathBuf> {
    Ok(cursor_rules_dir()?.join(CURSOR_RULE_FILENAME))
}

pub fn cursor_rule_installed() -> bool {
    cursor_rule_file_path()
        .ok()
        .map(|p| p.exists())
        .unwrap_or(false)
}

pub fn install_cursor_rule(content: &str) -> Result<()> {
    let dir = cursor_rules_dir()?;
    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    }
    let path = dir.join(CURSOR_RULE_FILENAME);
    fs::write(&path, content).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn uninstall_cursor_rule() -> Result<()> {
    let path = cursor_rule_file_path()?;
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

pub fn read_cursor_rule() -> Result<String> {
    let path = cursor_rule_file_path()?;
    if !path.exists() {
        anyhow::bail!("rule file not found at {}", path.display());
    }
    fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))
}
