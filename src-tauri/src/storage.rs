//! Log, temp files, attachments, cache, retention, GUI presence marker.

use anyhow::{anyhow, bail, Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use crate::{gui_alive_marker_name, user_data_dir, ControlStatus, QaAttachmentRef, LOG_FILE};

/// One JSON line per completed in-app round; complements `feedback_log.txt` when log pairing skips rows.
const QA_ARCHIVE_DIR: &str = "qa_archive";

/// One MCP feedback line from `feedback_log.txt`: `[timestamp] [SOURCE] content`.
fn parse_feedback_log_line(line: &str) -> Option<(&str, &str)> {
    let line = line.trim_end();
    if line.is_empty() {
        return None;
    }
    let line = line.strip_prefix('[')?;
    let (_ts, rest) = line.split_once(']')?;
    let rest = rest.trim_start().strip_prefix('[')?;
    let (source, rest) = rest.split_once(']')?;
    // `log_write(..., "MCP_CANCELLED", "")` yields `] ` with no space before newline — allow empty tail.
    let content = match rest.strip_prefix(' ') {
        Some(s) => s.trim_start(),
        None => rest.trim_start(),
    };
    Some((source, content))
}

/// Split `AI_REQUEST` body into optional `[session:…]` and retell text.
fn parse_ai_request_content(content: &str) -> (Option<String>, String) {
    let c = content.trim_start();
    if let Some(rest) = c.strip_prefix("[session:") {
        if let Some((id, tail)) = rest.split_once(']') {
            let retell = tail.trim_start().to_string();
            return (Some(id.trim().to_string()), retell);
        }
    }
    (None, content.to_string())
}

/// Parsed MCP human-in-the-loop stream from `feedback_log.txt` (paired FIFO queues).
#[derive(Debug, Clone, Default)]
pub struct McpFeedbackLogParse {
    /// Completed (session_id, retell, user_reply). `session_id` is None when the first
    /// `AI_REQUEST` had no `[session:…]` prefix (cannot be merged into a specific tab by id).
    pub completed: Vec<(Option<String>, String, String)>,
    /// Unclosed `AI_REQUEST` at EOF (no `USER_REPLY` / `AUTO_REPLY` yet).
    pub pending: Option<(Option<String>, String)>,
}

/// Walk `feedback_log.txt` and pair `AI_REQUEST` with `USER_REPLY` / `AUTO_REPLY`, with a
/// separate FIFO for MCP vs `CLI_REQUEST` so terminal runs do not steal MCP replies.
pub fn parse_feedback_log_mcp(config_dir: &Path) -> Result<McpFeedbackLogParse> {
    let path = config_dir.join(LOG_FILE);
    let text = if path.is_file() {
        read_text_file(&path)?
    } else {
        String::new()
    };

    use std::collections::VecDeque;
    let mut mcp_q: VecDeque<(Option<String>, String)> = VecDeque::new();
    let mut cli_q: VecDeque<(Option<String>, String)> = VecDeque::new();
    let mut completed: Vec<(Option<String>, String, String)> = Vec::new();

    for line in text.lines() {
        let Some((source, content)) = parse_feedback_log_line(line) else {
            continue;
        };
        match source {
            "AI_REQUEST" => {
                let (sid, retell) = parse_ai_request_content(content);
                mcp_q.push_back((sid, retell));
            }
            "CLI_REQUEST" => {
                cli_q.push_back((None, content.to_string()));
            }
            "USER_REPLY" => {
                if let Some((sid, retell)) = mcp_q.pop_front() {
                    completed.push((sid, retell, content.to_string()));
                }
            }
            "CLI_REPLY" => {
                if let Some((sid, retell)) = cli_q.pop_front() {
                    completed.push((sid, retell, content.to_string()));
                }
            }
            "AUTO_REPLY" => {
                if let Some((sid, retell)) = mcp_q.pop_front() {
                    completed.push((sid, retell, content.to_string()));
                }
            }
            "MCP_CANCELLED" => {
                let _ = mcp_q.pop_front();
            }
            _ => {}
        }
    }

    let pending = mcp_q.pop_front();
    Ok(McpFeedbackLogParse { completed, pending })
}

/// Normalize an **incoming** Answer string before appending `USER_REPLY` / `CLI_REPLY` to the log
/// (MCP may pass through the full JSON body from `GET /v1/feedback/wait` in edge cases).
pub fn normalize_logged_user_reply(reply: &str) -> String {
    let t = reply.trim();
    if !t.starts_with('{') {
        return t.to_string();
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(t) else {
        return t.to_string();
    };
    v.get("human")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| t.to_string())
}

/// True when `reply` looks like the GUI wait JSON (legacy log lines). Hydration skips these pairs.
pub fn is_feedback_tool_result_blob(reply: &str) -> bool {
    let t = reply.trim();
    if !t.starts_with('{') {
        return false;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(t) else {
        return false;
    };
    let has_human = v.get("human").is_some();
    let has_sid = v.get("relay_mcp_session_id").is_some();
    let has_cc = v.get("cmd_skill_count").is_some();
    has_human && (has_sid || has_cc)
}

/// Completed (retell, reply) pairs for a given `relay_mcp_session_id` from the log.
pub fn feedback_log_pairs_for_session(
    parse: &McpFeedbackLogParse,
    session_id: &str,
) -> Vec<(String, String)> {
    parse
        .completed
        .iter()
        .filter(|(sid, _, _)| sid.as_deref() == Some(session_id))
        .filter(|(_, _, reply)| !is_feedback_tool_result_blob(reply))
        .map(|(_, r, reply)| (r.clone(), reply.trim().to_string()))
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QaArchiveLine {
    retell: String,
    reply: String,
    #[serde(default)]
    skipped: bool,
    #[serde(default)]
    attachments: Vec<QaAttachmentRef>,
    #[serde(default)]
    retell_at: String,
    #[serde(default)]
    reply_at: String,
}

fn qa_archive_file_path(config_dir: &Path, session_id: &str) -> Option<PathBuf> {
    let t = session_id.trim();
    if t.is_empty()
        || !t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    Some(config_dir.join(QA_ARCHIVE_DIR).join(format!("{t}.jsonl")))
}

/// Append one completed round for hydrate fallback (plain JSON lines, easy to trim or delete per session).
#[allow(clippy::too_many_arguments)]
pub fn qa_archive_append(
    config_dir: &Path,
    session_id: &str,
    retell: &str,
    reply: &str,
    skipped: bool,
    attachments: &[QaAttachmentRef],
    retell_at: &str,
    reply_at: &str,
) -> Result<()> {
    let Some(path) = qa_archive_file_path(config_dir, session_id) else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("qa_archive mkdir")?;
    }
    let row = QaArchiveLine {
        retell: retell.to_string(),
        reply: reply.to_string(),
        skipped,
        attachments: attachments.to_vec(),
        retell_at: retell_at.to_string(),
        reply_at: reply_at.to_string(),
    };
    let mut line = serde_json::to_string(&row).context("qa_archive json")?;
    line.push('\n');
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .context("qa_archive open")?;
    file.write_all(line.as_bytes())?;
    file.flush().context("qa_archive flush")?;
    Ok(())
}

/// Read archived rounds in order; malformed lines are skipped.
/// Returns (retell, reply, skipped, attachments, retell_at, reply_at).
pub fn qa_archive_load_session(
    config_dir: &Path,
    session_id: &str,
) -> Vec<(String, String, bool, Vec<QaAttachmentRef>, String, String)> {
    let Some(path) = qa_archive_file_path(config_dir, session_id) else {
        return vec![];
    };
    let Ok(text) = read_text_file(&path) else {
        return vec![];
    };
    let mut out = Vec::new();
    for ln in text.lines() {
        let t = ln.trim();
        if t.is_empty() {
            continue;
        }
        if let Ok(row) = serde_json::from_str::<QaArchiveLine>(t) {
            out.push((
                row.retell,
                row.reply,
                row.skipped,
                row.attachments,
                row.retell_at,
                row.reply_at,
            ));
        }
    }
    out
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn timestamp_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn next_temp_suffix() -> String {
    let pid = std::process::id();
    let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_nanos();
    format!("{}_{}_{}", pid, nanos, seq)
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

/// Save feedback image under user data (`feedback_attachments/`).
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

/// Resolve a user-supplied path string to a canonical file under `feedback_attachments/`.
fn resolve_feedback_attachment_file(path: &str) -> Result<PathBuf> {
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
    Ok(canon)
}

/// Best-effort canonical path for a saved feedback attachment (same resolution as read APIs).
pub fn canonical_feedback_attachment_path(path: &str) -> Option<PathBuf> {
    resolve_feedback_attachment_file(path).ok()
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
    config_dir.join(gui_alive_marker_name())
}

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
    write!(f, "{}", std::process::id())?;
    f.flush()?;
    Ok(())
}

/// Read the PID from a per-IDE alive marker file and check if that process is still running.
pub fn is_gui_marker_alive_for_ide(ide: crate::ide::IdeKind) -> bool {
    let dir = match user_data_dir() {
        Ok(d) => d,
        Err(_) => return false,
    };
    let marker = dir.join(format!("relay_gui_{}_alive.marker", ide.cli_id()));
    let Ok(text) = fs::read_to_string(&marker) else {
        return false;
    };
    let Ok(pid) = text.trim().parse::<u32>() else {
        return false;
    };
    if pid == 0 {
        return false;
    }
    process_is_running(pid)
}

fn process_is_running(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        Path::new(&format!("/proc/{}", pid)).exists()
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                false
            } else {
                CloseHandle(h);
                true
            }
        }
    }
}

