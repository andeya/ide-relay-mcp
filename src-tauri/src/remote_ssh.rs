//! SSH reverse-tunnel manager for remote MCP.
//!
//! GUI (B) opens an SSH connection to A and creates a reverse port forward:
//!   A:127.0.0.1:TUNNEL_PORT → B:127.0.0.1:GUI_PORT
//! Then writes `gui_endpoint_<ide>.json` on A so A's MCP can reach B's GUI.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::remote_connection::{RemoteConnection, RemoteConnectionStatus, RemoteState};

/// Holds state for all active SSH tunnels.
pub struct SshTunnelManager {
    tunnels: Arc<Mutex<HashMap<String, TunnelHandle>>>,
    gui_port: u16,
    /// Bearer token for the local GUI HTTP server; used when writing endpoint files on A.
    #[allow(dead_code)]
    gui_token: String,
}

struct TunnelHandle {
    child: Option<Child>,
    state: RemoteState,
    tunnel_port: Option<u16>,
    connected_since: Option<String>,
    error: Option<String>,
    stop_flag: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemoteEndpoint {
    port: u16,
    token: String,
    #[serde(default)]
    pid: Option<u32>,
    #[serde(default)]
    remote_pair_id: Option<String>,
    #[serde(default)]
    remote_from: Option<String>,
}

fn lock_tunnels(
    tunnels: &Mutex<HashMap<String, TunnelHandle>>,
) -> std::sync::MutexGuard<'_, HashMap<String, TunnelHandle>> {
    tunnels.lock().unwrap_or_else(|e| e.into_inner())
}

impl SshTunnelManager {
    pub fn new(gui_port: u16, gui_token: String) -> Self {
        Self {
            tunnels: Arc::new(Mutex::new(HashMap::new())),
            gui_port,
            gui_token,
        }
    }

    pub fn status(&self, id: &str) -> Option<RemoteConnectionStatus> {
        let tunnels = lock_tunnels(&self.tunnels);
        tunnels.get(id).map(|h| RemoteConnectionStatus {
            id: id.to_string(),
            state: h.state,
            tunnel_local_port: h.tunnel_port,
            connected_since: h.connected_since.clone(),
            active_tabs: 0,
            error: h.error.clone(),
        })
    }

    pub fn all_statuses(&self) -> Vec<RemoteConnectionStatus> {
        let tunnels = lock_tunnels(&self.tunnels);
        tunnels
            .iter()
            .map(|(id, h)| RemoteConnectionStatus {
                id: id.clone(),
                state: h.state,
                tunnel_local_port: h.tunnel_port,
                connected_since: h.connected_since.clone(),
                active_tabs: 0,
                error: h.error.clone(),
            })
            .collect()
    }

    /// Start a reverse SSH tunnel for the given connection.
    pub fn connect(&self, conn: &RemoteConnection) -> Result<()> {
        {
            let tunnels = lock_tunnels(&self.tunnels);
            if let Some(existing) = tunnels.get(&conn.id) {
                if existing.state == RemoteState::Connected
                    || existing.state == RemoteState::Connecting
                {
                    anyhow::bail!("tunnel {} is already active", conn.id);
                }
            }
        }

        let stop_flag = Arc::new(AtomicBool::new(false));

        let mut cmd = build_ssh_tunnel_command(conn, self.gui_port);
        let child = cmd.spawn().context("failed to spawn ssh process")?;

        {
            let mut tunnels = lock_tunnels(&self.tunnels);
            tunnels.insert(
                conn.id.clone(),
                TunnelHandle {
                    child: Some(child),
                    state: RemoteState::Connecting,
                    tunnel_port: None,
                    connected_since: None,
                    error: None,
                    stop_flag: stop_flag.clone(),
                },
            );
        }

        let tunnels_ref = self.tunnels.clone();
        let conn_id = conn.id.clone();

        thread::spawn(move || {
            monitor_ssh_tunnel(tunnels_ref, conn_id, stop_flag);
        });

        Ok(())
    }

