//! UI locale, window dock position, and MCP pause settings.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Mutex;
use tauri::{PhysicalPosition, WebviewWindow};

/// Serialises read-modify-write cycles on `window_dock.json` so concurrent
/// callers (e.g. dock-position change + always-on-top toggle) cannot lose
/// each other's updates.
static DOCK_CONFIG_LOCK: Mutex<()> = Mutex::new(());

/// Visible strip (px) when the window is tucked to left/right edge.
pub const DOCK_EDGE_HIDE_PEEK_PX: i32 = 14;

/// Outer window left/right vs current monitor left/right (same coordinate space as Tauri `outer_position`).
fn window_horizontal_edges_vs_monitor(
    win: &WebviewWindow,
) -> std::result::Result<(i32, i32, i32, i32), String> {
    let pos = win.outer_position().map_err(|e| e.to_string())?;
    let outer = win.outer_size().map_err(|e| e.to_string())?;
    let win_l = pos.x;
    let win_r = pos.x + outer.width as i32;
    let mon = win
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no monitor".to_string())?;
    let scr_l = mon.position().x;
    let scr_r = scr_l + mon.size().width as i32;
    Ok((win_l, win_r, scr_l, scr_r))
}

/// True if part of the window's outer rect extends past the left or right edge of [`WebviewWindow::current_monitor`].
pub fn window_outer_straddles_screen_edge(
    win: &WebviewWindow,
) -> std::result::Result<bool, String> {
    let (win_l, win_r, scr_l, scr_r) = window_horizontal_edges_vs_monitor(win)?;
    Ok(win_l < scr_l || win_r > scr_r)
}

/// True if the OS cursor lies **outside** the window's outer frame (desktop coordinates).
///
/// Uses Tauri [`WebviewWindow::cursor_position`] with [`WebviewWindow::outer_position`] /
/// [`WebviewWindow::outer_size`] so the decision does not depend on a third-party global mouse crate
/// or mixed DPI heuristics.
#[cfg(desktop)]
pub fn desktop_cursor_outside_outer_window(
    win: &WebviewWindow,
) -> std::result::Result<bool, String> {
    let c = win.cursor_position().map_err(|e| e.to_string())?;
    let o = win.outer_position().map_err(|e| e.to_string())?;
    let s = win.outer_size().map_err(|e| e.to_string())?;
    let rx = c.x - o.x as f64;
    let ry = c.y - o.y as f64;
    let w = s.width as f64;
    let h = s.height as f64;
    const SL: f64 = 2.0;
    let outside = rx < -SL || rx >= w + SL || ry < -SL || ry >= h + SL;
    Ok(outside)
}

#[cfg(not(desktop))]
pub fn desktop_cursor_outside_outer_window(
    _win: &WebviewWindow,
) -> std::result::Result<bool, String> {
    Ok(false)
}

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
    /// When true, main window can tuck to the screen edge (see `dock_edge_hide` module).
    #[serde(default)]
    pub dock_edge_hide: bool,
    /// When true, keep main window always on top.
    #[serde(default)]
    pub window_always_on_top: bool,
    /// When true, closing the window hides it to the system tray instead of quitting.
    #[serde(default = "default_close_to_tray")]
    pub close_to_tray: bool,
}

fn default_window_dock() -> String {
    "left".to_string()
}

fn default_close_to_tray() -> bool {
    true
}

fn default_window_dock_config() -> WindowDockConfig {
    WindowDockConfig {
        dock: default_window_dock(),
        dock_edge_hide: false,
        window_always_on_top: false,
        close_to_tray: default_close_to_tray(),
    }
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

fn read_window_dock_config_or_default() -> WindowDockConfig {
    let Ok(dir) = user_data_dir() else {
        return default_window_dock_config();
    };
    let path = dir.join(WINDOW_DOCK_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return default_window_dock_config(),
    };
    serde_json::from_str(&text).unwrap_or_else(|_| default_window_dock_config())
}

fn write_window_dock_config(cfg: &WindowDockConfig) -> Result<()> {
    let dir = user_data_dir()?;
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join(WINDOW_DOCK_FILE),
        serde_json::to_string_pretty(cfg)?,
    )?;
    Ok(())
}

/// Atomically read-modify-write `window_dock.json` under [`DOCK_CONFIG_LOCK`].
fn update_window_dock_config(f: impl FnOnce(&mut WindowDockConfig)) -> Result<()> {
    let _guard = DOCK_CONFIG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut cfg = read_window_dock_config_or_default();
    f(&mut cfg);
    write_window_dock_config(&cfg)
}

