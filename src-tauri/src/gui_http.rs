//! Local HTTP for MCP ↔ GUI (`docs/HTTP_IPC.md`).

use crate::{
    allocate_chat_seq, apply_reply_for_tab, chat_title_for_seq, finish_tab_remove_empty_close,
    mcp_http, new_tab_id, push_qa_round, skip_open_round_for_tab, ControlStatus, FeedbackTabsState,
    LaunchState,
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
use std::collections::HashMap;
use std::net::TcpListener as StdTcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::Emitter;
use tokio::sync::oneshot;
use tower_http::limit::RequestBodyLimitLayer;

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
    let mut wtx = inner.wait_tx.lock().unwrap();
    let mut wrx = inner.wait_rx.lock().unwrap();
    if let Some(tx) = wtx.remove(rid) {
        let _ = tx.send(String::new());
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

    pub fn set_active_tab(&self, tab_id: &str) -> Result<(), String> {
        let mut g = self.0.tabs.lock().map_err(|e| e.to_string())?;
        if g.tabs.iter().any(|t| t.tab_id == tab_id) {
            g.active_tab_id = tab_id.to_string();
        }
        Ok(())
    }

    fn complete_request(&self, rid: &str, answer: String) {
        let tx = self.0.wait_tx.lock().unwrap().remove(rid);
        self.0.wait_rx.lock().unwrap().remove(rid);
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
        self.0.wait_tx.lock().unwrap().contains_key(&t.request_id)
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
        if self.0.wait_tx.lock().unwrap().contains_key(&t.request_id) {
            Some(ControlStatus::Active)
        } else {
            Some(ControlStatus::Cancelled)
        }
    }

    pub fn submit_tab_feedback(
        &self,
        tab_id: &str,
        feedback: String,
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
        let mut g = self.0.tabs.lock().map_err(|e| e.to_string())?;
        apply_reply_for_tab(&mut g, tab_id, &feedback, false);
        drop(g);
        self.complete_request(&rid, feedback);
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
            self.complete_request(&t.request_id, String::new());
        }
        let mut g = self.0.tabs.lock().map_err(|e| e.to_string())?;
        apply_reply_for_tab(&mut g, tab_id, "", true);
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
            self.complete_request(&t.request_id, String::new());
        }
        let mut g = self.0.tabs.lock().map_err(|e| e.to_string())?;
        apply_reply_for_tab(&mut g, tab_id, "", true);
        finish_tab_remove_empty_close(&mut g, tab_id, app);
        emit_tabs(app);
        Ok(())
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
    /// Accepted for API compatibility; GUI assigns **Chat N** from `client_tab_id` only.
    #[serde(default)]
    #[allow(dead_code)]
    session_title: String,
    #[serde(default)]
    client_tab_id: String,
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
    let client_tab_id = body.client_tab_id;

    let rid = match tokio::task::spawn_blocking(move || {
        let rid = uuid::Uuid::new_v4().to_string();
        let mut g = inner.tabs.lock().unwrap();
        g.tabs.retain(|t| !t.is_preview);
        if g.tabs.is_empty() {
            g.qa_rounds.clear();
        }

        let merge_idx = if !client_tab_id.is_empty() {
            g.tabs
                .iter()
                .position(|t| t.client_tab_id == client_tab_id && !t.is_preview)
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
            push_qa_round(&mut g, &retell, &tab_id, &client_tab_id);
            let t = &mut g.tabs[idx];
            t.retell = retell.clone();
            t.request_id = rid.clone();
            t.session_title.clear();
            t.tab_id = tab_id.clone();
            t.client_tab_id = client_tab_id.clone();
            if merge_was_active {
                g.active_tab_id = tab_id.clone();
            }
        } else {
            let tid = new_tab_id();
            push_qa_round(&mut g, &retell, &tid, &client_tab_id);
            let seq = allocate_chat_seq(&mut g, &client_tab_id);
            let title = chat_title_for_seq(seq);
            g.tabs.push(LaunchState {
                retell: retell.clone(),
                request_id: rid.clone(),
                title,
                session_title: String::new(),
                tab_id: tid.clone(),
                client_tab_id: client_tab_id.clone(),
                is_preview: false,
            });
        }

        if !g.tabs.is_empty() && !g.tabs.iter().any(|t| t.tab_id == g.active_tab_id) {
            g.active_tab_id = g.tabs[g.tabs.len() - 1].tab_id.clone();
        }
        drop(g);

        let (tx, rx) = oneshot::channel::<String>();
        {
            let mut wtx = inner.wait_tx.lock().unwrap();
            let mut wrx = inner.wait_rx.lock().unwrap();
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

    // POST without matching GET /wait would leak oneshot slots; clean up after 620s (past 600s wait timeout).
    let inner_orphan = st.inner.clone();
    let rid_orphan = rid.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(620)).await;
        let inner = inner_orphan;
        let rid = rid_orphan;
        let _ = tokio::task::spawn_blocking(move || {
            let mut wrx = inner.wait_rx.lock().unwrap();
            if !wrx.contains_key(&rid) {
                return;
            }
            wrx.remove(&rid);
            drop(wrx);
            let _ = inner.wait_tx.lock().unwrap().remove(&rid);
            let mut g = inner.tabs.lock().unwrap();
            if let Some(t) = g.tabs.iter().find(|t| t.request_id == rid).cloned() {
                if !t.is_preview {
                    apply_reply_for_tab(&mut g, &t.tab_id, "", true);
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
        let mut wrx = st.inner.wait_rx.lock().unwrap();
        wrx.remove(&rid)
    };
    let Some(rx) = rx else {
        return (StatusCode::NOT_FOUND, "unknown request_id").into_response();
    };

    match tokio::time::timeout(Duration::from_secs(600), rx).await {
        Ok(Ok(s)) => (
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; charset=utf-8",
            )],
            s,
        )
            .into_response(),
        _ => {
            let inner = st.inner.clone();
            let rid2 = rid.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let mut g = inner.tabs.lock().unwrap();
                if let Some(t) = g.tabs.iter().find(|t| t.request_id == rid2).cloned() {
                    if !t.is_preview {
                        apply_reply_for_tab(&mut g, &t.tab_id, "", true);
                        finish_tab_remove_empty_close(&mut g, &t.tab_id, &inner.app);
                    }
                }
                inner.wait_tx.lock().unwrap().remove(&rid2);
                emit_tabs(&inner.app);
            })
            .await;
            (
                [(
                    axum::http::header::CONTENT_TYPE,
                    "text/plain; charset=utf-8",
                )],
                String::new(),
            )
                .into_response()
        }
    }
}
