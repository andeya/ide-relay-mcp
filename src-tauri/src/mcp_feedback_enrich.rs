//! Optional `data_url` on `attachments[]` when `relay mcp` returns the tool result to the host.
//! GUI ↔ HTTP payloads stay path-only; enrichment runs only in the MCP client (`mcp_http`).
//!
//! **Single rule:** only `RELAY_MCP_INLINE_MAX_KB` is read. Unset or blank → default **512** KiB;
//! ≤0 → off; unparseable → off; >0 → cap in bytes (any `kind`, Relay attachment paths only).

use crate::storage::read_feedback_attachment_data_url_if_within;
use serde_json::{json, Value};

pub(crate) const RELAY_MCP_INLINE_MAX_KB: &str = "RELAY_MCP_INLINE_MAX_KB";

const DEFAULT_MAX_KB: u64 = 512;
const INLINE_MAX_CAP: u64 = 20 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InlinePolicy {
    enabled: bool,
    max_bytes: u64,
}

fn kb_to_max_bytes(kb: u64) -> u64 {
    kb.checked_mul(1024)
        .unwrap_or(INLINE_MAX_CAP)
        .clamp(1, INLINE_MAX_CAP)
}

fn default_policy() -> InlinePolicy {
    InlinePolicy {
        enabled: true,
        max_bytes: kb_to_max_bytes(DEFAULT_MAX_KB),
    }
}

/// `raw` is the env value string, or `None` if the variable is unset.
fn inline_policy_from_max_kb_raw(raw: Option<&str>) -> InlinePolicy {
    let Some(r) = raw else {
        return default_policy();
    };
    let s = r.trim();
    if s.is_empty() {
        return default_policy();
    }
    let Ok(kb) = s.parse::<i64>() else {
        return InlinePolicy {
            enabled: false,
            max_bytes: 0,
        };
    };
    if kb <= 0 {
        return InlinePolicy {
            enabled: false,
            max_bytes: 0,
        };
    }
    InlinePolicy {
        enabled: true,
        max_bytes: kb_to_max_bytes(kb as u64),
    }
}

fn resolve_inline_policy() -> InlinePolicy {
    inline_policy_from_max_kb_raw(std::env::var(RELAY_MCP_INLINE_MAX_KB).ok().as_deref())
}

/// Parse tool-result JSON, add `data_url` next to existing `path` when allowed (paths unchanged).
pub fn enrich_tool_result_json_for_mcp_host(body: String) -> String {
    let policy = resolve_inline_policy();
    if !policy.enabled {
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
    let max = policy.max_bytes;
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
        if kind.trim().is_empty() {
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

    #[test]
    fn policy_unset_is_default_512kb() {
        let p = super::inline_policy_from_max_kb_raw(None);
        assert!(p.enabled);
        assert_eq!(p.max_bytes, 512 * 1024);
    }

    #[test]
    fn policy_empty_string_is_default() {
        let p = super::inline_policy_from_max_kb_raw(Some(""));
        assert!(p.enabled);
        assert_eq!(p.max_bytes, 512 * 1024);
        let p = super::inline_policy_from_max_kb_raw(Some("   "));
        assert!(p.enabled);
    }

    #[test]
    fn policy_zero_disables() {
        let p = super::inline_policy_from_max_kb_raw(Some("0"));
        assert!(!p.enabled);
    }

    #[test]
    fn policy_negative_disables() {
        let p = super::inline_policy_from_max_kb_raw(Some("-1"));
        assert!(!p.enabled);
    }

    #[test]
    fn policy_positive_cap() {
        let p = super::inline_policy_from_max_kb_raw(Some("1024"));
        assert!(p.enabled);
        assert_eq!(p.max_bytes, 1024 * 1024);
    }

    #[test]
    fn policy_invalid_disables() {
        let p = super::inline_policy_from_max_kb_raw(Some("12abc"));
        assert!(!p.enabled);
    }

    #[test]
    fn enrich_invalid_json_unchanged() {
        let s = "not json".to_string();
        assert_eq!(enrich_tool_result_json_for_mcp_host(s.clone()), s);
    }

    #[test]
    fn enrich_adds_nothing_when_no_attachments() {
        use serde_json::json;
        use serde_json::Value;
        let s = r#"{"relay_mcp_session_id":"1","human":"x","cmd_skill_count":0}"#.to_string();
        let out = enrich_tool_result_json_for_mcp_host(s);
        let a: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            a,
            json!({"relay_mcp_session_id":"1","human":"x","cmd_skill_count":0})
        );
    }
}
