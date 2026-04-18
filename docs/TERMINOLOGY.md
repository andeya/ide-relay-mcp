# Relay — Domain terminology

Single source of truth for product copy and docs. Transport details: **[HTTP_IPC.md](HTTP_IPC.md)**.

---

## 1. One human-in-the-loop turn

| Term (EN)                | Summary           | Definition                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------ | ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Relay tool**           | MCP tool name     | MCP tool **`relay_interactive_feedback`**.                                                                                                                                                                                                                                                                                                                        |
| **Retell**               | Retell parameter  | MCP **`retell`** (required, non-empty). **This turn’s assistant reply** — verbatim, over **127.0.0.1 HTTP** (no shell limits).                                                                                                                                                                                                                                    |
| **Answer**               | Human submission  | Human submission; tool return JSON: plain **`human`** plus optional **`attachments`** (`{ kind, path }[]`); legacy `<<<RELAY_FEEDBACK_JSON>>>` in `human` is stripped on ingest.                                                                                                                                                                                  |
| **relay_mcp_session_id** | Session merge key | MCP **`relay_mcp_session_id`**: returned in JSON as ms timestamp; **must** be passed on next call. Tab label = **MM-DD HH:mm:ss**. **`commands`** and **`skills`** required when id is empty (either may be `[]`); when id is present they are **optional** and **merge** into the tab (dedupe by `id`) ([**RELAY_MCP_SESSION_ID.md**](RELAY_MCP_SESSION_ID.md)). |

**Relay GUI (install hub):** The green human-in-the-loop panel shows copyable **JSON** and per-IDE actions; the table above is the MCP tool contract, regardless of how the UI lists fields.

---

## 2. Binaries & modes

| Term                               | Definition                                                                                                                                                                                                                                                                                                                             |
| ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`relay` / `relay gui-<cli_id>`** | Hub window + **local HTTP** on loopback. Writes **`gui_endpoint_<cli_id>.json`** (e.g. `gui_endpoint_cursor.json`) under app **`config_dir()`** with `{ port, token, pid }`. Bare hub may still use legacy **`gui_endpoint.json`**.                                                                                                    |
| **`relay mcp-<cli_id>`**           | Stdio MCP server; IDE **`mcp.json`** uses **`args`** such as **`["mcp-cursor"]`**, **`["mcp-claudecode"]`**, **`["mcp-windsurf"]`**, **`["mcp-other"]`**. Optional **`--exe_in_wsl`** after that subcommand (Windows `relay.exe` + WSL-hosted MCP client). Talks to the GUI **only** via Bearer HTTP — see [HTTP_IPC.md](HTTP_IPC.md). |
| **`relay feedback --retell`**      | Terminal tryout; scans **`gui_endpoint_*.json`** / **`gui_endpoint.json`** for a healthy GUI or waits for one; Answer on stdout.                                                                                                                                                                                                       |

There is **no** `relay window` subcommand; long waits use one resident GUI plus HTTP, not a per-request subprocess UI.

---

## 3. Terminal PATH (install/uninstall markers)

| Term                            | Definition                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **RELAY_MCP_PATH block** (Unix) | Dedicated comment block in shell rc (`.zshrc`, `.bashrc`, `.profile`, fish `config.fish`). **BEGIN** / **END** markers allow install to replace and uninstall to strip exactly this block. Format: `# ----- BEGIN RELAY_MCP_PATH (managed by Relay app) -----`, then one line `export PATH="DIR:$PATH"` or `fish_add_path DIR`, then `# ----- END RELAY_MCP_PATH -----`. Legacy single-line marker `# Relay MCP PATH (managed by Relay app)` is still recognized. |
| **RelayMCPPath** (Windows)      | Dedicated registry value under **HKCU\Environment** storing the directory we added to user PATH. Install writes it after appending to Path; uninstall removes that directory from Path and deletes the value. If the value is missing (legacy install), uninstall removes the current process directory from Path.                                                                                                                                                |

---

## 4. Doc checklist

- [ ] **`retell`** = current-turn assistant reply (content itself), non-empty
- [ ] **PATH block** = BEGIN/END in rc (Unix), RelayMCPPath in HKCU\Environment (Windows); legacy single-line marker still supported on Unix
- [ ] **`relay_mcp_session_id`** = session merge key; tool returns JSON `{relay_mcp_session_id, human, cmd_skill_count}` (optional `attachments` `{kind, path}`; MCP may rewrite paths for WSL when **`relay mcp-<ide> … --exe_in_wsl`**); pass id on next call; if **`cmd_skill_count` is 0**, next call must repass commands+skills; tab label **MM-DD HH:mm:ss** (**RELAY_MCP_SESSION_ID.md**)
- [ ] Code comments in **English** where they explain implementation
