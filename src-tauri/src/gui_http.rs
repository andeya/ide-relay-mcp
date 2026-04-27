//! Local HTTP for MCP ↔ GUI (`docs/HTTP_IPC.md`).

use crate::{
    apply_hydration_bundle, apply_reply_for_tab, feedback_tool_result_string,
    finish_tab_remove_empty_close, format_session_id_as_title, hydration_bundle_per_tab, mcp_http,
    merge_command_items, new_tab_id, push_qa_round,
    reconcile_qa_rounds_when_tabs_empty_after_preview_strip, relay_mcp_session_id_now,
    session_id_from_tool_arg, skip_open_round_for_tab, CommandItem, ControlStatus,
    FeedbackTabsState, LaunchState, QaAttachmentRef,
};
use axum::{
    extract::{Path, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use rand::Rng;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet};
use std::net::TcpListener as StdTcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::Emitter;
use tokio::sync::oneshot;
use tower_http::limit::RequestBodyLimitLayer;

/// Stable JSON body when no tab matches (must stay in sync with [`crate::feedback_tool_result_string`] shape).
fn empty_tool_result_fallback() -> String {
    serde_json::json!({
        "relay_mcp_session_id": "",
        "human": "",
        "cmd_skill_count": 0,
    })
    .to_string()
}

fn lock_tabs(inner: &RelayGuiInner) -> std::sync::MutexGuard<'_, FeedbackTabsState> {
    inner
        .tabs
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn lock_wait_tx(
    inner: &RelayGuiInner,
) -> std::sync::MutexGuard<'_, HashMap<String, oneshot::Sender<String>>> {
    inner
        .wait_tx
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn lock_wait_rx(
    inner: &RelayGuiInner,
) -> std::sync::MutexGuard<'_, HashMap<String, oneshot::Receiver<String>>> {
    inner
        .wait_rx
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Represents a single active MCP → GUI connection (a pending feedback wait).
#[derive(Debug, Clone, Serialize)]
pub struct ActiveSession {
    pub request_id: String,
    pub relay_mcp_session_id: String,
    pub tab_id: String,
    pub title: String,
    pub mcp_pid: Option<u32>,
    pub mcp_hostname: Option<String>,
    /// `"local"` or `"remote"`.
    pub mcp_origin: String,
    pub ide_mode: Option<String>,
    pub connected_at: String,
}

#[derive(Clone)]
pub struct RelayGuiRuntime(Arc<RelayGuiInner>);

struct RelayGuiInner {
    tabs: Mutex<FeedbackTabsState>,
    hydrated_sessions: Mutex<HashSet<String>>,
    log_parse_cache: Mutex<Option<LogParseCache>>,
    hydration_running: AtomicBool,
    wait_tx: Mutex<HashMap<String, oneshot::Sender<String>>>,
    wait_rx: Mutex<HashMap<String, oneshot::Receiver<String>>>,
    active_sessions: Mutex<HashMap<String, ActiveSession>>,
    token: String,
    port: Mutex<Option<u16>>,
    app: tauri::AppHandle,
}

#[derive(Clone)]
struct LogParseCache {
    signature: Option<(u64, u128)>,
    parsed: crate::storage::McpFeedbackLogParse,
}

fn random_token() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

fn auth_ok(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| t == token)
        .unwrap_or(false)
}

/// Binds `127.0.0.1:0` and returns the listener plus chosen port.
fn gui_http_bind_listener() -> Result<(StdTcpListener, u16), String> {
    let listener = StdTcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("local_addr: {e}"))?
        .port();
    Ok((listener, port))
}

