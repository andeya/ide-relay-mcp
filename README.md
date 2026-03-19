<div align="center">

<br/>

<img src="src-tauri/icons/source/relay-icon.svg" alt="Relay" width="120" height="120"/>

# Relay

**Native human-in-the-loop for MCP — one binary, localhost HTTP, same tool turn.**

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-6366f1?style=flat-square" alt="License"/></a>
  <a href="https://tauri.app/"><img src="https://img.shields.io/badge/Tauri-2-24adc8?style=flat-square&logo=tauri&logoColor=white" alt="Tauri 2"/></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-MCP%20%2B%20HTTP-000000?style=flat-square&logo=rust&logoColor=white" alt="Rust"/></a>
  <a href="https://vuejs.org/"><img src="https://img.shields.io/badge/Vue-3-42b883?style=flat-square&logo=vuedotjs&logoColor=white" alt="Vue 3"/></a>
</p>

**[Download](https://github.com/andeya/ide-relay-mcp/releases/latest)** · **[简体中文](README_zh.md)** · **[Terminology](docs/TERMINOLOGY.md)** · **[HTTP IPC](docs/HTTP_IPC.md)** · **[Client tab ID](docs/CLIENT_TAB_ID.md)** · **[Frontend](docs/FRONTEND.md)** · **[Release & CI](docs/RELEASING.md)**

**Author:** andeya · [andeyalee@outlook.com](mailto:andeyalee@outlook.com)

<br/>

</div>

---

Relay is an **MCP server** that turns **`relay_interactive_feedback`** into a **blocking tool call**: the agent pauses, a **Tauri + Vue** window collects your **Answer**, and the **same** JSON-RPC round-trip returns it—no cloud relay, no stuffing giant assistant text through shell argv.

Inspired by [interactive-feedback-mcp](https://github.com/junanchn/interactive-feedback-mcp); Relay replaces per-invocation subprocess hacks with a **dedicated GUI process** and a small **loopback HTTP API** (Axum, Bearer token, `gui_endpoint.json` discovery).

<p align="center">
  <img src="docs/ScreenShot_1.png" alt="Relay MCP hub next to Cursor IDE" width="920" style="max-width:100%; height:auto;" />
</p>
<p align="center"><sub><strong>Relay hub</strong> next to your IDE — write your <strong>Answer</strong> (text + images) while the agent waits on the same <code>tools/call</code>.</sub></p>

---

## Why this shape

| Typical pain                                                     | What Relay does                                                                                                                                 |
| ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| **Retell** (full assistant reply) hits **ARG_MAX** / argv limits | **`retell` travels in HTTP POST JSON** — size bounded by body limit (16 MiB), not the shell.                                                    |
| Spawning a UI per tool call                                      | **One GUI process** (`relay` / `relay gui`); MCP only runs **`relay mcp`** on stdio.                                                            |
| Multiple IDE threads → tab chaos                                 | **`client_tab_id`** = workspace root + `\n` + first user message → stable **Chat N** merge key ([**CLIENT_TAB_ID.md**](docs/CLIENT_TAB_ID.md)). |

---

## Architecture (fact-checked against the repo)

- **`relay mcp`** — stdio MCP (`clap` subcommand). Handles `initialize`, `tools/list`, `tools/call`. Optional **instant auto-reply** (`0|…` lines in user-data rules files) short-circuits without opening the UI.
- **`relay` / `relay gui`** — Tauri app + **HTTP on `127.0.0.1:0`**. Writes **`{user_data}/gui_endpoint.json`** `{ port, token, pid }`; deletes it on exit.
- **Bridge** — Before each interactive call, MCP reads the endpoint file; if missing or unhealthy, **`spawn`s the same executable with `gui`**, polls up to **~45 s** (`ensure_gui_endpoint`). Then **`POST /v1/feedback`** → **`GET /v1/feedback/wait/:request_id`** (long poll, **~600 s** server / **~700 s** client timeout). Response is **plain text** = tool result.

```mermaid
flowchart LR
  IDE[IDE / Agent] -->|stdio JSON-RPC| MCP[relay mcp]
  MCP -->|read or spawn| GUI[relay gui]
  MCP <-->|127.0.0.1 Bearer| HTTP[Tauri HTTP API]
  HTTP <--> UI[Vue tabs]
  UI --- User((You))
  MCP -->|Answer string| IDE
```

Full API and security notes: **[docs/HTTP_IPC.md](docs/HTTP_IPC.md)**.

---

## MCP tool: `relay_interactive_feedback`

| Argument            | Required             | Meaning                                                  |
| ------------------- | -------------------- | -------------------------------------------------------- |
| **`retell`**        | ✅ non-empty         | This turn’s **user-visible assistant reply** (verbatim). |
| **`client_tab_id`** | strongly recommended | Merge key → **Chat 1, 2, …** per thread.                 |
| **`session_title`** | optional             | Log-friendly only; **GUI ignores** for titles.           |

**Pause MCP** (Settings): tool returns sentinel `<<<RELAY_MCP_PAUSED>>>` — agents should not call again until resumed.

---

## Quick start

1. **Get Relay** — Prefer the [latest release](https://github.com/andeya/ide-relay-mcp/releases/latest) (prebuilt installers for macOS, Linux, Windows). Or [build from source](#build): `npm ci && npm run build && npm run tauri build`.
2. Point your IDE’s MCP at the **`relay`** binary with args **`["mcp"]`** and a generous **`timeout`** (e.g. **600**).

```json
{
  "mcpServers": {
    "relay-mcp": {
      "command": "/path/to/relay",
      "args": ["mcp"],
      "timeout": 600,
      "autoApprove": ["relay_interactive_feedback"]
    }
  }
}
```

<p align="center">
  <img src="docs/ScreenShot_3.png" alt="Relay Settings Environment and MCP" width="440" style="max-width:100%; height:auto;" />
</p>
<p align="center"><sub><strong>Settings → Environment & MCP</strong> — terminal PATH, one-click <strong>Cursor / Windsurf</strong> install, copy MCP JSON, <strong>Pause MCP</strong>.</sub></p>

In-app **Settings → Environment & MCP**: copy JSON, **Cursor / Windsurf** one-click install, optional **PATH** persistence (Windows registry / shell rc). Rule prompts: **Settings → Rule prompts** (English blocks + IDE paste guide); source: [`src/cursorRulesTemplates.ts`](src/cursorRulesTemplates.ts).

Repo example: [`mcp.json`](mcp.json).

---

## What you get

- **Multi-tab hub** — New requests open or refresh tabs; non-active tabs can flash; **`client_tab_id`** merges streams.
- **Composer UX** — Enter submit, Shift+Enter newline, ⌘/Ctrl+Enter submit & close tab; images / paste supported; optional **`<<<RELAY_FEEDBACK_JSON>>>`** attachment convention.
- **Auto-reply** — `auto_reply_oneshot.txt` / `auto_reply_loop.txt` in user data; only **`0|reply`** lines (instant); see [Configuration](#configuration--paths).
- **Storage** — `feedback_log.txt`, locale, **attachment auto-purge** (default **30 days**, configurable or off in **Settings → Cache**).
- **CLI** — `relay feedback --retell "…"` prints **Answer** on stdout; **exit 1** on GUI failure or **`--timeout`**.

<p align="center">
  <img src="docs/ScreenShot_4.png" alt="Relay Settings Rule prompts" width="440" style="max-width:100%; height:auto;" />
</p>
<p align="center"><sub><strong>Settings → Rule prompts</strong> — Standard / Strict loop / Tool spec; <strong>Paste in IDE</strong> for human-in-the-loop rules.</sub></p>

<p align="center">
  <img src="docs/ScreenShot_5.png" alt="Relay Settings Cache" width="440" style="max-width:100%; height:auto;" />
</p>
<p align="center"><sub><strong>Settings → Cache</strong> — local attachment + log usage, <strong>Open folder</strong>, auto-clean attachments (default <strong>30 days</strong>).</sub></p>

---

## Binary surface

| Command                       | Role                                                               |
| ----------------------------- | ------------------------------------------------------------------ |
| `relay` · `relay gui`         | Hub + local HTTP server                                            |
| `relay mcp`                   | MCP stdio (what the IDE runs)                                      |
| `relay feedback --retell "…"` | Terminal tryout; `--timeout`, `--session-title`, `--client-tab-id` |

There is **no** `relay window`; the IDE never spawns per-request GUI children.

---

## Configuration & paths

| OS      | User data dir                              |
| ------- | ------------------------------------------ |
| macOS   | `~/Library/Application Support/relay-mcp/` |
| Linux   | `~/.config/relay-mcp/`                     |
| Windows | `%APPDATA%\relay-mcp\`                     |

Notable files: `feedback_log.txt`, `ui_locale.json`, `gui_endpoint.json` (while GUI runs), `relay_gui_alive.marker` (heartbeat), `mcp_paused.json`, `attachment_retention.json`, `auto_reply_*.txt` (optional).

---

## Build

```bash
npm install
npm run build          # Vite frontend
cargo build --manifest-path src-tauri/Cargo.toml --release
npm run tauri build    # installers / .app / etc.
```

**Develop:**

```bash
npm run lint && npm run typecheck   # ESLint: src/**/*.vue + src/**/*.ts
npm run tauri dev
```

**Icons** (from [`src-tauri/icons/source/relay-icon.svg`](src-tauri/icons/source/relay-icon.svg)):

```bash
npm run icons:build
```

CI (PR / `main`): lint, typecheck, Vite build, `cargo fmt`, `clippy -D warnings`, `cargo test` — see [docs/RELEASING.md](docs/RELEASING.md).

---

## Privacy

All **Answers**, logs, and GUI state stay **on your machine**. No built-in telemetry. Treat **`feedback_log.txt`** and MCP transcripts as sensitive.

---

## License

[MIT](LICENSE)
