//! MCP JSON-RPC server: concurrent tools/call, single-writer stdout, run_feedback_server / run_feedback_cli.

use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use crate::auto_reply::{auto_reply_peek, consume_oneshot};
use crate::config::read_mcp_paused;
use crate::mcp_http;
use crate::storage::{log_write, normalize_logged_user_reply, prepare_user_data_dir};
use crate::{CommandItem, MCP_PAUSED_TOOL_REPLY, TOOL_NAME};

/// MCP / JSON-RPC: client cancelled an in-flight request (LSP-style code used in the wild).
const JSONRPC_REQUEST_CANCELLED: i64 = -32800;

/// Max concurrent in-flight `relay_interactive_feedback` GUI rounds per MCP stdio connection.
const MAX_CONCURRENT_HIL: usize = 16;

#[inline]
fn mutex_lock_or_recover<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// --- Per-request state for cancel vs worker completion race ---

#[derive(Debug, Default)]
struct CallState {
    /// Set when any party has emitted the JSON-RPC result/error for this id.
    response_sent: AtomicBool,
    /// Set when host sends notifications/cancelled for this id (before worker finishes).
    cancelled: AtomicBool,
}

impl CallState {
    fn mark_cancelled(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

// --- Hil backend (real HTTP vs test mock) ---

pub(crate) trait HilFeedback: Send + Sync {
    fn feedback_round(
        &self,
        retell: &str,
        relay_mcp_session_id: &str,
        commands: Option<Vec<CommandItem>>,
        skills: Option<Vec<CommandItem>>,
    ) -> Result<String, String>;
}

#[derive(Debug, Default)]
struct McpHttpFeedback;

impl HilFeedback for McpHttpFeedback {
    fn feedback_round(
        &self,
        retell: &str,
        relay_mcp_session_id: &str,
        commands: Option<Vec<CommandItem>>,
        skills: Option<Vec<CommandItem>>,
    ) -> Result<String, String> {
        mcp_http::feedback_round(
            retell,
            relay_mcp_session_id,
            commands.as_deref(),
            skills.as_deref(),
        )
        .map_err(|e| e.to_string())
    }
}

// --- Single-writer stdout (JSON lines must not interleave) ---

#[derive(Clone)]
struct McpOutbound {
    tx: Sender<Value>,
}

impl McpOutbound {
    fn spawn_stdout_writer() -> Self {
        let (tx, rx) = mpsc::channel::<Value>();
        thread::spawn(move || {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            while let Ok(payload) = rx.recv() {
                if let Ok(line) = serde_json::to_string(&payload) {
                    let _ = writeln!(handle, "{}", line);
                    let _ = handle.flush();
                }
            }
        });
        Self { tx }
    }

    #[cfg(test)]
    fn new_for_test(tx: Sender<Value>) -> Self {
        Self { tx }
    }

    fn send_json(&self, v: Value) -> Result<()> {
        self.tx
            .send(v)
            .map_err(|_| anyhow::anyhow!("MCP outbound closed"))
    }

    fn send_result(&self, id: Value, result: Value) -> Result<()> {
        self.send_json(json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }

    fn send_error(&self, id: Value, code: i64, message: impl Into<String>) -> Result<()> {
        self.send_json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": code,
                "message": message.into()
            }
        }))
    }
}

fn respond_tool_result(out: &McpOutbound, id: Value, feedback: String) -> Result<()> {
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
    out.send_result(id, payload)
}

// --- Router context (router thread only mutates loop_index mutex indirectly via workers) ---

struct RouterCtx {
    config_dir: PathBuf,
    outbound: McpOutbound,
    pending: Arc<Mutex<HashMap<Value, Arc<CallState>>>>,
    loop_index: Arc<Mutex<usize>>,
    hil: Arc<dyn HilFeedback>,
}

