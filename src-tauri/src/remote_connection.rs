//! Remote connection configuration: persist SSH targets on the GUI side (B).
//!
//! Each `RemoteConnection` stores SSH coordinates needed to set up a reverse
//! tunnel from A → B so that A's MCP process can reach B's local GUI server.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::ide::IdeKind;
use crate::user_data_dir;

const REMOTE_CONNECTIONS_FILE: &str = "remote_connections.json";

static REMOTE_CONFIG_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteConnection {
    pub id: String,
    pub ssh_target: String,
    pub ssh_port: u16,
    #[serde(default)]
    pub ssh_key_path: Option<String>,
    #[serde(default)]
    pub proxy_jump: Option<String>,
    pub ide_kind: IdeKind,
    /// Shared secret for endpoint file protection on the remote host.
    pub pair_token: String,
    /// Override `relay` binary path on remote host A (auto-detect when None).
    #[serde(default)]
    pub remote_relay_path: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub last_connected_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConnectionStatus {
    pub id: String,
    pub state: RemoteState,
    #[serde(default)]
    pub tunnel_local_port: Option<u16>,
    #[serde(default)]
    pub connected_since: Option<String>,
    pub active_tabs: u32,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct RemoteConnectionsStore {
    #[serde(default)]
    pub(crate) connections: Vec<RemoteConnection>,
}

// ---------------------------------------------------------------------------
// Path-injectable internals (testable without global state)
// ---------------------------------------------------------------------------

fn store_file(dir: &Path) -> PathBuf {
    dir.join(REMOTE_CONNECTIONS_FILE)
}

fn read_store_from(dir: &Path) -> RemoteConnectionsStore {
    let path = store_file(dir);
    let Ok(text) = fs::read_to_string(&path) else {
        return RemoteConnectionsStore::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn write_store_to(dir: &Path, store: &RemoteConnectionsStore) -> Result<()> {
    fs::create_dir_all(dir)?;
    let json = serde_json::to_string_pretty(store).context("serialize remote connections")?;
    fs::write(store_file(dir), json).context("write remote connections")?;
    Ok(())
}

fn add_connection_in(dir: &Path, conn: RemoteConnection) -> Result<()> {
    let mut store = read_store_from(dir);
    if store.connections.iter().any(|c| c.id == conn.id) {
        anyhow::bail!("connection with id {} already exists", conn.id);
    }
    store.connections.push(conn);
    write_store_to(dir, &store)
}

fn update_connection_in(dir: &Path, conn: RemoteConnection) -> Result<()> {
    let mut store = read_store_from(dir);
    let Some(existing) = store.connections.iter_mut().find(|c| c.id == conn.id) else {
        anyhow::bail!("connection {} not found", conn.id);
    };
    *existing = conn;
    write_store_to(dir, &store)
}

fn remove_connection_in(dir: &Path, id: &str) -> Result<()> {
    let mut store = read_store_from(dir);
    let before = store.connections.len();
    store.connections.retain(|c| c.id != id);
    if store.connections.len() == before {
        anyhow::bail!("connection {} not found", id);
    }
    write_store_to(dir, &store)
}

// ---------------------------------------------------------------------------
// Public API (uses global user_data_dir + lock)
// ---------------------------------------------------------------------------

fn store_path() -> Result<PathBuf> {
    Ok(user_data_dir()?.join(REMOTE_CONNECTIONS_FILE))
}

fn read_store() -> RemoteConnectionsStore {
    let Ok(dir) = user_data_dir() else {
        return RemoteConnectionsStore::default();
    };
    read_store_from(&dir)
}

fn write_store(store: &RemoteConnectionsStore) -> Result<()> {
    let dir = user_data_dir()?;
    write_store_to(&dir, store)
}

pub fn list_connections() -> Vec<RemoteConnection> {
    let _guard = REMOTE_CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    read_store().connections
}

pub fn get_connection(id: &str) -> Option<RemoteConnection> {
    let _guard = REMOTE_CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    read_store().connections.into_iter().find(|c| c.id == id)
}

pub fn add_connection(conn: RemoteConnection) -> Result<()> {
    let _guard = REMOTE_CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = user_data_dir()?;
    add_connection_in(&dir, conn)
}

pub fn update_connection(conn: RemoteConnection) -> Result<()> {
    let _guard = REMOTE_CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = user_data_dir()?;
    update_connection_in(&dir, conn)
}

pub fn remove_connection(id: &str) -> Result<()> {
    let _guard = REMOTE_CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = user_data_dir()?;
    remove_connection_in(&dir, id)
}

pub fn touch_last_connected(id: &str) -> Result<()> {
    let _guard = REMOTE_CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut store = read_store();
    let Some(c) = store.connections.iter_mut().find(|c| c.id == id) else {
        anyhow::bail!("connection {} not found", id);
    };
    c.last_connected_at = Some(crate::storage::timestamp_string());
    write_store(&store)
}

/// Path to the connections file (exposed for cleanup / diagnostics only).
pub fn connections_file_path() -> Result<PathBuf> {
    store_path()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_conn(id: &str) -> RemoteConnection {
        RemoteConnection {
            id: id.into(),
            ssh_target: "user@host".into(),
            ssh_port: 22,
            ssh_key_path: None,
            proxy_jump: None,
            ide_kind: IdeKind::Cursor,
            pair_token: "secret".into(),
            remote_relay_path: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            last_connected_at: None,
        }
    }

    // -----------------------------------------------------------------------
    // Serialization / deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn store_round_trip() {
        let store = RemoteConnectionsStore {
            connections: vec![sample_conn("test-1")],
        };
        let json = serde_json::to_string(&store).unwrap();
        let parsed: RemoteConnectionsStore = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, store);
    }

    #[test]
    fn connection_serde_with_all_optional_fields() {
        let conn = RemoteConnection {
            id: "full".into(),
            ssh_target: "admin@10.0.0.5".into(),
            ssh_port: 2222,
            ssh_key_path: Some("/home/.ssh/id_ed25519".into()),
            proxy_jump: Some("jump@bastion.example.com".into()),
            ide_kind: IdeKind::ClaudeCode,
            pair_token: "tok-123".into(),
            remote_relay_path: Some("/opt/relay".into()),
            created_at: "2026-04-01T12:00:00Z".into(),
            last_connected_at: Some("2026-04-02T08:00:00Z".into()),
        };
        let json = serde_json::to_string(&conn).unwrap();
        let back: RemoteConnection = serde_json::from_str(&json).unwrap();
        assert_eq!(back, conn);
    }

    #[test]
    fn connection_serde_missing_optional_fields_defaults() {
        let json = r#"{
            "id": "min",
            "ssh_target": "user@host",
            "ssh_port": 22,
            "ide_kind": "cursor",
            "pair_token": "tok",
            "created_at": "2026-01-01"
        }"#;
        let conn: RemoteConnection = serde_json::from_str(json).unwrap();
        assert!(conn.ssh_key_path.is_none());
        assert!(conn.proxy_jump.is_none());
        assert!(conn.remote_relay_path.is_none());
        assert!(conn.last_connected_at.is_none());
    }

    #[test]
    fn remote_state_serde_snake_case() {
        assert_eq!(
            serde_json::to_string(&RemoteState::Disconnected).unwrap(),
            "\"disconnected\""
        );
        assert_eq!(
            serde_json::to_string(&RemoteState::Connected).unwrap(),
            "\"connected\""
        );
        assert_eq!(
            serde_json::to_string(&RemoteState::Connecting).unwrap(),
            "\"connecting\""
        );
        assert_eq!(
            serde_json::to_string(&RemoteState::Reconnecting).unwrap(),
            "\"reconnecting\""
        );
        assert_eq!(
            serde_json::to_string(&RemoteState::Error).unwrap(),
            "\"error\""
        );
    }

    #[test]
    fn remote_state_deserialize_snake_case() {
        let s: RemoteState = serde_json::from_str("\"disconnected\"").unwrap();
        assert_eq!(s, RemoteState::Disconnected);
        let s: RemoteState = serde_json::from_str("\"error\"").unwrap();
        assert_eq!(s, RemoteState::Error);
    }

    #[test]
    fn connection_status_round_trip() {
        let status = RemoteConnectionStatus {
            id: "s1".into(),
            state: RemoteState::Connected,
            tunnel_local_port: Some(39000),
            connected_since: Some("2026-04-01".into()),
            active_tabs: 3,
            error: None,
        };
        let json = serde_json::to_string(&status).unwrap();
        let back: RemoteConnectionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "s1");
        assert_eq!(back.state, RemoteState::Connected);
        assert_eq!(back.tunnel_local_port, Some(39000));
        assert_eq!(back.active_tabs, 3);
    }

    #[test]
    fn empty_store_deserializes_as_default() {
        let s: RemoteConnectionsStore = serde_json::from_str("{}").unwrap();
        assert!(s.connections.is_empty());
    }

    #[test]
    fn malformed_json_falls_back_to_default() {
        let s: RemoteConnectionsStore = serde_json::from_str("not valid json").unwrap_or_default();
        assert!(s.connections.is_empty());
    }

    #[test]
    fn ide_kind_variants_in_connection() {
        for (kind, expected) in [
            (IdeKind::Cursor, "cursor"),
            (IdeKind::ClaudeCode, "claude_code"),
            (IdeKind::Windsurf, "windsurf"),
            (IdeKind::Other, "other"),
        ] {
            let conn = RemoteConnection {
                ide_kind: kind,
                ..sample_conn("x")
            };
            let json = serde_json::to_string(&conn).unwrap();
            assert!(json.contains(expected), "expected {expected} in {json}");
        }
    }

    // -----------------------------------------------------------------------
    // CRUD with temp directory
    // -----------------------------------------------------------------------

    #[test]
    fn read_store_empty_dir_returns_default() {
        let tmp = TempDir::new().unwrap();
        let store = read_store_from(tmp.path());
        assert!(store.connections.is_empty());
    }

    #[test]
    fn write_and_read_store_round_trip() {
        let tmp = TempDir::new().unwrap();
        let store = RemoteConnectionsStore {
            connections: vec![sample_conn("c1"), sample_conn("c2")],
        };
        write_store_to(tmp.path(), &store).unwrap();
        let loaded = read_store_from(tmp.path());
        assert_eq!(loaded.connections.len(), 2);
        assert_eq!(loaded.connections[0].id, "c1");
        assert_eq!(loaded.connections[1].id, "c2");
    }

    #[test]
    fn add_connection_creates_file_and_persists() {
        let tmp = TempDir::new().unwrap();
        add_connection_in(tmp.path(), sample_conn("a1")).unwrap();
        let loaded = read_store_from(tmp.path());
        assert_eq!(loaded.connections.len(), 1);
        assert_eq!(loaded.connections[0].id, "a1");
    }

    #[test]
    fn add_connection_duplicate_id_fails() {
        let tmp = TempDir::new().unwrap();
        add_connection_in(tmp.path(), sample_conn("dup")).unwrap();
        let err = add_connection_in(tmp.path(), sample_conn("dup"));
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("already exists"), "got: {msg}");
    }

    #[test]
    fn add_multiple_connections() {
        let tmp = TempDir::new().unwrap();
        add_connection_in(tmp.path(), sample_conn("m1")).unwrap();
        add_connection_in(tmp.path(), sample_conn("m2")).unwrap();
        add_connection_in(tmp.path(), sample_conn("m3")).unwrap();
        let loaded = read_store_from(tmp.path());
        assert_eq!(loaded.connections.len(), 3);
    }

    #[test]
    fn update_connection_modifies_existing() {
        let tmp = TempDir::new().unwrap();
        add_connection_in(tmp.path(), sample_conn("u1")).unwrap();
        let mut updated = sample_conn("u1");
        updated.ssh_target = "newuser@newhost".into();
        updated.ssh_port = 2222;
        update_connection_in(tmp.path(), updated).unwrap();
        let loaded = read_store_from(tmp.path());
        assert_eq!(loaded.connections[0].ssh_target, "newuser@newhost");
        assert_eq!(loaded.connections[0].ssh_port, 2222);
    }

    #[test]
    fn update_connection_not_found_fails() {
        let tmp = TempDir::new().unwrap();
        let err = update_connection_in(tmp.path(), sample_conn("ghost"));
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn update_preserves_other_connections() {
        let tmp = TempDir::new().unwrap();
        add_connection_in(tmp.path(), sample_conn("k1")).unwrap();
        add_connection_in(tmp.path(), sample_conn("k2")).unwrap();
        let mut u = sample_conn("k1");
        u.ssh_target = "changed@host".into();
        update_connection_in(tmp.path(), u).unwrap();
        let loaded = read_store_from(tmp.path());
        assert_eq!(loaded.connections.len(), 2);
        assert_eq!(loaded.connections[0].ssh_target, "changed@host");
        assert_eq!(loaded.connections[1].ssh_target, "user@host");
    }

    #[test]
    fn remove_connection_deletes_entry() {
        let tmp = TempDir::new().unwrap();
        add_connection_in(tmp.path(), sample_conn("r1")).unwrap();
        add_connection_in(tmp.path(), sample_conn("r2")).unwrap();
        remove_connection_in(tmp.path(), "r1").unwrap();
        let loaded = read_store_from(tmp.path());
        assert_eq!(loaded.connections.len(), 1);
        assert_eq!(loaded.connections[0].id, "r2");
    }

    #[test]
    fn remove_connection_not_found_fails() {
        let tmp = TempDir::new().unwrap();
        let err = remove_connection_in(tmp.path(), "ghost");
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn remove_last_connection_leaves_empty_store() {
        let tmp = TempDir::new().unwrap();
        add_connection_in(tmp.path(), sample_conn("only")).unwrap();
        remove_connection_in(tmp.path(), "only").unwrap();
        let loaded = read_store_from(tmp.path());
        assert!(loaded.connections.is_empty());
        // File should still exist with empty connections array
        assert!(store_file(tmp.path()).exists());
    }

    #[test]
    fn read_store_from_corrupt_json_returns_default() {
        let tmp = TempDir::new().unwrap();
        let path = store_file(tmp.path());
        fs::write(&path, "{{{{not json}}}}").unwrap();
        let store = read_store_from(tmp.path());
        assert!(store.connections.is_empty());
    }

    #[test]
    fn write_store_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("a").join("b").join("c");
        let store = RemoteConnectionsStore {
            connections: vec![sample_conn("nested")],
        };
        write_store_to(&nested, &store).unwrap();
        let loaded = read_store_from(&nested);
        assert_eq!(loaded.connections.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn connection_with_empty_strings() {
        let conn = RemoteConnection {
            id: "".into(),
            ssh_target: "".into(),
            ssh_port: 0,
            ssh_key_path: Some("".into()),
            proxy_jump: Some("".into()),
            ide_kind: IdeKind::Other,
            pair_token: "".into(),
            remote_relay_path: Some("".into()),
            created_at: "".into(),
            last_connected_at: Some("".into()),
        };
        let json = serde_json::to_string(&conn).unwrap();
        let back: RemoteConnection = serde_json::from_str(&json).unwrap();
        assert_eq!(back, conn);
    }

    #[test]
    fn connection_with_unicode_in_target() {
        let conn = RemoteConnection {
            ssh_target: "用户@服务器.cn".into(),
            ..sample_conn("unicode")
        };
        let json = serde_json::to_string(&conn).unwrap();
        let back: RemoteConnection = serde_json::from_str(&json).unwrap();
        assert_eq!(back.ssh_target, "用户@服务器.cn");
    }

    #[test]
    fn add_then_update_then_remove_lifecycle() {
        let tmp = TempDir::new().unwrap();
        let conn = sample_conn("lifecycle");
        add_connection_in(tmp.path(), conn.clone()).unwrap();
        let mut updated = conn;
        updated.ssh_port = 9999;
        update_connection_in(tmp.path(), updated).unwrap();
        let loaded = read_store_from(tmp.path());
        assert_eq!(loaded.connections[0].ssh_port, 9999);
        remove_connection_in(tmp.path(), "lifecycle").unwrap();
        let loaded = read_store_from(tmp.path());
        assert!(loaded.connections.is_empty());
    }

    #[test]
    fn store_file_path_uses_constant_name() {
        let dir = Path::new("/tmp/test");
        assert_eq!(
            store_file(dir),
            PathBuf::from("/tmp/test/remote_connections.json")
        );
    }
}