    /// Disconnect and clean up the tunnel for `id`.
    pub fn disconnect(&self, id: &str) -> Result<()> {
        let mut tunnels = lock_tunnels(&self.tunnels);
        let Some(handle) = tunnels.get_mut(id) else {
            anyhow::bail!("no tunnel for {id}");
        };
        handle.stop_flag.store(true, Ordering::Release);
        if let Some(ref mut child) = handle.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        handle.child = None;
        handle.state = RemoteState::Disconnected;
        handle.error = None;
        Ok(())
    }

    /// Disconnect all tunnels (called on GUI exit).
    pub fn disconnect_all(&self) {
        let mut tunnels = lock_tunnels(&self.tunnels);
        for handle in tunnels.values_mut() {
            handle.stop_flag.store(true, Ordering::Release);
            if let Some(ref mut child) = handle.child {
                let _ = child.kill();
                let _ = child.wait();
            }
            handle.child = None;
            handle.state = RemoteState::Disconnected;
        }
    }
}

/// Background thread: poll the SSH child process and update state on exit.
fn monitor_ssh_tunnel(
    tunnels: Arc<Mutex<HashMap<String, TunnelHandle>>>,
    conn_id: String,
    stop_flag: Arc<AtomicBool>,
) {
    // Mark as connected after a brief startup period (SSH established if no early exit).
    thread::sleep(Duration::from_secs(3));
    {
        let mut t = lock_tunnels(&tunnels);
        if let Some(h) = t.get_mut(&conn_id) {
            if !stop_flag.load(Ordering::Acquire) && h.state == RemoteState::Connecting {
                if let Some(ref mut child) = h.child {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            h.state = RemoteState::Error;
                            h.error = Some(format!("ssh exited early: {status}"));
                            h.child = None;
                            return;
                        }
                        Ok(None) => {
                            h.state = RemoteState::Connected;
                            h.connected_since = Some(crate::storage::timestamp_string());
                        }
                        Err(e) => {
                            h.state = RemoteState::Error;
                            h.error = Some(format!("check ssh: {e}"));
                            return;
                        }
                    }
                }
            }
        }
    }

    loop {
        if stop_flag.load(Ordering::Acquire) {
            return;
        }
        thread::sleep(Duration::from_secs(2));

        let mut t = lock_tunnels(&tunnels);
        let Some(h) = t.get_mut(&conn_id) else {
            return;
        };
        if stop_flag.load(Ordering::Acquire) {
            return;
        }

        let exited = match h.child.as_mut().map(|c| c.try_wait()) {
            Some(Ok(Some(status))) => Some(status),
            Some(Err(e)) => {
                h.state = RemoteState::Error;
                h.error = Some(format!("check ssh: {e}"));
                h.child = None;
                return;
            }
            _ => None,
        };

        if let Some(status) = exited {
            h.state = RemoteState::Error;
            h.error = Some(format!("ssh exited: {status}"));
            h.child = None;
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// SSH command helpers (shared arg logic)
// ---------------------------------------------------------------------------

/// Appends common SSH connection args: key, port, ProxyJump.
fn apply_ssh_conn_args(cmd: &mut Command, conn: &RemoteConnection) {
    if let Some(ref key) = conn.ssh_key_path {
        if !key.is_empty() {
            cmd.arg("-i").arg(key);
        }
    }
    if conn.ssh_port != 22 {
        cmd.arg("-p").arg(conn.ssh_port.to_string());
    }
    if let Some(ref jump) = conn.proxy_jump {
        if !jump.is_empty() {
            cmd.arg("-J").arg(jump);
        }
    }
}

/// Build the SSH command for a reverse tunnel (long-running, `-N`).
fn build_ssh_tunnel_command(conn: &RemoteConnection, gui_port: u16) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.arg("-o")
        .arg("StrictHostKeyChecking=accept-new")
        .arg("-o")
        .arg("ServerAliveInterval=30")
        .arg("-o")
        .arg("ServerAliveCountMax=3")
        .arg("-o")
        .arg("ExitOnForwardFailure=yes")
        .arg("-N");

    apply_ssh_conn_args(&mut cmd, conn);

    cmd.arg("-R").arg(format!("0:127.0.0.1:{gui_port}"));

    cmd.arg(&conn.ssh_target);

    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    cmd
}

/// Shell-unsafe characters that must not appear in paths embedded in remote
/// SSH commands. Prevents command injection via user-controlled
/// `remote_relay_path`.
fn is_safe_remote_path(s: &str) -> bool {
    !s.is_empty()
        && !s.contains('\0')
        && !s.contains('`')
        && !s.contains("$(")
        && !s.contains("${")
        && !s.contains('"')
        && !s.contains('\'')
        && !s.contains(';')
        && !s.contains('|')
        && !s.contains('&')
        && !s.contains('\n')
        && !s.contains('\r')
        && !s.contains('>')
        && !s.contains('<')
}

/// Determine the relay data directory path on the remote host.
/// macOS: `$HOME/Library/Application Support/com.relay.relay-mcp/`
/// Linux: `$HOME/.config/relay-mcp/`
fn remote_relay_data_dir(conn: &RemoteConnection) -> Result<String> {
    if let Some(ref p) = conn.remote_relay_path {
        if !p.is_empty() {
            if !is_safe_remote_path(p) {
                anyhow::bail!("remote_relay_path contains unsafe characters: {:?}", p);
            }
            return Ok(p.clone());
        }
    }
    Ok(concat!(
        "$(if [ \"$(uname)\" = \"Darwin\" ]; then ",
        "echo \"$HOME/Library/Application Support/com.relay.relay-mcp\"; ",
        "else echo \"$HOME/.config/relay-mcp\"; fi)"
    )
    .to_string())
}

/// Write the remote endpoint file on remote host A via SSH exec.
/// Uses `_remote` suffix so A's local GUI endpoint is not overwritten.
pub fn write_remote_endpoint(
    conn: &RemoteConnection,
    tunnel_port: u16,
    gui_token: &str,
) -> Result<()> {
    let endpoint = RemoteEndpoint {
        port: tunnel_port,
        token: gui_token.to_string(),
        pid: None,
        remote_pair_id: Some(conn.id.clone()),
        remote_from: Some(conn.ssh_target.clone()),
    };
    let json = serde_json::to_string(&endpoint)?;
    let ide_id = conn.ide_kind.cli_id();
    let filename = format!("gui_endpoint_{ide_id}_remote.json");
    let data_dir = remote_relay_data_dir(conn)?;

    let script = format!(
        "RDIR=\"{data_dir}\" && mkdir -p \"$RDIR\" && printf '%s' '{}' > \"$RDIR/{filename}\"",
        json.replace('\'', "'\\''"),
    );

    let mut cmd = Command::new("ssh");
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    apply_ssh_conn_args(&mut cmd, conn);
    cmd.arg(&conn.ssh_target).arg(&script);

    let output = cmd.output().context("ssh exec for endpoint write")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("remote endpoint write failed: {stderr}");
    }
    Ok(())
}

