//! Cursor Usage monitoring: fetch usage data via IDE Bearer token (api2.cursor.sh)
//! with fallback to web cookie API (cursor.com/api).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[cfg(any(target_os = "macos", target_os = "linux"))]
use {
    aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit},
    hmac::Hmac,
    sha1::Sha1,
};

#[cfg(target_os = "macos")]
use security_framework::passwords::get_generic_password;

use crate::prepare_user_data_dir;

const SETTINGS_FILE: &str = "cursor_usage_settings.json";
const TOKEN_FILE: &str = "cursor_session_token";
const CURSOR_API_BASE: &str = "https://cursor.com/api";
const CURSOR_IDE_API_BASE: &str = "https://api2.cursor.sh";

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorUsageSettings {
    #[serde(default = "default_true")]
    pub refresh_on_new_session: bool,
    #[serde(default = "default_interval")]
    pub refresh_interval_minutes: u32,
}

fn default_true() -> bool {
    true
}
fn default_interval() -> u32 {
    30
}

impl Default for CursorUsageSettings {
    fn default() -> Self {
        Self {
            refresh_on_new_session: true,
            refresh_interval_minutes: 30,
        }
    }
}

fn settings_path() -> Result<PathBuf> {
    Ok(prepare_user_data_dir()?.join(SETTINGS_FILE))
}

pub fn read_cursor_usage_settings() -> CursorUsageSettings {
    let Ok(path) = settings_path() else {
        return CursorUsageSettings::default();
    };
    if !path.exists() {
        return CursorUsageSettings::default();
    }
    let Ok(text) = fs::read_to_string(&path) else {
        return CursorUsageSettings::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn write_cursor_usage_settings(s: &CursorUsageSettings) -> Result<()> {
    let path = settings_path()?;
    let json = serde_json::to_string_pretty(s).context("serialize cursor usage settings")?;
    fs::write(&path, json).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Token persistence
// ---------------------------------------------------------------------------

fn token_path() -> Result<PathBuf> {
    Ok(prepare_user_data_dir()?.join(TOKEN_FILE))
}

pub fn read_cursor_session_token() -> Result<String> {
    let path = token_path()?;
    if !path.exists() {
        return Ok(String::new());
    }
    let text = fs::read_to_string(&path).context("read cursor session token")?;
    Ok(text.trim().to_string())
}

pub fn write_cursor_session_token(token: &str) -> Result<()> {
    let path = token_path()?;
    fs::write(&path, token.trim()).with_context(|| format!("write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn clear_cursor_session_token() -> Result<()> {
    let path = token_path()?;
    if path.exists() {
        fs::remove_file(&path).context("remove cursor session token")?;
    }
    Ok(())
}

/// Locate the Cursor IDE `state.vscdb` SQLite database.
fn cursor_state_vscdb_path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home)
            .join("Library/Application Support/Cursor/User/globalStorage/state.vscdb"))
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").context("APPDATA not set")?;
        Ok(PathBuf::from(appdata).join("Cursor/User/globalStorage/state.vscdb"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let home = std::env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home).join(".config/Cursor/User/globalStorage/state.vscdb"))
    }
}

/// Read `cursorAuth/accessToken` from Cursor IDE's local SQLite database.
/// Used with `api2.cursor.sh` via Bearer auth (distinct from web cookie token).
pub fn auto_detect_cursor_token() -> Result<String> {
    let db_path = cursor_state_vscdb_path()?;
    if !db_path.exists() {
        anyhow::bail!("Cursor IDE database not found at {}", db_path.display());
    }
    let conn = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("open {}", db_path.display()))?;
    let token: String = conn
        .query_row(
            "SELECT value FROM ItemTable WHERE key = 'cursorAuth/accessToken'",
            [],
            |row| row.get(0),
        )
        .context("cursorAuth/accessToken not found in Cursor database")?;
    let token = token.trim().to_string();
    if token.is_empty() {
        anyhow::bail!("cursorAuth/accessToken is empty");
    }
    Ok(token)
}