/// Writes per-IDE endpoint file so MCP can discover port and bearer token.
fn gui_http_write_endpoint_file(port: u16, token: &str) -> Result<(), String> {
    let path = mcp_http::gui_endpoint_path().map_err(|e| format!("gui_endpoint_path: {e}"))?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let payload = serde_json::json!({
        "port": port,
        "token": token,
        "pid": std::process::id(),
    });
    std::fs::write(&path, payload.to_string()).map_err(|e| format!("write endpoint: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn run_gui_axum_server(std_listener: StdTcpListener, inner: Arc<RelayGuiInner>) {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("relay gui_http: runtime {}", e);
            return;
        }
    };
    if let Err(e) = std_listener.set_nonblocking(true) {
        eprintln!("relay gui_http: set_nonblocking {}", e);
        return;
    }

    rt.block_on(async move {
        let listener = match tokio::net::TcpListener::from_std(std_listener) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("relay gui_http: from_std {}", e);
                return;
            }
        };
        let axum_state = AxumState {
            inner: inner.clone(),
        };
        let app = Router::new()
            .route("/v1/health", get(health))
            .route("/v1/feedback", post(post_feedback))
            .route("/v1/feedback/wait/:rid", get(wait_feedback))
            .layer(RequestBodyLimitLayer::new(16 * 1024 * 1024))
            .with_state(axum_state);

        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("relay gui_http: serve {}", e);
        }
    });
}

/// Returns cached parse when log signature matches; otherwise parses disk and refreshes cache.
fn log_parse_cache_get_or_parse(
    inner: &Arc<RelayGuiInner>,
    data_dir: &std::path::Path,
    signature: Option<(u64, u128)>,
) -> Result<crate::storage::McpFeedbackLogParse, ()> {
    let mut cache = match inner.log_parse_cache.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    if let Some(cached) = cache.as_ref() {
        if cached.signature == signature {
            return Ok(cached.parsed.clone());
        }
    }
    let fresh = match crate::storage::parse_feedback_log_mcp(data_dir) {
        Ok(p) => p,
        Err(e) => {
            #[cfg(debug_assertions)]
            eprintln!("relay: parse_feedback_log_mcp: {e}");
            #[cfg(not(debug_assertions))]
            let _ = e;
            return Err(());
        }
    };
    *cache = Some(LogParseCache {
        signature,
        parsed: fresh.clone(),
    });
    Ok(fresh)
}

fn session_ids_from_tabs(g: &FeedbackTabsState) -> Vec<String> {
    g.tabs
        .iter()
        .filter(|t| !t.is_preview)
        .map(|t| t.relay_mcp_session_id.trim().to_string())
        .filter(|sid| !sid.is_empty())
        .collect()
}

fn hydration_needed_for_sessions(inner: &Arc<RelayGuiInner>, session_ids: &[String]) -> bool {
    if session_ids.is_empty() {
        return false;
    }
    let hs = match inner.hydrated_sessions.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    session_ids.iter().any(|sid| !hs.contains(sid))
}

/// Lightweight signature for `feedback_log.txt` to decide parse-cache reuse.
fn feedback_log_signature(data_dir: &std::path::Path) -> Option<(u64, u128)> {
    let p = data_dir.join(crate::LOG_FILE);
    let meta = std::fs::metadata(p).ok()?;
    let len = meta.len();
    let modified_nanos = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    Some((len, modified_nanos))
}

fn lock_active_sessions(
    inner: &RelayGuiInner,
) -> std::sync::MutexGuard<'_, HashMap<String, ActiveSession>> {
    inner
        .active_sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Cancel pending wait by request id using a precomputed empty tool result.
/// Use this when caller already holds `tabs` lock to avoid recursive lock.
fn cancel_wait_with_result(inner: &RelayGuiInner, rid: &str, empty_result: String) {
    let mut wtx = lock_wait_tx(inner);
    let mut wrx = lock_wait_rx(inner);
    if let Some(tx) = wtx.remove(rid) {
        let _ = tx.send(empty_result);
    }
    wrx.remove(rid);
    lock_active_sessions(inner).remove(rid);
}

fn emit_tabs(app: &tauri::AppHandle) {
    let _ = app.emit("relay_tabs_changed", ());
}

