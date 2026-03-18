# Relay

> Human feedback layer for AI IDEs

**[English](README.md)**

Relay MCP 是一个面向 AI IDE（如 [Cursor IDE](https://cursor.com)）的开源 MCP 工具。它会在代理请求中暂停，弹出原生桌面反馈窗口，并把用户回复返回到同一次交互中。

Relay 的写法受到 [interactive-feedback-mcp](https://github.com/junanchn/interactive-feedback-mcp) 项目的启发。

## 功能特性

- 基于 `Tauri + Rust + Vue` 的跨平台桌面界面
- 在 Windows、macOS、Linux 上提供统一的原生窗口体验
- 两个可执行入口共享同一套核心实现
- 首次启动会自动初始化用户数据目录
- 支持将文件拖入回复框
- 支持粘贴已复制的文件路径
- 支持自动回复规则，适合无人值守场景
- 所有交互都会写入同一用户数据目录中的 `feedback_log.txt`

## 仓库结构

- `src-tauri/` - Rust 后端、共享核心、MCP 服务端、CLI 辅助程序和 Tauri 窗口
- `src/` - Vue 前端
- `mcp.json` - MCP 配置示例

## 构建

先安装前端依赖并构建网页资源：

```bash
npm install
npm run build
```

然后构建 Rust 二进制：

```bash
cd src-tauri
cargo build --bins
```

开发模式下启动 Tauri 窗口：

```bash
npm run tauri dev
```

## 二进制文件

工作区会产出两个用户入口和一个桌面应用：

- `relay-server` - AI IDE 连接的 MCP 服务端
- `relay` - 直接启动同一套界面的命令行辅助程序
- `Relay` - 打包后的桌面反馈应用

在 Windows 上会自动带上 `.exe` 后缀。

## MCP 配置

把 AI IDE 指向构建后的 `relay-server`。示例：

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

在 Windows 机器上请替换为对应的 `.exe` 路径。

## 配置存储

Relay 会在首次启动时自动创建并管理用户数据目录中的自动回复文件，不需要手动指定路径。

常见位置如下：

- macOS：`~/Library/Application Support/relay-mcp/`
- Linux：`~/.config/relay-mcp/`
- Windows：`%APPDATA%\\relay-mcp\\`

该目录包含：

- `auto_reply_oneshot.txt`
- `auto_reply_loop.txt`
- `feedback_log.txt`

如果旧版本程序目录里已经存在自动回复文件，程序会尽量自动迁移到新的位置。

每一行规则格式如下：

```text
timeout_seconds|reply_text
```

`auto_reply_oneshot.txt` 会按顺序使用，命中后删除对应规则。`auto_reply_loop.txt` 会按顺序循环使用。

## 命令行用法

直接启动反馈窗口：

```bash
relay "工作摘要" 600
```

反馈会打印到 stdout。窗口超时或空提交时会输出空行。

## 工作原理

1. AI IDE 调用 `interactive_feedback`。
2. Rust 服务端先检查自动回复规则。
3. 如果没有立即命中的自动回复，就启动 `relay-gui`，把摘要和临时文件路径传给窗口。
4. Vue 窗口读取启动状态，轮询控制文件来感知超时或取消状态，提交时把反馈写入临时文件。
5. 服务端等待 GUI 退出，读取结果文件，再把反馈返回给 AI IDE。

## 参与贡献

欢迎提交 issue 和 pull request。请尽量保持改动与现有 MCP 协议、跨平台行为和 Relay 品牌命名一致。

## 许可证

MIT