/// Remove the remote endpoint file on remote host A via SSH exec.
pub fn remove_remote_endpoint(conn: &RemoteConnection) -> Result<()> {
    let ide_id = conn.ide_kind.cli_id();
    let filename = format!("gui_endpoint_{ide_id}_remote.json");
    let data_dir = remote_relay_data_dir(conn)?;

    let script = format!("RDIR=\"{data_dir}\" && rm -f \"$RDIR/{filename}\"");

    let mut cmd = Command::new("ssh");
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    apply_ssh_conn_args(&mut cmd, conn);
    cmd.arg(&conn.ssh_target).arg(&script);

    let output = cmd.output().context("ssh exec for endpoint remove")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("remote endpoint remove failed: {stderr}");
    }
    Ok(())
}

/// Set routing lock on the remote host via SSH.
/// Respects local pinning: if the remote file has `"pinned":true`,
/// the command fails without overwriting.
pub fn write_remote_routing_lock(conn: &RemoteConnection, prefer: &str) -> Result<()> {
    let data_dir = remote_relay_data_dir(conn)?;
    let ide_id = conn.ide_kind.cli_id();
    let filename = format!("gui_routing_lock_{ide_id}.json");
    let lock_json = serde_json::json!({
        "prefer": prefer,
        "set_by": "remote",
        "pinned": false,
    });

    let script = format!(
        concat!(
            "RDIR=\"{data_dir}\" && LOCK=\"$RDIR/{filename}\" && ",
            "if [ -f \"$LOCK\" ] && grep -q '\"pinned\"[[:space:]]*:[[:space:]]*true' \"$LOCK\"; then ",
            "echo 'LOCKED_BY_LOCAL' && exit 1; fi && ",
            "mkdir -p \"$RDIR\" && printf '%s' '{json}' > \"$LOCK\""
        ),
        data_dir = data_dir,
        filename = filename,
        json = lock_json.to_string().replace('\'', "'\\''"),
    );

    let mut cmd = Command::new("ssh");
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    apply_ssh_conn_args(&mut cmd, conn);
    cmd.arg(&conn.ssh_target).arg(&script);

    let output = cmd.output().context("ssh exec for routing lock")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("LOCKED_BY_LOCAL") {
            anyhow::bail!("routing is pinned by local user");
        }
        anyhow::bail!("remote routing lock write failed: {stderr}");
    }
    Ok(())
}

