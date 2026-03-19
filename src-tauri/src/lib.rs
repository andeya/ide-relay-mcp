//! MCP server (`relay mcp`), GUI (`relay` / `relay gui`). Vocabulary: `docs/TERMINOLOGY.md`.

use anyhow::{anyhow, bail, Context, Result};
use chrono::Local;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use tauri::Manager;
use tauri::{PhysicalPosition, WebviewWindow};

pub mod gui_http;
pub mod mcp_http;
pub mod mcp_setup;

pub const APP_NAME: &str = "Relay MCP";
pub const APP_QUALIFIER: &str = "com";
pub const APP_ORGANIZATION: &str = "relay";
pub const APP_DATA_DIR: &str = "relay-mcp";
pub const TOOL_NAME: &str = "relay_interactive_feedback";
pub const CONFIG_ONESHOT: &str = "auto_reply_oneshot.txt";
pub const CONFIG_LOOP: &str = "auto_reply_loop.txt";
pub const LOG_FILE: &str = "feedback_log.txt";
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub const GUI_ALIVE_MARKER: &str = "relay_gui_alive.marker";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoReplyRule {
    pub timeout_seconds: u64,
    pub text: String,
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
    /// OS window title — GUI sets **Chat N** via [`chat_title_for_seq`]; `session_title` usually empty.
    pub title: String,
    /// Legacy; HTTP still accepts `session_title` but GUI clears / ignores for display.
    pub session_title: String,
    pub tab_id: String,
    pub client_tab_id: String,
    pub is_preview: bool,
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
    /// Same IDE chat tab as `LaunchState.client_tab_id` (merge key).
    #[serde(default)]
    pub client_tab_id: String,
}

#[derive(Clone, Serialize)]
pub struct FeedbackTabsState {
    pub tabs: Vec<LaunchState>,
    pub active_tab_id: String,
    pub qa_rounds: Vec<QaRound>,
    #[serde(skip_serializing)]
    pub persist_hub: bool,
    /// First-seen `client_tab_id` → permanent display index (Chat N). Survives tab close.
    #[serde(skip_serializing)]
    pub client_tab_id_to_seq: HashMap<String, u32>,
    /// Monotonic counter; each new anonymous tab or new client_tab_id consumes the next number.
    #[serde(skip_serializing)]
    pub chat_seq_counter: u32,
}

/// Next global Chat index for this GUI process. Empty `client_tab_id` always gets a fresh number.
pub fn allocate_chat_seq(g: &mut FeedbackTabsState, client_tab_id: &str) -> u32 {
    let tid = client_tab_id.trim();
    if tid.is_empty() {
        g.chat_seq_counter = g.chat_seq_counter.saturating_add(1);
        return g.chat_seq_counter;
    }
    if let Some(&n) = g.client_tab_id_to_seq.get(tid) {
        return n;
    }
    g.chat_seq_counter = g.chat_seq_counter.saturating_add(1);
    let n = g.chat_seq_counter;
    g.client_tab_id_to_seq.insert(tid.to_string(), n);
    n
}

pub fn chat_title_for_seq(n: u32) -> String {
    format!("Chat {n}")
}

pub fn trim_qa_rounds(g: &mut FeedbackTabsState) {
    while g.qa_rounds.len() > QA_ROUNDS_CAP {
        g.qa_rounds.remove(0);
    }
}

pub fn push_qa_round(g: &mut FeedbackTabsState, retell: &str, tab_id: &str, client_tab_id: &str) {
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
        client_tab_id: client_tab_id.to_string(),
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

#[derive(Debug)]
struct ServerState {
    config_dir: PathBuf,
    stdout: io::Stdout,
    loop_index: usize,
}

impl ServerState {
    fn new(config_dir: PathBuf) -> Self {
        Self {
            config_dir,
            stdout: io::stdout(),
            loop_index: 0,
        }
    }

    fn send_json(&mut self, payload: &Value) -> Result<()> {
        let mut handle = self.stdout.lock();
        writeln!(handle, "{}", payload)?;
        handle.flush()?;
        Ok(())
    }

    fn send_result(&mut self, id: Value, result: Value) -> Result<()> {
        self.send_json(&json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }

    fn send_error(&mut self, id: Value, code: i64, message: impl Into<String>) -> Result<()> {
        self.send_json(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": code,
                "message": message.into()
            }
        }))
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

pub const UI_LOCALE_FILE: &str = "ui_locale.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct UiLocaleConfig {
    pub lang: String,
}

/// UI language persisted next to auto-reply config. Default `en`.
pub fn read_ui_locale() -> String {
    let path = match user_data_dir() {
        Ok(dir) => dir.join(UI_LOCALE_FILE),
        Err(_) => return "en".to_string(),
    };
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return "en".to_string(),
    };
    let cfg: UiLocaleConfig = match serde_json::from_str(&text) {
        Ok(c) => c,
        Err(_) => return "en".to_string(),
    };
    match cfg.lang.as_str() {
        "zh" => "zh".to_string(),
        _ => "en".to_string(),
    }
}

