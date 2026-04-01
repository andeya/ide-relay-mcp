//! IDE mode abstraction: Cursor, Claude Code, Windsurf, Other.
//! Each IDE variant knows its MCP config path, rule file path, and whether
//! it supports usage monitoring. The active IDE is process-global (no file
//! persistence) — determined by CLI subcommand or user selection at startup.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use crate::mcp_setup;
use crate::prepare_user_data_dir;

// ---------------------------------------------------------------------------
// Process-global IDE mode (set by CLI subcommand, switchable at runtime)
// ---------------------------------------------------------------------------

static PROCESS_IDE: RwLock<Option<IdeKind>> = RwLock::new(None);

pub fn set_process_ide(ide: IdeKind) {
    if let Ok(mut w) = PROCESS_IDE.write() {
        *w = Some(ide);
    }
}

pub fn get_process_ide() -> Option<IdeKind> {
    PROCESS_IDE.read().ok().and_then(|r| *r)
}

/// Window title based on current process IDE: `Relay-Cursor`, `Relay-Claude Code`, etc.
pub fn window_title() -> String {
    match get_process_ide() {
        Some(ide) => format!("Relay-{}", ide.label()),
        None => "Relay".to_string(),
    }
}

const APP_VERSION_FILE: &str = "app_version.json";

// ---------------------------------------------------------------------------
// IdeKind enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdeKind {
    Cursor,
    ClaudeCode,
    Windsurf,
    Other,
}

impl IdeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cursor => "Cursor",
            Self::ClaudeCode => "Claude Code",
            Self::Windsurf => "Windsurf",
            Self::Other => "Other",
        }
    }

    /// Lowercase identifier with no spaces, used in CLI subcommands and file names.
    pub fn cli_id(self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::ClaudeCode => "claudecode",
            Self::Windsurf => "windsurf",
            Self::Other => "other",
        }
    }
}

// ---------------------------------------------------------------------------
// IDE capability queries
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeCapabilities {
    pub supports_mcp_inject: bool,
    pub supports_rule_prompt: bool,
    pub supports_usage: bool,
}

pub fn capabilities(ide: IdeKind) -> IdeCapabilities {
    match ide {
        IdeKind::Cursor => IdeCapabilities {
            supports_mcp_inject: true,
            supports_rule_prompt: true,
            supports_usage: true,
        },
        IdeKind::ClaudeCode => IdeCapabilities {
            supports_mcp_inject: true,
            supports_rule_prompt: true,
            supports_usage: false,
        },
        IdeKind::Windsurf => IdeCapabilities {
            supports_mcp_inject: true,
            supports_rule_prompt: false,
            supports_usage: false,
        },
        IdeKind::Other => IdeCapabilities {
            supports_mcp_inject: false,
            supports_rule_prompt: false,
            supports_usage: false,
        },
    }
}

// ---------------------------------------------------------------------------
// Version tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct AppVersionConfig {
    version: String,
}

fn version_path() -> Result<PathBuf> {
    Ok(prepare_user_data_dir()?.join(APP_VERSION_FILE))
}

pub fn read_stored_version() -> Option<String> {
    let path = version_path().ok()?;
    if !path.exists() {
        return None;
    }
    let text = fs::read_to_string(&path).ok()?;
    let cfg: AppVersionConfig = serde_json::from_str(&text).ok()?;
    Some(cfg.version)
}

