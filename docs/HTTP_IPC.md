# MCP ↔ GUI: localhost HTTP

Architecture: the **MCP process** (`relay mcp`) and **GUI process** (`relay` / `relay gui`) coordinate only via HTTP on **127.0.0.1** plus on-disk **`gui_endpoint.json`** — no secondary child processes per request, no handshake txt, no `tab_inbox.jsonl`.

```mermaid
sequenceDiagram
    participant IDE as IDE_MCP
    participant Mcp as relay_mcp
    participant Http as GUI_HTTP
    participant UI as Vue_UI

    IDE->>Mcp: tools/call relay_interactive_feedback
    Mcp->>Mcp: read gui_endpoint.json or spawn relay gui
    Mcp->>Http: POST /v1/feedback
    Http->>UI: emit relay_tabs_changed
    Mcp->>Http: GET /v1/feedback/wait/:id
    UI->>Http: submit_tab_feedback
    Http-->>Mcp: JSON { relay_mcp_session_id, human, cmd_skill_count [, attachments] }
    Note right of Mcp: optional rewrite attachments[].path for WSL (RELAY_EXE_IN_WSL)
    Mcp-->>IDE: tool result
```

## Discovery and startup

- Path: `{user_data_dir}/gui_endpoint.json`
- Contents: `{ "port": u16, "token": string, "pid": u32 }`
- GUI binds **`127.0.0.1:0`**, writes a random token to the file; file is removed on process exit.
- **`relay mcp`** reads this file before each tool call; if missing or health fails, it **`spawn`s the current exe with arg `gui`**, polls until timeout (~**45s** in `ensure_gui_endpoint`).
- **Security**: loopback only; token in user data dir reduces accidental connection to the wrong local process; **does not** stop a malicious local process (same as any local IPC).

## Auth

- All APIs: `Authorization: Bearer <token>` (must match `gui_endpoint.json`).

## API

### `GET /v1/health`

- 200 = endpoint is up.

### `POST /v1/feedback`

- Body JSON: `retell` (required, non-empty after trim), `relay_mcp_session_id` (optional; **string or JSON number**, empty/absent/null = new session), `commands` / `skills` (JSON arrays of `{name, id, category?, description?}`). **New session:** both properties must be present; each array should list everything the IDE can expose for slash-completion — **`[]` only when the host truly has no items** (wire format still accepts empty arrays). **Existing session:** both optional; if present, **merged** with **dedupe by `id`** (existing wins). If the last tool result had `cmd_skill_count === 0`, the client should send both arrays again repopulated the same way.
- Behavior: non-empty `relay_mcp_session_id` merges into the tab with that id and cancels the previous in-flight wait; otherwise opens a new tab and assigns a new session id (ms timestamp). Tab label = **MM-DD HH:mm:ss** from that id.
- When the GUI had only a **preview** tab and it is stripped before handling the POST: if there are **no real tabs left**, the server clears **`qa_rounds`** only for a **new** session (empty `relay_mcp_session_id`). If the IDE passes an **existing** session id (e.g. user closed the tab and MCP calls again), **only rounds for that session id are kept** so bubble history is not wiped before re-open.
- Response: `{ "request_id": "<uuid>" }`
- Empty `retell` → **400**. See [RELAY_MCP_SESSION_ID.md](RELAY_MCP_SESSION_ID.md).

### `GET /v1/feedback/wait/:request_id`

