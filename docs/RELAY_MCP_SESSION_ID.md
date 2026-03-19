# `relay_mcp_session_id` and tab title **MM-DD HH:mm**

## Merge key: `relay_mcp_session_id`

- **First call:** Omit `relay_mcp_session_id` (or pass empty). Relay creates a new tab and generates a session id (millisecond timestamp). The tool returns JSON: `{"relay_mcp_session_id":"<ms>","human":"<user Answer>"}`.
- **You must remember** the returned `relay_mcp_session_id` and **reply to the `human` content**.
- **Next calls:** You **must** pass that `relay_mcp_session_id` so the request merges into the same Relay tab.

## Tab title: **MM-DD HH:mm**

- Relay formats the session id (ms timestamp) as **MM-DD HH:mm** and uses it as the **tab strip label**.
- The **main window title** is fixed as **"Relay"** (does not change with the active tab).

## `commands` and `skills`

- **New session** (no `relay_mcp_session_id`): you **must** pass both **`commands`** and **`skills`**: JSON arrays `[{ "name", "id", "category?", "description?" }]` (either may be `[]`). Relay binds them for **slash-completion** in the Answer input (typing `/` shows the list).
- **Existing session** (non-empty `relay_mcp_session_id`): passing **`commands`** and/or **`skills`** is **optional**. If sent, each list is **merged** into that tab’s existing `commands` / `skills`: new items are appended; any item whose **`id`** already exists is **skipped** (dedupe by `id`, first occurrence kept).
- **Prefer real lists on first call:** pass actual IDE / MCP descriptors when you can, not only `[]`, so the slash menu is useful.

## MCP tool result

- Every non-paused, non–auto-reply result is JSON: `{"relay_mcp_session_id":"<string>","human":"<string>"}`.
- `human` is the user’s Answer text (empty on dismiss/timeout).
- **Pause:** If the result contains `<<<RELAY_MCP_PAUSED>>>`, do not call the tool again until the user resumes in Settings.