/// Bring main window to foreground (minimized / hidden / behind other apps).
fn focus_main_window(app: &tauri::AppHandle) {
    use tauri::Manager;
    // Restore full window geometry before raising so new MCP retell is visible (edge tuck).
    let _ = crate::dock_edge_hide::expand_if_collapsed(app);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        // Re-apply always-on-top: hide→show can reset the window level on macOS.
        let _ = w.set_always_on_top(crate::read_window_always_on_top());
    }
}

impl RelayGuiRuntime {
    pub fn new(initial: FeedbackTabsState, app: tauri::AppHandle) -> Self {
        Self(Arc::new(RelayGuiInner {
            tabs: Mutex::new(initial),
            hydrated_sessions: Mutex::new(HashSet::new()),
            log_parse_cache: Mutex::new(None),
            hydration_running: AtomicBool::new(false),
            wait_tx: Mutex::new(HashMap::new()),
            wait_rx: Mutex::new(HashMap::new()),
            active_sessions: Mutex::new(HashMap::new()),
            token: random_token(),
            port: Mutex::new(None),
            app,
        }))
    }

    /// Binds `127.0.0.1:0`, writes endpoint file if IDE is set, serves until process exit.
    pub fn spawn_http_server(&self) -> anyhow::Result<u16> {
        let inner = self.0.clone();
        let token = inner.token.clone();
        let (tx_port, rx_port) = std::sync::mpsc::channel::<std::result::Result<u16, String>>();

        thread::spawn(move || {
            let (std_listener, port) = match gui_http_bind_listener() {
                Ok(x) => x,
                Err(msg) => {
                    eprintln!("relay gui_http: {}", msg);
                    let _ = tx_port.send(Err(msg));
                    return;
                }
            };
            if let Ok(mut p) = inner.port.lock() {
                *p = Some(port);
            }
            if crate::ide::get_process_ide().is_some() {
                if let Err(msg) = gui_http_write_endpoint_file(port, &token) {
                    eprintln!("relay gui_http: endpoint file: {}", msg);
                }
            }
            if tx_port.send(Ok(port)).is_err() {
                return;
            }

            run_gui_axum_server(std_listener, inner);
        });

        match rx_port.recv_timeout(Duration::from_secs(20)) {
            Ok(Ok(port)) => Ok(port),
            Ok(Err(msg)) => Err(anyhow::anyhow!("gui_http: {msg}")),
            Err(_) => Err(anyhow::anyhow!(
                "gui_http: HTTP server did not start within 20s (see stderr)"
            )),
        }
    }

    /// (Re-)write the per-IDE endpoint file so MCP processes can discover this GUI.
    /// Called when IDE mode is set/switched at runtime.
    pub fn write_endpoint_file(&self) -> Result<(), String> {
        let port = self
            .0
            .port
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .ok_or_else(|| "HTTP server not yet started".to_string())?;
        gui_http_write_endpoint_file(port, &self.0.token)
    }

    pub fn tabs_snapshot(&self) -> FeedbackTabsState {
        match self.0.tabs.lock() {
            Ok(g) => g.clone(),
            Err(e) => e.into_inner().clone(),
        }
    }

