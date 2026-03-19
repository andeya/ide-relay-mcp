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
        match mcp_http::feedback_round(
            &retell,
            &relay_mcp_session_id,
            commands.as_deref(),
            skills.as_deref(),
        ) {
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
    // Return same JSON shape as GUI feedback so the agent can parse relay_mcp_session_id and human.
    let auto_reply_result = json!({
        "relay_mcp_session_id": "",
        "human": rule.text
    })
    .to_string();
    respond_tool_result(state, rpc_id, auto_reply_result)?;
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
                            "description": "Human-in-the-loop: opens Relay for your Answer. Returns JSON with relay_mcp_session_id and human. No session id: pass both commands and skills (either may be empty). With session id: pass it each time; commands and skills are optional and merge into the tab lists (dedupe by id).",
                            "inputSchema": {
                                "type": "object",
                                "description": "retell required. Without relay_mcp_session_id: pass commands and skills. With relay_mcp_session_id: pass it; optionally pass commands and/or skills to append to that tab's lists (same id skipped, dedupe by id).",
                                "properties": {
                                    "retell": {
                                        "type": "string",
                                        "description": "Required. This turn's full assistant reply to the user (verbatim)."
                                    },
                                    "relay_mcp_session_id": {
                                        "type": "string",
                                        "description": "Optional on first call; required on subsequent calls. Returned in JSON as {\"relay_mcp_session_id\":\"<ms>\",\"human\":\"...\"}. Remember it and pass it on the next request."
                                    },
                                    "commands": {
                                        "type": "array",
                                        "description": "When relay_mcp_session_id is empty: required — pass all available IDE commands (or []). When relay_mcp_session_id is set: optional; if sent, merged into existing commands for that tab (dedupe by id). Shape: [{name, id, category?, description?}].",
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
                                        "description": "When relay_mcp_session_id is empty: required — pass all available IDE skills (or []). When relay_mcp_session_id is set: optional; if sent, merged into existing skills for that tab (dedupe by id). Same shape as commands.",
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
