//! GitHub release metadata for in-app “new version” affordance.

use serde::Serialize;
use std::cmp::Ordering;
use std::time::Duration;

/// Public repo page (click target).
pub const RELAY_REPO_HOME: &str = "https://github.com/andeya/ide-relay-mcp";
/// Latest GitHub Release page (installers / notes).
pub const RELAY_REPO_RELEASES_LATEST: &str =
    "https://github.com/andeya/ide-relay-mcp/releases/latest";
const GITHUB_API_LATEST: &str = "https://api.github.com/repos/andeya/ide-relay-mcp/releases/latest";

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseCheckPayload {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub check_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SemVer3(u32, u32, u32);

impl Ord for SemVer3 {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.0, self.1, self.2).cmp(&(other.0, other.1, other.2))
    }
}

impl PartialOrd for SemVer3 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn parse_semver_prefix(s: &str) -> Option<SemVer3> {
    let s = s.trim().trim_start_matches('v');
    let head = s.split(['-', '+'].as_slice()).next()?.trim();
    let mut parts = head.split('.');
    let a: u32 = parts.next()?.parse().ok()?;
    let b: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let c: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some(SemVer3(a, b, c))
}

pub fn check_latest_release(current_version: &str) -> ReleaseCheckPayload {
    let current_version = current_version.trim().to_string();
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();
    let resp = match agent
        .get(GITHUB_API_LATEST)
        .set(
            "User-Agent",
            "Relay-MCP (https://github.com/andeya/ide-relay-mcp; version check)",
        )
        .set("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(r) => r,
        Err(e) => {
            return ReleaseCheckPayload {
                current_version,
                latest_version: None,
                update_available: false,
                check_error: Some(format!("network: {e}")),
            };
        }
    };
    if resp.status() != 200 {
        return ReleaseCheckPayload {
            current_version,
            latest_version: None,
            update_available: false,
            check_error: Some(format!("HTTP {}", resp.status())),
        };
    }
    let j: serde_json::Value = match resp.into_json() {
        Ok(v) => v,
        Err(e) => {
            return ReleaseCheckPayload {
                current_version,
                latest_version: None,
                update_available: false,
                check_error: Some(format!("parse: {e}")),
            };
        }
    };
    let tag = j
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let latest_clean = tag.trim_start_matches('v').trim().to_string();
    if latest_clean.is_empty() {
        return ReleaseCheckPayload {
            current_version,
            latest_version: None,
            update_available: false,
            check_error: Some("missing tag_name".into()),
        };
    }
    let update = match (
        parse_semver_prefix(&current_version),
        parse_semver_prefix(&latest_clean),
    ) {
        (Some(cur), Some(lat)) => lat > cur,
        _ => false,
    };
    ReleaseCheckPayload {
        current_version: current_version.clone(),
        latest_version: Some(latest_clean),
        update_available: update,
        check_error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_gt() {
        assert!(parse_semver_prefix("1.2.0").unwrap() < parse_semver_prefix("1.3.0").unwrap());
        assert!(parse_semver_prefix("1.2.1").unwrap() > parse_semver_prefix("1.2.0").unwrap());
    }
}
