//! Permanent PATH for `relay` (user-level, cross-platform): install/uninstall.

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{gui_binary_name, relay_cli_directory};

const RELAY_PATH_BLOCK_BEGIN: &str = "# ----- BEGIN RELAY_MCP_PATH (managed by Relay app) -----";
const RELAY_PATH_BLOCK_END: &str = "# ----- END RELAY_MCP_PATH -----";
const RELAY_PATH_MARKER: &str = "# Relay MCP PATH (managed by Relay app)";

#[cfg(windows)]
const RELAY_MCP_PATH_REGISTRY_VALUE: &str = "RelayMCPPath";

fn user_home_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(windows)]
fn paths_same_bin_dir(a: &Path, b: &Path) -> bool {
    let ca = fs::canonicalize(a);
    let cb = fs::canonicalize(b);
    match (ca, cb) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

#[cfg(windows)]
fn windows_user_path_has_dir(target: &Path) -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(env) = hkcu.open_subkey("Environment") else {
        return false;
    };
    let Ok(path_val) = env.get_value::<String, _>("Path") else {
        return false;
    };
    for part in path_val.split(';') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if paths_same_bin_dir(Path::new(p), target) {
            return true;
        }
    }
    false
}

#[cfg(windows)]
fn windows_append_user_path(target: &Path) -> Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    if windows_user_path_has_dir(target) {
        return Ok(());
    }
    let dir_s = target.to_string_lossy().to_string();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (env, _) = hkcu
        .create_subkey("Environment")
        .context("open HKCU\\Environment")?;
    let path_val: String = env.get_value("Path").unwrap_or_default();
    let new_val = if path_val.trim().is_empty() {
        dir_s.clone()
    } else {
        format!("{};{}", path_val.trim_end_matches(';'), dir_s)
    };
    env.set_value("Path", &new_val)
        .context("set user Path in registry")?;
    env.set_value(RELAY_MCP_PATH_REGISTRY_VALUE, &dir_s)
        .context("set RelayMCPPath marker in registry")?;
    notify_windows_environment_path_changed();
    Ok(())
}

#[cfg(windows)]
fn notify_windows_environment_path_changed() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const HWND_BROADCAST: isize = 0xffff;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;
    #[link(name = "user32")]
    extern "system" {
        fn SendMessageTimeoutW(
            hwnd: isize,
            msg: u32,
            wparam: usize,
            lparam: *const u16,
            flags: u32,
            timeout: u32,
            result: *mut usize,
        ) -> isize;
    }
    let wide: Vec<u16> = OsStr::new("Environment")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut out = 0usize;
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            wide.as_ptr(),
            SMTO_ABORTIFHUNG,
            5000,
            &mut out,
        );
    }
}

#[cfg(not(windows))]
fn fish_config_path(home: &Path) -> PathBuf {
    home.join(".config").join("fish").join("config.fish")
}

#[cfg(not(windows))]
fn unix_rc_parse_path_line(line: &str) -> Option<PathBuf> {
    let line = line.trim_start();
    if line.starts_with("export PATH=\"") {
        let after_quote = line.strip_prefix("export PATH=\"")?;
        let dir = after_quote
            .split(":$PATH")
            .next()?
            .trim_end_matches('"')
            .trim();
        if dir.is_empty() {
            return None;
        }
        return Some(PathBuf::from(dir));
    }
    if line.starts_with("fish_add_path ") {
        let token = line.strip_prefix("fish_add_path ")?.trim();
        let path_str = if token.starts_with('"') {
            token
                .strip_prefix('"')?
                .strip_suffix('"')?
                .replace("\\\"", "\"")
        } else {
            token.to_string()
        };
        if path_str.is_empty() {
            return None;
        }
        return Some(PathBuf::from(path_str));
    }
    None
}

