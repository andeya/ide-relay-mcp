//! `relay` / `relay gui` (hub), `relay mcp`, `relay feedback` (terminal).

use std::thread;
use std::time::Duration;

use clap::{Parser, Subcommand};
use tauri::{Manager, State};

use relay_mcp::{
    gui_http::RelayGuiRuntime, refresh_gui_presence_marker, run_feedback_cli, ControlStatus,
    FeedbackTabsState, LaunchState, QaRound,
};

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
            default_value_t = 600,
            help = "Seconds to wait for submit"
        )]
        timeout: u64,
        #[arg(long, help = "Optional window title hint")]
        session_title: Option<String>,
        #[arg(
            long = "client-tab-id",
            help = "Stable tab id (same as MCP client_tab_id): merge into one Relay tab per id, or distinct ids for multiple tabs"
        )]
        client_tab_id: Option<String>,
    },
}

#[tauri::command]
fn get_feedback_tabs(state: State<'_, RelayGuiRuntime>) -> Result<FeedbackTabsState, String> {
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
    feedback: String,
    state: State<'_, RelayGuiRuntime>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    state.submit_tab_feedback(&tab_id, feedback, &app)
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
    Ok(RelayPathEnvStatus {
        configured: relay_mcp::relay_path_persistently_configured(),
        bin_dir: dir.to_string_lossy().into_owned(),
        platform,
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
fn save_feedback_attachment(name: String, bytes_b64: String) -> Result<String, String> {
    relay_mcp::save_feedback_attachment(&name, &bytes_b64)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn read_feedback_attachment_data_url(path: String) -> Result<String, String> {
    relay_mcp::read_feedback_attachment_data_url(&path).map_err(|e| e.to_string())
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
            client_tab_id: String::new(),
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
            get_cursor_mcp_json_path,
            install_mcp_to_cursor,
            uninstall_mcp_from_cursor,
            get_mcp_windsurf_installed,
            get_windsurf_mcp_json_path,
            install_mcp_to_windsurf,
            uninstall_mcp_from_windsurf,
            relay_full_install,
            relay_full_uninstall,
            save_feedback_attachment,
            read_feedback_attachment_data_url
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
            if let Err(e) = relay_mcp::run_feedback_server() {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Feedback {
            retell,
            timeout,
            session_title,
            client_tab_id,
        }) => {
            let st = session_title.as_deref().unwrap_or("");
            let ctid = client_tab_id.as_deref().unwrap_or("");
            if let Err(e) = run_feedback_cli(retell, timeout, st, ctid) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
}
