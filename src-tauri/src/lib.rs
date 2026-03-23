//! MCP server (`relay mcp`), GUI (`relay` / `relay gui`). Vocabulary: `docs/TERMINOLOGY.md`.

use anyhow::{anyhow, Context, Result};
use chrono::{Local, TimeZone, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tauri::Manager;

pub mod auto_reply;
pub mod config;
pub mod dock_edge_hide;
pub mod gui_http;
pub mod mcp_http;
pub mod mcp_setup;
mod mcp_wsl_paths;
pub mod path_persistence;
pub mod release_check;
pub mod server;
pub mod storage;

pub const APP_NAME: &str = "Relay MCP";
pub const APP_QUALIFIER: &str = "com";
pub const APP_ORGANIZATION: &str = "relay";
pub const APP_DATA_DIR: &str = "relay-mcp";
pub const TOOL_NAME: &str = "relay_interactive_feedback";
pub const LOG_FILE: &str = "feedback_log.txt";

pub const GUI_ALIVE_MARKER: &str = "relay_gui_alive.marker";

/// When true, MCP tool results rewrite `attachments[].path` to WSL `/mnt/...` form (`relay mcp --exe_in_wsl`).
pub fn set_mcp_wsl_path_rewrite_enabled(enabled: bool) {
    mcp_wsl_paths::set_mcp_wsl_path_rewrite_enabled(enabled);
}

// Re-export config for backward compatibility (read_ui_locale, write_window_dock, etc.).
pub use auto_reply::{auto_reply_peek, consume_oneshot, load_auto_reply_rules, AutoReplyRule};
pub use config::{
    collapse_window_for_edge_hide, desktop_cursor_outside_outer_window,
    mouse_in_dock_edge_peek_zone, mouse_in_dock_edge_peek_zone_window_only,
    position_main_window_for_dock, read_dock_edge_hide, read_mcp_paused, read_ui_locale,
    read_window_dock, window_nearest_horizontal_screen_edge_side,
    window_outer_straddles_screen_edge, write_dock_edge_hide, write_mcp_paused, write_ui_locale,
    write_window_dock,
};
pub use path_persistence::{
    gui_binary_path, persist_relay_cli_path, relay_path_config_reason,
    relay_path_persistently_configured, remove_relay_cli_path_persistent,
};
pub use server::{run_feedback_cli, run_feedback_server};
pub use storage::{
    clear_relay_attachments_cache, clear_relay_log_cache, feedback_log_pairs_for_session,
    log_write, make_temp_path, new_tab_id, normalize_logged_user_reply, parse_feedback_log_mcp,
    prepare_user_data_dir, purge_attachment_retention_bundled, purge_attachments_older_than_days,
    read_attachment_retention_days, read_control_status, read_text_file,
    refresh_gui_presence_marker, relay_cache_stats, remove_gui_presence_marker,
    run_attachment_retention_purge, save_feedback_attachment, write_attachment_retention_days,
    write_control_status, write_text_file, McpFeedbackLogParse, RelayCacheStats,
    DEFAULT_ATTACHMENT_RETENTION_DAYS,
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

/// `relay_mcp_session_id` from tool/HTTP JSON: accept string or number (some hosts send ms timestamp unquoted).
pub fn session_id_from_tool_arg(v: Option<&JsonValue>) -> String {
    match v {
        None | Some(JsonValue::Null) => String::new(),
        Some(JsonValue::String(s)) => s.trim().to_string(),
        Some(JsonValue::Number(n)) => n.to_string(),
        _ => String::new(),
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
    /// Tab strip label: MM-DD HH:mm:ss from [`format_session_id_as_title`] on `relay_mcp_session_id`.
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

/// Image/file paths returned to the MCP host alongside plain `human` (no `<<<RELAY_FEEDBACK_JSON>>>`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct QaAttachmentRef {
    pub kind: String,
    pub path: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct QaRound {
    pub retell: String,
    /// Plain user text only (attachments live in [`Self::reply_attachments`]).
    pub reply: String,
    #[serde(default)]
    pub skipped: bool,
    #[serde(default)]
    pub submitted: bool,
    pub tab_id: String,
    #[serde(default)]
    pub relay_mcp_session_id: String,
    #[serde(default)]
    pub reply_attachments: Vec<QaAttachmentRef>,
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

/// Format relay_mcp_session_id (ms timestamp) as tab title MM-DD HH:mm:ss in local timezone.
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
        .format("%m-%d %H:%M:%S")
        .to_string()
}

pub fn trim_qa_rounds(g: &mut FeedbackTabsState) {
    while g.qa_rounds.len() > QA_ROUNDS_CAP {
        g.qa_rounds.remove(0);
    }
}

/// Hub had only a preview tab and it was stripped; before attaching a new MCP round, reset stale
/// `qa_rounds` without discarding history for an IDE session the user is reconnecting.
pub fn reconcile_qa_rounds_when_tabs_empty_after_preview_strip(
    g: &mut FeedbackTabsState,
    relay_mcp_session_id: &str,
) {
    if !g.tabs.is_empty() {
        return;
    }
    if relay_mcp_session_id.trim().is_empty() {
        g.qa_rounds.clear();
    } else {
        let sid = relay_mcp_session_id.trim();
        g.qa_rounds.retain(|r| r.relay_mcp_session_id.trim() == sid);
    }
}

/// Merge `qa_rounds` for each tab when the **persisted** source has more completed MCP rounds
/// than in-memory submitted count. Source = `feedback_log.txt` FIFO pairs **or** `qa_archive`
/// lines, whichever has **more rows** for that `relay_mcp_session_id` (see `storage.rs`).
///
/// Open rounds from memory are re-appended only if the same `retell` does not already appear in
/// the chosen source (avoids duplicating after the file caught up). Same `retell` twice in
/// distinct rounds can drop the later in-flight round (rare).
/// Returns `true` if any session's `qa_rounds` were updated (for UI refresh).
pub fn hydrate_qa_rounds_from_feedback_log(g: &mut FeedbackTabsState) -> Result<bool> {
    let tabs: Vec<LaunchState> = g
        .tabs
        .iter()
        .filter(|t| !t.is_preview && !t.relay_mcp_session_id.trim().is_empty())
        .cloned()
        .collect();
    if tabs.is_empty() {
        return Ok(false);
    }

    let dir = prepare_user_data_dir()?;
    let parse = crate::storage::parse_feedback_log_mcp(&dir)?;

    let mut any_changed = false;
    for tab in tabs {
        let sid = tab.relay_mcp_session_id.trim();
        let from_log_pairs = crate::storage::feedback_log_pairs_for_session(&parse, sid);
        let from_arch = crate::storage::qa_archive_load_session(&dir, sid);
        let source: Vec<(String, String, bool, Vec<QaAttachmentRef>)> =
            if from_arch.len() > from_log_pairs.len() {
                from_arch
            } else {
                from_log_pairs
                    .into_iter()
                    .map(|(r, p)| (r, p, false, vec![]))
                    .collect()
            };

        let mem: Vec<QaRound> = g
            .qa_rounds
            .iter()
            .filter(|r| r.relay_mcp_session_id.trim() == sid)
            .cloned()
            .collect();

        let completed_mem = mem.iter().filter(|r| r.submitted).count();
        if source.is_empty() && mem.is_empty() {
            continue;
        }
        // Re-merge when the persisted source has more completed rounds than submitted in memory.
        if source.len() <= completed_mem && !mem.is_empty() {
            continue;
        }

        any_changed = true;
        g.qa_rounds.retain(|r| r.relay_mcp_session_id.trim() != sid);

        for (retell, reply, skipped_flag, att) in &source {
            g.qa_rounds.push(QaRound {
                retell: retell.clone(),
                reply: reply.clone(),
                skipped: *skipped_flag,
                submitted: true,
                tab_id: tab.tab_id.clone(),
                relay_mcp_session_id: sid.to_string(),
                reply_attachments: att.clone(),
            });
        }

        for r in mem.iter() {
            if !r.submitted {
                let already_in_source = source.iter().any(|(t, _, _, _)| t == &r.retell);
                if already_in_source {
                    continue;
                }
                g.qa_rounds.push(QaRound {
                    tab_id: tab.tab_id.clone(),
                    ..r.clone()
                });
            }
        }
    }
    trim_qa_rounds(g);
    Ok(any_changed)
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
        reply_attachments: vec![],
    });
    trim_qa_rounds(g);
}

pub fn skip_open_round_for_tab(g: &mut FeedbackTabsState, tab_id: &str) {
    for r in g.qa_rounds.iter_mut().rev() {
        if r.tab_id == tab_id && !r.submitted {
            r.skipped = true;
            r.submitted = true;
            let sid = r.relay_mcp_session_id.trim();
            if !sid.is_empty() {
                if let Ok(dir) = prepare_user_data_dir() {
                    if let Err(e) =
                        crate::storage::qa_archive_append(&dir, sid, &r.retell, "", true, &[])
                    {
                        #[cfg(debug_assertions)]
                        eprintln!("relay: qa_archive_append: {e:#}");
                        #[cfg(not(debug_assertions))]
                        let _ = e;
                    }
                }
            }
            return;
        }
    }
}

pub fn apply_reply_for_tab(
    g: &mut FeedbackTabsState,
    tab_id: &str,
    reply: &str,
    attachments: &[QaAttachmentRef],
    skipped: bool,
) {
    for r in g.qa_rounds.iter_mut().rev() {
        if r.tab_id == tab_id && !r.submitted {
            r.submitted = true;
            if skipped {
                r.skipped = true;
                r.reply.clear();
                r.reply_attachments.clear();
            } else {
                r.reply = reply.to_string();
                r.reply_attachments = attachments.to_vec();
            }
            let sid = r.relay_mcp_session_id.trim();
            if !sid.is_empty() {
                if let Ok(dir) = prepare_user_data_dir() {
                    let rep = if skipped { "" } else { reply };
                    let att = if skipped { &[][..] } else { attachments };
                    if let Err(e) =
                        crate::storage::qa_archive_append(&dir, sid, &r.retell, rep, skipped, att)
                    {
                        #[cfg(debug_assertions)]
                        eprintln!("relay: qa_archive_append: {e:#}");
                        #[cfg(not(debug_assertions))]
                        let _ = e;
                    }
                }
            }
            return;
        }
    }
}

/// Total slash-completion entries (commands + skills) stored on this tab.
pub fn cmd_skill_count(tab: &LaunchState) -> usize {
    tab.commands.as_ref().map(|v| v.len()).unwrap_or(0)
        + tab.skills.as_ref().map(|v| v.len()).unwrap_or(0)
}

/// JSON string for MCP / HTTP wait completion.
/// When `attachments` is non-empty, an `attachments` array is included (no `<<<RELAY_FEEDBACK_JSON>>>`).
/// WSL path rewrite for `attachments` runs in `relay mcp` via [`crate::mcp_http::feedback_round`].
pub fn feedback_tool_result_string(
    tab: &LaunchState,
    human: &str,
    attachments: &[QaAttachmentRef],
) -> String {
    if attachments.is_empty() {
        return serde_json::json!({
            "relay_mcp_session_id": tab.relay_mcp_session_id,
            "human": human,
            "cmd_skill_count": cmd_skill_count(tab),
        })
        .to_string();
    }
    serde_json::json!({
        "relay_mcp_session_id": tab.relay_mcp_session_id,
        "human": human,
        "cmd_skill_count": cmd_skill_count(tab),
        "attachments": attachments,
    })
    .to_string()
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

#[cfg(test)]
mod session_id_from_arg_tests {
    use super::session_id_from_tool_arg;
    use serde_json::json;

    #[test]
    fn string_trimmed() {
        assert_eq!(session_id_from_tool_arg(Some(&json!(" 177 \n"))), "177");
    }

    #[test]
    fn number_to_string() {
        assert_eq!(
            session_id_from_tool_arg(Some(&json!(1773986572770_i64))),
            "1773986572770"
        );
    }

    #[test]
    fn null_and_missing_empty() {
        assert_eq!(session_id_from_tool_arg(None), "");
        assert_eq!(session_id_from_tool_arg(Some(&json!(null))), "");
    }
}

#[cfg(test)]
mod reconcile_qa_rounds_tests {
    use super::{
        reconcile_qa_rounds_when_tabs_empty_after_preview_strip, FeedbackTabsState, LaunchState,
        QaRound,
    };

    fn state_with_rounds(sid: &str) -> FeedbackTabsState {
        FeedbackTabsState {
            tabs: vec![],
            active_tab_id: "".into(),
            qa_rounds: vec![QaRound {
                retell: "a".into(),
                reply: "b".into(),
                skipped: false,
                submitted: true,
                tab_id: "old".into(),
                relay_mcp_session_id: sid.into(),
                reply_attachments: vec![],
            }],
            persist_hub: false,
        }
    }

    #[test]
    fn empty_tabs_new_session_clears_rounds() {
        let mut g = state_with_rounds("1");
        reconcile_qa_rounds_when_tabs_empty_after_preview_strip(&mut g, "");
        assert!(g.qa_rounds.is_empty());
    }

    #[test]
    fn empty_tabs_reconnect_keeps_matching_session_rounds() {
        let mut g = state_with_rounds("42");
        reconcile_qa_rounds_when_tabs_empty_after_preview_strip(&mut g, "42");
        assert_eq!(g.qa_rounds.len(), 1);
        assert_eq!(g.qa_rounds[0].reply.as_str(), "b");
    }

    #[test]
    fn empty_tabs_reconnect_drops_other_sessions() {
        let mut g = state_with_rounds("1");
        reconcile_qa_rounds_when_tabs_empty_after_preview_strip(&mut g, "2");
        assert!(g.qa_rounds.is_empty());
    }

    #[test]
    fn non_empty_tabs_noop() {
        let mut g = FeedbackTabsState {
            tabs: vec![LaunchState {
                retell: "".into(),
                request_id: "".into(),
                title: "".into(),
                tab_id: "t".into(),
                relay_mcp_session_id: "".into(),
                is_preview: false,
                commands: None,
                skills: None,
            }],
            active_tab_id: "t".into(),
            qa_rounds: vec![QaRound {
                retell: "x".into(),
                reply: "y".into(),
                skipped: false,
                submitted: false,
                tab_id: "t".into(),
                relay_mcp_session_id: "".into(),
                reply_attachments: vec![],
            }],
            persist_hub: false,
        };
        reconcile_qa_rounds_when_tabs_empty_after_preview_strip(&mut g, "");
        assert_eq!(g.qa_rounds.len(), 1);
    }
}

#[cfg(test)]
mod feedback_tool_result_tests {
    use super::{
        cmd_skill_count, feedback_tool_result_string, CommandItem, LaunchState, QaAttachmentRef,
    };
    use serde_json::Value;

    fn sample_tab() -> LaunchState {
        LaunchState {
            retell: "r".into(),
            request_id: "req".into(),
            title: "t".into(),
            tab_id: "tid".into(),
            relay_mcp_session_id: "1700000000000".into(),
            is_preview: false,
            commands: Some(vec![CommandItem {
                name: "a".into(),
                id: "c1".into(),
                category: None,
                description: None,
            }]),
            skills: Some(vec![CommandItem {
                name: "s".into(),
                id: "s1".into(),
                category: None,
                description: None,
            }]),
        }
    }

    #[test]
    fn cmd_skill_count_sums_lists() {
        let t = sample_tab();
        assert_eq!(cmd_skill_count(&t), 2);
    }

    #[test]
    fn feedback_tool_result_json_shape() {
        let t = sample_tab();
        let s = feedback_tool_result_string(&t, "hello", &[]);
        let v: Value = serde_json::from_str(&s).expect("json");
        assert_eq!(v["relay_mcp_session_id"], "1700000000000");
        assert_eq!(v["human"], "hello");
        assert_eq!(v["cmd_skill_count"], 2);
        assert!(v.get("attachments").is_none());
    }

    #[test]
    fn feedback_tool_includes_attachments_array() {
        let t = sample_tab();
        let att = vec![QaAttachmentRef {
            kind: "image".into(),
            path: "/tmp/x.png".into(),
        }];
        let s = feedback_tool_result_string(&t, "hi", &att);
        let v: Value = serde_json::from_str(&s).expect("json");
        assert_eq!(v["human"], "hi");
        let a = v["attachments"].as_array().expect("arr");
        assert_eq!(a.len(), 1);
        assert_eq!(a[0]["kind"], "image");
    }
}