- **HTTP handler**: the Axum route **does not** apply a per-request socket timeout; it awaits a `oneshot` until the tab completes (submit, dismiss, supersede, or sender dropped).
- **60-minute idle cut-off**: when `POST /v1/feedback` returns a `request_id`, the server schedules a background task (≈ **60 min + 20 s**) that injects an **empty** `human` JSON result if the wait is still pending — same outcome as dismiss/timeout from the MCP user’s perspective (`human: ""`).
- Completes when the user submits an Answer, dismisses, that orphan task fires, or the tab is **superseded** by another `POST` for the same merged session (cancels the previous wait).
- Response: `Content-Type: application/json; charset=utf-8`, body includes **`relay_mcp_session_id`**, **`human`**, **`cmd_skill_count`**, and when the user attached images/files **`"attachments":[{"kind":"image"|"file","path":"..."}, ...]`** (`cmd_skill_count` = stored commands+skills on that tab; empty `human` on dismiss / idle timeout / supersede). Paths are local to the GUI host (Windows absolute paths). Before the IDE sees the `tools/call` result, **`relay mcp`** may **rewrite** each **`path`** to a WSL form (`/mnt/c/...`) when **`RELAY_EXE_IN_WSL`** is enabled (see below); HTTP payloads and on-disk history stay unchanged.

## MCP flow

1. Read `gui_endpoint.json`; if absent, spawn **`relay gui`** and poll.
2. `POST /v1/feedback` → `request_id`
3. `GET .../wait/:request_id` — long-lived response driven by GUI state (see above), not a fixed HTTP “61 minute” client timer.
4. Optionally transform JSON for the IDE: **`attachments[].path`** may be rewritten for **WSL-hosted agents** using Windows `relay.exe` (see **`RELAY_EXE_IN_WSL`**). The HTTP response body from step 3 is unchanged on disk/logging semantics except as consumed by MCP.
5. String returned as `tools/call` result.

### MCP-only: WSL path rewrite (`RELAY_EXE_IN_WSL`)

Set on the **`relay mcp`** process (Windows builds only). When the IDE runs the MCP client **inside WSL** but **`relay.exe` is the Windows binary**, agents need **`path`** strings they can open from Linux (e.g. `/mnt/c/Users/...` instead of `C:\Users\...`).

| Value (trimmed; ASCII case-insensitive)                      | Behavior                                                                                                                                                                                                                                                                                                                                  |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`1`** or **`true`**                                        | Replace each **`attachments[].path`** with the WSL `/mnt/<drive>/...` form when the value is a Windows drive path; if the string is only a Relay attachment filename or relative fragment, **`relay mcp`** first resolves it to a canonical Windows path (same rules as reading attachments), then maps it. UNC paths are left unchanged. |
| **Unset**, **empty**, **`0`**, **`false`**, or anything else | **Off** — paths in the tool result match the HTTP body (Windows).                                                                                                                                                                                                                                                                         |

### Where to set `env` (Cursor)