    /// Fills `qa_rounds` from `feedback_log.txt` and/or `qa_archive` when the persisted source has
    /// more completed rounds than in-memory submitted count (see `hydration_bundle_per_tab` /
    /// `apply_hydration_bundle`).
    pub fn hydrate_qa_from_log(&self) {
        if self
            .0
            .hydration_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        let inner = self.0.clone();
        thread::spawn(move || {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                hydrate_qa_from_log_background(&inner);
            }));
            inner.hydration_running.store(false, Ordering::Release);
        });
    }

    pub fn set_active_tab(&self, tab_id: &str) -> Result<(), String> {
        let mut g = self.0.tabs.lock().map_err(|e| e.to_string())?;
        if g.tabs.iter().any(|t| t.tab_id == tab_id) {
            g.active_tab_id = tab_id.to_string();
        }
        Ok(())
    }

    fn complete_request(&self, rid: &str, answer: String) {
        let tx = lock_wait_tx(&self.0).remove(rid);
        lock_wait_rx(&self.0).remove(rid);
        lock_active_sessions(&self.0).remove(rid);
        if let Some(tx) = tx {
            let _ = tx.send(answer);
        }
    }

    pub fn read_tab_status(&self, tab_id: &str) -> Option<ControlStatus> {
        let g = self.0.tabs.lock().ok()?;
        let t = g.tabs.iter().find(|x| x.tab_id == tab_id)?;
        if t.is_preview {
            return Some(ControlStatus::Active);
        }
        if t.request_id.is_empty() {
            return Some(ControlStatus::Idle);
        }
        if lock_wait_tx(&self.0).contains_key(&t.request_id) {
            Some(ControlStatus::Active)
        } else {
            Some(ControlStatus::Cancelled)
        }
    }

    pub fn submit_tab_feedback(
        &self,
        tab_id: &str,
        human: String,
        attachments: Vec<QaAttachmentRef>,
        app: &tauri::AppHandle,
    ) -> Result<(), String> {
        let t = {
            let g = self.0.tabs.lock().map_err(|e| e.to_string())?;
            g.tabs
                .iter()
                .find(|x| x.tab_id == tab_id)
                .cloned()
                .ok_or_else(|| "tab not found".to_string())?
        };
        if t.is_preview {
            return Err("cannot submit dev preview tab".into());
        }
        let rid = t.request_id.clone();
        if rid.is_empty() {
            return Err("no pending request".into());
        }
        let human_plain = strip_legacy_relay_marker_tail(&human);
        let result_string = feedback_tool_result_string(&t, &human_plain, &attachments);
        let mut g = self.0.tabs.lock().map_err(|e| e.to_string())?;
        apply_reply_for_tab(&mut g, tab_id, &human_plain, &attachments, false, false);
        drop(g);
        self.complete_request(&rid, result_string);
        let mut g = self.0.tabs.lock().map_err(|e| e.to_string())?;
        if let Some(t) = g.tabs.iter_mut().find(|x| x.tab_id == tab_id) {
            t.request_id.clear();
            t.retell.clear();
        }
        drop(g);
        emit_tabs(app);
        Ok(())
    }

    pub fn dismiss_feedback_tab(&self, tab_id: &str, app: &tauri::AppHandle) -> Result<(), String> {
        let t = {
            let g = self.0.tabs.lock().map_err(|e| e.to_string())?;
            g.tabs.iter().find(|x| x.tab_id == tab_id).cloned()
        };
        let Some(t) = t else {
            return Ok(());
        };
        if !t.request_id.is_empty() {
            let empty_result = feedback_tool_result_string(&t, "", &[]);
            self.complete_request(&t.request_id, empty_result);
        }
        let mut g = self.0.tabs.lock().map_err(|e| e.to_string())?;
        apply_reply_for_tab(&mut g, tab_id, "", &[], true, false);
        finish_tab_remove_empty_close(&mut g, tab_id, app);
        emit_tabs(app);
        Ok(())
    }

    pub fn rename_tab(
        &self,
        tab_id: &str,
        title: &str,
        app: &tauri::AppHandle,
    ) -> Result<(), String> {
        let new_title = truncate_title(title);
        if new_title.is_empty() {
            return Err("title must not be empty".to_string());
        }
        let mut g = self.0.tabs.lock().map_err(|e| e.to_string())?;
        let Some(t) = g.tabs.iter_mut().find(|x| x.tab_id == tab_id) else {
            return Ok(());
        };
        t.title = new_title;
        t.title_renamed_by_user = true;
        drop(g);
        emit_tabs(app);
        Ok(())
    }

    pub fn close_feedback_tab(&self, tab_id: &str, app: &tauri::AppHandle) -> Result<(), String> {
        let t = {
            let g = self.0.tabs.lock().map_err(|e| e.to_string())?;
            g.tabs.iter().find(|x| x.tab_id == tab_id).cloned()
        };
        let Some(t) = t else {
            return Ok(());
        };
        if t.is_preview {
            let mut g = self.0.tabs.lock().map_err(|e| e.to_string())?;
            finish_tab_remove_empty_close(&mut g, tab_id, app);
            emit_tabs(app);
            return Ok(());
        }
        if !t.request_id.is_empty() {
            let empty_result = feedback_tool_result_string(&t, "", &[]);
            self.complete_request(&t.request_id, empty_result);
        }
        let mut g = self.0.tabs.lock().map_err(|e| e.to_string())?;
        apply_reply_for_tab(&mut g, tab_id, "", &[], true, false);
        finish_tab_remove_empty_close(&mut g, tab_id, app);
        emit_tabs(app);
        Ok(())
    }

    // ---- Active session management ----

    pub fn list_active_sessions(&self) -> Vec<ActiveSession> {
        lock_active_sessions(&self.0).values().cloned().collect()
    }

    /// Disconnect a single session by request_id (sends empty reply to MCP).
    pub fn disconnect_session(&self, request_id: &str) -> Result<(), String> {
        let empty_result = {
            let g = lock_tabs(&self.0);
            g.tabs
                .iter()
                .find(|t| t.request_id == request_id)
                .map(|t| feedback_tool_result_string(t, "", &[]))
                .unwrap_or_else(empty_tool_result_fallback)
        };
        cancel_wait_with_result(&self.0, request_id, empty_result);
        emit_tabs(&self.0.app);
        Ok(())
    }

    /// Disconnect all active sessions (sends empty reply to every pending MCP wait).
    pub fn disconnect_all_sessions(&self) {
        let rids: Vec<String> = lock_active_sessions(&self.0).keys().cloned().collect();
        for rid in rids {
            let empty_result = {
                let g = lock_tabs(&self.0);
                g.tabs
                    .iter()
                    .find(|t| t.request_id == rid)
                    .map(|t| feedback_tool_result_string(t, "", &[]))
                    .unwrap_or_else(empty_tool_result_fallback)
            };
            cancel_wait_with_result(&self.0, &rid, empty_result);
        }
        emit_tabs(&self.0.app);
    }
}

