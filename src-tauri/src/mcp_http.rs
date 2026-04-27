//! MCP process: discover GUI HTTP endpoint and call feedback API.
//! Tool-result bodies from `GET /v1/feedback/wait` pass through [`crate::mcp_wsl_paths::transform_tool_result_json_for_mcp_host`] here (WSL attachment paths when `relay mcp-<ide> --exe_in_wsl`).
//!
//! ## Timeouts
//! - `GET /v1/feedback/wait/:id` is **completed by the GUI** (submit, dismiss, supersede,
//!   or ~60 min idle via orphan cleanup in `gui_http`). The HTTP route itself has no short socket timeout.
//! - GUI idle orphan timing is **configurable** (minutes, Settings → Application); MCP still treats the result like dismiss when it fires.
//! - This module sets a **24 h** read timeout on that GET as a **transport failsafe** only.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::user_data_dir;

/// Per-IDE endpoint file name. Falls back to generic when no IDE is set.
pub fn gui_endpoint_file_name() -> String {
    if let Some(ide) = crate::ide::get_process_ide() {
        format!("gui_endpoint_{}.json", ide.cli_id())
    } else {
        "gui_endpoint.json".to_string()
    }
}

/// Remote SSH tunnel endpoint file: `gui_endpoint_<ide>_remote.json`.
pub fn gui_remote_endpoint_file_name() -> String {
    if let Some(ide) = crate::ide::get_process_ide() {
        format!("gui_endpoint_{}_remote.json", ide.cli_id())
    } else {
        "gui_endpoint_remote.json".to_string()
    }
}

/// IDE-specific routing lock file name.
fn gui_routing_lock_file_name(ide: &crate::ide::IdeKind) -> String {
    format!("gui_routing_lock_{}.json", ide.cli_id())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiEndpoint {
    pub port: u16,
    pub token: String,
    #[serde(default)]
    pub pid: Option<u32>,
    /// `true` when this endpoint was loaded from the `_remote.json` file.
    #[serde(default)]
    pub remote: bool,
}

/// Routing lock: determines which GUI the MCP prefers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiRoutingLock {
    /// `"remote"` or `"local"`.
    pub prefer: String,
    #[serde(default)]
    pub set_by: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    /// When true, only the same `set_by` origin can overwrite this lock.
    /// Local user sets pinned=true to prevent remote preemption.
    #[serde(default)]
    pub pinned: bool,
}

pub fn gui_endpoint_path() -> Result<PathBuf> {
    Ok(user_data_dir()?.join(gui_endpoint_file_name()))
}