[Cursor’s MCP docs](https://cursor.com/docs/context/mcp) define two configuration files:

| Location    | Path                                    | Typical use                                                                                                                               |
| ----------- | --------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| **Project** | **`.cursor/mcp.json`** in the repo root | Same MCP setup for everyone who clones the repo; per-repo **`RELAY_EXE_IN_WSL`** in `env` when using WSL agents with Windows `relay.exe`. |
| **Global**  | **`~/.cursor/mcp.json`**                | Personal defaults for all workspaces.                                                                                                     |

STDIO servers support an **`env`** object on each server entry (and optional **`envFile`**). Values may use [config interpolation](https://cursor.com/docs/context/mcp) (e.g. **`${workspaceFolder}`**, **`${env:VAR}`**). Those variables are visible to **`relay mcp`** when Cursor spawns the process—**not** necessarily the same as variables you `export` in an interactive terminal.

### Example: `mcp.json` with `env` (Cursor)

WSL agent + Windows `relay.exe`: rewrite attachment paths in the MCP tool result to `/mnt/c/...` form:

```json
{
  "mcpServers": {
    "relay-mcp": {
      "command": "C:/Users/You/AppData/Local/Relay/relay.exe",
      "args": ["mcp"],
      "env": {
        "RELAY_EXE_IN_WSL": "1"
      }
    }
  }
}
```

**Other IDEs / older setups:** Some hosts expose only a **single** global MCP configuration. If there is no per-project file, use one global `env`, or define **two server names** (e.g. `relay` vs `relay-wsl`) with different `env` (such as `RELAY_EXE_IN_WSL`), or a small **wrapper** script that exports variables then `exec`s `relay mcp`. Relay does not read IDE config files itself—it only reads the **process environment** of `relay mcp`.

### MCP client (`relay mcp` → ureq)

- The HTTP **client** in `mcp_http::feedback_round` sets a **24 h** read timeout on the `GET .../wait` call as a transport-level failsafe (avoids a truly infinite block if the GUI misbehaves). **User-visible idle timeout remains ~60 minutes** from the GUI orphan task; the 24 h ceiling should not normally be hit in practice.

## MCP stdio: concurrency, cancellation, and errors

- **Concurrent HIL**: The MCP process uses a **JSON-RPC router** plus **background workers** for long `tools/call` rounds. Multiple in-flight `relay_interactive_feedback` calls on the **same stdio connection** are supported (bounded by **`MAX_CONCURRENT_HIL`** in `server.rs`, currently **16**; beyond that, new calls get **-32603**). Each worker talks to the GUI over HTTP independently. **`tools/list`**, **`ping`**, and **`initialize`** are answered immediately on the router thread—hosts can refresh tool metadata while other tabs are waiting on you.
- **Stdout**: All JSON-RPC lines are written through a **single writer** so responses never interleave.
- **`notifications/cancelled`**: The router matches `params.requestId` (or `request_id`) to a **pending** `tools/call` by JSON-RPC `id`. It responds with **-32800** for that `id` so the host does not hang. The matching HTTP `GET .../wait` in the worker may still run until the GUI completes; the host should not assume the Relay tab closes automatically.
- Malformed JSON lines: if an `"id"` can be scraped from the line, a **-32700** parse error is returned instead of silence.

## Frontend

- `listen("relay_tabs_changed")` → `get_feedback_tabs`; no inbox polling.
- **`get_feedback_tabs` (Tauri)**: before returning state, the GUI runs `hydrate_qa_rounds_from_feedback_log`: it reads `feedback_log.txt` and, for each non-preview tab with a non-empty `relay_mcp_session_id`, merges completed MCP rounds from the log when the log has **more** completed pairs than in-memory `qa_rounds` for that session (e.g. after a GUI restart while the IDE keeps the same session id). Only `AI_REQUEST` lines that include `[session:<id>]` are attributed; see `parse_feedback_log_mcp` in `storage.rs`. Open in-memory rounds are not duplicated if the same `retell` already appears in the merged log slice. If hydration **changes** `qa_rounds`, the app emits **`relay_tabs_changed`** so the Vue layer reloads tabs and the history strip updates without a manual refresh.
- **`feedback_log.txt`**: the MCP process writes **`USER_REPLY`** as the user’s **plain Answer** (`normalize_logged_user_reply` on ingest—handles accidental full JSON bodies). **Hydration** builds `(retell, reply)` pairs from the log but **drops** lines that still look like a `feedback/wait` tool-result blob (legacy), so those rounds do not reappear in the UI. The CLI still prints the **full JSON** to stdout.
- **`qa_archive/<session_id>.jsonl`**: each time the GUI completes a round (`apply_reply_for_tab` / `skip_open_round_for_tab`), one JSON line is appended (`retell`, `reply`, `skipped`, `attachments`). On **`get_feedback_tabs` → hydrate**, if the archive has **more** completed rows than log-derived pairs for that session, the archive wins so history survives weak log pairing / skipped blob lines. **Settings → Storage**: “clear log” / “clear all” **truncates `feedback_log.txt` and deletes `*.jsonl` files under `qa_archive/`** (other filenames are left untouched); usage size for the log card sums **`feedback_log.txt` + `qa_archive` `.jsonl` bytes only**. **Attachment retention** (days) also **deletes old `qa_archive/*.jsonl` by mtime**; `feedback_log.txt` is **not** auto-pruned by age (single append-only file—manual clear or a future rotation policy).

## Removed (legacy)

- `relay window`, `result_file` / `control_file`, `tab_inbox.jsonl`, CLI retell length budget, `compute_retell_inline_hint`.
