# Relay — Domain terminology

Single source of truth for product copy and docs. Transport details: **[HTTP_IPC.md](HTTP_IPC.md)**.

---

## 1. One human-in-the-loop turn

| Term (EN)         | Summary                | Definition                                                                                                                                                                                       |
| ----------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Relay tool**    | MCP tool name          | MCP tool **`relay_interactive_feedback`**.                                                                                                                                                       |
| **Retell**        | Retell parameter       | MCP **`retell`** (required, non-empty). **This turn’s assistant reply** — verbatim, over **127.0.0.1 HTTP** (no shell limits).                                                                   |
| **Answer**        | Human submission       | Human submission; tool return value (+ optional `<<<RELAY_FEEDBACK_JSON>>>` for attachments).                                                                                                    |
| **Client tab id** | Merge key              | MCP **`client_tab_id`**: **workspace root + newline + first user message** ([**CLIENT_TAB_ID.md**](CLIENT_TAB_ID.md)). Merge key; GUI binds **Chat N** on first sight, reuses **N** for that id. |
| **Session title** | Optional (GUI ignores) | MCP **`session_title`** — optional; **GUI ignores** for window label. Titles are **Chat N** (see **CLIENT_TAB_ID.md**).                                                                          |

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

## 3. Doc checklist

- [ ] **`retell`** = current-turn assistant reply (content itself), non-empty
- [ ] **`client_tab_id`** = workspace root + `\\n` + first user message → merge key; GUI shows **Chat N** per id (**CLIENT_TAB_ID.md**)
- [ ] Code comments in **English** where they explain implementation