/// Persisted horizontal dock; default **left**.
pub fn read_window_dock() -> String {
    let cfg = read_window_dock_config_or_default();
    match cfg.dock.as_str() {
        "center" => "center".to_string(),
        "right" => "right".to_string(),
        _ => "left".to_string(),
    }
}

pub fn read_dock_edge_hide() -> bool {
    read_window_dock_config_or_default().dock_edge_hide
}

pub fn read_window_always_on_top() -> bool {
    read_window_dock_config_or_default().window_always_on_top
}

pub fn write_window_dock(dock: &str) -> Result<()> {
    let d = dock.trim();
    if d != "left" && d != "center" && d != "right" {
        bail!("dock must be left, center, or right");
    }
    update_window_dock_config(|cfg| cfg.dock = d.to_string())
}

pub fn write_dock_edge_hide(enabled: bool) -> Result<()> {
    update_window_dock_config(|cfg| cfg.dock_edge_hide = enabled)
}

pub fn write_window_always_on_top(enabled: bool) -> Result<()> {
    update_window_dock_config(|cfg| cfg.window_always_on_top = enabled)
}

pub fn read_close_to_tray() -> bool {
    read_window_dock_config_or_default().close_to_tray
}

pub fn write_close_to_tray(enabled: bool) -> Result<()> {
    update_window_dock_config(|cfg| cfg.close_to_tray = enabled)
}

/// Slide main window so only a thin strip remains visible on the docked edge (left or right).
pub fn collapse_window_for_edge_hide(
    win: &WebviewWindow,
    dock: &str,
) -> std::result::Result<(), String> {
    if dock == "center" {
        return Ok(());
    }
    let outer = win.outer_size().map_err(|e| e.to_string())?;
    let w_win = outer.width as i32;
    let pos = win.outer_position().map_err(|e| e.to_string())?;
    let y = pos.y;
    let mon = win
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no monitor".to_string())?;
    let p = mon.position();
    let sz = mon.size();
    let mw = sz.width as i32;
    let margin = 12i32;
    let peek = DOCK_EDGE_HIDE_PEEK_PX.min(w_win.saturating_sub(1)).max(1);
    let x = if dock == "right" {
        p.x + mw - margin - peek
    } else {
        p.x + margin - (w_win - peek)
    };
    win.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Hover-to-expand when collapsed: **window outer rect only** (physical desktop pixels).
///
/// Does **not** use a full monitor-edge band — such a band
/// is wide enough that a cursor on the right/left side of the screen (but not on the peek strip
/// window) still matched, causing spurious expand/collapse.
///
/// **Coordinate space:** [`WebviewWindow::cursor_position`], [`WebviewWindow::outer_position`], and
/// [`WebviewWindow::outer_size`] are all physical pixels in the same desktop space (Tauri on
/// macOS/Windows). Older builds tried `mx/sf` and `mx*sf` “variants”; those produced false positives
/// when the real cursor was hundreds of px away (e.g. over another window) because one variant could
/// accidentally align with the outer rect.
pub fn mouse_in_dock_edge_peek_zone_window_only(
    win: &WebviewWindow,
    dock: &str,
    mx: i32,
    my: i32,
) -> std::result::Result<bool, String> {
    if dock == "center" {
        return Ok(false);
    }
    let pos = win.outer_position().map_err(|e| e.to_string())?;
    let sz = win.outer_size().map_err(|e| e.to_string())?;
    let wx0 = pos.x;
    let wy = pos.y;
    let ww = sz.width as i32;
    let wh = sz.height as i32;

    let y_ok = |cy: i32| cy >= wy.saturating_sub(4) && cy < wy + wh + 4;

    if mx >= wx0 && mx < wx0 + ww && y_ok(my) {
        return Ok(true);
    }

    Ok(false)
}

/// Always returns `"left"` or `"right"` — whichever horizontal screen edge
/// the window centre is closer to.  Used as the tuck side for edge-hide
/// collapse (both focus-loss and pointer-leave paths).
pub fn window_nearer_horizontal_edge_side(
    win: &WebviewWindow,
) -> std::result::Result<String, String> {
    let (win_l, win_r, scr_l, scr_r) = window_horizontal_edges_vs_monitor(win)?;
    let win_cx = (win_l + win_r) / 2;
    let scr_cx = (scr_l + scr_r) / 2;
    if win_cx <= scr_cx {
        Ok("left".to_string())
    } else {
        Ok("right".to_string())
    }
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

/// When true, `relay mcp-<ide>` skips GUI/auto-reply and returns a sentinel tool result immediately.
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