pub fn write_ui_locale(lang: &str) -> Result<()> {
    if lang != "en" && lang != "zh" {
        return Err(anyhow!("locale must be en or zh"));
    }
    let dir = user_data_dir()?;
    fs::create_dir_all(&dir)?;
    let path = dir.join(UI_LOCALE_FILE);
    let cfg = UiLocaleConfig {
        lang: lang.to_string(),
    };
    fs::write(path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}

pub const WINDOW_DOCK_FILE: &str = "window_dock.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowDockConfig {
    /// `left` | `center` | `right` — horizontal placement on the current monitor.
    #[serde(default = "default_window_dock")]
    pub dock: String,
}

fn default_window_dock() -> String {
    "left".to_string()
}

/// Persisted horizontal dock; default **left**.
pub fn read_window_dock() -> String {
    let Ok(dir) = user_data_dir() else {
        return "left".to_string();
    };
    let path = dir.join(WINDOW_DOCK_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return "left".to_string(),
    };
    let cfg: WindowDockConfig = match serde_json::from_str(&text) {
        Ok(c) => c,
        Err(_) => return "left".to_string(),
    };
    match cfg.dock.as_str() {
        "center" => "center".to_string(),
        "right" => "right".to_string(),
        _ => "left".to_string(),
    }
}

pub fn write_window_dock(dock: &str) -> Result<()> {
    let d = dock.trim();
    if d != "left" && d != "center" && d != "right" {
        bail!("dock must be left, center, or right");
    }
    let dir = user_data_dir()?;
    fs::create_dir_all(&dir)?;
    let path = dir.join(WINDOW_DOCK_FILE);
    let cfg = WindowDockConfig {
        dock: d.to_string(),
    };
    fs::write(path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}

/// Vertically centered on work area; horizontal by `dock`.
pub fn position_main_window_for_dock(
    win: &WebviewWindow,
    dock: &str,
) -> std::result::Result<(), String> {
    let outer = win.outer_size().map_err(|e| e.to_string())?;
    let w_win = outer.width as i32;
    let h_win = outer.height as i32;
    let mon = win
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no monitor".to_string())?;
    let p = mon.position();
    let sz = mon.size();
    let mw = sz.width as i32;
    let mh = sz.height as i32;
    let y = p.y + (mh.saturating_sub(h_win)) / 2;
    let margin = 12i32;
    let x = match dock {
        "center" => p.x + (mw.saturating_sub(w_win)) / 2,
        "right" => p.x + mw.saturating_sub(w_win).saturating_sub(margin),
        _ => p.x + margin,
    };
    win.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub const MCP_PAUSE_FILE: &str = "mcp_pause.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct McpPauseConfig {
    #[serde(default)]
    pub paused: bool,
}

/// When true, `relay mcp` skips GUI/auto-reply and returns a sentinel tool result immediately.
pub fn read_mcp_paused() -> bool {
    let Ok(dir) = user_data_dir() else {
        return false;
    };
    let path = dir.join(MCP_PAUSE_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return false,
    };
    serde_json::from_str::<McpPauseConfig>(&text)
        .map(|c| c.paused)
        .unwrap_or(false)
}

pub fn write_mcp_paused(paused: bool) -> Result<()> {
    let dir = user_data_dir()?;
    fs::create_dir_all(&dir)?;
    let path = dir.join(MCP_PAUSE_FILE);
    let cfg = McpPauseConfig { paused };
    fs::write(path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
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

// ---------------------------------------------------------------------------
// Permanent PATH for `relay` (user-level, cross-platform)
// ---------------------------------------------------------------------------

const RELAY_PATH_MARKER: &str = "# Relay MCP PATH (managed by Relay app)";

/// Directory containing the `relay` executable.
pub fn relay_cli_directory() -> Result<PathBuf> {
    current_exe_dir()
}

fn user_home_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(windows)]
fn paths_same_bin_dir(a: &Path, b: &Path) -> bool {
    let ca = fs::canonicalize(a);
    let cb = fs::canonicalize(b);
    match (ca, cb) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

#[cfg(windows)]
fn windows_user_path_has_dir(target: &Path) -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(env) = hkcu.open_subkey("Environment") else {
        return false;
    };
    let Ok(path_val) = env.get_value::<String, _>("Path") else {
        return false;
    };
    for part in path_val.split(';') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if paths_same_bin_dir(Path::new(p), target) {
            return true;
        }
    }
    false
}

#[cfg(windows)]
fn windows_append_user_path(target: &Path) -> Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    if windows_user_path_has_dir(target) {
        return Ok(());
    }
    let dir_s = target.to_string_lossy().to_string();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (env, _) = hkcu
        .create_subkey("Environment")
        .context("open HKCU\\Environment")?;
    let path_val: String = env.get_value("Path").unwrap_or_default();
    let new_val = if path_val.trim().is_empty() {
        dir_s
    } else {
        format!("{};{}", path_val.trim_end_matches(';'), dir_s)
    };
    env.set_value("Path", &new_val)
        .context("set user Path in registry")?;
    notify_windows_environment_path_changed();
    Ok(())
}