// ---------------------------------------------------------------------------
// API response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsagePlanBlock {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub used: f64,
    #[serde(default)]
    pub limit: f64,
    #[serde(default)]
    pub remaining: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageOnDemandBlock {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub used: f64,
    #[serde(default)]
    pub limit: f64,
    #[serde(default)]
    pub remaining: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IndividualUsage {
    #[serde(default)]
    pub plan: UsagePlanBlock,
    #[serde(default)]
    pub on_demand: UsageOnDemandBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TeamOnDemand {
    #[serde(default)]
    pub on_demand: UsageOnDemandBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CursorUsageSummary {
    #[serde(default)]
    pub billing_cycle_start: String,
    #[serde(default)]
    pub billing_cycle_end: String,
    #[serde(default)]
    pub membership_type: String,
    #[serde(default)]
    pub is_yearly_plan: bool,
    #[serde(default)]
    pub on_demand_auto_enabled: bool,
    #[serde(default)]
    pub individual_usage: IndividualUsage,
    #[serde(default)]
    pub team_usage: Option<TeamOnDemand>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    #[serde(default)]
    pub input_tokens: f64,
    #[serde(default)]
    pub output_tokens: f64,
    #[serde(default)]
    pub cache_read_tokens: f64,
    #[serde(default)]
    pub cache_write_tokens: f64,
    #[serde(default)]
    pub total_cents: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CursorUsageEvent {
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub requests_costs: Option<f64>,
    #[serde(default)]
    pub charged_cents: f64,
    #[serde(default)]
    pub is_chargeable: bool,
    #[serde(default)]
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CursorUsageEventsPage {
    #[serde(default)]
    pub total_usage_events_count: u64,
    #[serde(default)]
    pub usage_events_display: Vec<CursorUsageEvent>,
}

// ---------------------------------------------------------------------------
// IDE API response types (api2.cursor.sh — Bearer auth)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IdeModelUsage {
    #[serde(default)]
    pub num_requests: f64,
    #[serde(default)]
    pub num_requests_total: f64,
    #[serde(default)]
    pub num_tokens: f64,
    #[serde(default)]
    pub max_request_usage: Option<f64>,
    #[serde(default)]
    pub max_token_usage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IdeUsageResponse {
    #[serde(rename = "gpt-4", default)]
    pub gpt4: IdeModelUsage,
    #[serde(default)]
    pub start_of_month: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IdeStripeProfile {
    #[serde(default)]
    pub membership_type: String,
    #[serde(default)]
    pub is_team_member: bool,
    #[serde(default)]
    pub team_id: Option<u64>,
    #[serde(default)]
    pub team_membership_type: String,
    #[serde(default)]
    pub individual_membership_type: String,
    #[serde(default)]
    pub hard_spending_limit_cents: Option<f64>,
    #[serde(default)]
    pub soft_spending_limit_cents: Option<f64>,
    #[serde(default, rename = "usage_spending_limit_enabled")]
    pub usage_spending_limit_enabled: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// HTTP helpers
// ---------------------------------------------------------------------------

fn build_cookie_request(url: &str, token: &str) -> ureq::Request {
    ureq::get(url)
        .timeout(std::time::Duration::from_secs(15))
        .set("Cookie", &format!("WorkosCursorSessionToken={token}"))
        .set("Origin", "https://cursor.com")
        .set("Content-Type", "application/json")
}

fn build_cookie_post(url: &str, token: &str) -> ureq::Request {
    ureq::post(url)
        .timeout(std::time::Duration::from_secs(15))
        .set("Cookie", &format!("WorkosCursorSessionToken={token}"))
        .set("Origin", "https://cursor.com")
        .set("Content-Type", "application/json")
}

fn build_bearer_request(url: &str, token: &str) -> ureq::Request {
    ureq::get(url)
        .timeout(std::time::Duration::from_secs(15))
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn compute_cycle_end(start_iso: &str) -> String {
    if start_iso.is_empty() {
        return String::new();
    }
    if let Some(date_part) = start_iso.get(..10) {
        let parts: Vec<&str> = date_part.split('-').collect();
        if parts.len() == 3 {
            if let (Ok(y), Ok(m), Ok(d)) = (
                parts[0].parse::<i32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<u32>(),
            ) {
                let (ny, nm) = if m >= 12 { (y + 1, 1) } else { (y, m + 1) };
                return format!("{:04}-{:02}-{:02}T00:00:00.000Z", ny, nm, d);
            }
        }
    }
    String::new()
}

// ---------------------------------------------------------------------------
// IDE API fetch (primary — auto-detected token)
// ---------------------------------------------------------------------------

/// Fetch usage via api2.cursor.sh (IDE Bearer token).
/// Converts to `CursorUsageSummary` for unified frontend handling.
pub fn fetch_usage_via_ide_api(token: &str) -> Result<CursorUsageSummary> {
    let usage_url = format!("{CURSOR_IDE_API_BASE}/auth/usage");
    let profile_url = format!("{CURSOR_IDE_API_BASE}/auth/full_stripe_profile");

    let usage_resp = build_bearer_request(&usage_url, token)
        .call()
        .context("api2 auth/usage request failed")?;
    let ide_usage: IdeUsageResponse = usage_resp
        .into_json()
        .context("parse api2 auth/usage JSON")?;

    let profile_resp = build_bearer_request(&profile_url, token)
        .call()
        .context("api2 full_stripe_profile request failed")?;
    let profile: IdeStripeProfile = profile_resp
        .into_json()
        .context("parse api2 full_stripe_profile JSON")?;

    let limit = ide_usage.gpt4.max_request_usage.unwrap_or(500.0);
    let used = ide_usage.gpt4.num_requests;

    let billing_cycle_end = compute_cycle_end(&ide_usage.start_of_month);

    let on_demand_limit_cents = profile
        .hard_spending_limit_cents
        .or(profile.soft_spending_limit_cents)
        .unwrap_or(0.0);
    let on_demand_enabled =
        on_demand_limit_cents > 0.0 || profile.usage_spending_limit_enabled.unwrap_or(false);

    let is_yearly = profile
        .extra
        .get("isYearlyPlan")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let on_demand_auto = profile
        .extra
        .get("isOnBillableAuto")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut summary = CursorUsageSummary {
        billing_cycle_start: ide_usage.start_of_month.clone(),
        billing_cycle_end,
        membership_type: profile.membership_type,
        is_yearly_plan: is_yearly,
        on_demand_auto_enabled: on_demand_auto,
        individual_usage: IndividualUsage {
            plan: UsagePlanBlock {
                enabled: true,
                used,
                limit,
                remaining: (limit - used).max(0.0),
            },
            on_demand: UsageOnDemandBlock {
                enabled: on_demand_enabled,
                used: 0.0,
                limit: on_demand_limit_cents,
                remaining: on_demand_limit_cents,
            },
        },
        team_usage: None,
    };

    let web_token = get_web_session_token();

    let try_web_summary =
        |cookie: &str| -> Option<CursorUsageSummary> { fetch_usage_summary(cookie).ok() };

    let web_result = if let Some(ref cookie) = web_token {
        try_web_summary(cookie).or_else(|| {
            invalidate_ext_cookie_cache();
            let _ = clear_cursor_session_token();
            read_cursor_usage_ext_cookie()
                .ok()
                .and_then(|c| try_web_summary(&c))
        })
    } else {
        read_cursor_usage_ext_cookie()
            .ok()
            .and_then(|c| try_web_summary(&c))
    };

    if let Some(web_summary) = web_result {
        summary.individual_usage.on_demand = web_summary.individual_usage.on_demand;
        if web_summary.team_usage.is_some() {
            summary.team_usage = web_summary.team_usage;
        }
        if !web_summary.billing_cycle_end.is_empty() {
            summary.billing_cycle_end = web_summary.billing_cycle_end;
        }
    }

    Ok(summary)
}

// ---------------------------------------------------------------------------
// Web cookie API fetch (fallback — manually pasted token)
// ---------------------------------------------------------------------------

pub fn fetch_usage_summary(token: &str) -> Result<CursorUsageSummary> {
    let url = format!("{CURSOR_API_BASE}/usage-summary");
    let resp = build_cookie_request(&url, token)
        .call()
        .map_err(|e| match &e {
            ureq::Error::Status(code, resp) => {
                let body = resp.status_text().to_string();
                anyhow::anyhow!("usage-summary HTTP {code}: {body}")
            }
            _ => anyhow::anyhow!("usage-summary request: {e}"),
        })?;
    let body: CursorUsageSummary = resp.into_json().context("parse usage-summary JSON")?;
    Ok(body)
}

fn iso_to_epoch_ms(iso: &str) -> String {
    let date_part = iso.get(..10).unwrap_or(iso);
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() == 3 {
        if let (Ok(y), Ok(m), Ok(d)) = (
            parts[0].parse::<i64>(),
            parts[1].parse::<u32>(),
            parts[2].parse::<u32>(),
        ) {
            let days_from_epoch = {
                let mut total: i64 = 0;
                for yr in 1970..y {
                    total += if yr % 4 == 0 && (yr % 100 != 0 || yr % 400 == 0) {
                        366
                    } else {
                        365
                    };
                }
                let month_days = [
                    31,
                    if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                        29
                    } else {
                        28
                    },
                    31,
                    30,
                    31,
                    30,
                    31,
                    31,
                    30,
                    31,
                    30,
                    31,
                ];
                for &md in month_days.iter().take((m as usize - 1).min(12)) {
                    total += md as i64;
                }
                total + d as i64 - 1
            };
            return (days_from_epoch * 86400 * 1000).to_string();
        }
    }
    iso.to_string()
}

pub fn fetch_usage_events(
    token: &str,
    team_id: Option<u64>,
    user_id: Option<u64>,
    start_date: &str,
    end_date: &str,
    page: u32,
    page_size: u32,
) -> Result<CursorUsageEventsPage> {
    let url = format!("{CURSOR_API_BASE}/dashboard/get-filtered-usage-events");
    let start_ms = iso_to_epoch_ms(start_date);
    let end_ms = iso_to_epoch_ms(end_date);
    let mut body = serde_json::json!({
        "startDate": start_ms,
        "endDate": end_ms,
        "page": page,
        "pageSize": page_size,
    });
    if let Some(tid) = team_id {
        body["teamId"] = serde_json::json!(tid);
    }
    if let Some(uid) = user_id {
        body["userId"] = serde_json::json!(uid);
    }
    let resp = build_cookie_post(&url, token)
        .send_json(body)
        .map_err(|e| match &e {
            ureq::Error::Status(code, _) => anyhow::anyhow!("usage-events HTTP {code}"),
            _ => anyhow::anyhow!("usage-events request: {e}"),
        })?;
    let page_result: CursorUsageEventsPage = resp.into_json().context("parse usage-events JSON")?;
    Ok(page_result)
}

// ---------------------------------------------------------------------------
// Decrypt cursor-usage extension's stored WorkosCursorSessionToken
// ---------------------------------------------------------------------------

#[cfg(any(target_os = "macos", target_os = "linux"))]
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

static CACHED_EXT_COOKIE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Get web session token with priority: memory cache → local file → keychain/DPAPI decrypt.
pub fn get_web_session_token() -> Option<String> {
    CACHED_EXT_COOKIE
        .lock()
        .unwrap()
        .clone()
        .filter(|c| !c.is_empty())
        .or_else(|| {
            read_cursor_session_token()
                .ok()
                .filter(|t| !t.trim().is_empty())
        })
        .or_else(|| read_cursor_usage_ext_cookie().ok())
}

/// Read the WorkosCursorSessionToken stored by the cursor-usage extension.
/// Result is cached in memory + persisted to local token file.
pub fn read_cursor_usage_ext_cookie() -> Result<String> {
    if let Some(ref cached) = *CACHED_EXT_COOKIE.lock().unwrap() {
        if !cached.is_empty() {
            return Ok(cached.clone());
        }
    }
    let cookie = read_cursor_usage_ext_cookie_inner()?;
    *CACHED_EXT_COOKIE.lock().unwrap() = Some(cookie.clone());
    let _ = write_cursor_session_token(&cookie);
    Ok(cookie)
}

fn invalidate_ext_cookie_cache() {
    *CACHED_EXT_COOKIE.lock().unwrap() = None;
}

/// Read encrypted cookie JSON blob from state.vscdb.
fn read_encrypted_cookie_blob() -> Result<Vec<u8>> {
    let db_path = cursor_state_vscdb_path()?;
    let conn =
        rusqlite::Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("open state.vscdb: {}", db_path.display()))?;

    let secret_key = r#"secret://{"extensionId":"yossisa.cursor-usage","key":"cursor.cookie"}"#;
    let raw: String = conn
        .query_row(
            "SELECT value FROM ItemTable WHERE key = ?1",
            [secret_key],
            |row| row.get(0),
        )
        .context("cursor-usage cookie not found in state.vscdb")?;

    let buf: serde_json::Value =
        serde_json::from_str(&raw).context("parse encrypted cookie JSON")?;
    let data_arr = buf
        .get("data")
        .and_then(|v| v.as_array())
        .context("missing data array")?;
    let encrypted: Vec<u8> = data_arr
        .iter()
        .filter_map(|v| v.as_u64().map(|n| n as u8))
        .collect();
    Ok(encrypted)
}

/// macOS: Electron SafeStorage v10 — Keychain password + PBKDF2 + AES-128-CBC.
#[cfg(target_os = "macos")]
fn read_cursor_usage_ext_cookie_inner() -> Result<String> {
    let encrypted = read_encrypted_cookie_blob()?;
    if encrypted.len() < 3 || &encrypted[..3] != b"v10" {
        anyhow::bail!("unexpected encryption version (expected v10)");
    }
    let ciphertext = &encrypted[3..];

    let pw_bytes = get_generic_password("Cursor Safe Storage", "Cursor Key")
        .map_err(|e| anyhow::anyhow!("Keychain access denied for Cursor Safe Storage: {e}"))?;
    let password = String::from_utf8_lossy(&pw_bytes).to_string();
    if password.is_empty() {
        anyhow::bail!("Cursor Safe Storage password is empty");
    }

    let mut key = [0u8; 16];
    pbkdf2::pbkdf2::<Hmac<Sha1>>(password.as_bytes(), b"saltysalt", 1003, &mut key)
        .map_err(|e| anyhow::anyhow!("PBKDF2 error: {e}"))?;

    let iv = [b' '; 16];
    let mut buf = ciphertext.to_vec();
    let decrypted = Aes128CbcDec::new(&key.into(), &iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| anyhow::anyhow!("AES-CBC decrypt error: {e}"))?;

    let cookie = String::from_utf8(decrypted.to_vec()).context("decrypted cookie is not UTF-8")?;
    Ok(cookie)
}

/// Windows: Electron SafeStorage v10 — DPAPI CryptUnprotectData.
#[cfg(target_os = "windows")]
fn read_cursor_usage_ext_cookie_inner() -> Result<String> {
    use std::ptr;
    use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    let encrypted = read_encrypted_cookie_blob()?;
    if encrypted.len() < 3 || &encrypted[..3] != b"v10" {
        anyhow::bail!("unexpected encryption version (expected v10)");
    }
    let ciphertext = &encrypted[3..];

    unsafe {
        let mut input = CRYPT_INTEGER_BLOB {
            cbData: ciphertext.len() as u32,
            pbData: ciphertext.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };
        let ret = CryptUnprotectData(
            &mut input,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            &mut output,
        );
        if ret == 0 {
            anyhow::bail!("DPAPI CryptUnprotectData failed");
        }
        let decrypted = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        windows_sys::Win32::Foundation::LocalFree(output.pbData as _);
        let cookie = String::from_utf8(decrypted).context("decrypted cookie is not UTF-8")?;
        Ok(cookie)
    }
}

/// Linux: Electron SafeStorage v11 — GNOME Keyring password + PBKDF2 + AES-128-CBC.
#[cfg(target_os = "linux")]
fn read_cursor_usage_ext_cookie_inner() -> Result<String> {
    let encrypted = read_encrypted_cookie_blob()?;
    let (ciphertext, iterations) =
        if encrypted.len() >= 3 && (encrypted[..3] == *b"v11" || encrypted[..3] == *b"v10") {
            (&encrypted[3..], 1u32)
        } else {
            anyhow::bail!("unexpected encryption version prefix");
        };

    let password = linux_get_safe_storage_password()?;

    let mut key = [0u8; 16];
    pbkdf2::pbkdf2::<Hmac<Sha1>>(password.as_bytes(), b"saltysalt", iterations, &mut key)
        .map_err(|e| anyhow::anyhow!("PBKDF2 error: {e}"))?;

    let iv = [b' '; 16];
    let mut buf = ciphertext.to_vec();
    let decrypted = Aes128CbcDec::new(&key.into(), &iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| anyhow::anyhow!("AES-CBC decrypt error: {e}"))?;

    let cookie = String::from_utf8(decrypted.to_vec()).context("decrypted cookie is not UTF-8")?;
    Ok(cookie)
}

/// Retrieve Electron Safe Storage password from GNOME Keyring via secret-tool.
#[cfg(target_os = "linux")]
fn linux_get_safe_storage_password() -> Result<String> {
    for app_name in ["Cursor Safe Storage", "Cursor"] {
        let output = std::process::Command::new("secret-tool")
            .args(["lookup", "application", app_name])
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                let pw = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !pw.is_empty() {
                    return Ok(pw);
                }
            }
        }
    }
    // Chromium/Electron fallback when no keyring is available
    Ok("peanuts".to_string())
}