pub fn remove_gui_presence_marker() {
    if let Ok(dir) = user_data_dir() {
        let _ = fs::remove_file(gui_alive_marker_path(&dir));
    }
}

pub fn new_tab_id() -> String {
    format!("t_{}", next_temp_suffix())
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
    /// `qa_archive/*.jsonl` (hydrate fallback); cleared together with `feedback_log.txt`.
    pub qa_archive_bytes: u64,
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

fn is_qa_archive_jsonl(name: &OsStr) -> bool {
    Path::new(name)
        .extension()
        .and_then(|x| x.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
}

fn qa_archive_jsonl_bytes(path: &Path) -> std::io::Result<u64> {
    if !path.is_dir() {
        return Ok(0);
    }
    let mut n = 0u64;
    for e in fs::read_dir(path)? {
        let e = e?;
        if !e.file_type()?.is_file() {
            continue;
        }
        if !is_qa_archive_jsonl(&e.file_name()) {
            continue;
        }
        n += e.metadata()?.len();
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
    let qa_dir = base.join(QA_ARCHIVE_DIR);
    let qa_archive_bytes = qa_archive_jsonl_bytes(&qa_dir)?;
    Ok(RelayCacheStats {
        attachments_bytes,
        log_bytes,
        qa_archive_bytes,
        data_dir: base.display().to_string(),
    })
}

fn clear_qa_archive_files(base: &Path) -> Result<()> {
    let d = base.join(QA_ARCHIVE_DIR);
    if !d.is_dir() {
        return Ok(());
    }
    for e in fs::read_dir(&d)? {
        let e = e?;
        if !e.file_type()?.is_file() {
            continue;
        }
        if !is_qa_archive_jsonl(&e.file_name()) {
            continue;
        }
        let p = e.path();
        fs::remove_file(&p).with_context(|| format!("remove {}", p.display()))?;
    }
    Ok(())
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
    clear_qa_archive_files(&base).context("clear qa_archive")?;
    Ok(())
}

const ATTACHMENT_RETENTION_FILE: &str = "attachment_retention.json";
pub const DEFAULT_ATTACHMENT_RETENTION_DAYS: u32 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AttachmentRetentionConfig {
    #[serde(default)]
    pub days: Option<u32>,
}

fn attachment_retention_path() -> Result<PathBuf> {
    Ok(prepare_user_data_dir()?.join(ATTACHMENT_RETENTION_FILE))
}

pub fn parse_stored_attachment_retention_json(s: &str) -> Option<u32> {
    let Ok(c) = serde_json::from_str::<AttachmentRetentionConfig>(s) else {
        return Some(DEFAULT_ATTACHMENT_RETENTION_DAYS);
    };
    match c.days {
        None => None,
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

/// Remove `qa_archive/*.jsonl` whose modification time is older than `days` (same semantics as
/// attachment retention). Does not modify `feedback_log.txt` (append-only; use manual clear).
pub fn purge_qa_archive_older_than_days(days: u32) -> Result<u64> {
    if days == 0 {
        return Ok(0);
    }
    let base = prepare_user_data_dir()?;
    let dir = base.join(QA_ARCHIVE_DIR);
    if !dir.is_dir() {
        return Ok(0);
    }
    let cutoff =
        SystemTime::now() - std::time::Duration::from_secs(u64::from(days).saturating_mul(86400));
    let mut removed: u64 = 0;
    for e in fs::read_dir(&dir)? {
        let e = e?;
        if !e.file_type()?.is_file() {
            continue;
        }
        if !is_qa_archive_jsonl(&e.file_name()) {
            continue;
        }
        let meta = e.metadata()?;
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if modified < cutoff {
            removed = removed.saturating_add(meta.len());
            let p = e.path();
            let _ = fs::remove_file(&p);
        }
    }
    Ok(removed)
}

/// Attachment size freed + best-effort `qa_archive` purge (qa I/O errors are debug-printed only).
pub fn purge_attachment_retention_bundled(days: u32) -> Result<u64> {
    let freed = purge_attachments_older_than_days(days)?;
    if let Err(e) = purge_qa_archive_older_than_days(days) {
        #[cfg(debug_assertions)]
        eprintln!("relay: purge_qa_archive_older_than_days: {e:#}");
        #[cfg(not(debug_assertions))]
        let _ = e;
    }
    Ok(freed)
}

pub fn purge_attachments_older_than_days(days: u32) -> Result<u64> {
    if days == 0 {
        return Ok(0);
    }
    let dir = prepare_user_data_dir()?.join("feedback_attachments");
    if !dir.is_dir() {
        return Ok(0);
    }
    let cutoff =
        SystemTime::now() - std::time::Duration::from_secs(u64::from(days).saturating_mul(86400));
    let mut freed: u64 = 0;
    for e in fs::read_dir(&dir)? {
        let e = e?;
        if !e.file_type()?.is_file() {
            continue;
        }
        let file_name = e.file_name();
        let name = file_name.to_string_lossy();
        if !name.starts_with("relay_attach_") {
            continue;
        }
        let meta = e.metadata()?;
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
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
    purge_attachment_retention_bundled(d)
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

#[cfg(test)]
mod feedback_log_parse_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn pairs_session_and_user() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(LOG_FILE);
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(f, "[2025-01-01 00:00:00] [AI_REQUEST] [session:123] hello").unwrap();
        writeln!(f, "[2025-01-01 00:00:01] [USER_REPLY] world").unwrap();
        drop(f);

        let parse = parse_feedback_log_mcp(dir.path()).unwrap();
        assert_eq!(parse.completed.len(), 1);
        assert_eq!(parse.completed[0].0, Some("123".into()));
        assert_eq!(parse.completed[0].1, "hello");
        assert_eq!(parse.completed[0].2, "world");
        assert!(parse.pending.is_none());

        let pairs = feedback_log_pairs_for_session(&parse, "123");
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "hello");
        assert_eq!(pairs[0].1, "world");
    }

    #[test]
    fn mcp_cancelled_drops_pending_ai() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(LOG_FILE);
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(f, "[t] [AI_REQUEST] [session:1] a").unwrap();
        writeln!(f, "[t] [MCP_CANCELLED] ").unwrap();
        writeln!(f, "[t] [AI_REQUEST] [session:1] b").unwrap();
        writeln!(f, "[t] [USER_REPLY] ok").unwrap();
        drop(f);

        let parse = parse_feedback_log_mcp(dir.path()).unwrap();
        assert_eq!(parse.completed.len(), 1);
        assert_eq!(parse.completed[0].1, "b");
        assert_eq!(parse.completed[0].2, "ok");
    }

    #[test]
    fn cli_does_not_consume_mcp_user_reply() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(LOG_FILE);
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(f, "[t] [AI_REQUEST] [session:1] m").unwrap();
        writeln!(f, "[t] [CLI_REQUEST] c").unwrap();
        writeln!(f, "[t] [USER_REPLY] um").unwrap();
        writeln!(f, "[t] [CLI_REPLY] uc").unwrap();
        drop(f);

        let parse = parse_feedback_log_mcp(dir.path()).unwrap();
        assert_eq!(parse.completed.len(), 2);
        assert_eq!(parse.completed[0].1, "m");
        assert_eq!(parse.completed[0].2, "um");
        assert_eq!(parse.completed[1].1, "c");
        assert_eq!(parse.completed[1].2, "uc");
    }

    #[test]
    fn normalize_logged_user_reply_extracts_human_json() {
        let j = r#"{"cmd_skill_count":0,"human":"hi\nline","relay_mcp_session_id":"1"}"#;
        assert_eq!(normalize_logged_user_reply(j), "hi\nline");
        assert_eq!(normalize_logged_user_reply("plain text"), "plain text");
    }

    #[test]
    fn feedback_log_pairs_skip_tool_result_json_lines() {
        let parse = McpFeedbackLogParse {
            completed: vec![(
                Some("1".into()),
                "ai".into(),
                r#"{"human":"answer","relay_mcp_session_id":"1","cmd_skill_count":0}"#.into(),
            )],
            pending: None,
        };
        let pairs = feedback_log_pairs_for_session(&parse, "1");
        assert!(pairs.is_empty());
    }

    #[test]
    fn feedback_log_pairs_keep_plain_replies() {
        let parse = McpFeedbackLogParse {
            completed: vec![(Some("1".into()), "ai".into(), "plain".into())],
            pending: None,
        };
        let pairs = feedback_log_pairs_for_session(&parse, "1");
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "ai");
        assert_eq!(pairs[0].1, "plain");
    }

    #[test]
    fn qa_archive_append_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        qa_archive_append(
            dir.path(),
            "42",
            "r1",
            "a1",
            false,
            &[],
            "2025-01-01 10:00:00",
            "2025-01-01 10:01:00",
        )
        .unwrap();
        qa_archive_append(
            dir.path(),
            "42",
            "r2",
            "",
            true,
            &[],
            "2025-01-01 10:02:00",
            "",
        )
        .unwrap();
        let rows = qa_archive_load_session(dir.path(), "42");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "r1");
        assert_eq!(rows[0].1, "a1");
        assert!(!rows[0].2);
        assert_eq!(rows[0].4, "2025-01-01 10:00:00");
        assert_eq!(rows[0].5, "2025-01-01 10:01:00");
        assert_eq!(rows[1].0, "r2");
        assert!(rows[1].2);
    }
}
