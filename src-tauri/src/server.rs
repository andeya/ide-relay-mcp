//! MCP JSON-RPC server: ServerState, tools/call handling, run_feedback_server / run_feedback_cli.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crate::auto_reply::{auto_reply_peek, consume_oneshot};
use crate::config::read_mcp_paused;
use crate::mcp_http;
use crate::storage::{log_write, prepare_user_data_dir};
use crate::{CommandItem, MCP_PAUSED_TOOL_REPLY, TOOL_NAME};

/// MCP / JSON-RPC: client cancelled an in-flight request (LSP-style code used in the wild).
const JSONRPC_REQUEST_CANCELLED: i64 = -32800;

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

fn notification_cancel_targets(msg: &Value, pending_tools_call_id: &Value) -> bool {
    if msg.get("id").is_some() {
        return false;
    }
    if msg.get("method").and_then(Value::as_str) != Some("notifications/cancelled") {
        return false;
    }
    let Some(params) = msg.get("params") else {
        return false;
    };
    let rid = params.get("requestId").or_else(|| params.get("request_id"));
    match rid {
        Some(v) => v == pending_tools_call_id,
        None => false,
    }
}

fn handle_json_line(
    state: &mut ServerState,
    line: &str,
    stdin_rx: &Receiver<String>,
) -> Result<()> {
    match serde_json::from_str::<Value>(line) {
        Ok(msg) => dispatch_message(state, &msg, stdin_rx)?,
        Err(err) => {
            let sample: String = line.chars().take(200).collect();
            let _ = log_write(
                &state.config_dir,
                "JSON_PARSE_ERROR",
                &format!("{} | {}", err, sample),
            );
            if let Some(id) = scrape_jsonrpc_id(line) {
                state.send_error(id, -32700, format!("Parse error: {}", err))?;
            }
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

/// Log when there is no in-flight `tools/call` to attach this notification to.
fn handle_cancel_notification(state: &mut ServerState, msg: &Value) -> Result<()> {
    let sample: String = msg.to_string().chars().take(320).collect();
    let _ = log_write(&state.config_dir, "MCP_CANCELLED_ORPHAN", &sample);
    Ok(())
}

fn wait_feedback_round(
    state: &mut ServerState,
    rpc_id: Value,
    retell: String,
    relay_mcp_session_id: String,
    commands: Option<Vec<CommandItem>>,
    skills: Option<Vec<CommandItem>>,
    stdin_rx: &Receiver<String>,
) -> Result<()> {
    let (tool_tx, tool_rx) = mpsc::channel::<Result<String, String>>();
    let retell_t = retell.clone();
    let sid_t = relay_mcp_session_id.clone();
    thread::spawn(move || {
        let r = mcp_http::feedback_round(&retell_t, &sid_t, commands.as_deref(), skills.as_deref());
        let _ = tool_tx.send(r.map_err(|e| e.to_string()));
    });

    loop {
        match tool_rx.recv_timeout(Duration::from_millis(120)) {
            Ok(Ok(answer)) => {
                if !answer.is_empty() {
                    state.loop_index = 0;
                }
                let _ = log_write(&state.config_dir, "USER_REPLY", &answer);
                respond_tool_result(state, rpc_id, answer)?;
                return Ok(());
            }
            Ok(Err(e)) => {
                state.send_error(rpc_id, -32603, format!("Relay GUI: {}", e))?;
                return Ok(());
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                state.send_error(
                    rpc_id,
                    -32603,
                    "Relay GUI: internal error (feedback thread disconnected)",
                )?;
                return Ok(());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                while let Ok(line) = stdin_rx.try_recv() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<Value>(&line) {
                        Ok(msg) => {
                            if notification_cancel_targets(&msg, &rpc_id) {
                                let _ = log_write(&state.config_dir, "MCP_CANCELLED", "");
                                state.send_error(
                                    rpc_id.clone(),
                                    JSONRPC_REQUEST_CANCELLED,
                                    "Request cancelled (notifications/cancelled)",
                                )?;
                                return Ok(());
                            }
                            if msg.get("id").is_some() {
                                let mid = msg["id"].clone();
                                state.send_error(
                                    mid,
                                    -32603,
                                    "Relay: a tools/call is already waiting for human feedback; wait for the Answer or cancel that request",
                                )?;
                            }
                        }
                        Err(err) => {
                            let sample: String = line.chars().take(200).collect();
                            let _ = log_write(
                                &state.config_dir,
                                "JSON_PARSE_ERROR",
                                &format!("{} | {}", err, sample),
                            );
                            if let Some(id) = scrape_jsonrpc_id(&line) {
                                state.send_error(id, -32700, format!("Parse error: {}", err))?;
                            }
                        }
                    }
                }
            }
        }
    }
}

fn handle_tool_call(
    state: &mut ServerState,
    msg: &Value,
    stdin_rx: &Receiver<String>,
) -> Result<()> {
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

    let relay_mcp_session_id = arguments
        .get("relay_mcp_session_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

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
    let _ = log_write(&state.config_dir, "AI_REQUEST", &log_line);

    let Some((rule, is_oneshot)) = auto_reply_peek(&state.config_dir, state.loop_index) else {
        return wait_feedback_round(
            state,
            rpc_id,
            retell,
            relay_mcp_session_id,
            commands,
            skills,
            stdin_rx,
        );
    };

    if is_oneshot {
        consume_oneshot(&state.config_dir)?;
    }
    let _ = log_write(&state.config_dir, "AUTO_REPLY", &rule.text);
    // Return same JSON shape as GUI feedback so the agent can parse relay_mcp_session_id and human.
    let auto_reply_result = json!({
        "relay_mcp_session_id": "",
        "human": rule.text,
        "cmd_skill_count": 0
    })
    .to_string();
    respond_tool_result(state, rpc_id, auto_reply_result)?;
    state.loop_index = state.loop_index.saturating_add(1);
    Ok(())
}

fn dispatch_message(
    state: &mut ServerState,
    msg: &Value,
    stdin_rx: &Receiver<String>,
) -> Result<()> {
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
                            "description": "Human-in-the-loop: opens Relay for your Answer. Returns JSON with relay_mcp_session_id, human, and cmd_skill_count. New tab (no session): always include commands and skills arrays filled with every item the IDE/host can expose; use [] only if truly none. If cmd_skill_count was 0 on the last reply, next call must repopulate both arrays the same way. With session id: commands/skills optional; when sent, merged (dedupe by id).",
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
            handle_tool_call(state, msg, stdin_rx)?;
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
            handle_json_line(&mut state, &line, &rx)?;
        }

        if disconnected {
            break;
        }

        match rx.recv_timeout(Duration::from_millis(120)) {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }
                handle_json_line(&mut state, &line, &rx)?;
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
    fn notification_cancel_matches() {
        let pending = json!(7);
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": { "requestId": 7 }
        });
        assert!(notification_cancel_targets(&msg, &pending));
    }

    #[test]
    fn notification_cancel_request_id_alias() {
        let pending = json!("x");
        let msg = json!({
            "method": "notifications/cancelled",
            "params": { "request_id": "x" }
        });
        assert!(notification_cancel_targets(&msg, &pending));
    }

    #[test]
    fn notification_cancel_wrong_id() {
        let pending = json!(1);
        let msg = json!({
            "method": "notifications/cancelled",
            "params": { "requestId": 2 }
        });
        assert!(!notification_cancel_targets(&msg, &pending));
    }
}
