//! `relay gui-<ide>` (hub), `relay mcp-<ide>` (stdio MCP), `relay feedback` (terminal).
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use clap::{Parser, Subcommand};
use serde::Serialize;
use tauri::{Manager, State};

use relay_mcp::{
    dock_edge_hide::EdgeHideState, gui_http::RelayGuiRuntime, refresh_gui_presence_marker,
    run_feedback_cli, ControlStatus, FeedbackTabsState, LaunchState, QaRound,
};

/// Release Windows builds use the GUI subsystem; attach to the parent console so CLI subcommands
/// can print MCP JSON-RPC / `relay feedback` output when launched from cmd or PowerShell.
///
/// Skips attaching when stdout is already a pipe so IDE-hosted `relay mcp-<ide>` (stdio JSON-RPC) is
/// never redirected to a stray console.
#[cfg(all(target_os = "windows", not(debug_assertions)))]
fn try_attach_parent_console_for_cli() {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{GetFileType, FILE_TYPE_PIPE};
    use windows_sys::Win32::System::Console::{
        AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE,
    };
    unsafe {
        let h = GetStdHandle(STD_OUTPUT_HANDLE);
        if h != INVALID_HANDLE_VALUE && !h.is_null() && GetFileType(h) == FILE_TYPE_PIPE {
            return;
        }
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        // Ignore failure: no parent console, already attached, etc.
    }
}

#[cfg(not(all(target_os = "windows", not(debug_assertions))))]
fn try_attach_parent_console_for_cli() {}

#[derive(Parser)]
#[command(
    name = "relay",
    version = env!("CARGO_PKG_VERSION"),
    about = "Relay MCP — native human feedback for AI IDEs"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Helper to define per-IDE MCP subcommand flags.
#[derive(clap::Args, Clone)]
struct McpFlags {
    /// Rewrite `attachments[].path` in tool results to `/mnt/<drive>/...` for WSL-hosted agents (Windows `relay.exe` only).
    #[arg(long = "exe_in_wsl")]
    exe_in_wsl: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// MCP JSON-RPC on stdio for Cursor
    #[command(name = "mcp-cursor")]
    McpCursor(McpFlags),
    /// MCP JSON-RPC on stdio for Claude Code
    #[command(name = "mcp-claudecode")]
    McpClaudeCode(McpFlags),
    /// MCP JSON-RPC on stdio for Windsurf
    #[command(name = "mcp-windsurf")]
    McpWindsurf(McpFlags),
    /// MCP JSON-RPC on stdio for Other IDE
    #[command(name = "mcp-other")]
    McpOther(McpFlags),
    /// Open Relay window for Cursor
    #[command(name = "gui-cursor")]
    GuiCursor,
    /// Open Relay window for Claude Code
    #[command(name = "gui-claudecode")]
    GuiClaudeCode,
    /// Open Relay window for Windsurf
    #[command(name = "gui-windsurf")]
    GuiWindsurf,
    /// Open Relay window for Other IDE
    #[command(name = "gui-other")]
    GuiOther,
    /// Terminal: open feedback UI and print Answer to stdout when done
    Feedback {
        #[arg(
            long,
            help = "Assistant reply text for terminal tryout (same semantics as MCP retell)"
        )]
        retell: String,
        #[arg(
            short = 't',
            long,
            default_value_t = 60,
            help = "Minutes to wait for submit"
        )]
        timeout: u64,
        #[arg(
            long = "relay-mcp-session-id",
            help = "Session id (same as MCP relay_mcp_session_id): merge into one Relay tab; omit for new session."
        )]
        relay_mcp_session_id: Option<String>,
    },
}

#[tauri::command]
fn get_feedback_tabs(state: State<'_, RelayGuiRuntime>) -> Result<FeedbackTabsState, String> {
    state.hydrate_qa_from_log();
    Ok(state.tabs_snapshot())
}

#[tauri::command]
fn set_active_tab(tab_id: String, state: State<'_, RelayGuiRuntime>) -> Result<(), String> {
    state.set_active_tab(&tab_id)
}

