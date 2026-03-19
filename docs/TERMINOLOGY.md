# Relay — Domain terminology

Single source of truth for product copy and docs. Transport details: **[HTTP_IPC.md](HTTP_IPC.md)**.

---

## 1. One human-in-the-loop turn

| Term (EN)                | Summary           | Definition                                                                                                                                                                                                                                                                                                                                                     |
| ------------------------ | ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Relay tool**           | MCP tool name     | MCP tool **`relay_interactive_feedback`**.                                                                                                                                                                                                                                                                                                                     |
| **Retell**               | Retell parameter  | MCP **`retell`** (required, non-empty). **This turn’s assistant reply** — verbatim, over **127.0.0.1 HTTP** (no shell limits).                                                                                                                                                                                                                                 |
| **Answer**               | Human submission  | Human submission; tool return value (+ optional `<<<RELAY_FEEDBACK_JSON>>>` for attachments).                                                                                                                                                                                                                                                                  |
| **relay_mcp_session_id** | Session merge key | MCP **`relay_mcp_session_id`**: returned in JSON as ms timestamp; **must** be passed on next call. Tab label = **MM-DD HH:mm**. **`commands`** and **`skills`** required when id is empty (either may be `[]`); when id is present they are **optional** and **merge** into the tab (dedupe by `id`) ([**RELAY_MCP_SESSION_ID.md**](RELAY_MCP_SESSION_ID.md)). |

**Relay GUI (install hub):** The green human-in-the-loop panel shows copyable **JSON** and per-IDE actions; the table above is the MCP tool contract, regardless of how the UI lists fields.

---

## 2. Binaries & modes

| Term                          | Definition                                                                     |
| ----------------------------- | ------------------------------------------------------------------------------ |
| **`relay` / `relay gui`**     | Hub window + **local HTTP server** (`gui_endpoint.json`: port + Bearer token). |
| **`relay mcp`**               | stdio MCP server (`args`: `["mcp"]`). Talks to GUI only via that HTTP API.     |
| **`relay feedback --retell`** | Terminal tryout; opens GUI if needed; Answer on stdout.                        |

There is **no** `relay window` subcommand; the IDE never spawns per-request GUI processes.

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
- [ ] **`relay_mcp_session_id`** = session merge key; tool returns JSON `{relay_mcp_session_id, human, cmd_skill_count}`; pass id on next call; if **`cmd_skill_count` is 0**, next call must repass commands+skills; tab label **MM-DD HH:mm** (**RELAY_MCP_SESSION_ID.md**)
- [ ] Code comments in **English** where they explain implementation