pub fn write_stored_version(version: &str) -> Result<()> {
    let path = version_path()?;
    let json = serde_json::to_string_pretty(&AppVersionConfig {
        version: version.to_string(),
    })
    .context("serialize app version")?;
    fs::write(&path, json).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn current_binary_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Returns true if the binary version differs from the stored version
/// (i.e. the app was just upgraded/downgraded).
pub fn version_changed() -> bool {
    match read_stored_version() {
        Some(stored) => stored != current_binary_version(),
        None => true,
    }
}

// ---------------------------------------------------------------------------
// MCP inject — delegated to mcp_setup per IDE
// ---------------------------------------------------------------------------

pub fn mcp_json_path(ide: IdeKind) -> Result<PathBuf> {
    match ide {
        IdeKind::Cursor => mcp_setup::cursor_mcp_json_path(),
        IdeKind::ClaudeCode => mcp_setup::claude_code_mcp_json_path(),
        IdeKind::Windsurf => mcp_setup::windsurf_mcp_json_path(),
        IdeKind::Other => anyhow::bail!("MCP injection not supported for Other IDE"),
    }
}

pub fn has_relay_mcp(ide: IdeKind) -> bool {
    match ide {
        IdeKind::Cursor => mcp_setup::cursor_has_relay_mcp(),
        IdeKind::ClaudeCode => mcp_setup::claude_code_has_relay_mcp(),
        IdeKind::Windsurf => mcp_setup::windsurf_has_relay_mcp(),
        IdeKind::Other => false,
    }
}

pub fn install_relay_mcp(ide: IdeKind) -> Result<()> {
    match ide {
        IdeKind::Cursor => mcp_setup::install_relay_mcp_cursor(),
        IdeKind::ClaudeCode => mcp_setup::install_relay_mcp_claude_code(),
        IdeKind::Windsurf => mcp_setup::install_relay_mcp_windsurf(),
        IdeKind::Other => anyhow::bail!("MCP injection not supported for Other IDE"),
    }
}

pub fn uninstall_relay_mcp(ide: IdeKind) -> Result<()> {
    match ide {
        IdeKind::Cursor => mcp_setup::uninstall_relay_mcp_cursor(),
        IdeKind::ClaudeCode => mcp_setup::uninstall_relay_mcp_claude_code(),
        IdeKind::Windsurf => mcp_setup::uninstall_relay_mcp_windsurf(),
        IdeKind::Other => anyhow::bail!("MCP injection not supported for Other IDE"),
    }
}

// ---------------------------------------------------------------------------
// Rule prompt inject — per IDE
// ---------------------------------------------------------------------------

pub fn rule_file_path(ide: IdeKind) -> Result<PathBuf> {
    match ide {
        IdeKind::Cursor => mcp_setup::cursor_rule_file_path(),
        IdeKind::ClaudeCode => mcp_setup::claude_code_rule_file_path(),
        _ => anyhow::bail!("Rule prompts not supported for {}", ide.label()),
    }
}

pub fn rule_installed(ide: IdeKind) -> bool {
    match ide {
        IdeKind::Cursor => mcp_setup::cursor_rule_installed(),
        IdeKind::ClaudeCode => mcp_setup::claude_code_rule_installed(),
        _ => false,
    }
}

pub fn install_rule(ide: IdeKind, content: &str) -> Result<()> {
    match ide {
        IdeKind::Cursor => mcp_setup::install_cursor_rule(content),
        IdeKind::ClaudeCode => mcp_setup::install_claude_code_rule(content),
        _ => anyhow::bail!("Rule prompts not supported for {}", ide.label()),
    }
}

pub fn uninstall_rule(ide: IdeKind) -> Result<()> {
    match ide {
        IdeKind::Cursor => mcp_setup::uninstall_cursor_rule(),
        IdeKind::ClaudeCode => mcp_setup::uninstall_claude_code_rule(),
        _ => anyhow::bail!("Rule prompts not supported for {}", ide.label()),
    }
}

// ---------------------------------------------------------------------------
// Startup version check + auto-update rules
// ---------------------------------------------------------------------------

/// Called from the frontend after the IDE mode is known.
/// If the binary version differs from the stored version, writes the new
/// version and auto-updates rule prompts using the content supplied by
/// the TypeScript template (single source of truth).
pub fn check_and_upgrade_version(rule_content: &str) {
    if !version_changed() {
        return;
    }

    let Some(ide) = get_process_ide() else {
        let _ = write_stored_version(current_binary_version());
        return;
    };
    let caps = capabilities(ide);
    if !caps.supports_rule_prompt || !rule_installed(ide) {
        let _ = write_stored_version(current_binary_version());
        return;
    }
    if install_rule(ide, rule_content).is_ok() {
        let _ = write_stored_version(current_binary_version());
    }
}