/// Remove routing lock on the remote host via SSH.
pub fn clear_remote_routing_lock(conn: &RemoteConnection) -> Result<()> {
    let data_dir = remote_relay_data_dir(conn)?;
    let ide_id = conn.ide_kind.cli_id();
    let filename = format!("gui_routing_lock_{ide_id}.json");

    let script = format!(
        "RDIR=\"{data_dir}\" && rm -f \"$RDIR/{filename}\"",
        data_dir = data_dir,
        filename = filename,
    );

    let mut cmd = Command::new("ssh");
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    apply_ssh_conn_args(&mut cmd, conn);
    cmd.arg(&conn.ssh_target).arg(&script);

    let output = cmd.output().context("ssh exec for routing lock clear")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("remote routing lock clear failed: {stderr}");
    }
    Ok(())
}

/// Test SSH connectivity to the remote host.
pub fn test_ssh_connection(conn: &RemoteConnection) -> Result<String> {
    let mut cmd = Command::new("ssh");
    cmd.arg("-o")
        .arg("StrictHostKeyChecking=accept-new")
        .arg("-o")
        .arg("ConnectTimeout=10")
        .arg("-o")
        .arg("BatchMode=yes");

    apply_ssh_conn_args(&mut cmd, conn);

    cmd.arg(&conn.ssh_target)
        .arg("echo relay-ssh-ok && uname -a");

    let output = cmd.output().context("ssh test connection")?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.status.success() || !stdout.contains("relay-ssh-ok") {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("SSH connection test failed: {stderr}");
    }
    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ide::IdeKind;

    fn test_conn(id: &str, target: &str) -> RemoteConnection {
        RemoteConnection {
            id: id.into(),
            ssh_target: target.into(),
            ssh_port: 22,
            ssh_key_path: None,
            proxy_jump: None,
            ide_kind: IdeKind::Cursor,
            pair_token: "tok".into(),
            remote_relay_path: None,
            created_at: String::new(),
            last_connected_at: None,
        }
    }

    fn get_args(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect()
    }

    // -----------------------------------------------------------------------
    // apply_ssh_conn_args
    // -----------------------------------------------------------------------

    #[test]
    fn apply_ssh_conn_args_no_optional_fields() {
        let conn = test_conn("t", "user@host");
        let mut cmd = Command::new("ssh");
        apply_ssh_conn_args(&mut cmd, &conn);
        let args = get_args(&cmd);
        assert!(!args.contains(&"-i".to_string()));
        assert!(!args.contains(&"-p".to_string()));
        assert!(!args.contains(&"-J".to_string()));
    }

    #[test]
    fn apply_ssh_conn_args_with_key() {
        let conn = RemoteConnection {
            ssh_key_path: Some("/path/to/key".into()),
            ..test_conn("t", "user@host")
        };
        let mut cmd = Command::new("ssh");
        apply_ssh_conn_args(&mut cmd, &conn);
        let args = get_args(&cmd);
        let i_idx = args.iter().position(|a| a == "-i").unwrap();
        assert_eq!(args[i_idx + 1], "/path/to/key");
    }

    #[test]
    fn apply_ssh_conn_args_empty_key_skipped() {
        let conn = RemoteConnection {
            ssh_key_path: Some("".into()),
            ..test_conn("t", "user@host")
        };
        let mut cmd = Command::new("ssh");
        apply_ssh_conn_args(&mut cmd, &conn);
        let args = get_args(&cmd);
        assert!(!args.contains(&"-i".to_string()));
    }

    #[test]
    fn apply_ssh_conn_args_with_custom_port() {
        let conn = RemoteConnection {
            ssh_port: 2222,
            ..test_conn("t", "user@host")
        };
        let mut cmd = Command::new("ssh");
        apply_ssh_conn_args(&mut cmd, &conn);
        let args = get_args(&cmd);
        let p_idx = args.iter().position(|a| a == "-p").unwrap();
        assert_eq!(args[p_idx + 1], "2222");
    }

    #[test]
    fn apply_ssh_conn_args_port_22_no_flag() {
        let conn = test_conn("t", "user@host");
        let mut cmd = Command::new("ssh");
        apply_ssh_conn_args(&mut cmd, &conn);
        let args = get_args(&cmd);
        assert!(!args.contains(&"-p".to_string()));
    }

    #[test]
    fn apply_ssh_conn_args_with_proxy_jump() {
        let conn = RemoteConnection {
            proxy_jump: Some("jump@bastion".into()),
            ..test_conn("t", "user@host")
        };
        let mut cmd = Command::new("ssh");
        apply_ssh_conn_args(&mut cmd, &conn);
        let args = get_args(&cmd);
        let j_idx = args.iter().position(|a| a == "-J").unwrap();
        assert_eq!(args[j_idx + 1], "jump@bastion");
    }

    #[test]
    fn apply_ssh_conn_args_empty_proxy_jump_skipped() {
        let conn = RemoteConnection {
            proxy_jump: Some("".into()),
            ..test_conn("t", "user@host")
        };
        let mut cmd = Command::new("ssh");
        apply_ssh_conn_args(&mut cmd, &conn);
        let args = get_args(&cmd);
        assert!(!args.contains(&"-J".to_string()));
    }

    #[test]
    fn apply_ssh_conn_args_all_options() {
        let conn = RemoteConnection {
            ssh_port: 3333,
            ssh_key_path: Some("/key".into()),
            proxy_jump: Some("jump@host".into()),
            ..test_conn("t", "user@host")
        };
        let mut cmd = Command::new("ssh");
        apply_ssh_conn_args(&mut cmd, &conn);
        let args = get_args(&cmd);
        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"-J".to_string()));
    }

    // -----------------------------------------------------------------------
    // build_ssh_tunnel_command
    // -----------------------------------------------------------------------

    #[test]
    fn build_ssh_command_basic() {
        let conn = test_conn("t1", "user@host");
        let cmd = build_ssh_tunnel_command(&conn, 12345);
        let args = get_args(&cmd);
        assert!(
            args.contains(&"-N".to_string()),
            "should be no-command mode"
        );
        assert!(args.contains(&"0:127.0.0.1:12345".to_string()));
        assert!(args.contains(&"user@host".to_string()));
        assert!(args.contains(&"StrictHostKeyChecking=accept-new".to_string()));
        assert!(args.contains(&"ServerAliveInterval=30".to_string()));
        assert!(args.contains(&"ServerAliveCountMax=3".to_string()));
        assert!(args.contains(&"ExitOnForwardFailure=yes".to_string()));
    }

    #[test]
    fn build_ssh_command_with_key_and_port() {
        let conn = RemoteConnection {
            ssh_port: 2222,
            ssh_key_path: Some("/home/user/.ssh/my_key".into()),
            proxy_jump: Some("bastion@jump.example.com".into()),
            ide_kind: IdeKind::ClaudeCode,
            ..test_conn("t2", "dev@10.0.0.1")
        };
        let cmd = build_ssh_tunnel_command(&conn, 9999);
        let args = get_args(&cmd);
        assert!(args.contains(&"/home/user/.ssh/my_key".to_string()));
        assert!(args.contains(&"2222".to_string()));
        assert!(args.contains(&"bastion@jump.example.com".to_string()));
        assert!(args.contains(&"0:127.0.0.1:9999".to_string()));
    }

    #[test]
    fn build_ssh_command_target_is_last_non_stdio_arg() {
        let conn = test_conn("t", "target@remote");
        let cmd = build_ssh_tunnel_command(&conn, 1000);
        let args = get_args(&cmd);
        let target_pos = args.iter().rposition(|a| a == "target@remote").unwrap();
        let r_pos = args.iter().position(|a| a == "-R").unwrap();
        assert!(target_pos > r_pos, "target should come after -R");
    }

    #[test]
    fn build_ssh_command_different_ports() {
        for port in [1, 80, 443, 8080, 65535u16] {
            let conn = test_conn("t", "u@h");
            let cmd = build_ssh_tunnel_command(&conn, port);
            let args = get_args(&cmd);
            let expected = format!("0:127.0.0.1:{port}");
            assert!(args.contains(&expected), "missing {expected}");
        }
    }

    // -----------------------------------------------------------------------
    // RemoteEndpoint
    // -----------------------------------------------------------------------

    #[test]
    fn remote_endpoint_json_shape() {
        let ep = RemoteEndpoint {
            port: 39000,
            token: "abc".into(),
            pid: None,
            remote_pair_id: Some("uuid-1".into()),
            remote_from: Some("user@host".into()),
        };
        let json: serde_json::Value = serde_json::to_value(&ep).unwrap();
        assert_eq!(json["port"], 39000);
        assert!(json["pid"].is_null());
        assert_eq!(json["remote_pair_id"], "uuid-1");
        assert_eq!(json["token"], "abc");
        assert_eq!(json["remote_from"], "user@host");
    }

    #[test]
    fn remote_endpoint_without_optional_fields() {
        let ep = RemoteEndpoint {
            port: 8080,
            token: "tok".into(),
            pid: None,
            remote_pair_id: None,
            remote_from: None,
        };
        let json = serde_json::to_string(&ep).unwrap();
        let back: RemoteEndpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(back.port, 8080);
        assert!(back.remote_pair_id.is_none());
    }

    #[test]
    fn remote_endpoint_with_pid() {
        let ep = RemoteEndpoint {
            port: 1234,
            token: "t".into(),
            pid: Some(42),
            remote_pair_id: None,
            remote_from: None,
        };
        let v: serde_json::Value = serde_json::to_value(&ep).unwrap();
        assert_eq!(v["pid"], 42);
    }

    #[test]
    fn remote_endpoint_deserialized_from_minimal_json() {
        let json = r#"{"port":9999,"token":"x"}"#;
        let ep: RemoteEndpoint = serde_json::from_str(json).unwrap();
        assert_eq!(ep.port, 9999);
        assert_eq!(ep.token, "x");
        assert!(ep.pid.is_none());
        assert!(ep.remote_pair_id.is_none());
        assert!(ep.remote_from.is_none());
    }

    // -----------------------------------------------------------------------
    // remote_relay_data_dir
    // -----------------------------------------------------------------------

    #[test]
    fn remote_data_dir_default_is_cross_platform() {
        let conn = test_conn("t1", "user@host");
        let dir = remote_relay_data_dir(&conn).unwrap();
        assert!(
            dir.contains("uname"),
            "should contain cross-platform detection"
        );
        assert!(dir.contains("Darwin"), "should detect macOS");
        assert!(
            dir.contains("Library/Application Support"),
            "should have macOS path"
        );
        assert!(dir.contains(".config/relay-mcp"), "should have Linux path");
    }

    #[test]
    fn remote_data_dir_custom_overrides() {
        let conn = RemoteConnection {
            remote_relay_path: Some("/opt/relay-data".into()),
            ..test_conn("t1", "user@host")
        };
        assert_eq!(remote_relay_data_dir(&conn).unwrap(), "/opt/relay-data");
    }

    #[test]
    fn remote_data_dir_empty_string_falls_back_to_default() {
        let conn = RemoteConnection {
            remote_relay_path: Some("".into()),
            ..test_conn("t1", "user@host")
        };
        let dir = remote_relay_data_dir(&conn).unwrap();
        assert!(
            dir.contains("uname"),
            "empty path should fall back to default"
        );
    }

    #[test]
    fn remote_data_dir_none_falls_back_to_default() {
        let conn = RemoteConnection {
            remote_relay_path: None,
            ..test_conn("t1", "user@host")
        };
        let dir = remote_relay_data_dir(&conn).unwrap();
        assert!(dir.contains("uname"), "None should fall back to default");
    }

    #[test]
    fn remote_data_dir_rejects_unsafe_path() {
        let cases = vec![
            "$(rm -rf /)",
            "/path/with`backtick",
            "/path/with\"quote",
            "/path;rm -rf /",
            "/path|evil",
            "/path&bg",
            "/path\ninjection",
        ];
        for bad in cases {
            let conn = RemoteConnection {
                remote_relay_path: Some(bad.into()),
                ..test_conn("t1", "user@host")
            };
            assert!(
                remote_relay_data_dir(&conn).is_err(),
                "should reject unsafe path: {bad:?}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // SshTunnelManager state
    // -----------------------------------------------------------------------

    #[test]
    fn manager_new_has_empty_tunnels() {
        let mgr = SshTunnelManager::new(8080, "tok".into());
        assert!(mgr.all_statuses().is_empty());
    }

    #[test]
    fn manager_status_unknown_id_returns_none() {
        let mgr = SshTunnelManager::new(8080, "tok".into());
        assert!(mgr.status("nonexistent").is_none());
    }

    #[test]
    fn manager_disconnect_unknown_id_fails() {
        let mgr = SshTunnelManager::new(8080, "tok".into());
        let err = mgr.disconnect("ghost");
        assert!(err.is_err());
    }

    #[test]
    fn manager_disconnect_all_on_empty_is_noop() {
        let mgr = SshTunnelManager::new(8080, "tok".into());
        mgr.disconnect_all(); // should not panic
    }

    #[test]
    fn lock_tunnels_recovers_from_poison() {
        let m: Mutex<HashMap<String, TunnelHandle>> = Mutex::new(HashMap::new());
        // Poison the mutex
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = m.lock().unwrap();
            panic!("deliberate poison");
        }));
        assert!(m.is_poisoned());
        let g = lock_tunnels(&m);
        assert!(g.is_empty());
    }
}