/// Best-effort extract JSON-RPC `id` from a line that failed [`serde_json::from_str`].
fn scrape_jsonrpc_id(line: &str) -> Option<Value> {
    let key = "\"id\"";
    let idx = line.find(key)?;
    let mut rest = line[idx + key.len()..].trim_start();
    rest = rest.strip_prefix(':')?.trim_start();
    if let Some(stripped) = rest.strip_prefix('"') {
        let end = stripped.find('"')?;
        return Some(Value::String(stripped[..end].to_string()));
    }
    let mut end_byte = 0usize;
    for (i, c) in rest.char_indices() {
        if c == '-' || c.is_ascii_digit() {
            end_byte = i + c.len_utf8();
        } else {
            break;
        }
    }
    if end_byte == 0 {
        return None;
    }
    let n: i64 = rest[..end_byte].parse().ok()?;
    Some(Value::Number(n.into()))
}

fn cancel_notification_request_id(msg: &Value) -> Option<Value> {
    if msg.get("id").is_some() {
        return None;
    }
    if msg.get("method").and_then(Value::as_str) != Some("notifications/cancelled") {
        return None;
    }
    let params = msg.get("params")?;
    params
        .get("requestId")
        .or_else(|| params.get("request_id"))
        .cloned()
}

fn process_cancel_notification(ctx: &RouterCtx, msg: &Value) -> Result<()> {
    let Some(rid) = cancel_notification_request_id(msg) else {
        let sample: String = msg.to_string().chars().take(320).collect();
        let _ = log_write(&ctx.config_dir, "MCP_CANCELLED_ORPHAN", &sample);
        return Ok(());
    };

    let pending = mutex_lock_or_recover(&ctx.pending);
    let Some(st) = pending.get(&rid) else {
        drop(pending);
        let sample: String = msg.to_string().chars().take(320).collect();
        let _ = log_write(&ctx.config_dir, "MCP_CANCELLED_ORPHAN", &sample);
        return Ok(());
    };

    st.mark_cancelled();
    if st
        .response_sent
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok()
    {
        drop(pending);
        let _ = log_write(&ctx.config_dir, "MCP_CANCELLED", "");
        ctx.outbound.send_error(
            rid,
            JSONRPC_REQUEST_CANCELLED,
            "Request cancelled (notifications/cancelled)",
        )?;
    }
    Ok(())
}

fn handle_json_line(ctx: &mut RouterCtx, line: &str) -> Result<()> {
    match serde_json::from_str::<Value>(line) {
        Ok(msg) => dispatch_message(ctx, &msg)?,
        Err(err) => {
            let sample: String = line.chars().take(200).collect();
            let _ = log_write(
                &ctx.config_dir,
                "JSON_PARSE_ERROR",
                &format!("{} | {}", err, sample),
            );
            if let Some(id) = scrape_jsonrpc_id(line) {
                ctx.outbound
                    .send_error(id, -32700, format!("Parse error: {}", err))?;
            }
        }
    }
    Ok(())
}

fn spawn_hil_worker(
    ctx: &RouterCtx,
    rpc_id: Value,
    retell: String,
    relay_mcp_session_id: String,
    commands: Option<Vec<CommandItem>>,
    skills: Option<Vec<CommandItem>>,
) -> Result<()> {
    let state = Arc::new(CallState::default());
    {
        let mut pending = mutex_lock_or_recover(&ctx.pending);
        if pending.len() >= MAX_CONCURRENT_HIL {
            drop(pending);
            return ctx.outbound.send_error(
                rpc_id,
                -32603,
                format!(
                    "Relay: too many concurrent human feedback requests (max {})",
                    MAX_CONCURRENT_HIL
                ),
            );
        }
        pending.insert(rpc_id.clone(), Arc::clone(&state));
    }

    let outbound = ctx.outbound.clone();
    let pending_map = Arc::clone(&ctx.pending);
    let loop_index = Arc::clone(&ctx.loop_index);
    let config_dir = ctx.config_dir.clone();
    let hil = Arc::clone(&ctx.hil);
    let st = Arc::clone(&state);

    thread::spawn(move || {
        let result = hil.feedback_round(&retell, &relay_mcp_session_id, commands, skills);

        {
            let mut g = mutex_lock_or_recover(&pending_map);
            g.remove(&rpc_id);
        }

        if st
            .response_sent
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        match result {
            Ok(answer) => {
                if !answer.is_empty() {
                    *mutex_lock_or_recover(&loop_index) = 0;
                }
                let _ = log_write(
                    &config_dir,
                    "USER_REPLY",
                    &normalize_logged_user_reply(&answer),
                );
                let _ = respond_tool_result(&outbound, rpc_id, answer);
            }
            Err(e) => {
                let _ = outbound.send_error(rpc_id, -32603, format!("Relay GUI: {}", e));
            }
        }
    });

    Ok(())
}

