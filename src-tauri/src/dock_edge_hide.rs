//! Main window edge tuck (QQ-style): pointer leave → tuck; peek hover / focus → expand.
//!
//! **Model (first principles)**
//! - **Tuck** is driven only by the debounced webview `mouseleave` path, plus guards: unless the
//!   window **straddles** a screen edge ([`crate::window_outer_straddles_screen_edge`]), the OS cursor
//!   must be **outside** the native outer frame ([`crate::desktop_cursor_outside_outer_window`]);
//!   the window must be **near enough to a horizontal screen edge** ([`crate::window_nearest_horizontal_screen_edge_side`]);
//!   tuck side is the **nearer** left/right edge (not `window_dock.json`).
//! - Not during [`SUPPRESS_COLLAPSE_AFTER_PEEK_MS`] after a peek-driven expand.
//! - **Expand** uses [`crate::position_main_window_for_dock`] with the **edge used at tuck time** (stored in state).
//! - **Oscillation** (tuck → peek poll expands → `mouseleave` tucks again) is prevented by
//!   [`POST_COLLAPSE_PEEK_SUPPRESS_MS`]: right after a tuck, peek-hover expand is ignored briefly so
//!   the pointer can leave the peek strip without an immediate re-expand while the window settles.
//!   We intentionally do **not** defer tuck based on a screen-edge “shallow” band (that broke slow
//!   drags and duplicated DPI heuristics).
//! - **Peek hit test** uses [`crate::mouse_in_dock_edge_peek_zone_window_only`] only (no monitor-wide band).
//! - While tucked, the window uses always-on-top so the peek strip stays above other windows;
//!   when expanded, restore [`crate::read_window_always_on_top`] (user preference), not hard-coded off.
//! - **`set_window_dock` (GUI):** apply [`crate::position_main_window_for_dock`] first, then persist
//!   `window_dock.json` and clear [`EdgeHideState`], so a failed move never leaves stale state.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

/// Debounce before `dock_edge_hide_after_leave` — **single source of truth** for the GUI (see
/// `get_dock_edge_hide_ui_timing` in `main.rs`).
pub const SHELL_LEAVE_DEBOUNCE_MS: u64 = 120;

/// After hover-expanding from the peek strip, ignore `mouseleave`-driven tuck briefly (anti-flicker).
pub const SUPPRESS_COLLAPSE_AFTER_PEEK_MS: u64 = 280;

/// After tucking, ignore peek-hover expand until this much time has passed — breaks tuck/expand
/// feedback loops when the cursor stays near the screen edge (replaces the old shallow-strip defer).
pub const POST_COLLAPSE_PEEK_SUPPRESS_MS: u64 = 520;

/// Milliseconds since UNIX epoch (for collapse suppression after peek-expand).
fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// When false, background thread sleeps long and skips cursor polling (saves CPU).
static PEEK_FAST_POLL: AtomicBool = AtomicBool::new(false);

/// Require this many consecutive poll ticks with the cursor in the peek rect before expanding
/// (avoids expand/collapse flutter when coordinates disagree by one pixel).
const PEEK_HOVER_EXPAND_CONSECUTIVE: u32 = 3;
static PEEK_HOVER_IN_ZONE_STREAK: AtomicU32 = AtomicU32::new(0);

#[inline]
pub fn set_peek_fast_poll(active: bool) {
    PEEK_FAST_POLL.store(active, Ordering::Relaxed);
    if active {
        PEEK_HOVER_IN_ZONE_STREAK.store(0, Ordering::Relaxed);
    }
}

#[inline]
pub fn peek_fast_poll_wanted() -> bool {
    PEEK_FAST_POLL.load(Ordering::Relaxed)
}

#[derive(Default)]
pub struct EdgeHideState {
    pub collapsed: bool,
    /// `"left"` / `"right"` — screen edge used when tucking (geometry at tuck time).
    pub tuck_side: Option<String>,
    /// While `unix_ms() < this`, skip tuck from debounced shell `mouseleave` (peek-hover path).
    pub suppress_collapse_until_ms: u64,
    /// While `unix_ms() < this`, [`try_expand_from_peek_hover`] does not expand (post-tuck settle).
    pub suppress_peek_expand_until_ms: u64,
}

/// `Ok(true)` if the window was collapsed and restored using [`crate::position_main_window_for_dock`].
pub fn expand_if_collapsed(app: &AppHandle) -> Result<bool, String> {
    let Some(win) = app.get_webview_window("main") else {
        return Ok(false);
    };
    let Some(state) = app.try_state::<Mutex<EdgeHideState>>() else {
        return Ok(false);
    };
    let mut g = state.lock().map_err(|e| e.to_string())?;
    if !g.collapsed {
        return Ok(false);
    }
    let side = g.tuck_side.clone().unwrap_or_else(crate::read_window_dock);
    let restore_tuck_side = g.tuck_side.clone();
    g.collapsed = false;
    g.tuck_side = None;
    g.suppress_collapse_until_ms = 0;
    g.suppress_peek_expand_until_ms = 0;
    set_peek_fast_poll(false);
    drop(g);

    match crate::position_main_window_for_dock(&win, &side) {
        Ok(()) => {
            let _ = win.set_always_on_top(crate::read_window_always_on_top());
            Ok(true)
        }
        Err(e) => {
            if let Ok(mut g) = state.lock() {
                g.collapsed = true;
                g.tuck_side = restore_tuck_side;
                set_peek_fast_poll(true);
            }
            Err(e.to_string())
        }
    }
}

