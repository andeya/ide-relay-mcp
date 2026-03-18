# Relay — Domain terminology

Single source of truth for product copy and docs. Transport details: **[HTTP_IPC.md](HTTP_IPC.md)**.

---

## 1. One human-in-the-loop turn

| Term (EN)         | 中文          | Definition                                                                                                                                 |
| ----------------- | ------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| **Relay tool**    | Relay 工具    | MCP tool **`relay_interactive_feedback`**.                                                                                                  |
| **Retell**        | 复述字段      | MCP **`retell`** (required, non-empty). **This turn’s assistant reply** — verbatim, over **127.0.0.1 HTTP** (no shell limits). |
| **Answer**        | 你的回复      | Human submission; tool return value (+ optional `<<<RELAY_FEEDBACK_JSON>>>` for attachments).                                              |
| **Session title** | 会话标题      | MCP **`session_title`**. Shown in tab/window chrome when set.                                                                                |
| **Client tab id** | 客户端标签 ID | MCP **`client_tab_id`**. Stable per IDE chat tab; same id on every call in one tab → **one Relay tab**, updated on new requests.               |

---

## 2. Binaries & modes

| Term                          | Definition                                                                 |
| ----------------------------- | -------------------------------------------------------------------------- |
| **`relay` / `relay gui`**     | Hub window + **local HTTP server** (`gui_endpoint.json`: port + Bearer token). |
| **`relay mcp`**               | stdio MCP server (`args`: `["mcp"]`). Talks to GUI only via that HTTP API. |
| **`relay feedback --retell`** | Terminal tryout; opens GUI if needed; Answer on stdout.                    |

There is **no** `relay window` subcommand; the IDE never spawns per-request GUI processes.

---

## 3. Doc checklist

- [ ] **`retell`** = current-turn assistant reply (content itself), non-empty
- [ ] **`session_title` / `client_tab_id`** strongly recommended when the IDE exposes them
- [ ] Code comments in **English** where they explain implementation
