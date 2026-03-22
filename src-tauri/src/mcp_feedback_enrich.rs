//! Optional `data_url` on `attachments[]` when `relay mcp` returns the tool result to the host.
//! GUI ↔ HTTP payloads stay path-only; enrichment runs only in the MCP client (`mcp_http`).

use crate::storage::read_feedback_attachment_data_url_if_within;
use serde_json::{json, Value};

const DEFAULT_INLINE_MAX: u64 = 512 * 1024;
const INLINE_MAX_CAP: u64 = 20 * 1024 * 1024;

fn inline_max_bytes() -> u64 {
    std::env::var("RELAY_MCP_INLINE_MAX_BYTES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&n| n > 0 && n <= INLINE_MAX_CAP)
        .unwrap_or(DEFAULT_INLINE_MAX)
}

fn inline_disabled() -> bool {
    let Some(raw) = std::env::var("RELAY_MCP_INLINE_ATTACHMENTS").ok() else {
        return false;
    };
    let s = raw.to_ascii_lowercase();
    s == "0" || s == "false" || s == "off"
}

/// `RELAY_MCP_INLINE_ATTACHMENTS=all` (or `2`) — inline any `kind` under the size cap.
/// Otherwise default: only `kind == "image"` (case-insensitive).
fn inline_include_all_kinds() -> bool {
    matches!(
        std::env::var("RELAY_MCP_INLINE_ATTACHMENTS")
            .ok()
            .as_deref(),
        Some("all") | Some("2")
    )
}

fn should_try_inline(kind: &str) -> bool {
    if inline_include_all_kinds() {
        return true;
    }
    kind.eq_ignore_ascii_case("image")
}

/// Parse tool-result JSON, add `data_url` next to existing `path` when allowed (paths unchanged).
pub fn enrich_tool_result_json_for_mcp_host(body: String) -> String {
    if inline_disabled() {
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
    let max = inline_max_bytes();
    for item in arr.iter_mut() {
        let Some(o) = item.as_object_mut() else {
            continue;
        };
        if o.contains_key("data_url") {
            continue;
        }
        let Some(kind) = o.get("kind").and_then(|x| x.as_str()) else {
            continue;
        };
        let Some(path) = o.get("path").and_then(|x| x.as_str()) else {
            continue;
        };
        if path.trim().is_empty() {
            continue;
        }
        if !should_try_inline(kind) {
            continue;
        }
        if let Ok(url) = read_feedback_attachment_data_url_if_within(path, max) {
            o.insert("data_url".to_string(), json!(url));
        }
    }
    serde_json::to_string(&v).unwrap_or(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn enrich_invalid_json_unchanged() {
        let s = "not json".to_string();
        assert_eq!(enrich_tool_result_json_for_mcp_host(s.clone()), s);
    }

    #[test]
    fn enrich_adds_nothing_when_no_attachments() {
        use serde_json::json;
        let s = r#"{"relay_mcp_session_id":"1","human":"x","cmd_skill_count":0}"#.to_string();
        let out = enrich_tool_result_json_for_mcp_host(s);
        let a: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(a, json!({"relay_mcp_session_id":"1","human":"x","cmd_skill_count":0}));
    }
}
