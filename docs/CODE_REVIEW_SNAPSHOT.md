# Code review snapshot (GUI + core Rust)

Review scope: representative pass over `src/` (bootstrap, `App.vue`, composables, editor), `src-tauri/src/*.rs` (main, lib, gui_http, server, storage, config, dock_edge_hide, mcp_* , path_persistence, release_check). **Not** a line-by-line audit of every asset or test.

## Issues found and status

| ID | Area | Issue | Severity | Status |
|----|------|--------|----------|--------|
| R1 | `App.vue` | `qaRounds.length` watch only: after send, last round updates `submitted`/`reply` without length change → no scroll to bottom → ME bubble clipped | High | **Fixed**: second watch includes reply/attachment deps and calls `scrollQaToBottom()` |
| R2 | `style.css` | Last message flush with scroll bottom edge | Medium | **Fixed**: extra `padding-bottom` on scroll region |
| R3 | `dock_edge_hide.rs` | `expand_if_collapsed` forced `set_always_on_top(false)`, desync from persisted `window_always_on_top` | High | **Fixed**: use `read_window_always_on_top()` after expand |
| R4 | `main.rs` | `build(...).expect(...)` panics on misconfiguration | Medium | Open (acceptable bootstrap fail-fast) |
| R5 | `gui_http.rs` | Session hydration cache: disk can outpace in-memory merge for long-lived tab | Medium | Open (documented earlier; product decision) |
| R6 | `get_feedback_tabs` | Calls `hydrate_qa_from_log` on every snapshot | Low | Open (monitor if profiling shows cost) |

## Follow-up plan

1. After large refactors, run `cargo fmt --check` in CI (already in workflow).
2. If hydration drift is reported, consider revisiting `hydrated_sessions` policy for active tabs.
3. Optional: replace `build().expect` with user-visible error route (larger change).

## Implemented in session (summary)

- `src/App.vue`: last-round signature watch + `scrollQaToBottom`.
- `src/style.css`: `padding-bottom` for `.mainContextZoneScroll .mainSummaryScroll`.
- `src-tauri/src/dock_edge_hide.rs`: restore always-on-top from config after peek expand.