fn hydrate_qa_from_log_background(inner: &Arc<RelayGuiInner>) {
    // Keep `tabs` locked only for small snapshots. Log parsing can take seconds on a large
    // `feedback_log.txt`; holding `tabs` here blocked `get_feedback_tabs` → UI stuck on "Loading…".
    let session_ids: Vec<String> = {
        let g = match inner.tabs.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        session_ids_from_tabs(&g)
    };
    if !hydration_needed_for_sessions(inner, &session_ids) {
        return;
    }

    let tabs_for_hydrate: Vec<LaunchState> = {
        let g = match inner.tabs.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        g.tabs
            .iter()
            .filter(|t| !t.is_preview && !t.relay_mcp_session_id.trim().is_empty())
            .cloned()
            .collect()
    };
    if tabs_for_hydrate.is_empty() {
        return;
    }

    let data_dir = match crate::prepare_user_data_dir() {
        Ok(d) => d,
        Err(e) => {
            #[cfg(debug_assertions)]
            eprintln!("relay: prepare_user_data_dir: {e}");
            #[cfg(not(debug_assertions))]
            let _ = e;
            return;
        }
    };
    let signature = feedback_log_signature(&data_dir);
    let Ok(parsed) = log_parse_cache_get_or_parse(inner, &data_dir, signature) else {
        return;
    };
    // Archive reads happen here (no `tabs` lock) so `get_feedback_tabs` → `tabs_snapshot` is not
    // blocked on disk I/O inside `apply_hydration_bundle`.
    let bundle = hydration_bundle_per_tab(&parsed, &tabs_for_hydrate, &data_dir);
    let changed = {
        let mut g = match inner.tabs.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        apply_hydration_bundle(&mut g, &bundle)
    };
    {
        let mut cache = match inner.log_parse_cache.lock() {
            Ok(c) => c,
            Err(e) => e.into_inner(),
        };
        // In-memory rounds are now fresher than disk parse snapshot.
        *cache = None;
    }
    {
        let mut hs = match inner.hydrated_sessions.lock() {
            Ok(h) => h,
            Err(e) => e.into_inner(),
        };
        for sid in session_ids {
            hs.insert(sid);
        }
        if changed {
            emit_tabs(&inner.app);
        }
    }
}