/// Tell running apps to reload user environment (PATH in registry).
#[cfg(windows)]
fn notify_windows_environment_path_changed() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const HWND_BROADCAST: isize = 0xffff;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;
    #[link(name = "user32")]
    extern "system" {
        fn SendMessageTimeoutW(
            hwnd: isize,
            msg: u32,
            wparam: usize,
            lparam: *const u16,
            flags: u32,
            timeout: u32,
            result: *mut usize,
        ) -> isize;
    }
    let wide: Vec<u16> = OsStr::new("Environment")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut out = 0usize;
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            wide.as_ptr(),
            SMTO_ABORTIFHUNG,
            5000,
            &mut out,
        );
    }
}

#[cfg(not(windows))]
fn fish_config_path(home: &Path) -> PathBuf {
    home.join(".config").join("fish").join("config.fish")
}

#[cfg(not(windows))]
fn unix_rc_files_contain_marker(home: &Path) -> bool {
    let fish = fish_config_path(home);
    if let Ok(s) = fs::read_to_string(&fish) {
        if s.contains(RELAY_PATH_MARKER) {
            return true;
        }
    }
    for name in [".zshrc", ".bash_profile", ".bashrc", ".profile"] {
        let f = home.join(name);
        if let Ok(s) = fs::read_to_string(&f) {
            if s.contains(RELAY_PATH_MARKER) {
                return true;
            }
        }
    }
    false
}

/// Argument for `fish_add_path` (quote if path has spaces or specials).
#[cfg(not(windows))]
fn fish_add_path_token(dir: &Path) -> String {
    let s = dir.to_string_lossy();
    let safe = s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+:@".contains(c));
    if safe {
        s.into_owned()
    } else {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

#[cfg(not(windows))]
fn unix_append_fish_path_block(home: &Path, dir: &Path) -> Result<()> {
    let fish_path = fish_config_path(home);
    let use_fish = fish_path.exists()
        || std::env::var("SHELL")
            .map(|sh| sh.to_lowercase().contains("fish"))
            .unwrap_or(false);
    if !use_fish {
        return Ok(());
    }
    let token = fish_add_path_token(dir);
    let block = format!("\n{}\nfish_add_path {}\n", RELAY_PATH_MARKER, token);
    let existing = if fish_path.exists() {
        fs::read_to_string(&fish_path)?
    } else {
        String::new()
    };
    if existing.contains(RELAY_PATH_MARKER) {
        return Ok(());
    }
    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&block);
    if let Some(parent) = fish_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&fish_path, out).with_context(|| format!("write {}", fish_path.display()))?;
    Ok(())
}

#[cfg(not(windows))]
fn unix_append_path_block(home: &Path, dir: &Path) -> Result<()> {
    let block = format!(
        "\n{}\nexport PATH=\"{}:$PATH\"\n",
        RELAY_PATH_MARKER,
        dir.display()
    );

    fn append_rc(path: &Path, block: &str) -> Result<()> {
        let existing = if path.exists() {
            fs::read_to_string(path)?
        } else {
            String::new()
        };
        if existing.contains(RELAY_PATH_MARKER) {
            return Ok(());
        }
        let mut out = existing;
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(block);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, out).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    if cfg!(target_os = "macos") {
        append_rc(&home.join(".zshrc"), &block)?;
        let bash_profile = home.join(".bash_profile");
        if bash_profile.exists() {
            append_rc(&bash_profile, &block)?;
        }
    } else {
        let bashrc = home.join(".bashrc");
        let profile = home.join(".profile");
        if bashrc.exists() {
            append_rc(&bashrc, &block)?;
        } else if profile.exists() {
            append_rc(&profile, &block)?;
        } else {
            let content = block.trim_start();
            fs::write(&profile, content).context("write ~/.profile")?;
        }
    }
    unix_append_fish_path_block(home, dir)?;
    Ok(())
}

/// True if permanent user config already includes this app’s bin directory.
pub fn relay_path_persistently_configured() -> bool {
    #[cfg(windows)]
    {
        let Ok(dir) = relay_cli_directory() else {
            return false;
        };
        windows_user_path_has_dir(&dir)
    }
    #[cfg(not(windows))]
    {
        relay_cli_directory().is_ok()
            && user_home_dir()
                .map(|h| unix_rc_files_contain_marker(&h))
                .unwrap_or(false)
    }
}

/// Add relay bin dir to user PATH permanently (registry on Windows, shell rc on Unix).
/// Returns `"already"` if nothing to do, `"windows"` / `"unix"` after a successful write.
pub fn persist_relay_cli_path() -> Result<&'static str> {
    let dir = relay_cli_directory()?;
    let relay_bin = dir.join(gui_binary_name());
    if !relay_bin.exists() {
        return Err(anyhow!(
            "relay binary not found beside this app ({})",
            relay_bin.display()
        ));
    }
    if relay_path_persistently_configured() {
        return Ok("already");
    }
    #[cfg(windows)]
    {
        windows_append_user_path(&dir)?;
        return Ok("windows");
    }
    #[cfg(not(windows))]
    {
        let s = dir.to_string_lossy();
        if s.chars()
            .any(|c| matches!(c, '$' | '`' | '"' | '\n' | '\r'))
        {
            return Err(anyhow!(
                "Cannot add relay to PATH: install path contains shell-special characters ($, \", `, or newlines). Use a simpler install path."
            ));
        }
        let home = user_home_dir().ok_or_else(|| anyhow!("HOME / USERPROFILE not set"))?;
        unix_append_path_block(&home, &dir)?;
        Ok("unix")
    }
}

