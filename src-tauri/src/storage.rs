//! Log, temp files, attachments, cache, retention, GUI presence marker.

use anyhow::{anyhow, bail, Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use crate::{user_data_dir, ControlStatus, GUI_ALIVE_MARKER, LOG_FILE};

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

/// Read a saved feedback image as data URL.
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
    purge_attachments_older_than_days(d)
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