fn handle_tool_call(ctx: &mut RouterCtx, msg: &Value) -> Result<()> {
    let name = msg
        .get("params")
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("");

    if name != TOOL_NAME {
        ctx.outbound.send_error(
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
        ctx.outbound.send_error(
            msg["id"].clone(),
            -32602,
            "retell is required (non-empty): this turn's assistant reply to the user",
        )?;
        return Ok(());
    }

    let rpc_id = msg["id"].clone();
    if read_mcp_paused() {
        let _ = log_write(
            &ctx.config_dir,
            "MCP_PAUSED_BLOCK",
            &retell.chars().take(200).collect::<String>(),
        );
        respond_tool_result(&ctx.outbound, rpc_id, MCP_PAUSED_TOOL_REPLY.to_string())?;
        return Ok(());
    }

    let relay_mcp_session_id =
        crate::session_id_from_tool_arg(arguments.get("relay_mcp_session_id"));

    let commands: Option<Vec<CommandItem>> = arguments
        .get("commands")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let skills: Option<Vec<CommandItem>> = arguments
        .get("skills")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let log_line = if relay_mcp_session_id.is_empty() {
        retell.clone()
    } else {
        format!("[session:{}] {}", relay_mcp_session_id, retell)
    };
    let _ = log_write(&ctx.config_dir, "AI_REQUEST", &log_line);

    let loop_idx = *mutex_lock_or_recover(&ctx.loop_index);
    let Some((rule, is_oneshot)) = auto_reply_peek(&ctx.config_dir, loop_idx) else {
        return spawn_hil_worker(ctx, rpc_id, retell, relay_mcp_session_id, commands, skills);
    };

    if is_oneshot {
        consume_oneshot(&ctx.config_dir)?;
    }
    let _ = log_write(&ctx.config_dir, "AUTO_REPLY", &rule.text);
    let auto_reply_result = json!({
        "relay_mcp_session_id": "",
        "human": rule.text,
        "cmd_skill_count": 0,
    })
    .to_string();
    respond_tool_result(&ctx.outbound, rpc_id, auto_reply_result)?;
    *mutex_lock_or_recover(&ctx.loop_index) = loop_idx.saturating_add(1);
    Ok(())
}

fn dispatch_message(ctx: &mut RouterCtx, msg: &Value) -> Result<()> {
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        return Ok(());
    };

    if msg.get("id").is_none() {
        if method == "notifications/cancelled" {
            process_cancel_notification(ctx, msg)?;
        }
        return Ok(());
    }

    match method {
        "initialize" => {
            ctx.outbound.send_result(
                msg["id"].clone(),
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "relay-mcp", "version": env!("CARGO_PKG_VERSION") }
                }),
            )?;
        }
        "ping" => {
            ctx.outbound.send_result(msg["id"].clone(), json!({}))?;
        }
        "tools/list" => {
            ctx.outbound.send_result(
                msg["id"].clone(),
                json!({
                    "tools": [
                        {
                            "name": TOOL_NAME,
                            "description": "Human-in-the-loop: opens Relay for your Answer. Returns JSON with relay_mcp_session_id, human, cmd_skill_count, and optional attachments [{kind, path, data_url?}]. Paths point at Relay's saved files; relay mcp may add data_url (base64) alongside path — RELAY_MCP_INLINE_MAX_KB only: unset/empty=default 512 KiB cap, <=0 or invalid=off, >0=max inline KiB per file (any kind, Relay attachment paths only). New tab (no session): always include commands and skills arrays filled with every item the IDE/host can expose; use [] only if truly none. If cmd_skill_count was 0 on the last reply, next call must repopulate both arrays the same way. With session id: commands/skills optional; when sent, merged (dedupe by id).",
                            "inputSchema": {
                                "type": "object",
                                "description": "retell required. New session: include commands and skills with full IDE-provided lists when possible; [] only when host exposes no items. After cmd_skill_count 0, next call must include both arrays repopulated the same way. With relay_mcp_session_id: pass it; commands/skills optional unless repopulating after zero.",
                                "properties": {
                                    "retell": {
                                        "type": "string",
                                        "description": "Required. This turn's full assistant reply to the user (verbatim)."
                                    },
                                    "relay_mcp_session_id": {
                                        "type": "string",
                                        "description": "Omit or empty string for a new tab; otherwise pass the id from the previous tool result. If the last cmd_skill_count was 0, next call must again send commands and skills (full lists when possible, else [] only if none exist)."
                                    },
                                    "commands": {
                                        "type": "array",
                                        "description": "New tab: required as a property every call—array must list every IDE command you can obtain for slash-completion; use [] only if the host truly provides none (do not send empty when data exists). With session: optional; if sent, merged into this tab (dedupe by id). Shape: [{name, id, category?, description?}].",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "name": { "type": "string" },
                                                "id": { "type": "string" },
                                                "category": { "type": "string" },
                                                "description": { "type": "string" }
                                            }
                                        }
                                    },
                                    "skills": {
                                        "type": "array",
                                        "description": "Same rules as commands: new tab must include the array populated with every IDE skill you can obtain; [] only if none exist. With session: optional; merge (dedupe by id) if sent.",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "name": { "type": "string" },
                                                "id": { "type": "string" },
                                                "category": { "type": "string" },
                                                "description": { "type": "string" }
                                            }
                                        }
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
            handle_tool_call(ctx, msg)?;
        }
        _ => {
            ctx.outbound.send_error(
                msg["id"].clone(),
                -32601,
                format!("Method not found: {}", method),
            )?;
        }
    }

    Ok(())
}

fn run_router_loop(mut ctx: RouterCtx, inbound: Receiver<String>) -> Result<()> {
    let mut disconnected = false;
    loop {
        while let Ok(line) = inbound.try_recv() {
            if line.trim().is_empty() {
                continue;
            }
            handle_json_line(&mut ctx, &line)?;
        }

        if disconnected {
            break;
        }

        match inbound.recv_timeout(Duration::from_millis(120)) {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }
                handle_json_line(&mut ctx, &line)?;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                disconnected = true;
            }
        }
    }
    Ok(())
}