#[cfg(not(windows))]
fn unix_strip_relay_path_block(content: &str) -> String {
    if !content.contains(RELAY_PATH_MARKER) {
        return content.to_string();
    }
    let lines: Vec<&str> = content.lines().collect();
    let mut out: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end_matches('\r');
        if line == RELAY_PATH_MARKER {
            i += 1;
            if i < lines.len() {
                let next = lines[i].trim_start();
                if next.starts_with("export PATH=") || next.starts_with("fish_add_path") {
                    i += 1;
                }
            }
            continue;
        }
        out.push(lines[i]);
        i += 1;
    }
    let trail = content.ends_with('\n');
    let mut s = out.join("\n");
    if trail && !s.is_empty() && !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// Remove Relay-added PATH from shell rc files (Unix) or user Path (Windows).
pub fn remove_relay_cli_path_persistent() -> Result<()> {
    #[cfg(windows)]
    {
        let dir = relay_cli_directory()?;
        windows_remove_user_path_entry(&dir)
    }
    #[cfg(not(windows))]
    {
        let home = user_home_dir().ok_or_else(|| anyhow!("HOME not set"))?;
        let mut paths: Vec<PathBuf> = vec![fish_config_path(&home)];
        for name in [".zshrc", ".bash_profile", ".bashrc", ".profile"] {
            paths.push(home.join(name));
        }
        for p in paths {
            if !p.exists() {
                continue;
            }
            let s = fs::read_to_string(&p)?;
            if !s.contains(RELAY_PATH_MARKER) {
                continue;
            }
            let new_s = unix_strip_relay_path_block(&s);
            fs::write(&p, new_s).with_context(|| format!("write {}", p.display()))?;
        }
        Ok(())
    }
}

#[cfg(windows)]
fn windows_remove_user_path_entry(target: &Path) -> Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(env) = hkcu.open_subkey("Environment") else {
        return Ok(());
    };
    let Ok(path_val): Result<String, _> = env.get_value("Path") else {
        return Ok(());
    };
    let parts: Vec<String> = path_val
        .split(';')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .filter(|p| !paths_same_bin_dir(Path::new(p), target))
        .map(|s| s.to_string())
        .collect();
    let new_val = parts.join(";");
    let (envw, _) = hkcu
        .create_subkey("Environment")
        .context("open Environment for write")?;
    envw.set_value("Path", &new_val)
        .context("write user Path")?;
    notify_windows_environment_path_changed();
    Ok(())
}
pub fn gui_binary_path(exe_dir: &Path) -> PathBuf {
    exe_dir.join(gui_binary_name())
}

pub fn timestamp_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Append one line to `feedback_log.txt` under the user config directory (`config_dir`).
pub fn log_write(config_dir: &Path, source: &str, content: &str) -> Result<()> {
    let line = format!("[{}] [{}] {}\n", timestamp_string(), source, content);
    let path = config_dir.join(LOG_FILE);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .context("failed to open log file")?;
    file.write_all(line.as_bytes())?;
    file.flush()?;
    Ok(())
}

fn next_temp_suffix() -> String {
    let pid = std::process::id();
    let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_nanos();
    format!("{}_{}_{}", pid, nanos, seq)
}

pub fn make_temp_path(prefix: &str, ext: &str) -> PathBuf {
    let suffix = next_temp_suffix();
    let mut path = std::env::temp_dir();
    let file_name = if ext.is_empty() {
        format!("{}_{}", prefix, suffix)
    } else {
        format!("{}_{}.{}", prefix, suffix, ext)
    };
    path.push(file_name);
    path
}

