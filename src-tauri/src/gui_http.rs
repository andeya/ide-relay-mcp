//! Local HTTP for MCP ↔ GUI (`docs/HTTP_IPC.md`).

use crate::{
    apply_reply_for_tab, feedback_tool_result_string, finish_tab_remove_empty_close,
    format_session_id_as_title, hydrate_qa_rounds_from_feedback_log, mcp_http, merge_command_items,
    new_tab_id, push_qa_round, reconcile_qa_rounds_when_tabs_empty_after_preview_strip,
    relay_mcp_session_id_now, session_id_from_tool_arg, skip_open_round_for_tab, CommandItem,
    ControlStatus, FeedbackTabsState, LaunchState, QaAttachmentRef,
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
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::net::TcpListener as StdTcpListener;
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

#[derive(Clone)]
pub struct RelayGuiRuntime(Arc<RelayGuiInner>);

struct RelayGuiInner {
    tabs: Mutex<FeedbackTabsState>,
    wait_tx: Mutex<HashMap<String, oneshot::Sender<String>>>,
    wait_rx: Mutex<HashMap<String, oneshot::Receiver<String>>>,
    token: String,
    app: tauri::AppHandle,
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

fn cancel_wait(inner: &RelayGuiInner, rid: &str) {
    let empty_result = {
        let g = lock_tabs(inner);
        g.tabs
            .iter()
            .find(|t| t.request_id == rid)
            .map(|t| feedback_tool_result_string(t, "", &[]))
            .unwrap_or_else(empty_tool_result_fallback)
    };
    let mut wtx = lock_wait_tx(inner);
    let mut wrx = lock_wait_rx(inner);
    if let Some(tx) = wtx.remove(rid) {
        let _ = tx.send(empty_result);
    }
    wrx.remove(rid);
}

fn emit_tabs(app: &tauri::AppHandle) {
    let _ = app.emit("relay_tabs_changed", ());
}

/// Bring main window to foreground (minimized / hidden / behind other apps).
fn focus_main_window(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

impl RelayGuiRuntime {
    pub fn new(initial: FeedbackTabsState, app: tauri::AppHandle) -> Self {
        Self(Arc::new(RelayGuiInner {
            tabs: Mutex::new(initial),
            wait_tx: Mutex::new(HashMap::new()),
            wait_rx: Mutex::new(HashMap::new()),
            token: random_token(),
            app,
        }))
    }

    /// Binds `127.0.0.1:0`, writes `gui_endpoint.json`, serves until process exit.
    pub fn spawn_http_server(&self) -> anyhow::Result<u16> {
        let inner = self.0.clone();
        let token = inner.token.clone();
        let (tx_port, rx_port) = std::sync::mpsc::channel::<std::result::Result<u16, String>>();

        thread::spawn(move || {
            let std_listener = match StdTcpListener::bind("127.0.0.1:0") {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("relay gui_http: bind {}", e);
                    let _ = tx_port.send(Err(format!("bind: {e}")));
                    return;
                }
            };
            let port = match std_listener.local_addr() {
                Ok(a) => a.port(),
                Err(e) => {
                    eprintln!("relay gui_http: local_addr {}", e);
                    let _ = tx_port.send(Err(format!("local_addr: {e}")));
                    return;
                }
            };
            let path = match mcp_http::gui_endpoint_path() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("relay gui_http: path {}", e);
                    let _ = tx_port.send(Err(format!("gui_endpoint_path: {e}")));
                    return;
                }
            };
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let payload = serde_json::json!({
                "port": port,
                "token": token,
                "pid": std::process::id(),
            });
            if let Err(e) = std::fs::write(&path, payload.to_string()) {
                eprintln!("relay gui_http: write endpoint {}", e);
                let _ = tx_port.send(Err(format!("write endpoint: {e}")));
                return;
            }
            if tx_port.send(Ok(port)).is_err() {
                return;
            }

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

            // `TcpListener::from_std` must run inside Tokio 1.x runtime (reactor registration).
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
        });

        match rx_port.recv_timeout(Duration::from_secs(20)) {
            Ok(Ok(port)) => Ok(port),
            Ok(Err(msg)) => Err(anyhow::anyhow!("gui_http: {msg}")),
            Err(_) => Err(anyhow::anyhow!(
                "gui_http: HTTP server did not start within 20s (see stderr)"
            )),
        }
    }

    pub fn tabs_snapshot(&self) -> FeedbackTabsState {
        match self.0.tabs.lock() {
            Ok(g) => g.clone(),
            Err(e) => e.into_inner().clone(),
        }
    }

    /// Fills `qa_rounds` from `feedback_log.txt` and/or `qa_archive` when the persisted source has
    /// more completed rounds than in-memory submitted count (see `hydrate_qa_rounds_from_feedback_log`).
    pub fn hydrate_qa_from_log(&self) {
        let mut g = match self.0.tabs.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        match hydrate_qa_rounds_from_feedback_log(&mut g) {
            Ok(true) => emit_tabs(&self.0.app),
            Err(e) => {
                #[cfg(debug_assertions)]
                eprintln!("relay: hydrate_qa_rounds_from_feedback_log: {e}");
                #[cfg(not(debug_assertions))]
                let _ = e;
            }
            Ok(false) => {}
        }
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
        if let Some(tx) = tx {
            let _ = tx.send(answer);
        }
    }

    pub fn tab_request_pending(&self, tab_id: &str) -> bool {
        let g = match self.0.tabs.lock() {
            Ok(x) => x,
            Err(_) => return false,
        };
        let Some(t) = g.tabs.iter().find(|x| x.tab_id == tab_id) else {
            return false;
        };
        if t.request_id.is_empty() {
            return false;
        }
        lock_wait_tx(&self.0).contains_key(&t.request_id)
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
        apply_reply_for_tab(&mut g, tab_id, &human_plain, &attachments, false);
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
        apply_reply_for_tab(&mut g, tab_id, "", &[], true);
        finish_tab_remove_empty_close(&mut g, tab_id, app);
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
        apply_reply_for_tab(&mut g, tab_id, "", &[], true);
        finish_tab_remove_empty_close(&mut g, tab_id, app);
        emit_tabs(app);
        Ok(())
    }
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
    let inner = st.inner.clone();
    let retell = body.retell;
    let relay_mcp_session_id = session_id_from_tool_arg(Some(&body.relay_mcp_session_id));
    let commands = body.commands;
    let skills = body.skills;

    let rid = match tokio::task::spawn_blocking(move || {
        let rid = uuid::Uuid::new_v4().to_string();
        let mut g = lock_tabs(&inner);
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
                cancel_wait(&inner, &old_rid);
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
            if merge_was_active {
                g.active_tab_id = tab_id.clone();
            }
        } else {
            let sid = if relay_mcp_session_id.is_empty() {
                relay_mcp_session_id_now()
            } else {
                relay_mcp_session_id.clone()
            };
            let title = format_session_id_as_title(&sid);
            let tid = new_tab_id();
            push_qa_round(&mut g, &retell, &tid, &sid);
            g.tabs.push(LaunchState {
                retell: retell.clone(),
                request_id: rid.clone(),
                title,
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
        drop(g);

        let (tx, rx) = oneshot::channel::<String>();
        {
            let mut wtx = lock_wait_tx(&inner);
            let mut wrx = lock_wait_rx(&inner);
            wtx.insert(rid.clone(), tx);
            wrx.insert(rid.clone(), rx);
        }

        emit_tabs(&inner.app);
        focus_main_window(&inner.app);
        rid
    })
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    // POST without matching GET /wait would leak oneshot slots; clean up after 60min+20s (past 60min wait timeout).
    let inner_orphan = st.inner.clone();
    let rid_orphan = rid.clone();
    const WAIT_TIMEOUT_SECS: u64 = 60 * 60; // 60 minutes
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(WAIT_TIMEOUT_SECS + 20)).await;
        let inner = inner_orphan;
        let rid = rid_orphan;
        let _ = tokio::task::spawn_blocking(move || {
            let mut wrx = lock_wait_rx(&inner);
            if !wrx.contains_key(&rid) {
                return;
            }
            wrx.remove(&rid);
            drop(wrx);
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
                    apply_reply_for_tab(&mut g, &t.tab_id, "", &[], true);
                    finish_tab_remove_empty_close(&mut g, &t.tab_id, &inner.app);
                }
            }
            emit_tabs(&inner.app);
        })
        .await;
    });

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
        Ok(s) => s,
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
                        apply_reply_for_tab(&mut g, &t.tab_id, "", &[], true);
                        finish_tab_remove_empty_close(&mut g, &t.tab_id, &inner_cleanup.app);
                    }
                }
                lock_wait_tx(&inner_cleanup).remove(&rid_cleanup);
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
