//! MCP server (`relay mcp`), GUI (`relay` / `relay gui`). Vocabulary: `docs/TERMINOLOGY.md`.

use anyhow::{anyhow, Context, Result};
use chrono::{Local, TimeZone, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tauri::Manager;

pub mod auto_reply;
pub mod config;
pub mod gui_http;
pub mod mcp_http;
pub mod mcp_setup;
pub mod path_persistence;
pub mod server;
pub mod storage;

pub const APP_NAME: &str = "Relay MCP";
pub const APP_QUALIFIER: &str = "com";
pub const APP_ORGANIZATION: &str = "relay";
pub const APP_DATA_DIR: &str = "relay-mcp";
pub const TOOL_NAME: &str = "relay_interactive_feedback";
pub const LOG_FILE: &str = "feedback_log.txt";

pub const GUI_ALIVE_MARKER: &str = "relay_gui_alive.marker";

// Re-export config for backward compatibility (read_ui_locale, write_window_dock, etc.).
pub use auto_reply::{auto_reply_peek, consume_oneshot, load_auto_reply_rules, AutoReplyRule};
pub use config::{
    position_main_window_for_dock, read_mcp_paused, read_ui_locale, read_window_dock,
    write_mcp_paused, write_ui_locale, write_window_dock,
};
pub use path_persistence::{
    gui_binary_path, persist_relay_cli_path, relay_path_config_reason,
    relay_path_persistently_configured, remove_relay_cli_path_persistent,
};
pub use server::{run_feedback_cli, run_feedback_server};
pub use storage::{
    clear_relay_attachments_cache, clear_relay_log_cache, log_write, make_temp_path, new_tab_id,
    prepare_user_data_dir, purge_attachments_older_than_days, read_attachment_retention_days,
    read_control_status, read_feedback_attachment_data_url, read_text_file,
    refresh_gui_presence_marker, relay_cache_stats, remove_gui_presence_marker,
    run_attachment_retention_purge, save_feedback_attachment, write_attachment_retention_days,
    write_control_status, write_text_file, RelayCacheStats, DEFAULT_ATTACHMENT_RETENTION_DAYS,
};

/// Cursor/IDE command item for slash-completion in Relay input; bound to relay_mcp_session_id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandItem {
    pub name: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Append `incoming` items to `existing`, skipping any whose `id` already exists (dedupe by `id`).
pub fn merge_command_items(
    existing: Option<Vec<CommandItem>>,
    incoming: Option<Vec<CommandItem>>,
) -> Option<Vec<CommandItem>> {
    let Some(incoming) = incoming else {
        return existing;
    };
    if incoming.is_empty() {
        return existing;
    }
    let mut out = existing.unwrap_or_default();
    let mut seen: HashSet<String> = out.iter().map(|c| c.id.clone()).collect();
    for c in incoming {
        if seen.insert(c.id.clone()) {
            out.push(c);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlStatus {
    Active,
    /// Submitted Answer; waiting for next MCP `retell` on this tab.
    Idle,
    TimedOut,
    Cancelled,
}

impl ControlStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ControlStatus::Active => "active",
            ControlStatus::Idle => "idle",
            ControlStatus::TimedOut => "timed_out",
            ControlStatus::Cancelled => "cancelled",
        }
    }
}

impl FromStr for ControlStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "active" => Ok(ControlStatus::Active),
            "idle" => Ok(ControlStatus::Idle),
            "timed_out" => Ok(ControlStatus::TimedOut),
            "cancelled" => Ok(ControlStatus::Cancelled),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchState {
    /// MCP `retell`: this turn's user-visible assistant reply (verbatim).
    pub retell: String,
    /// Correlates with HTTP wait channel; empty for hub preview.
    pub request_id: String,
    /// Tab strip label: MM-DD HH:mm from [`format_session_id_as_title`] on `relay_mcp_session_id`.
    pub title: String,
    pub tab_id: String,
    /// Merge key; generated as ms timestamp for new tabs, reused when merging.
    pub relay_mcp_session_id: String,
    pub is_preview: bool,
    /// Cursor commands bound to this session for slash-completion in input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<CommandItem>>,
    /// Skills (same shape as commands) for slash-completion in input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<CommandItem>>,
}

pub const QA_ROUNDS_CAP: usize = 250;

#[derive(Clone, Serialize, Deserialize)]
pub struct QaRound {
    pub retell: String,
    pub reply: String,
    #[serde(default)]
    pub skipped: bool,
    #[serde(default)]
    pub submitted: bool,
    pub tab_id: String,
    #[serde(default)]
    pub relay_mcp_session_id: String,
}

#[derive(Clone, Serialize)]
pub struct FeedbackTabsState {
    pub tabs: Vec<LaunchState>,
    pub active_tab_id: String,
    pub qa_rounds: Vec<QaRound>,
    #[serde(skip_serializing)]
    pub persist_hub: bool,
}

/// Current time as ms timestamp string for new relay_mcp_session_id.
pub fn relay_mcp_session_id_now() -> String {
    Utc::now().timestamp_millis().to_string()
}

/// Format relay_mcp_session_id (ms timestamp) as tab title MM-DD HH:mm in local timezone.
pub fn format_session_id_as_title(session_id: &str) -> String {
    let Ok(ms) = session_id.trim().parse::<i64>() else {
        return "Chat".to_string();
    };
    let secs = ms / 1000;
    let nsecs = ((ms % 1000) * 1_000_000) as u32;
    let Some(dt_utc) = Utc.timestamp_opt(secs, nsecs).single() else {
        return "Chat".to_string();
    };
    dt_utc
        .with_timezone(&Local)
        .format("%m-%d %H:%M")
        .to_string()
}

pub fn trim_qa_rounds(g: &mut FeedbackTabsState) {
    while g.qa_rounds.len() > QA_ROUNDS_CAP {
        g.qa_rounds.remove(0);
    }
}

pub fn push_qa_round(
    g: &mut FeedbackTabsState,
    retell: &str,
    tab_id: &str,
    relay_mcp_session_id: &str,
) {
    let s = retell.trim();
    if s.is_empty() {
        return;
    }
    g.qa_rounds.push(QaRound {
        retell: s.to_string(),
        reply: String::new(),
        skipped: false,
        submitted: false,
        tab_id: tab_id.to_string(),
        relay_mcp_session_id: relay_mcp_session_id.to_string(),
    });
    trim_qa_rounds(g);
}

pub fn skip_open_round_for_tab(g: &mut FeedbackTabsState, tab_id: &str) {
    for r in g.qa_rounds.iter_mut().rev() {
        if r.tab_id == tab_id && !r.submitted {
            r.skipped = true;
            r.submitted = true;
            return;
        }
    }
}

pub fn apply_reply_for_tab(g: &mut FeedbackTabsState, tab_id: &str, reply: &str, skipped: bool) {
    for r in g.qa_rounds.iter_mut().rev() {
        if r.tab_id == tab_id && !r.submitted {
            r.submitted = true;
            if skipped {
                r.skipped = true;
            } else {
                r.reply = reply.to_string();
            }
            return;
        }
    }
}

pub fn finish_tab_remove_empty_close(
    g: &mut FeedbackTabsState,
    tab_id: &str,
    app: &tauri::AppHandle,
) {
    let was_preview = g
        .tabs
        .iter()
        .find(|t| t.tab_id == tab_id)
        .map(|t| t.is_preview)
        .unwrap_or(false);
    g.tabs.retain(|t| t.tab_id != tab_id);
    if g.tabs.is_empty() {
        if g.persist_hub && !was_preview {
            if let Ok(preview) = dev_preview_launch_state() {
                let tid = preview.tab_id.clone();
                push_qa_round(g, preview.retell.trim(), &tid, "");
                g.tabs.push(preview);
                g.active_tab_id = tid;
                trim_qa_rounds(g);
                let _ = refresh_gui_presence_marker();
                return;
            }
        }
        remove_gui_presence_marker();
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.close();
        }
        return;
    }
    if g.active_tab_id == tab_id {
        g.active_tab_id = g.tabs[0].tab_id.clone();
    }
}