#[tauri::command]
fn read_tab_status(
    tab_id: String,
    state: State<'_, RelayGuiRuntime>,
) -> Result<Option<ControlStatus>, String> {
    Ok(state.read_tab_status(&tab_id))
}

#[tauri::command]
fn submit_tab_feedback(
    tab_id: String,
    human: String,
    attachments: Vec<relay_mcp::QaAttachmentRef>,
    state: State<'_, RelayGuiRuntime>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    state.submit_tab_feedback(&tab_id, human, attachments, &app)
}

#[tauri::command]
fn dismiss_feedback_tab(
    tab_id: String,
    state: State<'_, RelayGuiRuntime>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    state.dismiss_feedback_tab(&tab_id, &app)
}

#[tauri::command]
fn close_feedback_tab(
    tab_id: String,
    state: State<'_, RelayGuiRuntime>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    state.close_feedback_tab(&tab_id, &app)
}

#[tauri::command]
fn get_ui_locale() -> String {
    relay_mcp::read_ui_locale()
}

#[tauri::command]
fn set_ui_locale(lang: String) -> Result<(), String> {
    relay_mcp::write_ui_locale(lang.trim()).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_window_dock() -> String {
    relay_mcp::read_window_dock()
}

#[tauri::command]
fn get_window_always_on_top() -> bool {
    relay_mcp::read_window_always_on_top()
}

/// Called after pointer left the webview (debounced) — tuck to screen edge when enabled.
#[tauri::command]
fn dock_edge_hide_after_leave(app: tauri::AppHandle) -> Result<(), String> {
    relay_mcp::dock_edge_hide::collapse_after_leave(&app)
}

#[tauri::command]
fn get_dock_edge_hide() -> bool {
    relay_mcp::read_dock_edge_hide()
}

/// Timing for dock-edge tuck UI — single source of truth (see `dock_edge_hide` constants).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DockEdgeHideUiTiming {
    shell_leave_debounce_ms: u64,
    suppress_after_peek_ms: u64,
}

#[tauri::command]
fn get_dock_edge_hide_ui_timing() -> DockEdgeHideUiTiming {
    DockEdgeHideUiTiming {
        shell_leave_debounce_ms: relay_mcp::dock_edge_hide::SHELL_LEAVE_DEBOUNCE_MS,
        suppress_after_peek_ms: relay_mcp::dock_edge_hide::SUPPRESS_COLLAPSE_AFTER_PEEK_MS,
    }
}

#[tauri::command]
fn set_dock_edge_hide(enabled: bool, app: tauri::AppHandle) -> Result<(), String> {
    relay_mcp::write_dock_edge_hide(enabled).map_err(|e| e.to_string())?;
    if !enabled {
        let _ = relay_mcp::dock_edge_hide::expand_if_collapsed(&app);
    }
    Ok(())
}

/// Recover from edge tuck when peek hover / focus did not expand (hotkey from the webview).
#[tauri::command]
fn dock_edge_force_expand(app: tauri::AppHandle) -> Result<bool, String> {
    relay_mcp::dock_edge_hide::expand_if_collapsed(&app)
}

#[tauri::command]
fn set_window_dock(
    dock: String,
    app: tauri::AppHandle,
    edge_state: State<'_, Mutex<EdgeHideState>>,
) -> Result<(), String> {
    let d = dock.trim();
    let Some(w) = app.get_webview_window("main") else {
        return Err("main window missing".to_string());
    };
    // Apply geometry first so we never clear edge state if positioning fails; then persist dock.
    relay_mcp::position_main_window_for_dock(&w, d).map_err(|e| e.to_string())?;
    relay_mcp::write_window_dock(d).map_err(|e| e.to_string())?;
    let _ = w.set_always_on_top(relay_mcp::read_window_always_on_top());
    relay_mcp::dock_edge_hide::set_peek_fast_poll(false);
    if let Ok(mut g) = edge_state.lock() {
        g.collapsed = false;
        g.tuck_side = None;
        g.suppress_collapse_until_ms = 0;
        g.suppress_peek_expand_until_ms = 0;
    }
    Ok(())
}