const MAX_TITLE_CHARS: usize = 60;

fn truncate_title(raw: &str) -> String {
    let t = raw.trim();
    if t.chars().count() <= MAX_TITLE_CHARS {
        return t.to_string();
    }
    t.chars().take(MAX_TITLE_CHARS).collect()
}

/// If old clients still send `<<<RELAY_FEEDBACK_JSON>>>` inside `human`, keep only the caption.
fn strip_legacy_relay_marker_tail(s: &str) -> String {
    const M: &str = "<<<RELAY_FEEDBACK_JSON>>>";
    let t = s.trim();
    match t.find(M) {
        Some(i) => t[..i].trim_end().to_string(),
        None => t.to_string(),
    }
}

#[derive(Clone)]
struct AxumState {
    inner: Arc<RelayGuiInner>,
}

async fn health(State(st): State<AxumState>, headers: HeaderMap) -> impl IntoResponse {
    if !auth_ok(&headers, &st.inner.token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    StatusCode::OK.into_response()
}

/// Bundled metadata from the MCP POST body (reduces argument count).
struct FeedbackPostParams {
    retell: String,
    relay_mcp_session_id: String,
    commands: Option<Vec<CommandItem>>,
    skills: Option<Vec<CommandItem>>,
    title: Option<String>,
    mcp_pid: Option<u32>,
    mcp_hostname: Option<String>,
    mcp_origin: Option<String>,
    ide_mode: Option<String>,
}

fn post_feedback_apply_state(inner: &Arc<RelayGuiInner>, p: FeedbackPostParams) -> String {
    let FeedbackPostParams {
        retell,
        relay_mcp_session_id,
        commands,
        skills,
        title,
        mcp_pid,
        mcp_hostname,
        mcp_origin,
        ide_mode,
    } = p;
    let rid = uuid::Uuid::new_v4().to_string();
    let mut g = lock_tabs(inner);
    g.tabs.retain(|t| !t.is_preview);
    reconcile_qa_rounds_when_tabs_empty_after_preview_strip(&mut g, &relay_mcp_session_id);

    let merge_idx = if !relay_mcp_session_id.is_empty() {
        g.tabs
            .iter()
            .position(|t| t.relay_mcp_session_id == relay_mcp_session_id && !t.is_preview)
    } else {
        None
    };

    if let Some(idx) = merge_idx {
        let old_rid = g.tabs[idx].request_id.clone();
        if !old_rid.is_empty() {
            let empty_result = feedback_tool_result_string(&g.tabs[idx], "", &[]);
            cancel_wait_with_result(inner, &old_rid, empty_result);
        }
        let old_tid = g.tabs[idx].tab_id.clone();
        let merge_was_active = g.active_tab_id == old_tid;
        skip_open_round_for_tab(&mut g, &old_tid);
        let tab_id = new_tab_id();
        push_qa_round(&mut g, &retell, &tab_id, &relay_mcp_session_id);
        let t = &mut g.tabs[idx];
        t.retell = retell.clone();
        t.request_id = rid.clone();
        t.tab_id = tab_id.clone();
        t.commands = merge_command_items(t.commands.clone(), commands.clone());
        t.skills = merge_command_items(t.skills.clone(), skills.clone());
        if !t.title_renamed_by_user {
            if let Some(new_title) = title
                .as_ref()
                .map(|s| truncate_title(s))
                .filter(|s| !s.is_empty())
            {
                t.title = new_title;
            }
        }
        if merge_was_active {
            g.active_tab_id = tab_id.clone();
        }
    } else {
        let sid = if relay_mcp_session_id.is_empty() {
            relay_mcp_session_id_now()
        } else {
            relay_mcp_session_id.clone()
        };
        let title = title
            .map(|t| truncate_title(&t))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| format_session_id_as_title(&sid));
        let tid = new_tab_id();
        push_qa_round(&mut g, &retell, &tid, &sid);
        g.tabs.push(LaunchState {
            retell: retell.clone(),
            request_id: rid.clone(),
            title,
            title_renamed_by_user: false,
            tab_id: tid.clone(),
            relay_mcp_session_id: sid.clone(),
            is_preview: false,
            commands: commands.clone(),
            skills: skills.clone(),
        });
    }

    if !g.tabs.is_empty() && !g.tabs.iter().any(|t| t.tab_id == g.active_tab_id) {
        g.active_tab_id = g.tabs[g.tabs.len() - 1].tab_id.clone();
    }

    let session_snapshot = g.tabs.iter().find(|t| t.request_id == rid).map(|t| {
        (
            t.tab_id.clone(),
            t.relay_mcp_session_id.clone(),
            t.title.clone(),
        )
    });
    drop(g);

    let (tx, rx) = oneshot::channel::<String>();
    {
        let mut wtx = lock_wait_tx(inner);
        let mut wrx = lock_wait_rx(inner);
        wtx.insert(rid.clone(), tx);
        wrx.insert(rid.clone(), rx);
    }

    if let Some((tab_id, sid, title)) = session_snapshot {
        let session = ActiveSession {
            request_id: rid.clone(),
            relay_mcp_session_id: sid,
            tab_id,
            title,
            mcp_pid,
            mcp_hostname,
            mcp_origin: mcp_origin.unwrap_or_else(|| "local".to_string()),
            ide_mode,
            connected_at: crate::storage::timestamp_string(),
        };
        if let Ok(mut m) = inner.active_sessions.lock() {
            m.insert(rid.clone(), session);
        }
    }

    emit_tabs(&inner.app);
    focus_main_window(&inner.app);
    rid
}

