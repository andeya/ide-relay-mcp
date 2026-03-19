//! UI locale, window dock position, and MCP pause settings.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use tauri::{PhysicalPosition, WebviewWindow};

use crate::user_data_dir;

pub const UI_LOCALE_FILE: &str = "ui_locale.json";
pub const WINDOW_DOCK_FILE: &str = "window_dock.json";
pub const MCP_PAUSE_FILE: &str = "mcp_pause.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct UiLocaleConfig {
    pub lang: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowDockConfig {
    #[serde(default = "default_window_dock")]
    pub dock: String,
}

fn default_window_dock() -> String {
    "left".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpPauseConfig {
    #[serde(default)]
    pub paused: bool,
}

/// UI language persisted next to auto-reply config. Default `en`.
pub fn read_ui_locale() -> String {
    let path = match user_data_dir() {
        Ok(dir) => dir.join(UI_LOCALE_FILE),
        Err(_) => return "en".to_string(),
    };
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return "en".to_string(),
    };
    let cfg: UiLocaleConfig = match serde_json::from_str(&text) {
        Ok(c) => c,
        Err(_) => return "en".to_string(),
    };
    match cfg.lang.as_str() {
        "zh" => "zh".to_string(),
        _ => "en".to_string(),
    }
}

pub fn write_ui_locale(lang: &str) -> Result<()> {
    if lang != "en" && lang != "zh" {
        return Err(anyhow!("locale must be en or zh"));
    }
    let dir = user_data_dir()?;
    fs::create_dir_all(&dir)?;
    let path = dir.join(UI_LOCALE_FILE);
    let cfg = UiLocaleConfig {
        lang: lang.to_string(),
    };
    fs::write(path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}

/// Persisted horizontal dock; default **left**.
pub fn read_window_dock() -> String {
    let Ok(dir) = user_data_dir() else {
        return "left".to_string();
    };
    let path = dir.join(WINDOW_DOCK_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return "left".to_string(),
    };
    let cfg: WindowDockConfig = match serde_json::from_str(&text) {
        Ok(c) => c,
        Err(_) => return "left".to_string(),
    };
    match cfg.dock.as_str() {
        "center" => "center".to_string(),
        "right" => "right".to_string(),
        _ => "left".to_string(),
    }
}

pub fn write_window_dock(dock: &str) -> Result<()> {
    let d = dock.trim();
    if d != "left" && d != "center" && d != "right" {
        bail!("dock must be left, center, or right");
    }
    let dir = user_data_dir()?;
    fs::create_dir_all(&dir)?;
    let path = dir.join(WINDOW_DOCK_FILE);
    let cfg = WindowDockConfig {
        dock: d.to_string(),
    };
    fs::write(path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}

/// Vertically centered on work area; horizontal by `dock`.
pub fn position_main_window_for_dock(
    win: &WebviewWindow,
    dock: &str,
) -> std::result::Result<(), String> {
    let outer = win.outer_size().map_err(|e| e.to_string())?;
    let w_win = outer.width as i32;
    let h_win = outer.height as i32;
    let mon = win
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no monitor".to_string())?;
    let p = mon.position();
    let sz = mon.size();
    let mw = sz.width as i32;
    let mh = sz.height as i32;
    let y = p.y + (mh.saturating_sub(h_win)) / 2;
    let margin = 12i32;
    let x = match dock {
        "center" => p.x + (mw.saturating_sub(w_win)) / 2,
        "right" => p.x + mw.saturating_sub(w_win).saturating_sub(margin),
        _ => p.x + margin,
    };
    win.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// When true, `relay mcp` skips GUI/auto-reply and returns a sentinel tool result immediately.
pub fn read_mcp_paused() -> bool {
    let Ok(dir) = user_data_dir() else {
        return false;
    };
    let path = dir.join(MCP_PAUSE_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return false,
    };
    serde_json::from_str::<McpPauseConfig>(&text)
        .map(|c| c.paused)
        .unwrap_or(false)
}

pub fn write_mcp_paused(paused: bool) -> Result<()> {
    let dir = user_data_dir()?;
    fs::create_dir_all(&dir)?;
    let path = dir.join(MCP_PAUSE_FILE);
    let cfg = McpPauseConfig { paused };
    fs::write(path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}
