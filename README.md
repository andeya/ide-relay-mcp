# Interactive Feedback MCP

**[中文版](README_zh.md)**

An MCP (Model Context Protocol) tool for [Cursor IDE](https://cursor.com) that lets the AI pause and wait for user feedback within a single request, reducing billing costs by keeping multi-turn interactions in one session.

## Why?

Cursor charges per request. Without this tool, each round of feedback requires a new request. With Interactive Feedback MCP, the AI presents its work summary, waits for your input, and continues — all within one request.

## Features

- Single-request feedback loop — AI pauses and waits for user input without consuming extra requests
- Always-on-top native Win32 feedback window
- Drag & drop files onto the window or paste copied files to insert their paths
- Auto-reply rules for unattended operation (oneshot and loop modes)
- Window management — empty windows close when no longer needed; windows with user input stay open to avoid losing text
- All interactions (AI request / user reply / auto-reply) logged with timestamps to `feedback_log.txt`

## Quick Start

### 1. Download

Download the latest release from [Releases](https://github.com/panyuQ/interactive-feedback-mcp/releases). You'll get two files:

- `feedback_server.exe` — the MCP server
- `feedback_gui.exe` — the GUI window

Place them in the same folder (e.g. `C:\Tools\interactive-feedback-mcp\`).

### 2. Configure MCP

Open Cursor Settings → MCP → Add a new MCP server, or manually edit your `mcp.json`:

**For global use** — edit `%USERPROFILE%\.cursor\mcp.json`:

```json
{
  "mcpServers": {
    "interactive-feedback-mcp": {
      "command": "C:\\path\\to\\feedback_server.exe",
      "timeout": 600,
      "autoApprove": ["interactive_feedback"]
    }
  }
}
```

**For a specific project** — create `.cursor/mcp.json` in the project root with the same content.

Replace the path with the actual location on your machine. Use double backslashes in JSON. `timeout` is in seconds — 600 gives you 10 minutes to respond. `autoApprove` skips the manual approval popup each time the AI calls the tool.

### 3. Configure Cursor Rules

Add the following to Cursor Settings → Rules:

```
After completing your full response, you MUST call interactive_feedback as your very last action.
If the feedback is empty, end the request.
```

This tells the AI to always pause for feedback before finishing. When you submit empty or close the window, the AI ends the request.

### 4. Use

Start a chat. After the AI responds, a feedback window pops up with the AI's work summary:

- Type feedback and submit (<kbd>Ctrl+Enter</kbd> or button) → AI continues in the same request
- Close the window or submit empty → AI ends the request

## How It Works

1. AI calls `interactive_feedback` with a work summary.
2. If there's an auto-reply rule with timeout `0`, the server returns it immediately — no GUI.
3. Otherwise, the server launches `feedback_gui.exe`, passing the summary via command-line arguments. The GUI shows the summary (read-only) and a text input box, always on top.
4. The server waits for one of three events:
   - **User submits** — GUI writes feedback to a temp file and exits. The server reads it and returns the feedback to the AI. Non-empty feedback resets the auto-reply loop index.
   - **Auto-reply timeout** — the auto-reply text is returned to the AI. The GUI window is handled as described in [Window Behavior](#window-behavior).
   - **Request cancelled by Cursor** — no result is returned. The window title changes to `[Cancelled]`.
5. The server's main loop uses `WaitForMultipleObjects` to watch stdin data, GUI process exit, and auto-reply timeout simultaneously, so it can respond to Cursor messages at any time while waiting for feedback.

## Window Behavior

When a window becomes obsolete (superseded by a new tool call, replaced by auto-reply, or request cancelled):

- **Input box is empty** → window closes automatically.
- **Input box has content** → window stays open with title `[Superseded]` or `[Cancelled]`, so your text is not lost. Submitting from such a window has no effect — the server is no longer listening for it.

When the AI makes a new tool call while an old window is still open, the server notifies the old window (close or mark superseded), then launches a new one.

## Auto-Reply

When you step away, auto-reply rules let the AI session continue without manual input.

### Setup

Place config files in the same folder as `feedback_server.exe`. Format: `timeout_seconds|reply_text` per line. `#` for comments. Files are reloaded from disk on each tool call, so you can edit them anytime with a text editor.

### `auto_reply_oneshot.txt`

Rules used once in order. After a rule fires, it is permanently deleted from the file. Oneshot rules always take priority over loop rules.

```
0|Continue with the implementation.
```

### `auto_reply_loop.txt`

Rules cycle in order, wrapping around to the start after the last one. The cycle index resets when a user submits non-empty feedback.

```
540|Waiting for user. 19 checks remaining. Call MCP again.
540|Waiting for user. 18 checks remaining. Call MCP again.
...
540|Waiting for user. 1 check remaining. Call MCP again.
540|User away too long. Do NOT call MCP again.
```

When `timeout_seconds` is `0`, the auto-reply fires immediately and the GUI is never opened.

## Architecture

```
Cursor  ←— stdio JSON-RPC —→  feedback_server.exe  ←— args / temp file —→  feedback_gui.exe
                                                    ←— Win32 messages ——→
```

Two processes: the MCP server communicates with Cursor via stdin/stdout using JSON-RPC 2.0 (one JSON per line). The GUI is a separate Win32 process — this separation is necessary because Cursor occupies the server's stdin/stdout. Communication between them: command-line arguments (at launch), Windows messages (at runtime, for supersede/cancel notifications), and a temp file (for returning feedback).

## Build from Source

C++17, CMake 3.10+. Only dependency is nlohmann/json (bundled as `json.hpp`).

```bash
mkdir build && cd build && cmake .. && cmake --build .
```

Output: `build/bin/feedback_server.exe` and `build/bin/feedback_gui.exe`.

## License

MIT
