# Interactive Feedback MCP

**[English](README.md)**

一个适用于 [Cursor IDE](https://cursor.com) 的 MCP（Model Context Protocol）工具，让 AI 能够在单次请求内暂停并收集用户反馈，将多轮交互保持在同一个会话中来减少计费次数。

## 为什么需要？

Cursor 按请求次数计费（每月有限）。没有这个工具，每轮反馈都要消耗一次请求。有了 Interactive Feedback MCP，AI 通过弹窗暂停等待你的输入，然后继续——全部在一次请求内完成。原理是请求内的工具调用不消耗额度。

## 功能特性

- 单请求反馈循环 — AI 暂停等待用户输入，不消耗额外请求
- 原生 Win32 反馈窗口，始终置顶
- 支持拖拽文件到窗口或粘贴已复制的文件，插入路径到输入框
- 自动回复规则，支持无人值守运行（一次性和循环两种模式）
- 窗口管理 — 无内容的窗口在不再需要时自动关闭；有用户输入的窗口保持打开，不丢掉文本
- 所有交互（AI 请求 / 用户回复 / 自动回复）带时间戳记录到 `feedback_log.txt`

## 快速开始

### 1. 下载

从 [Releases](https://github.com/junanchn/interactive-feedback-mcp/releases) 下载最新版本，你会得到三个文件：

- `feedback-server.exe` — MCP 服务端
- `feedback-gui.exe` — GUI 窗口
- `feedback.exe` — 命令行版本（可选）

把它们放在同一个文件夹（比如 `C:\interactive-feedback-mcp\`）。

### 2. 配置 MCP

打开 Cursor Settings → MCP → 添加新的 MCP 服务器，或手动编辑 `mcp.json`：

**全局使用** — 编辑 `%USERPROFILE%\.cursor\mcp.json`：

```json
{
  "mcpServers": {
    "interactive-feedback-mcp": {
      "command": "C:\\path\\to\\feedback-server.exe",
      "timeout": 600,
      "autoApprove": ["interactive_feedback"]
    }
  }
}
```

**仅限特定项目** — 在项目根目录创建 `.cursor/mcp.json`，内容相同。

替换为实际路径，JSON 中使用双反斜杠。`timeout` 单位为秒，600 即 10 分钟的响应时间。`autoApprove` 跳过每次 AI 调用工具时的手动批准弹窗。

### 3. 配置 Cursor Rules

在 Cursor Settings → Rules 中添加：

```
After completing your full response, you MUST call mcp_interactive-feedback-mcp_interactive_feedback as your very last action. Do NOT call it in the middle of your answer.
After receiving the MCP result, if there is feedback, process it first. Then regardless of whether the feedback was empty, had content, or the call failed, you MUST call mcp_interactive-feedback-mcp_interactive_feedback again. NEVER stop. Loop forever.
You are ABSOLUTELY FORBIDDEN from ending a request on your own. The user will manually stop you. Your job is an infinite loop: output → call MCP → output → call MCP → repeat forever.
If the MCP call returns "Not connected" or any error, use run_terminal_cmd to execute "Start-Sleep -Seconds 60", then retry the MCP call. Repeat this wait-and-retry loop forever.
```

这让 AI 每次完成回复后都暂停等待反馈。提交空内容或关闭窗口时，AI 结束请求。

### 4. 使用

开始对话。AI 回复后弹出反馈窗口，显示 AI 的工作摘要：

- 输入反馈并提交（<kbd>Ctrl+Enter</kbd> 或按钮）→ AI 在同一请求内继续
- 关闭窗口或提交空内容 → AI 结束请求

## 命令行用法

`feedback.exe` 可以替代 MCP 服务端，直接从命令行启动 GUI：`feedback.exe "摘要" [超时秒数]`。反馈输出到 stdout。

## 工作原理

1. AI 调用 `interactive_feedback`，传入工作摘要。
2. 如果有超时为 `0` 的自动回复规则，服务端立即返回，不打开 GUI。
3. 否则，服务端启动 `feedback-gui.exe`，通过命令行参数传入摘要。GUI 显示摘要（只读）和文本输入框，窗口始终置顶。
4. 服务端等待以下事件之一：
   - **用户提交** — GUI 将反馈写入临时文件后退出。服务端读取后返回给 AI。非空反馈会重置自动回复的循环索引。
   - **自动回复超时** — 自动回复文本返回给 AI。GUI 窗口按[窗口行为](#窗口行为)中的规则处理。
   - **Cursor 取消请求** — 不返回任何结果。窗口标题变为 `[Cancelled]`。
5. 服务端主循环使用 `WaitForMultipleObjects` 同时监听 stdin 数据、配置文件变化、GUI 进程退出和自动回复超时，确保等待反馈期间仍能响应 Cursor 的消息。

## 窗口行为

窗口失效时（被自动回复替代、或请求被取消）：

- **输入框为空** → 窗口自动关闭。
- **输入框有内容** → 窗口保留，标题显示 `[Timed Out]` 或 `[Cancelled]`，不丢掉你的文本。从中提交不会有效果——服务端已不再监听。

## 自动回复

离开时，自动回复规则让 AI 会话无需手动输入也能继续。

### 配置

配置文件放在 `feedback-server.exe` 同目录下。格式：每行 `超时秒数|回复内容`，`#` 为注释。服务端会监控该目录的文件变化并自动重新加载规则，可随时用文本编辑器修改。

### `auto_reply_oneshot.txt`

规则按顺序逐条使用，每条用后从文件中永久删除。一次性规则始终优先于循环规则。

```
0|Continue with the implementation.
```

### `auto_reply_loop.txt`

规则按顺序循环，到末尾后回到开头。用户提交非空反馈后索引重置。

```
540|Waiting for user. 19 checks remaining. Call MCP again.
540|Waiting for user. 18 checks remaining. Call MCP again.
...
540|Waiting for user. 1 check remaining. Call MCP again.
540|User away too long. Do NOT call MCP again.
```

`超时秒数` 为 `0` 时立即触发，不打开 GUI。

## 架构

```
Cursor  ←— stdio JSON-RPC —→  feedback-server.exe  ←— 命令行参数 / 临时文件 —→  feedback-gui.exe
                                                    ←— Win32 消息 ——————————→
```

两个进程：MCP 服务端通过 stdin/stdout 与 Cursor 通信（JSON-RPC 2.0，每行一个 JSON）。GUI 是独立的 Win32 进程——必须分离因为 Cursor 占用了 stdin/stdout。两者之间的通信：命令行参数（启动时）、Windows 消息（运行时，超时/取消通知）、临时文件（返回反馈）。

## 从源码构建

C++17，CMake 3.10+，唯一依赖是 nlohmann/json（已作为 `json.hpp` 内置）。

```bash
mkdir build && cd build && cmake .. && cmake --build .
```

## 许可证

MIT