fn current_exe_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("failed to resolve current executable")?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("executable directory not found"))
}

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_DATA_DIR)
        .ok_or_else(|| anyhow!("failed to resolve user data directory"))
}

pub fn user_data_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().to_path_buf())
}

/// Returned to the IDE when MCP is user-paused (Settings).
pub const MCP_PAUSED_TOOL_REPLY: &str = "<<<RELAY_MCP_PAUSED>>>\nRelay MCP is paused in the Relay app (Settings). Do not call relay_interactive_feedback again unless the user has resumed. Tell the user to open Relay → Settings and turn off “Pause MCP”.\n（用户在 Relay 设置中已暂停 MCP；请勿再次调用本工具，请用户先在设置中恢复。）";

/// Single `relay` binary: `relay mcp`, `relay feedback` (terminal), `relay` / `relay gui` (hub + HTTP IPC).
pub fn gui_binary_name() -> &'static str {
    if cfg!(windows) {
        "relay.exe"
    } else {
        "relay"
    }
}

/// Directory containing the `relay` executable.
pub fn relay_cli_directory() -> Result<PathBuf> {
    current_exe_dir()
}

pub fn launch_state_preview() -> LaunchState {
    LaunchState {
        retell: "Waiting for your IDE. When the AI sends a message, it will appear here and you can reply in the box below."
            .to_string(),
        request_id: String::new(),
        title: "Chat".to_string(),
        tab_id: new_tab_id(),
        relay_mcp_session_id: String::new(),
        is_preview: true,
        commands: None,
        skills: None,
    }
}

/// Hub / `tauri dev` — placeholder tab until MCP delivers real requests.
pub fn dev_preview_launch_state() -> Result<LaunchState> {
    Ok(launch_state_preview())
}

#[cfg(test)]
mod merge_command_items_tests {
    use super::{merge_command_items, CommandItem};

    fn item(id: &str, name: &str) -> CommandItem {
        CommandItem {
            name: name.to_string(),
            id: id.to_string(),
            category: None,
            description: None,
        }
    }

    #[test]
    fn dedupe_by_id_appends_only_new() {
        let a = merge_command_items(
            Some(vec![item("1", "A"), item("2", "B")]),
            Some(vec![item("2", "Dup"), item("3", "C")]),
        )
        .unwrap();
        assert_eq!(a.len(), 3);
        assert_eq!(a[0].id, "1");
        assert_eq!(a[1].id, "2");
        assert_eq!(a[2].id, "3");
        assert_eq!(a[1].name, "B");
    }

    #[test]
    fn none_incoming_leaves_existing() {
        let e = Some(vec![item("x", "X")]);
        let got = merge_command_items(e.clone(), None).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "x");
    }

    #[test]
    fn empty_incoming_leaves_existing() {
        let e = Some(vec![item("x", "X")]);
        let got = merge_command_items(e, Some(vec![])).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "x");
    }
}