fn run_feedback_server_with_hil(config_dir: PathBuf, hil: Arc<dyn HilFeedback>) -> Result<()> {
    let outbound = McpOutbound::spawn_stdout_writer();
    let ctx = RouterCtx {
        config_dir: config_dir.clone(),
        outbound,
        pending: Arc::new(Mutex::new(HashMap::new())),
        loop_index: Arc::new(Mutex::new(0)),
        hil,
    };

    let (tx, rx) = mpsc::channel::<String>();
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

    run_router_loop(ctx, rx)
}

#[cfg(test)]
fn run_feedback_server_testable(
    config_dir: PathBuf,
    hil: Arc<dyn HilFeedback>,
    outbound: McpOutbound,
    inbound: Receiver<String>,
) -> Result<()> {
    let ctx = RouterCtx {
        config_dir,
        outbound,
        pending: Arc::new(Mutex::new(HashMap::new())),
        loop_index: Arc::new(Mutex::new(0)),
        hil,
    };
    run_router_loop(ctx, inbound)
}

pub fn run_feedback_server() -> Result<()> {
    let config_dir = prepare_user_data_dir()?;
    run_feedback_server_with_hil(config_dir, Arc::new(McpHttpFeedback))
}

pub fn run_feedback_cli(
    retell: String,
    timeout_seconds: u64,
    relay_mcp_session_id: &str,
) -> Result<()> {
    let config_dir = prepare_user_data_dir()?;
    let _ = log_write(&config_dir, "CLI_REQUEST", &retell);
    let sid = relay_mcp_session_id.to_string();
    let retell_for_thread = retell.clone();
    let (tx, rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let r = mcp_http::feedback_round(&retell_for_thread, &sid, None, None);
        let _ = tx.send(r);
    });
    let wait = Duration::from_secs(timeout_seconds.max(1));
    match rx.recv_timeout(wait) {
        Ok(Ok(answer)) => {
            let _ = log_write(
                &config_dir,
                "CLI_REPLY",
                &normalize_logged_user_reply(&answer),
            );
            println!("{}", answer);
            Ok(())
        }
        Ok(Err(e)) => {
            let _ = log_write(&config_dir, "CLI_ERR", &e.to_string());
            Err(e)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let _ = log_write(&config_dir, "CLI_TIMEOUT", &retell);
            let secs = timeout_seconds.max(1);
            Err(anyhow::anyhow!("timed out after {} min", secs.div_ceil(60)))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(anyhow::anyhow!(
            "internal error: feedback thread disconnected"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scrape_jsonrpc_id_string() {
        let line = r#"{"jsonrpc":"2.0","method":"ping","id":"abc"}"#;
        assert_eq!(scrape_jsonrpc_id(line), Some(Value::String("abc".into())));
    }

    #[test]
    fn scrape_jsonrpc_id_number() {
        let line = r#"{"jsonrpc":"2.0","method":"ping","id": 42}"#;
        assert_eq!(scrape_jsonrpc_id(line), Some(Value::Number(42.into())));
    }

    #[test]
    fn scrape_jsonrpc_id_none_on_garbage() {
        assert_eq!(scrape_jsonrpc_id("not json"), None);
    }

    #[test]
    fn cancel_notification_request_id_matches() {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": { "requestId": 7 }
        });
        assert_eq!(cancel_notification_request_id(&msg), Some(json!(7)));
    }

    #[test]
    fn cancel_notification_request_id_alias() {
        let msg = json!({
            "method": "notifications/cancelled",
            "params": { "request_id": "x" }
        });
        assert_eq!(
            cancel_notification_request_id(&msg),
            Some(Value::String("x".into()))
        );
    }

    /// Blocks after signalling `started` until `unblock` receives one message.
    struct BlockingHil {
        started: std::sync::mpsc::SyncSender<()>,
        unblock: Mutex<Option<Receiver<()>>>,
    }

    impl HilFeedback for BlockingHil {
        fn feedback_round(
            &self,
            _retell: &str,
            _relay_mcp_session_id: &str,
            _commands: Option<Vec<CommandItem>>,
            _skills: Option<Vec<CommandItem>>,
        ) -> Result<String, String> {
            let _ = self.started.send(());
            let rx = self
                .unblock
                .lock()
                .unwrap()
                .take()
                .expect("unblock rx setup");
            rx.recv().map_err(|_| "unblock closed".to_string())?;
            Ok(r#"{"relay_mcp_session_id":"1","human":"ok","cmd_skill_count":0}"#.to_string())
        }
    }

    #[test]
    fn tools_list_succeeds_while_hil_worker_blocked() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = dir.path().to_path_buf();

        let (started_tx, started_rx) = mpsc::sync_channel(0);
        let (unblock_tx, unblock_rx) = mpsc::channel::<()>();

        let hil: Arc<dyn HilFeedback> = Arc::new(BlockingHil {
            started: started_tx,
            unblock: Mutex::new(Some(unblock_rx)),
        });

        let (in_tx, in_rx) = mpsc::channel::<String>();
        let (out_tx, out_rx) = mpsc::channel::<Value>();
        let outbound = McpOutbound::new_for_test(out_tx);

        let _server = thread::spawn(move || {
            run_feedback_server_testable(cfg, hil, outbound, in_rx).unwrap();
        });

        let call = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": TOOL_NAME,
                "arguments": { "retell": "hello" }
            }
        });
        in_tx.send(serde_json::to_string(&call).unwrap()).unwrap();

        started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("hil started");

        let list = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });
        in_tx.send(serde_json::to_string(&list).unwrap()).unwrap();

        let r_list = out_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("tools/list response");
        assert_eq!(r_list["id"], json!(2));
        assert!(r_list.get("result").is_some());
        let tools = r_list["result"]["tools"].as_array().expect("tools array");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], json!(TOOL_NAME));

        unblock_tx.send(()).unwrap();

        let r_call = out_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("tools/call response");
        assert_eq!(r_call["id"], json!(1));
        assert!(r_call.get("result").is_some());

        drop(in_tx);
        let _ = _server.join();
    }

    #[test]
    fn cancel_inflight_tools_call_returns_32800() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = dir.path().to_path_buf();

        let (started_tx, started_rx) = mpsc::sync_channel(0);
        let (unblock_tx, unblock_rx) = mpsc::channel::<()>();

        let hil: Arc<dyn HilFeedback> = Arc::new(BlockingHil {
            started: started_tx,
            unblock: Mutex::new(Some(unblock_rx)),
        });

        let (in_tx, in_rx) = mpsc::channel::<String>();
        let (out_tx, out_rx) = mpsc::channel::<Value>();
        let outbound = McpOutbound::new_for_test(out_tx);

        let hil_srv = Arc::clone(&hil);
        let _server = thread::spawn(move || {
            run_feedback_server_testable(cfg, hil_srv, outbound, in_rx).unwrap();
        });

        let call = json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call",
            "params": {
                "name": TOOL_NAME,
                "arguments": { "retell": "cancel-me" }
            }
        });
        in_tx.send(serde_json::to_string(&call).unwrap()).unwrap();

        started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("hil started");

        let cancel = json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": { "requestId": 9 }
        });
        in_tx.send(serde_json::to_string(&cancel).unwrap()).unwrap();

        let r = out_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("cancel response");
        assert_eq!(r["id"], json!(9));
        assert_eq!(r["error"]["code"], json!(JSONRPC_REQUEST_CANCELLED));

        let _ = unblock_tx.send(());
        drop(in_tx);
        let _ = _server.join();
    }

    struct ImmediateHil {
        out: std::sync::Mutex<Vec<String>>,
    }

    impl HilFeedback for ImmediateHil {
        fn feedback_round(
            &self,
            _retell: &str,
            _relay_mcp_session_id: &str,
            _commands: Option<Vec<CommandItem>>,
            _skills: Option<Vec<CommandItem>>,
        ) -> Result<String, String> {
            let idx = {
                let mut g = mutex_lock_or_recover(&self.out);
                g.push("a".into());
                g.len()
            };
            Ok(format!(
                r#"{{"relay_mcp_session_id":"{idx}","human":"h","cmd_skill_count":0}}"#
            ))
        }
    }

    #[test]
    fn two_concurrent_tools_call_both_complete() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = dir.path().to_path_buf();

        let hil: Arc<dyn HilFeedback> = Arc::new(ImmediateHil {
            out: std::sync::Mutex::new(vec![]),
        });

        let (in_tx, in_rx) = mpsc::channel::<String>();
        let (out_tx, out_rx) = mpsc::channel::<Value>();
        let outbound = McpOutbound::new_for_test(out_tx);

        let hil_srv = Arc::clone(&hil);
        let _server = thread::spawn(move || {
            run_feedback_server_testable(cfg, hil_srv, outbound, in_rx).unwrap();
        });

        for id in [1_u64, 2_u64] {
            let call = json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": TOOL_NAME,
                    "arguments": { "retell": format!("r{id}") }
                }
            });
            in_tx.send(serde_json::to_string(&call).unwrap()).unwrap();
        }

        let mut seen = std::collections::HashSet::new();
        for _ in 0..2 {
            let r = out_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("response");
            let id = r["id"].as_u64().expect("id u64");
            assert!(seen.insert(id));
            assert!(r.get("result").is_some());
        }
        assert_eq!(seen, [1, 2].into_iter().collect());

        drop(in_tx);
        let _ = _server.join();
    }
}