#[cfg(not(windows))]
fn unix_rc_extract_relay_path(content: &str) -> Option<PathBuf> {
    let lines: Vec<&str> = content.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_end_matches('\r');
        if trimmed == RELAY_PATH_BLOCK_BEGIN {
            for line in lines.iter().skip(i + 1) {
                let raw = line.trim_end_matches('\r').trim();
                if raw == RELAY_PATH_BLOCK_END {
                    break;
                }
                if let Some(p) = unix_rc_parse_path_line(raw) {
                    return Some(p);
                }
            }
            continue;
        }
        if trimmed == RELAY_PATH_MARKER {
            let next = lines.get(i + 1)?.trim_start();
            if let Some(p) = unix_rc_parse_path_line(next) {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn unix_rc_content_path_matches(content: &str, dir: &Path) -> bool {
    let Some(written) = unix_rc_extract_relay_path(content) else {
        return false;
    };
    let Ok(current_canon) = dir.canonicalize() else {
        return false;
    };
    let Ok(written_canon) = written.canonicalize() else {
        return false;
    };
    written_canon == current_canon
}

#[cfg(not(windows))]
fn unix_rc_find_configured_path(home: &Path) -> Option<PathBuf> {
    let check = |path: &Path| {
        let s = fs::read_to_string(path).ok()?;
        unix_rc_extract_relay_path(&s)
    };
    let fish = fish_config_path(home);
    if let Some(p) = check(&fish) {
        return Some(p);
    }
    for name in [".zshrc", ".bash_profile", ".bashrc", ".profile"] {
        if let Some(p) = check(&home.join(name)) {
            return Some(p);
        }
    }
    None
}

#[cfg(not(windows))]
fn unix_rc_path_matches_current(home: &Path, current_dir: &Path) -> bool {
    let Ok(current_canon) = current_dir.canonicalize() else {
        return false;
    };
    let check = |path: &Path| {
        let s = fs::read_to_string(path).ok()?;
        let written = unix_rc_extract_relay_path(&s)?;
        let written_canon = written.canonicalize().ok()?;
        Some(written_canon == current_canon)
    };
    let fish = fish_config_path(home);
    if check(&fish).unwrap_or(false) {
        return true;
    }
    for name in [".zshrc", ".bash_profile", ".bashrc", ".profile"] {
        if check(&home.join(name)).unwrap_or(false) {
            return true;
        }
    }
    false
}

#[cfg(not(windows))]
fn fish_add_path_token(dir: &Path) -> String {
    let s = dir.to_string_lossy();
    let safe = s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+:@".contains(c));
    if safe {
        s.into_owned()
    } else {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

#[cfg(not(windows))]
fn unix_strip_relay_path_block(content: &str) -> String {
    if !content.contains(RELAY_PATH_MARKER) && !content.contains(RELAY_PATH_BLOCK_BEGIN) {
        return content.to_string();
    }
    let lines: Vec<&str> = content.lines().collect();
    let mut out: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end_matches('\r');
        if line == RELAY_PATH_BLOCK_BEGIN {
            i += 1;
            while i < lines.len() && lines[i].trim_end_matches('\r').trim() != RELAY_PATH_BLOCK_END
            {
                i += 1;
            }
            if i < lines.len() {
                i += 1;
            }
            continue;
        }
        if line == RELAY_PATH_MARKER {
            i += 1;
            if i < lines.len() {
                let next = lines[i].trim_start();
                if next.starts_with("export PATH=") || next.starts_with("fish_add_path") {
                    i += 1;
                }
            }
            continue;
        }
        out.push(lines[i]);
        i += 1;
    }
    let trail = content.ends_with('\n');
    let mut s = out.join("\n");
    if trail && !s.is_empty() && !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

#[cfg(not(windows))]
fn unix_append_fish_path_block(home: &Path, dir: &Path) -> Result<()> {
    let fish_path = fish_config_path(home);
    let use_fish = fish_path.exists()
        || std::env::var("SHELL")
            .map(|sh| sh.to_lowercase().contains("fish"))
            .unwrap_or(false);
    if !use_fish {
        return Ok(());
    }
    let token = fish_add_path_token(dir);
    let block = format!(
        "\n{}\nfish_add_path {}\n{}\n",
        RELAY_PATH_BLOCK_BEGIN, token, RELAY_PATH_BLOCK_END
    );
    let existing = if fish_path.exists() {
        fs::read_to_string(&fish_path)?
    } else {
        String::new()
    };
    let has_block =
        existing.contains(RELAY_PATH_BLOCK_BEGIN) || existing.contains(RELAY_PATH_MARKER);
    let mut out = if has_block {
        if unix_rc_content_path_matches(&existing, dir) {
            return Ok(());
        }
        unix_strip_relay_path_block(&existing)
    } else {
        existing
    };
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&block);
    if let Some(parent) = fish_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&fish_path, out).with_context(|| format!("write {}", fish_path.display()))?;
    Ok(())
}

#[cfg(not(windows))]
fn unix_append_path_block(home: &Path, dir: &Path) -> Result<()> {
    let block = format!(
        "\n{}\nexport PATH=\"{}:$PATH\"\n{}\n",
        RELAY_PATH_BLOCK_BEGIN,
        dir.display(),
        RELAY_PATH_BLOCK_END
    );

    fn append_rc(path: &Path, block: &str, dir: &Path) -> Result<()> {
        let existing = if path.exists() {
            fs::read_to_string(path)?
        } else {
            String::new()
        };
        let has_block =
            existing.contains(RELAY_PATH_BLOCK_BEGIN) || existing.contains(RELAY_PATH_MARKER);
        let mut out = if has_block {
            if unix_rc_content_path_matches(&existing, dir) {
                return Ok(());
            }
            unix_strip_relay_path_block(&existing)
        } else {
            existing
        };
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(block);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, out).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    if cfg!(target_os = "macos") {
        append_rc(&home.join(".zshrc"), &block, dir)?;
        let bash_profile = home.join(".bash_profile");
        if bash_profile.exists() {
            append_rc(&bash_profile, &block, dir)?;
        }
    } else {
        let bashrc = home.join(".bashrc");
        let profile = home.join(".profile");
        let zshrc = home.join(".zshrc");
        if bashrc.exists() {
            append_rc(&bashrc, &block, dir)?;
        }
        if zshrc.exists() {
            append_rc(&zshrc, &block, dir)?;
        }
        if !bashrc.exists() && profile.exists() {
            append_rc(&profile, &block, dir)?;
        } else if !bashrc.exists() && !zshrc.exists() {
            let content = block.trim_start();
            fs::write(&profile, content).context("write ~/.profile")?;
        }
    }
    unix_append_fish_path_block(home, dir)?;
    Ok(())
}

#[cfg(windows)]
fn windows_remove_user_path_entry(target: &Path) -> Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (env, _) = hkcu
        .create_subkey("Environment")
        .context("open HKCU\\Environment")?;
    let path_val: String = env.get_value("Path").unwrap_or_default();
    let to_remove: Option<String> = env.get_value(RELAY_MCP_PATH_REGISTRY_VALUE).ok();
    let parts: Vec<String> = path_val
        .split(';')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .filter(|p| {
            let p_path = Path::new(p);
            if let Some(ref marked) = to_remove {
                if paths_same_bin_dir(p_path, Path::new(marked)) {
                    return false;
                }
            }
            !paths_same_bin_dir(p_path, target)
        })
        .map(|s| s.to_string())
        .collect();
    let new_val = parts.join(";");
    env.set_value("Path", &new_val).context("write user Path")?;
    let _ = env.delete_value(RELAY_MCP_PATH_REGISTRY_VALUE);
    notify_windows_environment_path_changed();
    Ok(())
}

/// True if permanent user config already includes this app's bin directory.
pub fn relay_path_persistently_configured() -> bool {
    let Ok(dir) = relay_cli_directory() else {
        return false;
    };
    let bin_exists = dir.join(gui_binary_name()).exists();
    #[cfg(windows)]
    {
        bin_exists && windows_user_path_has_dir(&dir)
    }
    #[cfg(not(windows))]
    {
        bin_exists
            && user_home_dir()
                .map(|h| unix_rc_path_matches_current(&h, &dir))
                .unwrap_or(false)
    }
}

/// When PATH is not configured, returns a short reason for the user to fix manually.
#[allow(unreachable_code)]
pub fn relay_path_config_reason() -> Option<String> {
    if relay_path_persistently_configured() {
        return None;
    }
    let dir = relay_cli_directory().ok()?;
    let bin_exists = dir.join(gui_binary_name()).exists();
    if !bin_exists {
        return Some(format!(
            "Relay binary not found at {} (install or run from app directory).",
            dir.join(gui_binary_name()).display()
        ));
    }
    #[cfg(windows)]
    {
        if !windows_user_path_has_dir(&dir) {
            return Some("User PATH does not include the current relay directory. Use one-click install or add it manually.".to_string());
        }
        return None;
    }
    #[cfg(not(windows))]
    {
        let home = match user_home_dir() {
            Some(h) => h,
            None => return Some("HOME not set; cannot detect shell rc.".to_string()),
        };
        if let Some(written) = unix_rc_find_configured_path(&home) {
            let current = dir.canonicalize().unwrap_or(dir.clone());
            let written_display = written
                .canonicalize()
                .unwrap_or(written.clone())
                .display()
                .to_string();
            let current_display = current.display().to_string();
            return Some(format!(
                "Configured path in rc does not match current run path (configured: {}, current: {}). Re-run one-click install to overwrite, or edit rc manually.",
                written_display,
                current_display
            ));
        }
        return Some("No Relay PATH block found in shell rc (.zshrc, .bashrc, .profile, or fish). Use one-click install.".to_string());
    }
    None
}

/// Add relay bin dir to user PATH permanently (registry on Windows, shell rc on Unix).
pub fn persist_relay_cli_path() -> Result<&'static str> {
    let dir = relay_cli_directory()?;
    let relay_bin = dir.join(gui_binary_name());
    if !relay_bin.exists() {
        return Err(anyhow!(
            "relay binary not found beside this app ({})",
            relay_bin.display()
        ));
    }
    if relay_path_persistently_configured() {
        return Ok("already");
    }
    #[cfg(windows)]
    {
        windows_append_user_path(&dir)?;
        return Ok("windows");
    }
    #[cfg(not(windows))]
    {
        let s = dir.to_string_lossy();
        if s.chars()
            .any(|c| matches!(c, '$' | '`' | '"' | '\n' | '\r'))
        {
            return Err(anyhow!(
                "Cannot add relay to PATH: install path contains shell-special characters ($, \", `, or newlines). Use a simpler install path."
            ));
        }
        let home = user_home_dir().ok_or_else(|| anyhow!("HOME / USERPROFILE not set"))?;
        unix_append_path_block(&home, &dir)?;
        Ok("unix")
    }
}

/// Remove Relay-added PATH from shell rc files (Unix) or user Path (Windows).
pub fn remove_relay_cli_path_persistent() -> Result<()> {
    #[cfg(windows)]
    {
        let dir = relay_cli_directory()?;
        windows_remove_user_path_entry(&dir)
    }
    #[cfg(not(windows))]
    {
        let home = user_home_dir().ok_or_else(|| anyhow!("HOME not set"))?;
        let mut paths: Vec<PathBuf> = vec![fish_config_path(&home)];
        for name in [".zshrc", ".bash_profile", ".bashrc", ".profile"] {
            paths.push(home.join(name));
        }
        for p in paths {
            if !p.exists() {
                continue;
            }
            let s = fs::read_to_string(&p)?;
            if !s.contains(RELAY_PATH_MARKER) && !s.contains(RELAY_PATH_BLOCK_BEGIN) {
                continue;
            }
            let new_s = unix_strip_relay_path_block(&s);
            fs::write(&p, new_s).with_context(|| format!("write {}", p.display()))?;
        }
        Ok(())
    }
}

pub fn gui_binary_path(exe_dir: &Path) -> PathBuf {
    exe_dir.join(gui_binary_name())
}
