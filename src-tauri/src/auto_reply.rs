//! Auto-reply rules: oneshot and loop; load, peek, consume.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

pub const CONFIG_ONESHOT: &str = "auto_reply_oneshot.txt";
pub const CONFIG_LOOP: &str = "auto_reply_loop.txt";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoReplyRule {
    pub timeout_seconds: u64,
    pub text: String,
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
    path: std::path::PathBuf,
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