/// Read the routing lock preference for the current IDE.
pub fn read_routing_lock() -> Option<GuiRoutingLock> {
    let ide = crate::ide::get_process_ide()?;
    let dir = user_data_dir().ok()?;
    let path = dir.join(gui_routing_lock_file_name(&ide));
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Write a routing lock for a specific IDE. Callable from GUI settings.
/// If an existing lock is `pinned` and was set by a different origin,
/// this write is rejected.
pub fn write_routing_lock(
    ide: crate::ide::IdeKind,
    prefer: &str,
    set_by: &str,
    pinned: bool,
) -> Result<()> {
    let dir = user_data_dir()?;
    let path = dir.join(gui_routing_lock_file_name(&ide));
    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(existing) = serde_json::from_str::<GuiRoutingLock>(&text) {
            if existing.pinned {
                let same_origin = existing.set_by.as_deref() == Some(set_by);
                if !same_origin {
                    anyhow::bail!(
                        "routing is pinned by {}",
                        existing.set_by.as_deref().unwrap_or("unknown")
                    );
                }
            }
        }
    }
    let lock = GuiRoutingLock {
        prefer: prefer.to_string(),
        set_by: Some(set_by.to_string()),
        timestamp: Some(crate::storage::timestamp_string()),
        pinned,
    };
    let json = serde_json::to_string_pretty(&lock)?;
    fs::write(&path, &json)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Remove the routing lock for a specific IDE.
pub fn clear_routing_lock(ide: crate::ide::IdeKind) -> Result<()> {
    let dir = user_data_dir()?;
    let path = dir.join(gui_routing_lock_file_name(&ide));
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// Try to parse and health-check an endpoint file, tagging it as local or remote.
fn try_read_healthy_endpoint(path: &PathBuf, is_remote: bool) -> Option<GuiEndpoint> {
    let text = fs::read_to_string(path).ok()?;
    let mut ep: GuiEndpoint = serde_json::from_str(&text).ok()?;
    ep.remote = is_remote;
    if health_ok(&ep) {
        Some(ep)
    } else {
        None
    }
}

pub fn read_gui_endpoint() -> Result<Option<GuiEndpoint>> {
    let dir = user_data_dir()?;
    let lock = read_routing_lock();
    let prefer_remote = lock.as_ref().map(|l| l.prefer == "remote").unwrap_or(false);

    let local_path = dir.join(gui_endpoint_file_name());
    let remote_path = dir.join(gui_remote_endpoint_file_name());

    if prefer_remote {
        if let Some(ep) = try_read_healthy_endpoint(&remote_path, true) {
            return Ok(Some(ep));
        }
        if let Some(ep) = try_read_healthy_endpoint(&local_path, false) {
            return Ok(Some(ep));
        }
    } else {
        if let Some(ep) = try_read_healthy_endpoint(&local_path, false) {
            return Ok(Some(ep));
        }
        if let Some(ep) = try_read_healthy_endpoint(&remote_path, true) {
            return Ok(Some(ep));
        }
    }
    Ok(None)
}

/// Check whether a GUI process for a specific IDE is alive.
/// First checks the PID-based marker file (instant, no network); falls back to HTTP health.
pub fn is_ide_gui_alive(ide: crate::ide::IdeKind) -> bool {
    if crate::is_gui_marker_alive_for_ide(ide) {
        return true;
    }
    let Ok(dir) = user_data_dir() else {
        return false;
    };
    let ep_path = dir.join(format!("gui_endpoint_{}.json", ide.cli_id()));
    let Ok(text) = fs::read_to_string(&ep_path) else {
        return false;
    };
    let Ok(ep) = serde_json::from_str::<GuiEndpoint>(&text) else {
        return false;
    };
    health_ok(&ep)
}

fn health_ok(ep: &GuiEndpoint) -> bool {
    let url = format!("http://127.0.0.1:{}/v1/health", ep.port);
    ureq::get(&url)
        .set("Authorization", &format!("Bearer {}", ep.token))
        .timeout(Duration::from_secs(2))
        .call()
        .map(|r| r.status() == 200)
        .unwrap_or(false)
}

/// Scan all IDE-specific endpoint files for a healthy GUI (used by `relay feedback`
/// which runs without a specific IDE mode).
fn find_any_healthy_gui_endpoint() -> Option<GuiEndpoint> {
    let dir = user_data_dir().ok()?;
    let ides = [
        crate::ide::IdeKind::Cursor,
        crate::ide::IdeKind::ClaudeCode,
        crate::ide::IdeKind::Windsurf,
        crate::ide::IdeKind::Other,
    ];

    let prefer_remote = |ide: &crate::ide::IdeKind| -> bool {
        let lock_path = dir.join(gui_routing_lock_file_name(ide));
        fs::read_to_string(lock_path)
            .ok()
            .and_then(|t| serde_json::from_str::<GuiRoutingLock>(&t).ok())
            .map(|l| l.prefer == "remote")
            .unwrap_or(false)
    };

    for ide in &ides {
        let local = dir.join(format!("gui_endpoint_{}.json", ide.cli_id()));
        let remote = dir.join(format!("gui_endpoint_{}_remote.json", ide.cli_id()));

        if prefer_remote(ide) {
            if let Some(ep) = try_read_healthy_endpoint(&remote, true) {
                return Some(ep);
            }
            if let Some(ep) = try_read_healthy_endpoint(&local, false) {
                return Some(ep);
            }
        } else {
            if let Some(ep) = try_read_healthy_endpoint(&local, false) {
                return Some(ep);
            }
            if let Some(ep) = try_read_healthy_endpoint(&remote, true) {
                return Some(ep);
            }
        }
    }

    let generic = dir.join("gui_endpoint.json");
    if let Some(ep) = try_read_healthy_endpoint(&generic, false) {
        return Some(ep);
    }
    let generic_remote = dir.join("gui_endpoint_remote.json");
    try_read_healthy_endpoint(&generic_remote, true)
}

fn spawn_gui() -> Result<()> {
    let exe = std::env::current_exe().context("current_exe")?;
    let ide = crate::ide::get_process_ide()
        .context("cannot spawn GUI: no IDE mode set for this MCP process")?;
    let gui_subcmd = format!("gui-{}", ide.cli_id());
    let mut cmd = Command::new(exe);
    cmd.arg(&gui_subcmd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().context("spawn relay gui-<ide>")?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Wait until GUI exposes a healthy HTTP endpoint (spawn GUI if needed).
/// When no IDE is set (e.g. `relay feedback`), scans all IDE endpoint files.
///
/// Routing priority: healthy local > healthy remote > spawn local.
/// This ensures: (a) user at A keeps using local GUI, (b) headless server A
/// with an SSH tunnel from B uses B's remote GUI without spawning a pointless
/// local process, (c) fresh install spawns local when nothing else is available.
pub fn ensure_gui_endpoint(max_wait: Duration) -> Result<GuiEndpoint> {
    let deadline = Instant::now() + max_wait;
    let has_ide = crate::ide::get_process_ide().is_some();
    let mut spawned = false;

    loop {
        if Instant::now() > deadline {
            anyhow::bail!("Relay GUI did not become ready within {:?}", max_wait);
        }

        // read_gui_endpoint / find_any_healthy_gui_endpoint already verify
        // health internally (local first, then remote).
        if has_ide {
            if let Some(ep) = read_gui_endpoint()? {
                return Ok(ep);
            }
        } else if let Some(ep) = find_any_healthy_gui_endpoint() {
            return Ok(ep);
        }

        // No healthy endpoint (local or remote) found — spawn local GUI.
        if !spawned && has_ide {
            let _ = spawn_gui();
            spawned = true;
        }

        thread::sleep(Duration::from_millis(120));
    }
}

/// Cross-platform hostname (no extra crate).
fn mcp_hostname() -> String {
    #[cfg(unix)]
    {
        let mut buf = [0u8; 256];
        if unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) } == 0 {
            let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
            return String::from_utf8_lossy(&buf[..end]).into_owned();
        }
    }
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_default()
}

/// POST feedback + block on wait until user answers (or JSON with human "" on dismiss/timeout).
pub fn feedback_round(
    retell: &str,
    relay_mcp_session_id: &str,
    commands: Option<&[crate::CommandItem]>,
    skills: Option<&[crate::CommandItem]>,
    title: Option<&str>,
) -> Result<String> {
    let ep = ensure_gui_endpoint(Duration::from_secs(45))?;

    let post_url = format!("http://127.0.0.1:{}/v1/feedback", ep.port);
    let mut body = serde_json::json!({
        "retell": retell,
        "relay_mcp_session_id": relay_mcp_session_id,
        "mcp_pid": std::process::id(),
        "mcp_hostname": mcp_hostname(),
        "mcp_origin": if ep.remote { "remote" } else { "local" },
    });
    if let Some(ide) = crate::ide::get_process_ide() {
        body["ide_mode"] = serde_json::json!(ide.cli_id());
    }
    if let Some(cmd_list) = commands {
        body["commands"] = serde_json::to_value(cmd_list).unwrap_or(serde_json::json!([]));
    }
    if let Some(skill_list) = skills {
        body["skills"] = serde_json::to_value(skill_list).unwrap_or(serde_json::json!([]));
    }
    if let Some(t) = title {
        body["title"] = serde_json::json!(t);
    }
    let resp = ureq::post(&post_url)
        .set("Authorization", &format!("Bearer {}", ep.token))
        .send_json(body)
        .map_err(|e| anyhow!("POST /v1/feedback: {}", e))?;

    if resp.status() >= 400 {
        anyhow::bail!("POST /v1/feedback: HTTP {}", resp.status());
    }

    let v: serde_json::Value = resp.into_json().context("feedback JSON")?;
    let rid = v
        .get("request_id")
        .and_then(|x| x.as_str())
        .context("request_id")?;

    let wait_url = format!("http://127.0.0.1:{}/v1/feedback/wait/{}", ep.port, rid);
    let ans = ureq::get(&wait_url)
        .set("Authorization", &format!("Bearer {}", ep.token))
        .timeout(Duration::from_secs(24 * 60 * 60)) // 24h; server wait has no timeout, client needs a cap to avoid true infinite
        .call()
        .map_err(|e| anyhow!("GET wait: {}", e))?
        .into_string()
        .unwrap_or_default();

    Ok(crate::mcp_wsl_paths::transform_tool_result_json_for_mcp_host(ans))
}
