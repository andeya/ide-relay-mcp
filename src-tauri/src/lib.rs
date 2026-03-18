use anyhow::{anyhow, Context, Result};
use chrono::Local;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

pub const APP_NAME: &str = "Relay MCP";
pub const APP_QUALIFIER: &str = "com";
pub const APP_ORGANIZATION: &str = "relay";
pub const APP_DATA_DIR: &str = "relay-mcp";
pub const TOOL_NAME: &str = "interactive_feedback";
pub const CONFIG_ONESHOT: &str = "auto_reply_oneshot.txt";
pub const CONFIG_LOOP: &str = "auto_reply_loop.txt";
pub const LOG_FILE: &str = "feedback_log.txt";
const DEFAULT_ONESHOT_TEMPLATE: &str =
    "# Relay MCP one-shot auto-reply rules\n# Format: timeout_seconds|reply_text\n";
const DEFAULT_LOOP_TEMPLATE: &str =
    "# Relay MCP loop auto-reply rules\n# Format: timeout_seconds|reply_text\n";

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoReplyRule {
    pub timeout_seconds: u64,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlStatus {
    Active,
    TimedOut,
    Cancelled,
}

impl ControlStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ControlStatus::Active => "active",
            ControlStatus::TimedOut => "timed_out",
            ControlStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim() {
            "active" => Some(ControlStatus::Active),
            "timed_out" => Some(ControlStatus::TimedOut),
            "cancelled" => Some(ControlStatus::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchState {
    pub summary: String,
    pub result_file: String,
    pub control_file: String,
    pub title: String,
}

#[derive(Debug)]
pub struct PendingRequest {
    pub id: Value,
    pub summary: String,
    pub result_file: PathBuf,
    pub control_file: PathBuf,
    pub created_at: Instant,
    pub child: Option<Child>,
    pub detached: bool,
}

#[derive(Debug)]
struct ServerState {
    binary_dir: PathBuf,
    config_dir: PathBuf,
    stdout: io::Stdout,
    active: Vec<PendingRequest>,
    detached: Vec<PendingRequest>,
    loop_index: usize,
}

impl ServerState {
    fn new(binary_dir: PathBuf, config_dir: PathBuf) -> Self {
        Self {
            binary_dir,
            config_dir,
            stdout: io::stdout(),
            active: Vec::new(),
            detached: Vec::new(),
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

pub fn legacy_config_path(binary_dir: &Path, file_name: &str) -> PathBuf {
    binary_dir.join(file_name)
}

pub fn gui_binary_name() -> &'static str {
    if cfg!(windows) {
        "relay-gui.exe"
    } else {
        "relay-gui"
    }
}

pub fn server_binary_name() -> &'static str {
    if cfg!(windows) {
        "relay-server.exe"
    } else {
        "relay-server"
    }
}

pub fn cli_binary_name() -> &'static str {
    if cfg!(windows) {
        "relay.exe"
    } else {
        "relay"
    }
}

pub fn gui_binary_path(exe_dir: &Path) -> PathBuf {
    exe_dir.join(gui_binary_name())
}

pub fn timestamp_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn log_write(exe_dir: &Path, source: &str, content: &str) -> Result<()> {
    let line = format!("[{}] [{}] {}\n", timestamp_string(), source, content);
    let path = exe_dir.join(LOG_FILE);
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
            return ControlStatus::from_str(value);
        }
    }
    None
}

pub fn write_control_status(path: &Path, status: ControlStatus) -> Result<()> {
    write_text_file(path, &format!("status={}\n", status.as_str()))
}

pub fn launch_gui(
    exe_dir: &Path,
    summary: &str,
    result_file: &Path,
    control_file: &Path,
) -> Result<Child> {
    let gui_path = gui_binary_path(exe_dir);
    if !gui_path.exists() {
        return Err(anyhow!(
            "missing required GUI binary: {}",
            gui_path.display()
        ));
    }
    let child = Command::new(&gui_path)
        .arg(summary)
        .arg(result_file)
        .arg(control_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to launch {}", gui_path.display()))?;
    Ok(child)
}

fn json_id_key(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
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

fn ensure_text_file(path: &Path, default_content: &str, legacy_path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    if legacy_path.exists() {
        fs::copy(legacy_path, path).with_context(|| {
            format!(
                "failed to migrate legacy config {} -> {}",
                legacy_path.display(),
                path.display()
            )
        })?;
        return Ok(());
    }

    fs::write(path, default_content)?;
    Ok(())
}

pub fn prepare_user_data_dir(binary_dir: &Path) -> Result<PathBuf> {
    let config_dir = user_data_dir()?;
    fs::create_dir_all(&config_dir)?;

    let oneshot = config_dir.join(CONFIG_ONESHOT);
    let loop_rules = config_dir.join(CONFIG_LOOP);
    let legacy_oneshot = legacy_config_path(binary_dir, CONFIG_ONESHOT);
    let legacy_loop = legacy_config_path(binary_dir, CONFIG_LOOP);

    ensure_text_file(&oneshot, DEFAULT_ONESHOT_TEMPLATE, &legacy_oneshot)?;
    ensure_text_file(&loop_rules, DEFAULT_LOOP_TEMPLATE, &legacy_loop)?;

    Ok(config_dir)
}

pub fn auto_reply_peek(exe_dir: &Path, loop_index: usize) -> Option<(AutoReplyRule, bool)> {
    let oneshot = load_auto_reply_rules(&exe_dir.join(CONFIG_ONESHOT));
    if let Some(rule) = oneshot.first().cloned() {
        return Some((rule, true));
    }

    let loop_rules = load_auto_reply_rules(&exe_dir.join(CONFIG_LOOP));
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

pub fn consume_oneshot(exe_dir: &Path) -> Result<()> {
    let path = exe_dir.join(CONFIG_ONESHOT);
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
    let payload = json!({
        "content": [
            {
                "type": "text",
                "text": json!({ "interactive_feedback": feedback }).to_string()
            }
        ]
    });
    state.send_result(id, payload)
}

fn handle_gui_exit(state: &mut ServerState, idx: usize) -> Result<()> {
    let mut request = state.active.remove(idx);
    let feedback = match read_trimmed_text(&request.result_file) {
        Ok(text) => text,
        Err(_) => String::new(),
    };
    if !feedback.is_empty() {
        state.loop_index = 0;
    }
    let _ = log_write(&state.config_dir, "USER_REPLY", &feedback);
    respond_tool_result(state, request.id.clone(), feedback)?;
    if let Some(mut child) = request.child.take() {
        let _ = child.wait();
    }
    let _ = fs::remove_file(&request.result_file);
    let _ = fs::remove_file(&request.control_file);
    Ok(())
}

fn process_active_children(state: &mut ServerState) -> Result<()> {
    let mut idx = 0;
    while idx < state.active.len() {
        let child_exited = if let Some(child) = state.active[idx].child.as_mut() {
            child.try_wait()?.is_some()
        } else {
            true
        };

        if child_exited {
            handle_gui_exit(state, idx)?;
        } else {
            idx += 1;
        }
    }

    let mut detached_idx = 0;
    while detached_idx < state.detached.len() {
        let child_exited = if let Some(child) = state.detached[detached_idx].child.as_mut() {
            child.try_wait()?.is_some()
        } else {
            true
        };

        if child_exited {
            let mut request = state.detached.remove(detached_idx);
            if let Some(mut child) = request.child.take() {
                let _ = child.wait();
            }
            let _ = fs::remove_file(&request.result_file);
            let _ = fs::remove_file(&request.control_file);
        } else {
            detached_idx += 1;
        }
    }
    Ok(())
}

fn create_request_paths() -> (PathBuf, PathBuf) {
    let result_file = make_temp_path("feedback_result", "txt");
    let control_file = make_temp_path("feedback_control", "txt");
    (result_file, control_file)
}

fn launch_pending_request(state: &mut ServerState, id: Value, summary: String) -> Result<()> {
    let (result_file, control_file) = create_request_paths();
    let child = launch_gui(&state.binary_dir, &summary, &result_file, &control_file)?;
    write_control_status(&control_file, ControlStatus::Active)?;

    state.active.push(PendingRequest {
        id,
        summary,
        result_file,
        control_file,
        created_at: Instant::now(),
        child: Some(child),
        detached: false,
    });
    Ok(())
}

fn timeout_front_request(state: &mut ServerState) -> Result<()> {
    let Some(front) = state.active.first() else {
        return Ok(());
    };

    let Some((rule, is_oneshot)) = auto_reply_peek(&state.config_dir, state.loop_index) else {
        return Ok(());
    };

    if front.created_at.elapsed() < Duration::from_secs(rule.timeout_seconds) {
        return Ok(());
    }

    let mut request = state.active.remove(0);
    write_control_status(&request.control_file, ControlStatus::TimedOut)?;
    if is_oneshot {
        consume_oneshot(&state.config_dir)?;
    }
    let _ = log_write(&state.config_dir, "AUTO_REPLY", &rule.text);
    respond_tool_result(state, request.id.clone(), rule.text.clone())?;
    if !rule.text.is_empty() {
        state.loop_index = state.loop_index.saturating_add(1);
    }

    request.detached = true;
    state.detached.push(request);

    Ok(())
}

fn cancel_request(state: &mut ServerState, request_id: &Value) -> Result<()> {
    let Some(idx) = state
        .active
        .iter()
        .position(|request| json_id_key(&request.id) == json_id_key(request_id))
    else {
        return Ok(());
    };

    let mut request = state.active.remove(idx);
    write_control_status(&request.control_file, ControlStatus::Cancelled)?;
    if let Some(child) = request.child.take() {
        state.detached.push(PendingRequest {
            id: request.id,
            summary: request.summary,
            result_file: request.result_file,
            control_file: request.control_file,
            created_at: request.created_at,
            child: Some(child),
            detached: true,
        });
    }
    Ok(())
}

fn handle_cancel_notification(state: &mut ServerState, msg: &Value) -> Result<()> {
    let Some(params) = msg.get("params") else {
        return Ok(());
    };
    let Some(request_id) = params.get("requestId") else {
        return Ok(());
    };
    cancel_request(state, request_id)
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

    let summary = msg
        .get("params")
        .and_then(|params| params.get("arguments"))
        .and_then(|arguments| arguments.get("summary"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let _ = log_write(&state.config_dir, "AI_REQUEST", &summary);

    let request_id = msg["id"].clone();
    let Some((rule, is_oneshot)) = auto_reply_peek(&state.config_dir, state.loop_index) else {
        launch_pending_request(state, request_id, summary)?;
        return Ok(());
    };

    if rule.timeout_seconds == 0 {
        if is_oneshot {
            consume_oneshot(&state.config_dir)?;
        }
        let _ = log_write(&state.config_dir, "AUTO_REPLY", &rule.text);
        respond_tool_result(state, request_id, rule.text)?;
        state.loop_index = state.loop_index.saturating_add(1);
        return Ok(());
    }

    launch_pending_request(state, request_id, summary)?;
    Ok(())
}

fn dispatch_message(state: &mut ServerState, msg: &Value) -> Result<()> {
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        return Ok(());
    };

    if !msg.get("id").is_some() {
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
                            "description": "Pause execution and request human feedback before proceeding.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "summary": {
                                        "type": "string",
                                        "description": "Concise summary of the work completed so far."
                                    }
                                },
                                "required": ["summary"]
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

fn cleanup_pending_requests(state: &mut ServerState) {
    for request in state.active.iter_mut().chain(state.detached.iter_mut()) {
        if let Some(mut child) = request.child.take() {
            if let Ok(None) = child.try_wait() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
        let _ = fs::remove_file(&request.result_file);
        let _ = fs::remove_file(&request.control_file);
    }
    state.active.clear();
    state.detached.clear();
}

pub fn run_feedback_server() -> Result<()> {
    let binary_dir = current_exe_dir()?;
    let config_dir = prepare_user_data_dir(&binary_dir)?;
    let mut state = ServerState::new(binary_dir, config_dir);
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

        process_active_children(&mut state)?;
        timeout_front_request(&mut state)?;

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

    cleanup_pending_requests(&mut state);
    Ok(())
}

pub fn run_feedback_cli(summary: String, timeout_seconds: u64) -> Result<()> {
    let binary_dir = current_exe_dir()?;
    let config_dir = prepare_user_data_dir(&binary_dir)?;
    let result_file = make_temp_path("feedback_direct_result", "txt");
    let control_file = make_temp_path("feedback_direct_control", "txt");

    let mut child = launch_gui(&binary_dir, &summary, &result_file, &control_file)?;
    write_control_status(&control_file, ControlStatus::Active)?;
    let _ = log_write(&config_dir, "CLI_REQUEST", &summary);

    let wait_for = Duration::from_secs(timeout_seconds.max(1));
    let started = Instant::now();

    loop {
        if let Some(status) = child.try_wait()? {
            let _ = status;
            let feedback = read_trimmed_text(&result_file).unwrap_or_default();
            let _ = log_write(&config_dir, "CLI_REPLY", &feedback);
            println!("{}", feedback);
            let _ = fs::remove_file(&result_file);
            let _ = fs::remove_file(&control_file);
            return Ok(());
        }

        if started.elapsed() >= wait_for {
            let _ = write_control_status(&control_file, ControlStatus::TimedOut);
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&result_file);
            let _ = fs::remove_file(&control_file);
            let _ = log_write(&config_dir, "CLI_TIMEOUT", &summary);
            println!();
            return Ok(());
        }

        thread::sleep(Duration::from_millis(100));
    }
}

pub fn launch_state_from_args(
    summary: String,
    result_file: PathBuf,
    control_file: PathBuf,
) -> LaunchState {
    LaunchState {
        summary,
        result_file: result_file.to_string_lossy().to_string(),
        control_file: control_file.to_string_lossy().to_string(),
        title: APP_NAME.to_string(),
    }
}

pub fn read_launch_args() -> Result<(String, PathBuf, PathBuf)> {
    let mut args = std::env::args().skip(1);
    let summary = args
        .next()
        .ok_or_else(|| anyhow!("missing summary argument"))?;
    let result_file = args
        .next()
        .ok_or_else(|| anyhow!("missing result file argument"))?;
    let control_file = args
        .next()
        .ok_or_else(|| anyhow!("missing control file argument"))?;
    Ok((
        summary,
        PathBuf::from(result_file),
        PathBuf::from(control_file),
    ))
}
