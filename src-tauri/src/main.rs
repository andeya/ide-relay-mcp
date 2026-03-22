//! `relay` / `relay gui` (hub), `relay mcp`, `relay feedback` (terminal).
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use clap::{Parser, Subcommand};
use tauri::{Manager, State};

use relay_mcp::{
    gui_http::RelayGuiRuntime, refresh_gui_presence_marker, run_feedback_cli, ControlStatus,
    FeedbackTabsState, LaunchState, QaRound,
};

/// Release Windows builds use the GUI subsystem; attach to the parent console so CLI subcommands
/// can print MCP JSON-RPC / `relay feedback` output when launched from cmd or PowerShell.
///
/// Skips attaching when stdout is already a pipe so IDE-hosted `relay mcp` (stdio JSON-RPC) is
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
        if h != INVALID_HANDLE_VALUE && h != 0 && GetFileType(h) == FILE_TYPE_PIPE {
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

#[derive(Subcommand)]
enum Commands {
    /// MCP JSON-RPC on stdio — set IDE command to this binary and args to `mcp`
    Mcp,
    /// Open Relay window (same as running `relay` with no subcommand)
    Gui,
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
fn set_window_dock(dock: String, app: tauri::AppHandle) -> Result<(), String> {
    let d = dock.trim();
    relay_mcp::write_window_dock(d).map_err(|e| e.to_string())?;
    if let Some(w) = app.get_webview_window("main") {
        let _ = relay_mcp::position_main_window_for_dock(&w, d);
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
fn get_mcp_config_json() -> Result<String, String> {
    relay_mcp::mcp_setup::mcp_config_json_pretty().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_mcp_cursor_installed() -> bool {
    relay_mcp::mcp_setup::cursor_has_relay_mcp()
}

#[derive(serde::Serialize)]
struct McpStatus {
    installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[tauri::command]
fn get_mcp_cursor_status() -> McpStatus {
    let installed = relay_mcp::mcp_setup::cursor_has_relay_mcp();
    McpStatus {
        installed,
        reason: if installed {
            None
        } else {
            relay_mcp::mcp_setup::cursor_relay_mcp_reason()
        },
    }
}

#[tauri::command]
fn get_cursor_mcp_json_path() -> Result<String, String> {
    relay_mcp::mcp_setup::cursor_mcp_json_path()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn install_mcp_to_cursor() -> Result<(), String> {
    relay_mcp::mcp_setup::install_relay_mcp_cursor().map_err(|e| e.to_string())
}

#[tauri::command]
fn uninstall_mcp_from_cursor() -> Result<(), String> {
    relay_mcp::mcp_setup::uninstall_relay_mcp_cursor().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_mcp_windsurf_installed() -> bool {
    relay_mcp::mcp_setup::windsurf_has_relay_mcp()
}

#[tauri::command]
fn get_mcp_windsurf_status() -> McpStatus {
    let installed = relay_mcp::mcp_setup::windsurf_has_relay_mcp();
    McpStatus {
        installed,
        reason: if installed {
            None
        } else {
            relay_mcp::mcp_setup::windsurf_relay_mcp_reason()
        },
    }
}

#[tauri::command]
fn get_windsurf_mcp_json_path() -> Result<String, String> {
    relay_mcp::mcp_setup::windsurf_mcp_json_path()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn install_mcp_to_windsurf() -> Result<(), String> {
    relay_mcp::mcp_setup::install_relay_mcp_windsurf().map_err(|e| e.to_string())
}

#[tauri::command]
fn uninstall_mcp_from_windsurf() -> Result<(), String> {
    relay_mcp::mcp_setup::uninstall_relay_mcp_windsurf().map_err(|e| e.to_string())
}

#[tauri::command]
fn relay_full_install() -> Result<serde_json::Value, String> {
    relay_mcp::mcp_setup::full_install_integrated().map_err(|e| e.to_string())
}

#[tauri::command]
fn relay_full_uninstall() -> Result<(), String> {
    relay_mcp::mcp_setup::full_uninstall_integrated().map_err(|e| e.to_string())
}

#[tauri::command]
fn open_relay_data_folder() -> Result<(), String> {
    let p = relay_mcp::prepare_user_data_dir().map_err(|e| e.to_string())?;
    opener::open(&p).map_err(|e| e.to_string())
}

#[tauri::command]
fn check_github_latest_release() -> relay_mcp::release_check::ReleaseCheckPayload {
    relay_mcp::release_check::check_latest_release(env!("CARGO_PKG_VERSION"))
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

#[tauri::command]
fn read_feedback_attachment_data_url(path: String) -> Result<String, String> {
    relay_mcp::read_feedback_attachment_data_url(&path).map_err(|e| e.to_string())
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
        }]
    };
    let initial_state = FeedbackTabsState {
        tabs: vec![initial],
        active_tab_id,
        qa_rounds,
        persist_hub,
    };

    let mut builder = tauri::Builder::default();
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }));
    }
    builder
        .setup(move |app| {
            let handle = app.handle().clone();
            let runtime = RelayGuiRuntime::new(initial_state, handle);
            if let Err(e) = runtime.spawn_http_server() {
                eprintln!("relay: failed to start HTTP IPC: {e}");
                std::process::exit(1);
            }
            app.manage(runtime);
            let h = app.handle().clone();
            let dock0 = relay_mcp::read_window_dock();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(160));
                let Some(win) = h.get_webview_window("main") else {
                    return;
                };
                let _ = relay_mcp::position_main_window_for_dock(&win, &dock0);
            });
            thread::spawn(|| loop {
                let _ = refresh_gui_presence_marker();
                thread::sleep(Duration::from_secs(3));
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
            set_window_dock,
            get_mcp_paused,
            set_mcp_paused,
            get_relay_path_env_status,
            configure_relay_path_env_permanent,
            get_mcp_config_json,
            get_mcp_cursor_installed,
            get_mcp_cursor_status,
            get_cursor_mcp_json_path,
            install_mcp_to_cursor,
            uninstall_mcp_from_cursor,
            get_mcp_windsurf_installed,
            get_mcp_windsurf_status,
            get_windsurf_mcp_json_path,
            install_mcp_to_windsurf,
            uninstall_mcp_from_windsurf,
            relay_full_install,
            relay_full_uninstall,
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
            read_feedback_attachment_data_url,
            read_dragged_image_preview,
            validate_feedback_attachment_path,
            read_local_file_bytes_b64
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Relay")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                relay_mcp::remove_gui_presence_marker();
                if let Ok(p) = relay_mcp::mcp_http::gui_endpoint_path() {
                    let _ = std::fs::remove_file(p);
                }
            }
        });
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        None | Some(Commands::Gui) => {
            let state = relay_mcp::dev_preview_launch_state().unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            });
            run_tauri(state);
        }
        Some(Commands::Mcp) => {
            try_attach_parent_console_for_cli();
            if let Err(e) = relay_mcp::run_feedback_server() {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
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