/// Save feedback image under user data (`feedback_attachments/`) so history thumbnails keep working after OS temp cleanup.
pub fn save_feedback_attachment(name: &str, bytes_b64: &str) -> Result<PathBuf> {
    use base64::Engine;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(bytes_b64.trim())
        .map_err(|e| anyhow!("base64: {}", e))?;
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .filter(|e| e.len() <= 8 && e.chars().all(|c| c.is_alphanumeric()))
        .unwrap_or("png");
    let dir = prepare_user_data_dir()?.join("feedback_attachments");
    fs::create_dir_all(&dir).with_context(|| format!("mkdir {}", dir.display()))?;
    let file_name = format!("relay_attach_{}.{}", next_temp_suffix(), ext);
    let path = dir.join(&file_name);
    fs::write(&path, &raw).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

const FEEDBACK_ATTACH_MAX_BYTES: u64 = 20 * 1024 * 1024;

fn is_safe_relay_attachment_filename(name: &str) -> bool {
    if !name.starts_with("relay_attach_") || name.len() > 256 {
        return false;
    }
    if name.contains("..") {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

/// Read a saved feedback image as data URL (only files under user `feedback_attachments/`).
pub fn read_feedback_attachment_data_url(path: &str) -> Result<String> {
    let raw = path
        .trim()
        .trim_matches(|c| c == '"' || c == '\'' || c == '\u{feff}');
    let raw = raw.strip_prefix("file://").unwrap_or(raw);
    #[cfg(windows)]
    let raw = raw.trim_start_matches('/');
    let p = PathBuf::from(raw);
    let name = p
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("invalid path"))?;
    if !is_safe_relay_attachment_filename(name) {
        bail!("not a relay attachment");
    }
    let base = prepare_user_data_dir()?.join("feedback_attachments");
    fs::create_dir_all(&base).with_context(|| format!("mkdir {}", base.display()))?;
    let base_canon =
        fs::canonicalize(&base).with_context(|| format!("canonicalize {}", base.display()))?;
    let candidate = base.join(name);
    let canon = fs::canonicalize(&candidate).context("attachment not found")?;
    if !canon.starts_with(&base_canon) {
        bail!("path outside feedback_attachments");
    }
    let len = fs::metadata(&canon)?.len();
    if len > FEEDBACK_ATTACH_MAX_BYTES {
        bail!("attachment too large");
    }
    let bytes = fs::read(&canon)?;
    let ext = canon
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    };
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

pub fn write_text_file(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file =
        File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
    file.write_all(text.as_bytes())?;
    file.flush()?;
    Ok(())
}

pub fn read_text_file(path: &Path) -> Result<String> {
    let mut text = String::new();
    let mut file =
        File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    file.read_to_string(&mut text)?;
    Ok(text)
}

pub fn trim_eol(mut text: String) -> String {
    while text.ends_with('\n') || text.ends_with('\r') {
        text.pop();
    }
    text
}

pub fn read_trimmed_text(path: &Path) -> Result<String> {
    Ok(trim_eol(read_text_file(path)?))
}

pub fn read_control_status(path: &Path) -> Option<ControlStatus> {
    let text = read_text_file(path).ok()?;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("status=") {
            return value.parse().ok();
        }
    }
    None
}

pub fn write_control_status(path: &Path, status: ControlStatus) -> Result<()> {
    write_text_file(path, &format!("status={}\n", status.as_str()))
}

fn gui_alive_marker_path(config_dir: &Path) -> PathBuf {
    config_dir.join(GUI_ALIVE_MARKER)
}

/// GUI calls this every ~3s so the MCP server can route extra tabs via inbox instead of spawning.
pub fn refresh_gui_presence_marker() -> Result<()> {
    let dir = user_data_dir()?;
    fs::create_dir_all(&dir)?;
    let path = gui_alive_marker_path(&dir);
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .context("gui presence marker")?;
    f.write_all(b"1")?;
    f.flush()?;
    Ok(())
}

pub fn remove_gui_presence_marker() {
    if let Ok(dir) = user_data_dir() {
        let _ = fs::remove_file(gui_alive_marker_path(&dir));
    }
}

pub(crate) fn new_tab_id() -> String {
    format!("t_{}", next_temp_suffix())
}

pub fn load_auto_reply_rules(path: &Path) -> Vec<AutoReplyRule> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => return Vec::new(),
    };

    let mut rules = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((timeout, reply)) = line.split_once('|') else {
            continue;
        };
        let Ok(timeout_seconds) = timeout.trim().parse::<u64>() else {
            continue;
        };
        rules.push(AutoReplyRule {
            timeout_seconds,
            text: reply.to_string(),
        });
    }
    rules
}

pub fn prepare_user_data_dir() -> Result<PathBuf> {
    let config_dir = user_data_dir()?;
    fs::create_dir_all(&config_dir)?;
    Ok(config_dir)
}

#[derive(Debug, Clone, Serialize)]
pub struct RelayCacheStats {
    pub attachments_bytes: u64,
    pub log_bytes: u64,
    pub data_dir: String,
}

fn dir_files_total_bytes(path: &Path) -> std::io::Result<u64> {
    if !path.is_dir() {
        return Ok(0);
    }
    let mut n = 0u64;
    for e in fs::read_dir(path)? {
        let e = e?;
        if e.file_type()?.is_file() {
            n += e.metadata()?.len();
        }
    }
    Ok(n)
}

pub fn relay_cache_stats() -> Result<RelayCacheStats> {
    let base = prepare_user_data_dir()?;
    let attach_dir = base.join("feedback_attachments");
    let attachments_bytes = dir_files_total_bytes(&attach_dir)?;
    let log_path = base.join(LOG_FILE);
    let log_bytes = if log_path.is_file() {
        fs::metadata(&log_path)?.len()
    } else {
        0
    };
    Ok(RelayCacheStats {
        attachments_bytes,
        log_bytes,
        data_dir: base.display().to_string(),
    })
}

pub fn clear_relay_attachments_cache() -> Result<()> {
    let base = prepare_user_data_dir()?;
    let d = base.join("feedback_attachments");
    if d.is_dir() {
        for e in fs::read_dir(&d)? {
            let e = e?;
            if e.file_type()?.is_file() {
                let _ = fs::remove_file(e.path());
            }
        }
    }
    Ok(())
}

