//! MCP process: discover GUI HTTP endpoint and call feedback API.
//! Tool-result bodies from `GET /v1/feedback/wait` pass through [`crate::mcp_wsl_paths::transform_tool_result_json_for_mcp_host`] here (WSL attachment paths when enabled).
//!
//! ## Timeouts
//! - `GET /v1/feedback/wait/:id` is **completed by the GUI** (submit, dismiss, supersede,
//!   or ~60 min idle via orphan cleanup in `gui_http`). The HTTP route itself has no short socket timeout.
//! - This module sets a **24 h** read timeout on that GET as a **transport failsafe** only.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::user_data_dir;

pub const GUI_ENDPOINT_FILE: &str = "gui_endpoint.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiEndpoint {
    pub port: u16,
    pub token: String,
    #[serde(default)]
    pub pid: Option<u32>,
}

pub fn gui_endpoint_path() -> Result<PathBuf> {
    Ok(user_data_dir()?.join(GUI_ENDPOINT_FILE))
}

pub fn read_gui_endpoint() -> Result<Option<GuiEndpoint>> {
    let p = gui_endpoint_path()?;
    let Ok(text) = fs::read_to_string(&p) else {
        return Ok(None);
    };
    Ok(serde_json::from_str(&text).ok())
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

fn spawn_gui() -> Result<()> {
    let exe = std::env::current_exe().context("current_exe")?;
    let mut cmd = Command::new(exe);
    cmd.arg("gui")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // Windows: without this, spawning `relay gui` from `relay mcp` often opens a blank console window.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn().context("spawn relay gui")?;
    Ok(())
}

/// Wait until GUI exposes a healthy HTTP endpoint (spawn GUI if needed).
pub fn ensure_gui_endpoint(max_wait: Duration) -> Result<GuiEndpoint> {
    let deadline = Instant::now() + max_wait;
    let mut spawned = false;

    loop {
        if Instant::now() > deadline {
            anyhow::bail!("Relay GUI did not become ready within {:?}", max_wait);
        }

        if let Some(ref ep) = read_gui_endpoint()? {
            if health_ok(ep) {
                return Ok(ep.clone());
            }
        }

        if !spawned {
            let _ = spawn_gui();
            spawned = true;
        }

        thread::sleep(Duration::from_millis(120));
    }
}

/// POST feedback + block on wait until user answers (or JSON with human "" on dismiss/timeout).
pub fn feedback_round(
    retell: &str,
    relay_mcp_session_id: &str,
    commands: Option<&[crate::CommandItem]>,
    skills: Option<&[crate::CommandItem]>,
) -> Result<String> {
    let ep = ensure_gui_endpoint(Duration::from_secs(45))?;

    let post_url = format!("http://127.0.0.1:{}/v1/feedback", ep.port);
    let mut body = serde_json::json!({
        "retell": retell,
        "relay_mcp_session_id": relay_mcp_session_id,
    });
    if let Some(cmd_list) = commands {
        body["commands"] = serde_json::to_value(cmd_list).unwrap_or(serde_json::json!([]));
    }
    if let Some(skill_list) = skills {
        body["skills"] = serde_json::to_value(skill_list).unwrap_or(serde_json::json!([]));
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