#[tauri::command]
fn set_window_always_on_top(enabled: bool, app: tauri::AppHandle) -> Result<(), String> {
    relay_mcp::write_window_always_on_top(enabled).map_err(|e| e.to_string())?;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_always_on_top(enabled);
    }
    Ok(())
}

#[tauri::command]
fn get_mcp_paused() -> bool {
    relay_mcp::read_mcp_paused()
}

#[tauri::command]
fn set_mcp_paused(paused: bool) -> Result<(), String> {
    relay_mcp::write_mcp_paused(paused).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
struct RelayPathEnvStatus {
    configured: bool,
    bin_dir: String,
    platform: &'static str,
    /// When not configured, reason for the user to fix manually.
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[tauri::command]
fn get_relay_path_env_status() -> Result<RelayPathEnvStatus, String> {
    let dir = relay_mcp::relay_cli_directory().map_err(|e| e.to_string())?;
    let platform = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "other"
    };
    let configured = relay_mcp::relay_path_persistently_configured();
    Ok(RelayPathEnvStatus {
        configured,
        bin_dir: dir.to_string_lossy().into_owned(),
        platform,
        reason: if configured {
            None
        } else {
            relay_mcp::relay_path_config_reason()
        },
    })
}

#[tauri::command]
fn configure_relay_path_env_permanent() -> Result<String, String> {
    relay_mcp::persist_relay_cli_path()
        .map(|s| s.to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_relay_path_env() -> Result<(), String> {
    relay_mcp::remove_relay_cli_path_persistent().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_mcp_config_json() -> Result<String, String> {
    relay_mcp::mcp_setup::mcp_config_json_pretty().map_err(|e| e.to_string())
}

#[tauri::command]
fn open_relay_data_folder() -> Result<(), String> {
    let p = relay_mcp::prepare_user_data_dir().map_err(|e| e.to_string())?;
    opener::open(&p).map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_github_latest_release() -> relay_mcp::release_check::ReleaseCheckPayload {
    let version = env!("CARGO_PKG_VERSION");
    tokio::task::spawn_blocking(move || relay_mcp::release_check::check_latest_release(version))
        .await
        .unwrap_or_else(|_| relay_mcp::release_check::ReleaseCheckPayload {
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            latest_version: None,
            update_available: false,
            check_error: Some("internal: join error".into()),
        })
}

#[tauri::command]
fn open_relay_github_repo(releases_latest: Option<bool>) -> Result<(), String> {
    let url = if releases_latest == Some(true) {
        relay_mcp::release_check::RELAY_REPO_RELEASES_LATEST
    } else {
        relay_mcp::release_check::RELAY_REPO_HOME
    };
    opener::open(url).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_relay_cache_stats() -> Result<relay_mcp::RelayCacheStats, String> {
    relay_mcp::relay_cache_stats().map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_relay_cache_attachments() -> Result<(), String> {
    relay_mcp::clear_relay_attachments_cache().map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_relay_cache_logs() -> Result<(), String> {
    relay_mcp::clear_relay_log_cache().map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_relay_cache_all() -> Result<(), String> {
    relay_mcp::clear_relay_attachments_cache().map_err(|e| e.to_string())?;
    relay_mcp::clear_relay_log_cache().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_attachment_retention_days() -> Option<u32> {
    relay_mcp::read_attachment_retention_days()
}

#[tauri::command]
fn set_attachment_retention_days(days: Option<u32>) -> Result<u64, String> {
    let d = days.filter(|x| *x > 0 && *x <= 3660);
    relay_mcp::write_attachment_retention_days(d).map_err(|e| e.to_string())?;
    if let Some(n) = d {
        relay_mcp::purge_attachment_retention_bundled(n).map_err(|e| e.to_string())
    } else {
        Ok(0)
    }
}

#[tauri::command]
fn run_attachment_retention_purge() -> Result<u64, String> {
    relay_mcp::run_attachment_retention_purge().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_feedback_attachment(name: String, bytes_b64: String) -> Result<String, String> {
    relay_mcp::save_feedback_attachment(&name, &bytes_b64)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
struct DraggedImagePreview {
    data_base64: String,
    name: String,
    mime: String,
}

/// Read a local image path (drag-drop) so the webview can show a thumbnail instead of pasting paths.
#[tauri::command]
fn read_dragged_image_preview(path: String) -> Result<DraggedImagePreview, String> {
    let p = Path::new(path.trim());
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return Err("unsupported image type".to_string()),
    };
    let meta = fs::metadata(p).map_err(|e| e.to_string())?;
    const MAX_BYTES: u64 = 25 * 1024 * 1024;
    if meta.len() > MAX_BYTES {
        return Err("image too large (max 25MB)".to_string());
    }
    let bytes = fs::read(p).map_err(|e| e.to_string())?;
    let name = p
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image.png".to_string());
    Ok(DraggedImagePreview {
        data_base64: STANDARD.encode(&bytes),
        name,
        mime: mime.to_string(),
    })
}

const MAX_ATTACH_BYTES: u64 = 50 * 1024 * 1024;

/// Path must be a regular file and within size limit (before reading bytes).
#[tauri::command]
fn validate_feedback_attachment_path(path: String) -> Result<(), String> {
    let p = Path::new(path.trim());
    let meta = fs::metadata(p).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a file".to_string());
    }
    if meta.len() > MAX_ATTACH_BYTES {
        return Err("file too large (max 50MB)".to_string());
    }
    Ok(())
}

/// Read arbitrary local file bytes as base64 (max [`MAX_ATTACH_BYTES`]).
///
/// **Trust**: Intended only for paths the user chose in the native file/drag-drop flow. Not a
/// sandbox escape hatch — callers must not forward untrusted remote paths into this command.
#[tauri::command]
fn read_local_file_bytes_b64(path: String) -> Result<String, String> {
    let p = Path::new(path.trim());
    let meta = fs::metadata(p).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a file".to_string());
    }
    if meta.len() > MAX_ATTACH_BYTES {
        return Err("file too large (max 50MB)".to_string());
    }
    let bytes = fs::read(p).map_err(|e| e.to_string())?;
    Ok(STANDARD.encode(&bytes))
}

// ---------------------------------------------------------------------------
// Cursor Usage commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_cursor_usage_settings() -> relay_mcp::cursor_usage::CursorUsageSettings {
    relay_mcp::cursor_usage::read_cursor_usage_settings()
}

#[tauri::command]
fn set_cursor_usage_settings(
    settings: relay_mcp::cursor_usage::CursorUsageSettings,
) -> Result<(), String> {
    relay_mcp::cursor_usage::write_cursor_usage_settings(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_cursor_session_token(token: String) -> Result<(), String> {
    relay_mcp::cursor_usage::write_cursor_session_token(&token).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_cursor_session_token() -> Result<String, String> {
    relay_mcp::cursor_usage::read_cursor_session_token().map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_cursor_session_token() -> Result<(), String> {
    relay_mcp::cursor_usage::clear_cursor_session_token().map_err(|e| e.to_string())
}

#[tauri::command]
async fn fetch_cursor_usage_events(
    start_date: String,
    end_date: String,
    page: u32,
    page_size: u32,
) -> Result<relay_mcp::cursor_usage::CursorUsageEventsPage, String> {
    tokio::task::spawn_blocking(move || {
        let token = relay_mcp::cursor_usage::get_web_session_token()
            .ok_or_else(|| "no cursor session token available".to_string())?;
        relay_mcp::cursor_usage::fetch_usage_events(
            &token,
            None,
            None,
            &start_date,
            &end_date,
            page,
            page_size,
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("task join error: {e}"))?
}

/// Fetch usage via IDE's api2.cursor.sh (auto-reads token from IDE database).
#[tauri::command]
async fn fetch_cursor_usage_via_ide() -> Result<relay_mcp::cursor_usage::CursorUsageSummary, String>
{
    tokio::task::spawn_blocking(|| {
        let token =
            relay_mcp::cursor_usage::auto_detect_cursor_token().map_err(|e| e.to_string())?;
        relay_mcp::cursor_usage::fetch_usage_via_ide_api(&token).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("task join error: {e}"))?
}

// ---------------------------------------------------------------------------
// IDE mode commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_ide_binding() -> Option<relay_mcp::ide::IdeKind> {
    relay_mcp::ide::get_process_ide()
}

/// Set IDE mode for the current process. Called from the selection page or
/// settings switch — also writes the per-IDE endpoint/marker files so MCP
/// processes targeting this IDE can discover the GUI.
/// Cleans up old per-IDE files when switching from one IDE to another.
/// Rejects the switch if the target IDE already has a running GUI process.
#[tauri::command]
fn set_ide_binding(
    ide: relay_mcp::ide::IdeKind,
    state: State<'_, RelayGuiRuntime>,
) -> Result<(), String> {
    let current = relay_mcp::ide::get_process_ide();
    if current != Some(ide) && relay_mcp::mcp_http::is_ide_gui_alive(ide) {
        return Err(format!(
            "Another Relay GUI process is already running in {} mode. Only one process per IDE mode is allowed.",
            ide.label()
        ));
    }

    let old_endpoint = relay_mcp::mcp_http::gui_endpoint_path().ok();
    let old_marker = relay_mcp::user_data_dir()
        .ok()
        .map(|d| d.join(relay_mcp::gui_alive_marker_name()));

    relay_mcp::ide::set_process_ide(ide);
    state.write_endpoint_file().map_err(|e| e.to_string())?;
    let _ = refresh_gui_presence_marker();

    if let Some(old) = old_endpoint {
        let new_endpoint = relay_mcp::mcp_http::gui_endpoint_path().ok();
        if new_endpoint.as_ref() != Some(&old) {
            let _ = std::fs::remove_file(&old);
        }
    }
    if let Some(old) = old_marker {
        let new_marker = relay_mcp::user_data_dir()
            .ok()
            .map(|d| d.join(relay_mcp::gui_alive_marker_name()));
        if new_marker.as_ref() != Some(&old) {
            let _ = std::fs::remove_file(&old);
        }
    }
    Ok(())
}

#[tauri::command]
fn get_window_title() -> String {
    relay_mcp::ide::window_title()
}

#[tauri::command]
fn recheck_version_upgrade(rule_content: String) {
    relay_mcp::ide::check_and_upgrade_version(&rule_content);
}

#[tauri::command]
fn get_ide_capabilities(ide: relay_mcp::ide::IdeKind) -> relay_mcp::ide::IdeCapabilities {
    relay_mcp::ide::capabilities(ide)
}

#[tauri::command]
fn get_current_ide_capabilities() -> Option<relay_mcp::ide::IdeCapabilities> {
    relay_mcp::ide::get_process_ide().map(relay_mcp::ide::capabilities)
}

#[tauri::command]
fn ide_has_relay_mcp() -> bool {
    relay_mcp::ide::get_process_ide()
        .map(relay_mcp::ide::has_relay_mcp)
        .unwrap_or(false)
}

#[tauri::command]
fn ide_install_relay_mcp() -> Result<(), String> {
    let ide =
        relay_mcp::ide::get_process_ide().ok_or_else(|| "no IDE mode configured".to_string())?;
    relay_mcp::ide::install_relay_mcp(ide).map_err(|e| e.to_string())
}

#[tauri::command]
fn ide_uninstall_relay_mcp() -> Result<(), String> {
    let ide =
        relay_mcp::ide::get_process_ide().ok_or_else(|| "no IDE mode configured".to_string())?;
    relay_mcp::ide::uninstall_relay_mcp(ide).map_err(|e| e.to_string())
}

#[tauri::command]
fn ide_mcp_json_path() -> Result<String, String> {
    let ide =
        relay_mcp::ide::get_process_ide().ok_or_else(|| "no IDE mode configured".to_string())?;
    relay_mcp::ide::mcp_json_path(ide)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn ide_rule_installed() -> bool {
    relay_mcp::ide::get_process_ide()
        .map(relay_mcp::ide::rule_installed)
        .unwrap_or(false)
}

#[tauri::command]
fn ide_install_rule(content: String) -> Result<(), String> {
    let ide =
        relay_mcp::ide::get_process_ide().ok_or_else(|| "no IDE mode configured".to_string())?;
    relay_mcp::ide::install_rule(ide, &content).map_err(|e| e.to_string())
}

#[tauri::command]
fn ide_uninstall_rule() -> Result<(), String> {
    let ide =
        relay_mcp::ide::get_process_ide().ok_or_else(|| "no IDE mode configured".to_string())?;
    relay_mcp::ide::uninstall_rule(ide).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    opener::open(&url).map_err(|e| e.to_string())
}

fn run_tauri(initial: LaunchState) {
    let _ = refresh_gui_presence_marker();
    let persist_hub = true;
    let active_tab_id = initial.tab_id.clone();
    let tid = initial.tab_id.clone();
    let qa_rounds = if initial.retell.trim().is_empty() {
        vec![]
    } else {
        vec![QaRound {
            retell: initial.retell.trim().to_string(),
            reply: String::new(),
            skipped: false,
            submitted: false,
            tab_id: tid,
            relay_mcp_session_id: String::new(),
            reply_attachments: vec![],
            retell_at: relay_mcp::storage::timestamp_string(),
            reply_at: String::new(),
        }]
    };
    let initial_state = FeedbackTabsState {
        tabs: vec![initial],
        active_tab_id,
        qa_rounds,
        persist_hub,
    };

    let app = tauri::Builder::default()
        .manage(Mutex::new(EdgeHideState::default()))
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::Focused(focused) = event {
                relay_mcp::dock_edge_hide::handle_main_window_focus(window.app_handle(), *focused);
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            let runtime = RelayGuiRuntime::new(initial_state, handle);
            if let Err(e) = runtime.spawn_http_server() {
                eprintln!("relay: failed to start HTTP IPC: {e}");
                std::process::exit(1);
            }
            app.manage(runtime);
            let h = app.handle().clone();
            let peek_h = app.handle().clone();
            let dock0 = relay_mcp::read_window_dock();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(160));
                let Some(win) = h.get_webview_window("main") else {
                    return;
                };
                let _ = win.set_title(&relay_mcp::ide::window_title());
                let _ = relay_mcp::position_main_window_for_dock(&win, &dock0);
                let _ = win.set_always_on_top(relay_mcp::read_window_always_on_top());
            });
            thread::spawn(|| loop {
                let _ = refresh_gui_presence_marker();
                thread::sleep(Duration::from_secs(3));
            });
            thread::spawn(move || loop {
                let ms = if relay_mcp::dock_edge_hide::peek_fast_poll_wanted() {
                    120
                } else {
                    900
                };
                thread::sleep(Duration::from_millis(ms));
                relay_mcp::dock_edge_hide::try_expand_from_peek_hover(&peek_h);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_feedback_tabs,
            set_active_tab,
            read_tab_status,
            submit_tab_feedback,
            close_feedback_tab,
            dismiss_feedback_tab,
            get_ui_locale,
            set_ui_locale,
            get_window_dock,
            get_window_always_on_top,
            set_window_dock,
            set_window_always_on_top,
            get_dock_edge_hide,
            get_dock_edge_hide_ui_timing,
            set_dock_edge_hide,
            dock_edge_hide_after_leave,
            dock_edge_force_expand,
            get_mcp_paused,
            set_mcp_paused,
            get_relay_path_env_status,
            configure_relay_path_env_permanent,
            remove_relay_path_env,
            get_mcp_config_json,
            open_relay_data_folder,
            check_github_latest_release,
            open_relay_github_repo,
            get_relay_cache_stats,
            clear_relay_cache_attachments,
            clear_relay_cache_logs,
            clear_relay_cache_all,
            get_attachment_retention_days,
            set_attachment_retention_days,
            run_attachment_retention_purge,
            save_feedback_attachment,
            read_dragged_image_preview,
            validate_feedback_attachment_path,
            read_local_file_bytes_b64,
            get_cursor_usage_settings,
            set_cursor_usage_settings,
            set_cursor_session_token,
            get_cursor_session_token,
            clear_cursor_session_token,
            fetch_cursor_usage_events,
            fetch_cursor_usage_via_ide,
            open_url,
            get_ide_binding,
            set_ide_binding,
            get_window_title,
            recheck_version_upgrade,
            get_ide_capabilities,
            get_current_ide_capabilities,
            ide_has_relay_mcp,
            ide_install_relay_mcp,
            ide_uninstall_relay_mcp,
            ide_mcp_json_path,
            ide_rule_installed,
            ide_install_rule,
            ide_uninstall_rule
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Relay");
    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = &event {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
        if let tauri::RunEvent::Exit = event {
            relay_mcp::remove_gui_presence_marker();
            if let Ok(p) = relay_mcp::mcp_http::gui_endpoint_path() {
                let _ = std::fs::remove_file(p);
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = app;
    });
}

fn run_gui_with_ide(ide: relay_mcp::ide::IdeKind) {
    if relay_mcp::mcp_http::is_ide_gui_alive(ide) {
        eprintln!(
            "Error: Another Relay GUI process is already running in {} mode.\n\
             Only one process per IDE mode is allowed.",
            ide.label()
        );
        std::process::exit(1);
    }
    relay_mcp::ide::set_process_ide(ide);
    let state = relay_mcp::dev_preview_launch_state();
    run_tauri(state);
}

fn run_mcp(ide: relay_mcp::ide::IdeKind, flags: McpFlags) {
    relay_mcp::ide::set_process_ide(ide);
    relay_mcp::set_mcp_wsl_path_rewrite_enabled(flags.exe_in_wsl);
    try_attach_parent_console_for_cli();
    if let Err(e) = relay_mcp::run_feedback_server() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        None => {
            // Bare `relay`: open GUI without IDE — shows selection page.
            let state = relay_mcp::dev_preview_launch_state();
            run_tauri(state);
        }
        Some(Commands::GuiCursor) => run_gui_with_ide(relay_mcp::ide::IdeKind::Cursor),
        Some(Commands::GuiClaudeCode) => run_gui_with_ide(relay_mcp::ide::IdeKind::ClaudeCode),
        Some(Commands::GuiWindsurf) => run_gui_with_ide(relay_mcp::ide::IdeKind::Windsurf),
        Some(Commands::GuiOther) => run_gui_with_ide(relay_mcp::ide::IdeKind::Other),
        Some(Commands::McpCursor(f)) => run_mcp(relay_mcp::ide::IdeKind::Cursor, f),
        Some(Commands::McpClaudeCode(f)) => run_mcp(relay_mcp::ide::IdeKind::ClaudeCode, f),
        Some(Commands::McpWindsurf(f)) => run_mcp(relay_mcp::ide::IdeKind::Windsurf, f),
        Some(Commands::McpOther(f)) => run_mcp(relay_mcp::ide::IdeKind::Other, f),
        Some(Commands::Feedback {
            retell,
            timeout,
            relay_mcp_session_id,
        }) => {
            try_attach_parent_console_for_cli();
            let sid = relay_mcp_session_id.as_deref().unwrap_or("");
            let timeout_seconds = timeout.saturating_mul(60);
            if let Err(e) = run_feedback_cli(retell, timeout_seconds, sid) {
                eprintln!("relay feedback: {e}");
                std::process::exit(1);
            }
        }
    }
}
