//! Windows path → WSL `/mnt/x/...` for MCP when GUI is Windows and this process runs on Linux (e.g. WSL).

use serde_json::Value;

fn collapse_slashes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_slash = false;
    for c in s.chars() {
        if c == '/' {
            if !prev_slash {
                out.push('/');
                prev_slash = true;
            }
        } else {
            prev_slash = false;
            out.push(c);
        }
    }
    out
}

/// Convert a Windows absolute path to WSL `/mnt/x/...` when possible.
pub fn windows_path_to_wsl(win_path: &str) -> Option<String> {
    let mut s = win_path.trim();
    if s.is_empty() {
        return None;
    }

    if s.starts_with(r"\\?\") {
        s = &s[4..];
        if s.starts_with("UNC\\") || s.starts_with("UNC/") {
            return None;
        }
    }

    if s.starts_with(r"\\") {
        return None;
    }

    if s.starts_with('/') {
        return None;
    }

    let bytes = s.as_bytes();
    if bytes.len() < 2 || bytes[1] != b':' {
        return None;
    }
    let d = bytes[0];
    if !d.is_ascii_alphabetic() {
        return None;
    }

    let mut rest = &s[2..];
    if rest.starts_with('\\') || rest.starts_with('/') {
        rest = &rest[1..];
    }
    let rest = rest.replace('\\', "/");
    let rest = rest.trim_start_matches('/');

    let mut out = format!("/mnt/{}/", (d as char).to_ascii_lowercase());
    out.push_str(rest);
    Some(collapse_slashes(&out).trim_end_matches('/').to_string())
}

/// When `relay mcp` runs on Linux (e.g. WSL) and the GUI reports `relay_gui_platform: windows`,
/// replace each convertible Windows `attachments[].path` with `/mnt/...` only (no duplicate Windows paths).
pub fn post_process_feedback_body_for_linux_win_gui(body: String) -> String {
    if !cfg!(target_os = "linux") {
        return body;
    }
    let Ok(mut v) = serde_json::from_str::<Value>(&body) else {
        return body;
    };
    let Some(obj) = v.as_object_mut() else {
        return body;
    };
    if obj.get("relay_gui_platform").and_then(|x| x.as_str()) != Some("windows") {
        return body;
    }
    let Some(att) = obj.get_mut("attachments") else {
        return serde_json::to_string(&v).unwrap_or(body);
    };
    let Some(arr) = att.as_array_mut() else {
        return serde_json::to_string(&v).unwrap_or(body);
    };
    if arr.is_empty() {
        return serde_json::to_string(&v).unwrap_or(body);
    }

    for item in arr.iter_mut() {
        let Some(o) = item.as_object_mut() else {
            continue;
        };
        let Some(path) = o.get("path").and_then(|p| p.as_str()) else {
            continue;
        };
        if let Some(wsl) = windows_path_to_wsl(path) {
            o.insert("path".to_string(), Value::String(wsl));
        }
    }
    serde_json::to_string(&v).unwrap_or(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wsl_backslash_users() {
        assert_eq!(
            windows_path_to_wsl(r"C:\Users\KSO\file.png").as_deref(),
            Some("/mnt/c/Users/KSO/file.png")
        );
    }

    #[test]
    fn post_process_replaces_paths_on_linux_when_gui_windows() {
        let j = r#"{"relay_mcp_session_id":"1","human":"x","cmd_skill_count":0,"relay_gui_platform":"windows","attachments":[{"kind":"file","path":"C:\\a\\b.txt"}]}"#;
        let out = post_process_feedback_body_for_linux_win_gui(j.into());
        if cfg!(target_os = "linux") {
            let v: Value = serde_json::from_str(&out).unwrap();
            let a = v["attachments"].as_array().unwrap();
            assert_eq!(a.len(), 1);
            assert_eq!(a[0]["path"], "/mnt/c/a/b.txt");
        } else {
            assert_eq!(out, j);
        }
    }

    #[test]
    fn post_process_noop_when_gui_not_windows() {
        let j = r#"{"relay_mcp_session_id":"1","human":"x","cmd_skill_count":0,"relay_gui_platform":"macos","attachments":[{"kind":"file","path":"C:\\a"}]}"#;
        let out = post_process_feedback_body_for_linux_win_gui(j.into());
        assert_eq!(out, j);
    }

    #[test]
    fn post_process_leaves_non_convertible_path_on_linux() {
        let j = r#"{"relay_mcp_session_id":"1","human":"x","cmd_skill_count":0,"relay_gui_platform":"windows","attachments":[{"kind":"file","path":"\\\\server\\share\\x"}]}"#;
        let out = post_process_feedback_body_for_linux_win_gui(j.into());
        if cfg!(target_os = "linux") {
            let v: Value = serde_json::from_str(&out).unwrap();
            assert_eq!(v["attachments"][0]["path"], r"\\server\share\x");
        } else {
            assert_eq!(out, j);
        }
    }
}