pub fn handle_main_window_focus(app: &AppHandle, focused: bool) {
    if focused {
        let _ = expand_if_collapsed(app);
    } else {
        collapse_on_focus_lost(app);
    }
}

/// Tuck when the window loses focus (user clicked another app).
///
/// Same guards as [`collapse_after_leave`] except the cursor-outside-window
/// check is skipped — focus loss is a definitive signal that the user has
/// moved to another application.
fn collapse_on_focus_lost(app: &AppHandle) {
    if !crate::read_dock_edge_hide() {
        return;
    }
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let Some(state) = app.try_state::<Mutex<EdgeHideState>>() else {
        return;
    };
    let g = match state.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if g.collapsed {
        return;
    }
    if unix_ms() < g.suppress_collapse_until_ms {
        return;
    }
    drop(g);

    let side = match crate::window_nearest_horizontal_screen_edge_side(&win) {
        Ok(Some(s)) => s,
        _ => return,
    };

    if crate::collapse_window_for_edge_hide(&win, &side).is_err() {
        return;
    }

    let Ok(mut g) = state.lock() else { return };
    if g.collapsed {
        return;
    }
    let now = unix_ms();
    g.collapsed = true;
    g.tuck_side = Some(side);
    g.suppress_peek_expand_until_ms = now.saturating_add(POST_COLLAPSE_PEEK_SUPPRESS_MS);
    drop(g);

    let _ = win.set_always_on_top(true);
    set_peek_fast_poll(true);
}

pub fn try_expand_from_peek_hover(app: &AppHandle) {
    if !peek_fast_poll_wanted() {
        return;
    }
    let hide = crate::read_dock_edge_hide();
    if !hide {
        return;
    }
    let Some(state) = app.try_state::<Mutex<EdgeHideState>>() else {
        return;
    };
    let (collapsed, peek_suppressed, tuck_side) = state
        .lock()
        .map(|g| {
            (
                g.collapsed,
                unix_ms() < g.suppress_peek_expand_until_ms,
                g.tuck_side.clone(),
            )
        })
        .unwrap_or((false, false, None));
    let peek_side = tuck_side.unwrap_or_else(crate::read_window_dock);
    if peek_side == "center" {
        return;
    }
    if !collapsed {
        return;
    }
    if peek_suppressed {
        PEEK_HOVER_IN_ZONE_STREAK.store(0, Ordering::Relaxed);
        return;
    }
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let c = match win.cursor_position() {
        Ok(p) => p,
        Err(_) => return,
    };
    let mx = c.x.round() as i32;
    let my = c.y.round() as i32;
    let Ok(in_zone) = crate::mouse_in_dock_edge_peek_zone_window_only(&win, &peek_side, mx, my)
    else {
        return;
    };
    if !in_zone {
        PEEK_HOVER_IN_ZONE_STREAK.store(0, Ordering::Relaxed);
        return;
    }
    let streak = PEEK_HOVER_IN_ZONE_STREAK.fetch_add(1, Ordering::Relaxed) + 1;
    if streak < PEEK_HOVER_EXPAND_CONSECUTIVE {
        return;
    }
    PEEK_HOVER_IN_ZONE_STREAK.store(0, Ordering::Relaxed);
    if let Ok(true) = expand_if_collapsed(app) {
        if let Some(state) = app.try_state::<Mutex<EdgeHideState>>() {
            if let Ok(mut g) = state.lock() {
                g.suppress_collapse_until_ms =
                    unix_ms().saturating_add(SUPPRESS_COLLAPSE_AFTER_PEEK_MS);
            }
        }
    }
}

/// Pointer left webview (debounced) — tuck when enabled.
pub fn collapse_after_leave(app: &AppHandle) -> Result<(), String> {
    let hide = crate::read_dock_edge_hide();
    if !hide {
        return Ok(());
    }
    let Some(win) = app.get_webview_window("main") else {
        return Ok(());
    };
    let Some(state) = app.try_state::<Mutex<EdgeHideState>>() else {
        return Ok(());
    };
    let g = state.lock().map_err(|e| e.to_string())?;
    if g.collapsed {
        return Ok(());
    }
    if unix_ms() < g.suppress_collapse_until_ms {
        return Ok(());
    }
    drop(g);

    let side = match crate::window_nearest_horizontal_screen_edge_side(&win)? {
        Some(s) => s,
        None => return Ok(()),
    };

    // If the window already straddles a screen edge, the pointer can still lie inside the full outer
    // rect (which includes off-screen pixels); do not require cursor-outside-outer in that case.
    let straddles = match crate::window_outer_straddles_screen_edge(&win) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    if !straddles {
        match crate::desktop_cursor_outside_outer_window(&win) {
            Ok(true) => {}
            Ok(false) | Err(_) => return Ok(()),
        }
    }

    crate::collapse_window_for_edge_hide(&win, &side)?;

    let mut g = state.lock().map_err(|e| e.to_string())?;
    if g.collapsed {
        return Ok(());
    }
    let now = unix_ms();
    g.collapsed = true;
    g.tuck_side = Some(side);
    g.suppress_peek_expand_until_ms = now.saturating_add(POST_COLLAPSE_PEEK_SUPPRESS_MS);
    drop(g);

    // Peek strip must stay above other windows or hover-to-expand cannot receive the cursor.
    let _ = win.set_always_on_top(true);
    set_peek_fast_poll(true);
    Ok(())
}