fn feedback_orphan_wait_cleanup(inner: Arc<RelayGuiInner>, rid: String) {
    let mut wrx = lock_wait_rx(&inner);
    if !wrx.contains_key(&rid) {
        return;
    }
    wrx.remove(&rid);
    drop(wrx);
    lock_active_sessions(&inner).remove(&rid);
    let mut g = lock_tabs(&inner);
    let empty_result = g
        .tabs
        .iter()
        .find(|t| t.request_id == rid)
        .map(|t| feedback_tool_result_string(t, "", &[]))
        .unwrap_or_else(empty_tool_result_fallback);
    if let Some(tx) = lock_wait_tx(&inner).remove(&rid) {
        let _ = tx.send(empty_result);
    }
    if let Some(t) = g.tabs.iter().find(|t| t.request_id == rid).cloned() {
        if !t.is_preview {
            apply_reply_for_tab(&mut g, &t.tab_id, "", &[], true, true);
            finish_tab_remove_empty_close(&mut g, &t.tab_id, &inner.app);
        }
    }
    emit_tabs(&inner.app);
    let _ = inner.app.emit("relay_idle_timeout", ());
}

fn spawn_feedback_orphan_wait_timer(inner: Arc<RelayGuiInner>, rid: String) {
    let mins = crate::config::read_feedback_idle_timeout_minutes() as u64;
    if mins == 0 {
        return; // keep-alive mode: never auto-timeout
    }
    let wait_secs = mins.saturating_mul(60).saturating_add(20);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(wait_secs)).await;
        let _ = tokio::task::spawn_blocking(move || feedback_orphan_wait_cleanup(inner, rid)).await;
    });
}

#[derive(Deserialize)]
struct PostFeedbackBody {
    retell: String,
    /// Accept string or number (same as MCP tool args).
    #[serde(default)]
    relay_mcp_session_id: JsonValue,
    #[serde(default)]
    commands: Option<Vec<CommandItem>>,
    #[serde(default)]
    skills: Option<Vec<CommandItem>>,
    /// IDE cli_id that the MCP process was launched with (e.g. "cursor").
    #[serde(default)]
    ide_mode: Option<String>,
    /// Agent-provided descriptive title for a new session tab.
    #[serde(default)]
    title: Option<String>,
    /// MCP process PID.
    #[serde(default)]
    mcp_pid: Option<u32>,
    /// Hostname where the MCP process runs.
    #[serde(default)]
    mcp_hostname: Option<String>,
    /// `"local"` or `"remote"` — derived from the endpoint file used.
    #[serde(default)]
    mcp_origin: Option<String>,
}

