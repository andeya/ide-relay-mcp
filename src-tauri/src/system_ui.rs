//! System tray icon, macOS Dock menu, and shared "new window" spawning.

use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager,
};

/// Spawn a detached instance of the current executable.
///
/// Used by both the system tray "New Window" action and the macOS Dock menu item.
fn spawn_new_window() {
    let Ok(exe) = std::env::current_exe() else {
        eprintln!("relay: cannot resolve current executable path; New Window aborted");
        return;
    };
    let _ = std::process::Command::new(exe)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

// ---------------------------------------------------------------------------
// System Tray (cross-platform)
// ---------------------------------------------------------------------------

/// Build the system-tray icon with Show / New Window / Quit actions.
///
/// Language follows the **app config** (`read_ui_locale`) so it stays consistent
/// with the in-app UI.
pub fn setup_system_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let is_zh = relay_mcp::config::read_ui_locale() == "zh";
    let show_label = if is_zh { "显示窗口" } else { "Show Window" };
    let new_window_label = if is_zh { "新建窗口" } else { "New Window" };
    let quit_label = if is_zh { "退出" } else { "Quit" };

    let show = MenuItemBuilder::with_id("show", show_label).build(app)?;
    let new_window = MenuItemBuilder::with_id("new_window", new_window_label).build(app)?;
    let quit = MenuItemBuilder::with_id("quit", quit_label).build(app)?;
    let menu = MenuBuilder::new(app)
        .items(&[&show, &new_window])
        .separator()
        .item(&quit)
        .build()?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("default window icon must be set in tauri.conf.json")?;

    // Tauri internally registers the tray on the app handle, so a binding that
    // merely outlives setup is sufficient to keep it alive.
    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip(relay_mcp::ide::window_title())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }
            "new_window" => spawn_new_window(),
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(w) = tray.app_handle().get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }
        })
        .build(app)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// macOS Dock menu (native ObjC runtime)
// ---------------------------------------------------------------------------

/// Returns `true` when the macOS system preferred language begins with "zh".
#[cfg(target_os = "macos")]
fn macos_system_locale_is_zh() -> bool {
    std::process::Command::new("defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.contains("\"zh"))
        .unwrap_or(false)
}

/// Register a native macOS Dock right-click menu with a "New Window" item.
///
/// Tauri/tao does not expose the NSApplication dock menu API, so we dynamically
/// inject `applicationDockMenu:` into the existing app-delegate class via the
/// Objective-C runtime.  The menu item spawns a new process (same as the tray
/// icon "New Window" action).
///
/// Language follows the **macOS system locale** so it matches the standard Dock
/// items (Hide, Quit, etc.) that macOS always renders in the system language.
#[cfg(target_os = "macos")]
pub fn setup_macos_dock_menu() {
    let is_zh = macos_system_locale_is_zh();

    type Id = *mut std::ffi::c_void;
    type Sel = *mut std::ffi::c_void;
    type Cls = *mut std::ffi::c_void;

    #[link(name = "objc", kind = "dylib")]
    #[allow(improper_ctypes)]
    extern "C" {
        fn objc_getClass(name: *const std::ffi::c_char) -> Cls;
        fn sel_registerName(name: *const std::ffi::c_char) -> Sel;
        fn objc_msgSend();
        fn class_addMethod(
            cls: Cls,
            sel: Sel,
            imp: *const std::ffi::c_void,
            types: *const std::ffi::c_char,
        ) -> bool;
        fn object_getClass(obj: Id) -> Cls;
    }

    unsafe fn msg0(obj: Id, s: Sel) -> Id {
        let f: unsafe extern "C" fn(Id, Sel) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        f(obj, s)
    }
    unsafe fn msg1(obj: Id, s: Sel, a: Id) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, Id) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        f(obj, s, a)
    }
    unsafe fn msg3(obj: Id, s: Sel, a: Id, b: Sel, c: Id) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, Id, Sel, Id) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        f(obj, s, a, b, c)
    }
    unsafe fn sel(n: &[u8]) -> Sel {
        sel_registerName(n.as_ptr() as *const _)
    }
    unsafe fn clz(n: &[u8]) -> Cls {
        objc_getClass(n.as_ptr() as *const _)
    }
    unsafe fn nsstr(s: &str) -> Id {
        let c = std::ffi::CString::new(s).unwrap();
        msg1(
            clz(b"NSString\0"),
            sel(b"stringWithUTF8String:\0"),
            c.as_ptr() as Id,
        )
    }

    static DOCK_MENU: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

    extern "C" fn handle_new_window(_this: Id, _cmd: Sel, _sender: Id) {
        spawn_new_window();
    }

    extern "C" fn provide_dock_menu(_this: Id, _cmd: Sel, _app: Id) -> Id {
        DOCK_MENU.get().copied().unwrap_or(0) as Id
    }

    unsafe {
        let menu = msg0(msg0(clz(b"NSMenu\0"), sel(b"alloc\0")), sel(b"init\0"));

        let label = if is_zh {
            "\u{65b0}\u{5efa}\u{7a97}\u{53e3}"
        } else {
            "New Window"
        };
        let action = sel(b"dockNewWindow:\0");
        let item = msg3(
            msg0(clz(b"NSMenuItem\0"), sel(b"alloc\0")),
            sel(b"initWithTitle:action:keyEquivalent:\0"),
            nsstr(label),
            action,
            nsstr(""),
        );
        msg1(menu, sel(b"addItem:\0"), item);
        msg0(menu, sel(b"retain\0"));
        let _ = DOCK_MENU.set(menu as usize);

        let app = msg0(clz(b"NSApplication\0"), sel(b"sharedApplication\0"));
        let delegate = msg0(app, sel(b"delegate\0"));
        if delegate.is_null() {
            eprintln!("relay: NSApp delegate is nil; dock menu not installed");
            return;
        }
        let delegate_cls = object_getClass(delegate);
        msg1(item, sel(b"setTarget:\0"), delegate);

        class_addMethod(
            delegate_cls,
            action,
            handle_new_window as *const std::ffi::c_void,
            c"v@:@".as_ptr(),
        );
        class_addMethod(
            delegate_cls,
            sel(b"applicationDockMenu:\0"),
            provide_dock_menu as *const std::ffi::c_void,
            c"@@:@".as_ptr(),
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub fn setup_macos_dock_menu() {}
