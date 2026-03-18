<div align="center">

<br/>

<img src="src-tauri/icons/source/relay-icon.svg" alt="Relay" width="132" height="132"/>

# Relay

### Human feedback layer for AI IDEs

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-6366f1?style=flat-square" alt="License"/></a>
  <a href="https://tauri.app/"><img src="https://img.shields.io/badge/Tauri-2-24adc8?style=flat-square&logo=tauri&logoColor=white" alt="Tauri"/></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-backend-000000?style=flat-square&logo=rust&logoColor=white" alt="Rust"/></a>
  <a href="https://vuejs.org/"><img src="https://img.shields.io/badge/Vue-3-42b883?style=flat-square&logo=vuedotjs&logoColor=white" alt="Vue"/></a>
</p>

**[简体中文](README_zh.md)** · **[Terminology](docs/TERMINOLOGY.md)** · **[HTTP IPC (architecture)](docs/HTTP_IPC.md)**

**Author:** andeya · [andeyalee@outlook.com](mailto:andeyalee@outlook.com)

<br/>

</div>

---

Relay MCP is an open-source MCP tool for AI IDEs such as [Cursor](https://cursor.com). It **pauses** an agent request, opens a **native desktop** UI for your **Answer**, and returns it in the **same** tool turn. Terminology: **[docs/TERMINOLOGY.md](docs/TERMINOLOGY.md)**.

Inspired by [interactive-feedback-mcp](https://github.com/junanchn/interactive-feedback-mcp).

|          |                                                                                              |
| :------- | :------------------------------------------------------------------------------------------- |
| **Logo** | Human → **pause gate** (feedback) → AI — cyan/violet flow, amber bars for “waiting for you”. |

---

## Value & how it works

**What Relay is for**

- The agent calls **`relay_interactive_feedback`**. **`retell`** = **this turn’s user-visible assistant reply** (verbatim), sent **in full** over **127.0.0.1 HTTP**. Optional **`session_title`**, **`client_tab_id`**. Your reply is the **Answer**.
- **`relay mcp`** opens or talks to the Relay UI for your **Answer**, or returns text from **auto-reply rules** (no window).
- The **Answer** is normal tool output — same chat turn — **human-in-the-loop** without relying on the in-IDE chat box alone.

**How it runs**

1. The IDE runs **`relay mcp`** over **stdio**. The agent issues **`tools/call`** for **`relay_interactive_feedback`**.
2. The server **checks instant auto-reply rules** (`0|reply`). If one applies, it returns that text immediately (no window).
3. Otherwise it ensures the **Relay GUI** is running (`relay` / `relay gui`), then **`POST /v1/feedback`** + **`GET .../wait`** on **localhost HTTP** (see **[docs/HTTP_IPC.md](docs/HTTP_IPC.md)**). The GUI shows a **tab** (or merges by **`client_tab_id`**).
4. Your **Answer** completes the wait and is returned as the **tool result**.

**Tabs:** Keep Relay running when possible. New MCP calls add or refresh tabs; non-active tabs **flash** on new requests. If the GUI is closed, the next call spawns it again.

---

## Highlights

- **Stack** — `Tauri + Rust + Vue`, Windows / macOS / Linux
- **Flow** — `relay mcp` ↔ localhost HTTP ↔ Relay GUI (tabs); see [HTTP_IPC.md](docs/HTTP_IPC.md)
- **DX** — **Retell / Answer** thread; **Enter** to send (window stays); **Shift+Enter** newline; **⌘/Ctrl+Enter** send & close current tab; paste or attach images
- **Ops** — Optional instant auto-reply (config lines like `0|your reply`); `feedback_log.txt`

---

## Repository layout

| Path         | Role                                                           |
| ------------ | -------------------------------------------------------------- |
| `src-tauri/` | Rust backend, MCP server, CLI, Tauri window                    |
| `src/`       | Vue UI (`App.vue` + composables under `src/composables/`)      |
| `docs/`      | **[TERMINOLOGY.md](docs/TERMINOLOGY.md)** — product vocabulary |
| `mcp.json`   | MCP client example                                             |

### Development

```bash
npm install
npm run lint       # ESLint on `src/**/*.vue`
npm run typecheck  # `vue-tsc --noEmit`
npm run tauri dev
```

### Regenerate icons

Source: [`src-tauri/icons/source/relay-icon.svg`](src-tauri/icons/source/relay-icon.svg) (passed to `tauri icon` as SVG).

```bash
npm run icons:build
```

Needs **Node** (`@tauri-apps/cli`). Writes desktop, **iOS**, **Android**, and **Windows Store** assets under `src-tauri/icons/`.

---

## Build

```bash
npm install
npm run build
```

From repo root (no `cd`):

```bash
cargo build --manifest-path src-tauri/Cargo.toml --release
```

**Production app bundle** (installers / `.app` / etc.):

```bash
npm run tauri build
```

```bash
npm run tauri dev
```

---

## Privacy & data

Relay keeps **Answers** and state **on your machine** (logs, locale, `gui_endpoint.json` while the GUI runs). **No** built-in telemetry or cloud upload. Treat `feedback_log.txt` and MCP transcripts as sensitive.

---

## Binary & commands

One executable **`relay`** (Windows: `relay.exe`). Subcommands via **clap**:

| Invocation                    | Purpose                                                                                      |
| ----------------------------- | -------------------------------------------------------------------------------------------- |
| `relay` / `relay gui`         | Open Relay hub window                                                                        |
| `relay mcp`                   | MCP stdio server for the IDE                                                                 |
| `relay feedback --retell "…"` | **Terminal only:** **Answer** on stdout (`--timeout`, `--session-title`, `--client-tab-id`). |
| _(removed)_                   | **`relay window`** — replaced by HTTP IPC; IDE only runs **`relay mcp`**.                    |

Packaged **Relay.app** / installer ships this single binary.

---

## MCP configuration

`command` = path to **`relay`**, **`args`** = **`["mcp"]`**. Examples:

| Environment                              | `command` (example)                            |
| ---------------------------------------- | ---------------------------------------------- |
| **macOS** — Relay.app in `/Applications` | `/Applications/Relay.app/Contents/MacOS/relay` |
| **Windows**                              | `C:\Program Files\Relay\relay.exe`             |
| **Linux** / **from source**              | `…/target/release/relay`                       |

If the app is elsewhere (e.g. `~/Applications` on macOS), change the path accordingly.

```json
{
  "mcpServers": {
    "relay-mcp": {
      "command": "/Applications/Relay.app/Contents/MacOS/relay",
      "args": ["mcp"],
      "timeout": 600,
      "autoApprove": ["relay_interactive_feedback"]
    }
  }
}
```

- **`timeout`**: Waiting for your **Answer** can take a while; the example uses `600`. If the tool is cancelled before you submit, raise the tool-call wait limit in the IDE MCP settings.

See [`mcp.json`](mcp.json) in the repo (replace `command` with your path).

| Argument                          | Required              | Role                                        |
| --------------------------------- | --------------------- | ------------------------------------------- |
| `retell`                          | yes (non-empty)       | **This turn’s assistant reply** (verbatim). |
| `session_title` / `client_tab_id` | no (strongly advised) | Tab label; stable id per conversation tab.  |

### Window behavior

After **instant auto-reply** or **IDE cancel**: empty draft may close the tab; cancelled/timed-out state is shown if you had text. Submitting an **Answer** returns you to the hub; the app stays open.

### Rule prompts (English)

Tool: **`relay_interactive_feedback`**. Prompts are **English-only** for stricter model behavior. In the Relay app, **⚙ Settings → Rule prompts**: pick Standard / Strict loop / Tool spec, copy, and follow the per-IDE paste guide on the same screen. Source: [`src/cursorRulesTemplates.ts`](src/cursorRulesTemplates.ts) (English + Chinese mirror).

---

## Configuration storage

Relay bootstraps under your user data directory (no manual path).

| OS      | Path                                       |
| ------- | ------------------------------------------ |
| macOS   | `~/Library/Application Support/relay-mcp/` |
| Linux   | `~/.config/relay-mcp/`                     |
| Windows | `%APPDATA%\relay-mcp\`                     |

Files may include: `feedback_log.txt`, `ui_locale.json` (`en` / `zh`), `gui_endpoint.json` (while GUI is running), `relay_gui_alive.marker` (heartbeat), auto-reply rule files.

**Optional instant auto-reply** (no window): create **`auto_reply_oneshot.txt`** and/or **`auto_reply_loop.txt`** in that folder (they are **not** created automatically). Only **`0|`** lines apply — immediate reply; any other `N|` is ignored:

```text
0|reply_text
```

---

## CLI usage

Put the folder containing **`relay`** on your **PATH**, or use full paths. Run **`relay --help`** for all options.

Open **⚙ Settings**: **EN / 中文** toggles UI language; **Environment & MCP** and **Rule prompts**.

| OS          | Add to PATH (same as before — folder with `relay`)                |
| ----------- | ----------------------------------------------------------------- |
| **macOS**   | e.g. `export PATH="/Applications/Relay.app/Contents/MacOS:$PATH"` |
| **Linux**   | e.g. `export PATH="$HOME/.local/bin:$PATH"` for custom installs   |
| **Windows** | Add directory containing `relay.exe` to user **Path**             |

Terminal (**Answer** on stdout):

```bash
relay feedback --retell "Work recap" --timeout 600
relay feedback --retell "Work recap" --session-title "Chat title"
# Same id → same Relay tab as MCP; different ids → multiple tabs in one window
relay feedback --retell "…" --client-tab-id "my-terminal-session"
```

Stdout = **Answer**; empty line on timeout / empty submit. The IDE runs **`relay mcp`** only — subcommands, no positional args.

---

## License

[MIT](LICENSE)