pub fn clear_relay_log_cache() -> Result<()> {
    let base = prepare_user_data_dir()?;
    let p = base.join(LOG_FILE);
    if p.exists() {
        fs::write(&p, b"").context("truncate log")?;
    }
    Ok(())
}

const ATTACHMENT_RETENTION_FILE: &str = "attachment_retention.json";
/// When `attachment_retention.json` is missing, purge attachments older than this many days.
/// Choosing "Off" in Settings writes the file with `"days": null` (no auto-purge).
pub const DEFAULT_ATTACHMENT_RETENTION_DAYS: u32 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AttachmentRetentionConfig {
    #[serde(default)]
    pub days: Option<u32>,
}

fn attachment_retention_path() -> Result<PathBuf> {
    Ok(prepare_user_data_dir()?.join(ATTACHMENT_RETENTION_FILE))
}

fn parse_stored_attachment_retention_json(s: &str) -> Option<u32> {
    let Ok(c) = serde_json::from_str::<AttachmentRetentionConfig>(s) else {
        return Some(DEFAULT_ATTACHMENT_RETENTION_DAYS);
    };
    match c.days {
        None => None, // explicit "keep all" after user chose Off
        Some(d) if (1..=3660).contains(&d) => Some(d),
        Some(_) => Some(DEFAULT_ATTACHMENT_RETENTION_DAYS),
    }
}

pub fn read_attachment_retention_days() -> Option<u32> {
    let Ok(path) = attachment_retention_path() else {
        return None;
    };
    if !path.exists() {
        return Some(DEFAULT_ATTACHMENT_RETENTION_DAYS);
    }
    let Ok(s) = fs::read_to_string(&path) else {
        return Some(DEFAULT_ATTACHMENT_RETENTION_DAYS);
    };
    parse_stored_attachment_retention_json(&s)
}

pub fn write_attachment_retention_days(days: Option<u32>) -> Result<()> {
    let path = attachment_retention_path()?;
    let c = AttachmentRetentionConfig { days };
    fs::write(
        &path,
        serde_json::to_string_pretty(&c).context("serialize retention")?,
    )
    .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Deletes `relay_attach_*` files under `feedback_attachments/` older than `days` (by mtime).
pub fn purge_attachments_older_than_days(days: u32) -> Result<u64> {
    if days == 0 {
        return Ok(0);
    }
    let dir = prepare_user_data_dir()?.join("feedback_attachments");
    if !dir.is_dir() {
        return Ok(0);
    }
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(u64::from(days).saturating_mul(86400));
    let mut freed: u64 = 0;
    for e in fs::read_dir(&dir)? {
        let e = e?;
        if !e.file_type()?.is_file() {
            continue;
        }
        let name = e.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("relay_attach_") {
            continue;
        }
        let meta = e.metadata()?;
        let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        if modified < cutoff {
            freed = freed.saturating_add(meta.len());
            let _ = fs::remove_file(e.path());
        }
    }
    Ok(freed)
}

pub fn run_attachment_retention_purge() -> Result<u64> {
    let Some(d) = read_attachment_retention_days() else {
        return Ok(0);
    };
    purge_attachments_older_than_days(d)
}

/// Instant auto-reply only: rules must use `0|reply`. Lines with non-zero timeout are ignored.
pub fn auto_reply_peek(config_dir: &Path, loop_index: usize) -> Option<(AutoReplyRule, bool)> {
    let oneshot: Vec<AutoReplyRule> = load_auto_reply_rules(&config_dir.join(CONFIG_ONESHOT))
        .into_iter()
        .filter(|r| r.timeout_seconds == 0)
        .collect();
    if let Some(rule) = oneshot.first().cloned() {
        return Some((rule, true));
    }

    let loop_rules: Vec<AutoReplyRule> = load_auto_reply_rules(&config_dir.join(CONFIG_LOOP))
        .into_iter()
        .filter(|r| r.timeout_seconds == 0)
        .collect();
    if loop_rules.is_empty() {
        return None;
    }

    let rule = loop_rules[loop_index % loop_rules.len()].clone();
    Some((rule, false))
}

struct LockFile {
    path: PathBuf,
}

impl Drop for LockFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn acquire_lock(path: &Path, timeout: Duration) -> Option<LockFile> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(_) => {
                return Some(LockFile {
                    path: path.to_path_buf(),
                });
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                thread::sleep(Duration::from_millis(25));
            }
            Err(_) => return None,
        }
    }
    None
}

pub fn consume_oneshot(config_dir: &Path) -> Result<()> {
    let path = config_dir.join(CONFIG_ONESHOT);
    let lock_path = path.with_extension("lock");
    let Some(_lock) = acquire_lock(&lock_path, Duration::from_secs(2)) else {
        return Ok(());
    };

    let original = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return Ok(()),
    };

    let mut removed = false;
    let mut lines = Vec::new();
    for line in original.lines() {
        let trimmed = line.trim_end_matches('\r');
        let is_rule = !trimmed.is_empty() && !trimmed.starts_with('#') && trimmed.contains('|');
        if is_rule && !removed {
            removed = true;
            continue;
        }
        lines.push(line.to_string());
    }

    if !removed {
        return Ok(());
    }

    if lines.is_empty() {
        let _ = fs::remove_file(&path);
        return Ok(());
    }

    let mut rewritten = lines.join("\n");
    if !rewritten.ends_with('\n') {
        rewritten.push('\n');
    }
    fs::write(&path, rewritten)?;
    Ok(())
}

