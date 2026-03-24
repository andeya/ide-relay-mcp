# `relay_mcp_session_id` and tab title **MM-DD HH:mm:ss**

## Merge key: `relay_mcp_session_id`

- **First call:** Omit `relay_mcp_session_id` (or pass empty). Relay creates a new tab and generates a session id (millisecond timestamp). The tool returns JSON including `relay_mcp_session_id`, `human`, and `cmd_skill_count`.
- **You must remember** the returned `relay_mcp_session_id` and **reply to the `human` content**.
- **Next calls:** You **must** pass that `relay_mcp_session_id` so the request merges into the same Relay tab.

## Tab title: **MM-DD HH:mm:ss**

- Relay formats the session id (ms timestamp) as **MM-DD HH:mm:ss** (local time) and uses it as the **tab strip label**.
- The **main window title** is fixed as **"Relay"** (does not change with the active tab).

## `commands` and `skills`

- **New session** (no `relay_mcp_session_id`): every call **must** include both **`commands`** and **`skills`** as arrays. Each array **should list every item the IDE/host can expose** (`{ name, id, category?, description? }`) for **slash-completion**. Use **`[]` only when the host truly provides no items** — do not send empty arrays when you could populate them.
- **Existing session** (non-empty `relay_mcp_session_id`): **`commands`** / **`skills`** are **optional** unless you need to repopulate (see below). If sent, lists are **merged** into that tab: new items appended; duplicate **`id`** skipped (first wins).
- **When `cmd_skill_count` is 0:** On the next call (not pause sentinel), **must** include **`commands`** and **`skills`** again, filled with everything the IDE **can** enumerate — **`[]` only if there is nothing to enumerate**.

## MCP tool result

- Every non-paused, non–auto-reply result is JSON: `{"relay_mcp_session_id":"<string>","human":"<string>","cmd_skill_count":<number>}` (plus optional `attachments` with `{kind, path}`; `relay mcp` may rewrite `path` to WSL form when started with `--exe_in_wsl` — see [HTTP_IPC.md](HTTP_IPC.md)).
- **`cmd_skill_count`** is the number of command + skill items currently stored on that Relay tab (length of slash menu source).
- `human` is the user’s Answer text (empty on dismiss/timeout). **Agent loop:** the turn ends only when both `human` and `attachments` are empty (or attachments absent); if the user submitted only attachments (no text), treat attachments as input and reply.
- **Pause:** If the result contains `<<<RELAY_MCP_PAUSED>>>`, do not call the tool again until the user resumes in Settings.