async fn post_feedback(
    State(st): State<AxumState>,
    headers: HeaderMap,
    Json(body): Json<PostFeedbackBody>,
) -> impl IntoResponse {
    if !auth_ok(&headers, &st.inner.token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if body.retell.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            "retell is required and must be non-empty",
        )
            .into_response();
    }
    if let Some(ref mcp_ide) = body.ide_mode {
        if let Some(gui_ide) = crate::ide::get_process_ide() {
            if mcp_ide != gui_ide.cli_id() {
                return (
                    StatusCode::CONFLICT,
                    format!(
                        "IDE mode mismatch: MCP is '{}' but GUI is '{}' ({})",
                        mcp_ide,
                        gui_ide.cli_id(),
                        gui_ide.label()
                    ),
                )
                    .into_response();
            }
        }
    }
    let inner = st.inner.clone();
    let params = FeedbackPostParams {
        retell: body.retell,
        relay_mcp_session_id: session_id_from_tool_arg(Some(&body.relay_mcp_session_id)),
        commands: body.commands,
        skills: body.skills,
        title: body.title,
        mcp_pid: body.mcp_pid,
        mcp_hostname: body.mcp_hostname,
        mcp_origin: body.mcp_origin,
        ide_mode: body.ide_mode,
    };

    let rid = match tokio::task::spawn_blocking(move || {
        post_feedback_apply_state(&inner, params)
    })
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    spawn_feedback_orphan_wait_timer(st.inner.clone(), rid.clone());

    Json(serde_json::json!({ "request_id": rid })).into_response()
}

async fn wait_feedback(
    State(st): State<AxumState>,
    headers: HeaderMap,
    Path(rid): Path<String>,
) -> impl IntoResponse {
    if !auth_ok(&headers, &st.inner.token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let rx = {
        let mut wrx = lock_wait_rx(&st.inner);
        wrx.remove(&rid)
    };
    let Some(rx) = rx else {
        return (StatusCode::NOT_FOUND, "unknown request_id").into_response();
    };

    // No timeout: wait until user submits or dismisses (oneshot completes). If sender is dropped without send, treat as empty.
    let result = match rx.await {
        Ok(s) => {
            lock_active_sessions(&st.inner).remove(&rid);
            s
        }
        Err(_) => {
            let inner = st.inner.clone();
            let rid2 = rid.clone();
            let empty_result = tokio::task::spawn_blocking({
                let inner = inner.clone();
                move || {
                    let g = lock_tabs(&inner);
                    g.tabs
                        .iter()
                        .find(|t| t.request_id == rid2)
                        .map(|t| feedback_tool_result_string(t, "", &[]))
                        .unwrap_or_else(empty_tool_result_fallback)
                }
            })
            .await
            .unwrap_or_else(|_| empty_tool_result_fallback());
            let inner_cleanup = st.inner.clone();
            let rid_cleanup = rid.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let mut g = lock_tabs(&inner_cleanup);
                if let Some(t) = g.tabs.iter().find(|t| t.request_id == rid_cleanup).cloned() {
                    if !t.is_preview {
                        apply_reply_for_tab(&mut g, &t.tab_id, "", &[], true, false);
                        finish_tab_remove_empty_close(&mut g, &t.tab_id, &inner_cleanup.app);
                    }
                }
                lock_wait_tx(&inner_cleanup).remove(&rid_cleanup);
                lock_active_sessions(&inner_cleanup).remove(&rid_cleanup);
                emit_tabs(&inner_cleanup.app);
            })
            .await;
            empty_result
        }
    };
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )],
        result,
    )
        .into_response()
}