fn handle_json_line(state: &mut ServerState, line: &str) -> Result<()> {
    match serde_json::from_str::<Value>(line) {
        Ok(msg) => dispatch_message(state, &msg)?,
        Err(err) => {
            let sample: String = line.chars().take(200).collect();
            let _ = log_write(
                &state.config_dir,
                "JSON_PARSE_ERROR",
                &format!("{} | {}", err, sample),
            );
        }
    }
    Ok(())
}

fn respond_tool_result(state: &mut ServerState, id: Value, feedback: String) -> Result<()> {
    let mut inner = serde_json::Map::new();
    inner.insert(TOOL_NAME.to_string(), json!(feedback));
    let payload = json!({
        "content": [
            {
                "type": "text",
                "text": serde_json::Value::Object(inner).to_string()
            }
        ]
    });
    state.send_result(id, payload)
}

fn handle_cancel_notification(_state: &mut ServerState, _msg: &Value) -> Result<()> {
    Ok(())
}

fn handle_tool_call(state: &mut ServerState, msg: &Value) -> Result<()> {
    let name = msg
        .get("params")
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("");

    if name != TOOL_NAME {
        state.send_error(
            msg["id"].clone(),
            -32601,
            format!("Unrecognized tool: {}", name),
        )?;
        return Ok(());
    }

    let arguments = msg
        .get("params")
        .and_then(|params| params.get("arguments"))
        .cloned()
        .unwrap_or_else(|| json!({}));

    let retell = arguments
        .get("retell")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if retell.trim().is_empty() {
        state.send_error(
            msg["id"].clone(),
            -32602,
            "retell is required (non-empty): this turn's assistant reply to the user",
        )?;
        return Ok(());
    }

    let rpc_id = msg["id"].clone();
    if read_mcp_paused() {
        let _ = log_write(
            &state.config_dir,
            "MCP_PAUSED_BLOCK",
            &retell.chars().take(200).collect::<String>(),
        );
        respond_tool_result(state, rpc_id, MCP_PAUSED_TOOL_REPLY.to_string())?;
        return Ok(());
    }

    let session_title = arguments
        .get("session_title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let client_tab_id = arguments
        .get("client_tab_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let log_line = match (session_title.is_empty(), client_tab_id.is_empty()) {
        (true, true) => retell.clone(),
        (false, true) => format!("[{}] {}", session_title, retell),
        (true, false) => format!("[tab:{}] {}", client_tab_id, retell),
        (false, false) => format!("[{}][tab:{}] {}", session_title, client_tab_id, retell),
    };
    let _ = log_write(&state.config_dir, "AI_REQUEST", &log_line);

    let Some((rule, is_oneshot)) = auto_reply_peek(&state.config_dir, state.loop_index) else {
        match mcp_http::feedback_round(&retell, &session_title, &client_tab_id) {
            Ok(answer) => {
                if !answer.is_empty() {
                    state.loop_index = 0;
                }
                let _ = log_write(&state.config_dir, "USER_REPLY", &answer);
                respond_tool_result(state, rpc_id, answer)?;
            }
            Err(e) => {
                state.send_error(rpc_id, -32603, format!("Relay GUI: {}", e))?;
            }
        }
        return Ok(());
    };

    if is_oneshot {
        consume_oneshot(&state.config_dir)?;
    }
    let _ = log_write(&state.config_dir, "AUTO_REPLY", &rule.text);
    respond_tool_result(state, rpc_id, rule.text)?;
    state.loop_index = state.loop_index.saturating_add(1);
    Ok(())
}

fn dispatch_message(state: &mut ServerState, msg: &Value) -> Result<()> {
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        return Ok(());
    };

    if msg.get("id").is_none() {
        if method == "notifications/cancelled" {
            handle_cancel_notification(state, msg)?;
        }
        return Ok(());
    }

    match method {
        "initialize" => {
            state.send_result(
                msg["id"].clone(),
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "relay-mcp", "version": env!("CARGO_PKG_VERSION") }
                }),
            )?;
        }
        "ping" => {
            state.send_result(msg["id"].clone(), json!({}))?;
        }
        "tools/list" => {
            state.send_result(
                msg["id"].clone(),
                json!({
                    "tools": [
                        {
                            "name": TOOL_NAME,
                            "description": "Human-in-the-loop: opens Relay for your Answer. Pass client_tab_id (merge key); GUI shows Chat N. See Relay rules / docs/CLIENT_TAB_ID.md.",
                            "inputSchema": {
                                "type": "object",
                                "description": "retell required. client_tab_id strongly recommended (workspace root + newline + first user message). session_title optional, ignored by GUI.",
                                "properties": {
                                    "retell": {
                                        "type": "string",
                                        "description": "Required. This turn's full assistant reply to the user (verbatim)."
                                    },
                                    "session_title": {
                                        "type": "string",
                                        "description": "Optional; GUI ignores. Window title is Chat 1, Chat 2, … assigned per client_tab_id."
                                    },
                                    "client_tab_id": {
                                        "type": "string",
                                        "description": "Strongly recommended: '{workspace_root}\\n{first user message}' (500-char cap). Same every turn; merge key → stable Chat N."
                                    }
                                },
                                "required": ["retell"]
                            }
                        }
                    ]
                }),
            )?;
        }
        "tools/call" => {
            handle_tool_call(state, msg)?;
        }
        _ => {
            state.send_error(
                msg["id"].clone(),
                -32601,
                format!("Method not found: {}", method),
            )?;
        }
    }

    Ok(())
}

