# Relay

> Human feedback layer for AI IDEs

**[中文版](README_zh.md)**

Relay MCP is an open-source MCP tool for AI IDEs such as [Cursor IDE](https://cursor.com). It pauses an agent request, shows a native desktop feedback window, and returns the human response inside the same interaction.

Relay was inspired by [interactive-feedback-mcp](https://github.com/junanchn/interactive-feedback-mcp).

## Highlights

- Cross-platform desktop UI powered by `Tauri + Rust + Vue`
- Native feedback window on Windows, macOS, and Linux
- Two executable entry points that share one core implementation
- Automatic config bootstrap in the user data directory
- Drag and drop files into the response box
- Paste copied file paths directly into the editor
- Automatic reply rules for unattended runs
- Timestamped logs written to the same user data directory

## Repository Layout

- `src-tauri/` - Rust backend, shared core, MCP server, CLI helper, and Tauri window
- `src/` - Vue user interface
- `mcp.json` - MCP client configuration example

## Build

Install the frontend dependencies and build the web assets:

```bash
npm install
npm run build
```

Build the Rust binaries:

```bash
cd src-tauri
cargo build --bins
```

Run the desktop app in development mode:

```bash
npm run tauri dev
```

## Binaries

The workspace produces two user-facing executables and one desktop app:

- `relay-server` - MCP server that AI IDEs connect to
- `relay` - command-line helper that launches the same UI
- `Relay` - the packaged desktop feedback app

On Windows, the binary names automatically receive the `.exe` suffix.

## MCP Configuration

Point your AI IDE to the built `relay-server` binary. Example:

```json
{
  "mcpServers": {
    "relay-mcp": {
      "command": "/absolute/path/to/relay-server",
      "args": [],
      "timeout": 6000,
      "autoApprove": ["interactive_feedback"]
    }
  }
}
```

Use the equivalent Windows path ending in `.exe` on Windows.

## Configuration Storage

Relay creates and manages its auto-reply files in your user data directory on first launch. You never need to choose or pass a path manually.

Typical locations are:

- macOS: `~/Library/Application Support/relay-mcp/`
- Linux: `~/.config/relay-mcp/`
- Windows: `%APPDATA%\\relay-mcp\\`

The directory contains:

- `auto_reply_oneshot.txt`
- `auto_reply_loop.txt`
- `feedback_log.txt`

Existing auto-reply files from the legacy installation directory are migrated automatically when possible.

Each non-comment line uses this format:

```text
timeout_seconds|reply_text
```

`auto_reply_oneshot.txt` consumes the first matching rule and deletes it after use. `auto_reply_loop.txt` cycles through rules in order.

## CLI Usage

Launch the desktop UI directly without MCP:

```bash
relay "Work summary" 600
```

The feedback is printed to stdout. If the window times out or closes empty, the command prints an empty line.

## How It Works

1. An AI IDE calls `interactive_feedback`.
2. The Rust server checks auto-reply rules first.
3. Otherwise it launches `relay-gui` with the summary and temporary file paths.
4. The Vue window reads its launch state, watches the control file for timeout or cancellation, and writes feedback to a temp file on submit.
5. The server waits for the GUI process to exit, reads the result file, and returns the text to the AI IDE.

## Contributing

Contributions, bug reports, and platform-specific feedback are welcome. Please keep changes aligned with the existing MCP contract, the Relay brand, and the cross-platform behavior.

## License

MIT
