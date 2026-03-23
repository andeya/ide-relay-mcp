//! MCP-only: optional `attachments[].path` rewrite for WSL-hosted agents calling Windows `relay.exe`.
//!
//! Enable with **`relay mcp --exe_in_wsl`**. Each `attachments[].path` in the MCP `tools/call` result is
//! rewritten from a Windows path to `/mnt/<drive>/...`. GUI HTTP payloads and on-disk history stay Windows paths.
#![cfg_attr(not(windows), allow(dead_code))] // Non-Windows builds omit MCP path transform; helpers still run in unit tests.

use std::sync::atomic::{AtomicBool, Ordering};

static MCP_WSL_PATH_REWRITE_ENABLED: AtomicBool = AtomicBool::new(false);

/// Called from `relay mcp` after CLI parse, before the stdio loop.
pub(crate) fn set_mcp_wsl_path_rewrite_enabled(enabled: bool) {
    MCP_WSL_PATH_REWRITE_ENABLED.store(enabled, Ordering::Relaxed);
}

/// True when `relay mcp` was started with `--exe_in_wsl`.
fn mcp_wsl_path_rewrite_enabled() -> bool {
    MCP_WSL_PATH_REWRITE_ENABLED.load(Ordering::Relaxed)
}

/// Strips `\\?\` long-path prefix; leaves UNC verbatim paths detectable via `\\` start.
fn strip_verbatim_long_path_prefix(s: &str) -> &str {
    if s.len() >= 4 && s.starts_with("\\\\?\\") {
        let rest = &s[4..];
        if rest.len() >= 4 && rest[..4].eq_ignore_ascii_case("UNC\\") {
            return s;
        }
        return rest;
    }
    s
}

/// Converts a Windows absolute path to WSL `/mnt/<drive>/...` when possible.
/// Returns [`None`] for UNC, non–drive-letter absolutes, or empty remainder edge cases we skip.
pub fn windows_abs_path_to_wsl_mnt(path: &str) -> Option<String> {
    let s = path.trim();
    if s.is_empty() {
        return None;
    }
    let s = strip_verbatim_long_path_prefix(s);
    if s.starts_with("\\\\") {
        return None;
    }
    let mut chars = s.chars();
    let d0 = chars.next()?;
    let c1 = chars.next()?;
    if !d0.is_ascii_alphabetic() || c1 != ':' {
        return None;
    }
    let drive = d0.to_ascii_lowercase();
    let rest: String = chars.collect();
    let rest = rest.trim_start_matches(['\\', '/']);
    if rest.is_empty() {
        return Some(format!("/mnt/{}/", drive));
    }
    let posix = rest.replace('\\', "/");
    Some(format!("/mnt/{}/{}", drive, posix))
}

#[cfg(windows)]
fn transform_one_attachment_path(path: &str) -> String {
    if let Some(wsl) = windows_abs_path_to_wsl_mnt(path) {
        return wsl;
    }
    if let Some(pb) = crate::storage::canonical_feedback_attachment_path(path) {
        let s = pb.to_string_lossy().to_string();
        if let Some(wsl) = windows_abs_path_to_wsl_mnt(&s) {
            return wsl;
        }
    }
    path.to_string()
}

/// Parse tool-result JSON; when enabled on Windows, rewrite each `attachments[].path` for WSL consumers.
#[cfg(not(windows))]
pub fn transform_tool_result_json_for_mcp_host(body: String) -> String {
    body
}

