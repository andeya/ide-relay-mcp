# `client_tab_id` and window title **Chat N**

## Merge key: `client_tab_id`

Model-side convention (unchanged):

1. **Workspace root path** + newline + **first user message in this thread** (if too long, use only the first **500** characters of that message).
2. **Reuse the exact same string** on every turn within that thread.

## Title: **Chat {N}** (assigned by GUI)

- **`session_title` is no longer** used as the window title; `POST /v1/feedback` may still send `session_title` (old clients), but the **GUI ignores** it for labeling.
- Inside the Relay process:
  - **`chat_seq_counter`**: monotonically increasing;
  - **`client_tab_id_to_seq`**: each **`client_tab_id` seen for the first time** → bound to the current sequence **N** and **kept forever** (**closing a tab does not** reuse that **N** for another id).
- **No** `client_tab_id` (empty): every new tab consumes the **next** sequence number (no merge).

| Behavior                 | Result                                        |
| ------------------------ | --------------------------------------------- |
| New id, first request    | **Chat N** (N = next global number)           |
| Same id again            | Merges into existing tab, still **Chat N**    |
| Same id after tab closed | Still **Chat N** (mapping retained)           |
| Empty `client_tab_id`    | New tab each time, **Chat N+1** (next number) |

## Limitations

- Same path + identical first message → same `client_tab_id` → same Relay tab; change the opening line to split threads.
- **Sequence and id→N mapping live in memory only**: after quitting Relay and reopening, **Chat numbers and mappings reset** (reassigned from 1).

## MCP logging

The model may still pass `session_title` in the tool (e.g. for IDE-side logs); the GUI **does not** use it as the window title.