pub fn run_feedback_server() -> Result<()> {
    let config_dir = prepare_user_data_dir()?;
    let mut state = ServerState::new(config_dir);
    let (tx, rx): (Sender<String>, Receiver<String>) = mpsc::channel();

    thread::spawn(move || {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin.lock());
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut disconnected = false;
    loop {
        while let Ok(line) = rx.try_recv() {
            if line.trim().is_empty() {
                continue;
            }
            handle_json_line(&mut state, &line)?;
        }

        if disconnected {
            break;
        }

        match rx.recv_timeout(Duration::from_millis(120)) {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }
                handle_json_line(&mut state, &line)?;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                disconnected = true;
            }
        }
    }

    Ok(())
}

pub fn run_feedback_cli(
    retell: String,
    timeout_seconds: u64,
    session_title: &str,
    client_tab_id: &str,
) -> Result<()> {
    let config_dir = prepare_user_data_dir()?;
    let _ = log_write(&config_dir, "CLI_REQUEST", &retell);
    let st = session_title.to_string();
    let ctid = client_tab_id.to_string();
    let retell_for_thread = retell.clone();
    let (tx, rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let r = mcp_http::feedback_round(&retell_for_thread, &st, &ctid);
        let _ = tx.send(r);
    });
    let wait = Duration::from_secs(timeout_seconds.max(1));
    match rx.recv_timeout(wait) {
        Ok(Ok(answer)) => {
            let _ = log_write(&config_dir, "CLI_REPLY", &answer);
            println!("{}", answer);
            Ok(())
        }
        Ok(Err(e)) => {
            let _ = log_write(&config_dir, "CLI_ERR", &e.to_string());
            Err(e)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let _ = log_write(&config_dir, "CLI_TIMEOUT", &retell);
            Err(anyhow::anyhow!(
                "timed out after {}s",
                timeout_seconds.max(1)
            ))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(anyhow::anyhow!(
            "internal error: feedback thread disconnected"
        )),
    }
}

/// Truncate long text (not used for Relay GUI tab chrome; GUI uses [`chat_title_for_seq`] only).
pub fn window_title_for_session(session_title: &str) -> String {
    let t = session_title.trim();
    if t.is_empty() {
        return "Chat".to_string();
    }
    let n = t.chars().count();
    let s: String = t.chars().take(72).collect();
    let suffix = if n > 72 { "…" } else { "" };
    format!("{s}{suffix}")
}

static UNNAMED_CHAT_SEQ: AtomicU64 = AtomicU64::new(0);

/// Unused by GUI since [`allocate_chat_seq`]; kept for external callers / tests.
#[allow(dead_code)]
pub fn next_unnamed_chat_title() -> String {
    let n = UNNAMED_CHAT_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    format!("Chat {n}")
}

pub fn launch_state_preview() -> LaunchState {
    let session_title = String::new();
    let title = "Chat".to_string();
    LaunchState {
        retell: "No MCP request yet.\n\n• retell = this turn's assistant reply.\n• IDE: relay + [\"mcp\"]. Terminal: relay feedback --retell. See docs/TERMINOLOGY.md."
            .to_string(),
        request_id: String::new(),
        title,
        session_title,
        tab_id: new_tab_id(),
        client_tab_id: String::new(),
        is_preview: true,
    }
}

/// Hub / `tauri dev` — placeholder tab until MCP delivers real requests.
pub fn dev_preview_launch_state() -> Result<LaunchState> {
    Ok(launch_state_preview())
}

#[cfg(test)]
mod attachment_retention_tests {
    use super::{parse_stored_attachment_retention_json, DEFAULT_ATTACHMENT_RETENTION_DAYS};

    #[test]
    fn json_days_null_is_off() {
        assert_eq!(
            parse_stored_attachment_retention_json(r#"{"days":null}"#),
            None
        );
    }

    #[test]
    fn json_days_30() {
        assert_eq!(
            parse_stored_attachment_retention_json(r#"{"days":30}"#),
            Some(30)
        );
    }

    #[test]
    fn invalid_json_falls_back_to_default() {
        assert_eq!(
            parse_stored_attachment_retention_json("not json"),
            Some(DEFAULT_ATTACHMENT_RETENTION_DAYS)
        );
    }

    #[test]
    fn json_out_of_range_uses_default() {
        assert_eq!(
            parse_stored_attachment_retention_json(r#"{"days":99999}"#),
            Some(DEFAULT_ATTACHMENT_RETENTION_DAYS)
        );
    }
}