/// Parse tool-result JSON; when enabled on Windows, rewrite each `attachments[].path` for WSL consumers.
#[cfg(windows)]
pub fn transform_tool_result_json_for_mcp_host(body: String) -> String {
    use serde_json::{json, Value};

    if !mcp_wsl_path_rewrite_enabled() {
        return body;
    }
    let Ok(mut v) = serde_json::from_str::<Value>(&body) else {
        return body;
    };
    let Some(obj) = v.as_object_mut() else {
        return body;
    };
    let Some(att) = obj.get_mut("attachments") else {
        return serde_json::to_string(&v).unwrap_or(body);
    };
    let Some(arr) = att.as_array_mut() else {
        return serde_json::to_string(&v).unwrap_or(body);
    };
    for item in arr.iter_mut() {
        let Some(o) = item.as_object_mut() else {
            continue;
        };
        let Some(path) = o.get("path").and_then(|x| x.as_str()) else {
            continue;
        };
        let new_path = transform_one_attachment_path(path);
        o.insert("path".to_string(), json!(new_path));
    }
    serde_json::to_string(&v).unwrap_or(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static MCP_WSL_PATH_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn wsl_mapping_basic_backslash() {
        assert_eq!(
            windows_abs_path_to_wsl_mnt(r"C:\Users\KSO\AppData\Local\Relay\relay.exe").as_deref(),
            Some("/mnt/c/Users/KSO/AppData/Local/Relay/relay.exe")
        );
    }

    #[test]
    fn wsl_mapping_basic_slash() {
        assert_eq!(
            windows_abs_path_to_wsl_mnt("C:/Users/foo/bar").as_deref(),
            Some("/mnt/c/Users/foo/bar")
        );
    }

    #[test]
    fn wsl_mapping_verbatim_prefix() {
        assert_eq!(
            windows_abs_path_to_wsl_mnt(r"\\?\C:\Users\test\file.png").as_deref(),
            Some("/mnt/c/Users/test/file.png")
        );
    }

    #[test]
    fn wsl_mapping_unc_none() {
        assert_eq!(
            windows_abs_path_to_wsl_mnt(r"\\?\UNC\server\share\file.txt"),
            None
        );
        assert_eq!(
            windows_abs_path_to_wsl_mnt(r"\\server\share\file.txt"),
            None
        );
    }

    #[test]
    fn wsl_mapping_drive_only() {
        assert_eq!(
            windows_abs_path_to_wsl_mnt(r"C:\").as_deref(),
            Some("/mnt/c/")
        );
    }

    #[test]
    fn wsl_mapping_trims_whitespace() {
        assert_eq!(
            windows_abs_path_to_wsl_mnt("  C:/x/y  ").as_deref(),
            Some("/mnt/c/x/y")
        );
    }

    #[test]
    fn mcp_wsl_path_rewrite_follows_cli_flag() {
        let _g = MCP_WSL_PATH_TEST_LOCK.lock().unwrap();
        set_mcp_wsl_path_rewrite_enabled(false);
        assert!(!mcp_wsl_path_rewrite_enabled());
        set_mcp_wsl_path_rewrite_enabled(true);
        assert!(mcp_wsl_path_rewrite_enabled());
        set_mcp_wsl_path_rewrite_enabled(false);
        assert!(!mcp_wsl_path_rewrite_enabled());
    }

    #[test]
    fn transform_noop_when_disabled() {
        let _g = MCP_WSL_PATH_TEST_LOCK.lock().unwrap();
        set_mcp_wsl_path_rewrite_enabled(false);
        let s = r#"{"relay_mcp_session_id":"1","human":"","cmd_skill_count":0,"attachments":[{"kind":"image","path":"C:\\a.png"}]}"#.to_string();
        assert_eq!(transform_tool_result_json_for_mcp_host(s.clone()), s);
    }

    #[cfg(windows)]
    #[test]
    fn transform_rewrites_when_enabled() {
        let _g = MCP_WSL_PATH_TEST_LOCK.lock().unwrap();
        set_mcp_wsl_path_rewrite_enabled(true);
        let s = r#"{"relay_mcp_session_id":"1","human":"","cmd_skill_count":0,"attachments":[{"kind":"image","path":"C:\\Users\\x\\y.png"}]}"#.to_string();
        let out = transform_tool_result_json_for_mcp_host(s);
        assert!(out.contains("/mnt/c/Users/x/y.png"));
        set_mcp_wsl_path_rewrite_enabled(false);
    }
}
