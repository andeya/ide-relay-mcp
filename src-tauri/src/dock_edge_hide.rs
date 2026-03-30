//! Main window edge tuck — focus-loss driven.
//!
//! **Model (first principles)**
//!
//! 1. **Tuck trigger — focus loss (primary):** When the main window loses
//!    focus, it immediately tucks to whichever horizontal screen edge is
//!    nearer.  [`crate::window_nearer_horizontal_edge_side`] always returns
//!    `"left"` or `"right"` — no "near-edge" proximity threshold is required.
//!
//! 2. **Tuck trigger — pointer leave (secondary):** When the cursor leaves
//!    the webview (debounced) AND is outside the native outer frame, the
//!    window tucks to the nearer edge.  This covers the case where the user
//!    moves the pointer away without switching focus.  A straddle-guard is
//!    kept so that partially off-screen windows skip the cursor-outside check.
//!
//! 3. **Expand triggers:** Window gains focus · peek-hover strip · manual
//!    command.  On expand the original tuck side is used to position the
//!    window back via [`crate::position_main_window_for_dock`].
//!
//! 4. **Anti-oscillation:**
//!    - [`SUPPRESS_COLLAPSE_AFTER_PEEK_MS`]: after a peek-hover expand, tuck
//!      from either path is suppressed briefly.
//!    - [`POST_COLLAPSE_PEEK_SUPPRESS_MS`]: after tucking, peek-hover expand
//!      is ignored so the pointer can leave the strip without an immediate
//!      bounce.
//!
//! 5. **Always-on-top while tucked:** the peek strip must stay above other
//!    windows; when expanded, restore the user preference via
//!    [`crate::read_window_always_on_top`].
//!
//! 6. **`set_window_dock` (GUI):** apply [`crate::position_main_window_for_dock`]
//!    first, then persist `window_dock.json` and clear [`EdgeHideState`].

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

/// Debounce before `dock_edge_hide_after_leave` — **single source of truth** for the GUI (see
/// `get_dock_edge_hide_ui_timing` in `main.rs`).
pub const SHELL_LEAVE_DEBOUNCE_MS: u64 = 120;

/// After hover-expanding from the peek strip, ignore tuck briefly (anti-flicker).
pub const SUPPRESS_COLLAPSE_AFTER_PEEK_MS: u64 = 280;

/// After tucking, ignore peek-hover expand until this much time has passed — breaks tuck/expand
/// feedback loops when the cursor stays near the screen edge.
pub const POST_COLLAPSE_PEEK_SUPPRESS_MS: u64 = 520;

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
    /// While `unix_ms() < this`, skip tuck from both focus-loss and mouseleave paths.
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

// ---------------------------------------------------------------------------
// Shared collapse logic
// ---------------------------------------------------------------------------

/// Core tuck routine used by both focus-loss and pointer-leave paths.
/// Determines the nearer screen edge via [`crate::window_nearer_horizontal_edge_side`]
/// and moves the window off-screen to that side.
fn do_collapse(_app: &AppHandle, win: &tauri::WebviewWindow, state: &Mutex<EdgeHideState>) {
    let side = match crate::window_nearer_horizontal_edge_side(win) {
        Ok(s) => s,
        Err(_) => return,
    };

    if crate::collapse_window_for_edge_hide(win, &side).is_err() {
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

/// Common pre-collapse guards shared by both tuck paths.
/// Returns `true` if tuck should proceed.
fn should_collapse(state: &Mutex<EdgeHideState>) -> bool {
    let g = match state.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    if g.collapsed {
        return false;
    }
    if unix_ms() < g.suppress_collapse_until_ms {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Tuck path 1: focus loss
// ---------------------------------------------------------------------------

/// Tuck when the main window loses OS focus (user clicked another app).
///
/// No cursor-position check is needed — focus loss is a definitive signal.
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
    if !should_collapse(&state) {
        return;
    }
    do_collapse(app, &win, &state);
}

// ---------------------------------------------------------------------------
// Tuck path 2: pointer left webview (debounced)
// ---------------------------------------------------------------------------

/// Pointer left webview (debounced) — tuck when enabled.
pub fn collapse_after_leave(app: &AppHandle) -> Result<(), String> {
    if !crate::read_dock_edge_hide() {
        return Ok(());
    }
    let Some(win) = app.get_webview_window("main") else {
        return Ok(());
    };
    let Some(state) = app.try_state::<Mutex<EdgeHideState>>() else {
        return Ok(());
    };
    if !should_collapse(&state) {
        return Ok(());
    }

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

    do_collapse(app, &win, &state);
    Ok(())
}

// ---------------------------------------------------------------------------
// Peek-hover expand
// ---------------------------------------------------------------------------

pub fn try_expand_from_peek_hover(app: &AppHandle) {
    if !peek_fast_poll_wanted() {
        return;
    }
    if !crate::read_dock_edge_hide() {
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
