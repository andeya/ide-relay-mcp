<div align="center">

<br/>

<img src="src-tauri/icons/source/relay-icon.svg" alt="Relay" width="132" height="132"/>

# Relay

### 面向 AI IDE 的人工反馈层

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-6366f1?style=flat-square" alt="License"/></a>
  <a href="https://tauri.app/"><img src="https://img.shields.io/badge/Tauri-2-24adc8?style=flat-square&logo=tauri&logoColor=white" alt="Tauri"/></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-backend-000000?style=flat-square&logo=rust&logoColor=white" alt="Rust"/></a>
  <a href="https://vuejs.org/"><img src="https://img.shields.io/badge/Vue-3-42b883?style=flat-square&logo=vuedotjs&logoColor=white" alt="Vue"/></a>
</p>

**[English](README.md)** · **[领域用语](docs/TERMINOLOGY.md)** · **[HTTP IPC 架构](docs/HTTP_IPC.md)**

**作者：** andeya · [andeyalee@outlook.com](mailto:andeyalee@outlook.com)

<br/>

</div>

---

Relay MCP 在代理请求中**暂停**，打开**原生桌面**界面收集 **Answer（你的回复）**，并在**同一次工具调用**里返回给模型。用语见 **[docs/TERMINOLOGY.md](docs/TERMINOLOGY.md)**。

思路参考 [interactive-feedback-mcp](https://github.com/junanchn/interactive-feedback-mcp)。

|              |                                                                           |
| :----------- | :------------------------------------------------------------------------ |
| **图标含义** | 人类端 → **暂停/反馈门** → AI 端；青紫通路 + 中央琥珀色「等待你」双竖条。 |

---

## 价值与原理

**能做什么**

- 智能体调用 **`relay_interactive_feedback`**，必须传 **`retell`**（本轮助手对用户可见回复原文），经 **127.0.0.1 HTTP** 传到 Relay 窗口。可选 **`session_title`**、**`client_tab_id`**。你提交 **Answer**。
- **`relay mcp`** 在需要时拉起或与 Relay 窗口通信收集 **Answer**，也可按 **自动回复规则**直接返回（不弹窗）。
- **Answer** 作为工具输出回到智能体，**同一轮对话继续**。

**工作原理**

1. IDE 运行 **`relay mcp`**（stdio）。智能体发起 **`relay_interactive_feedback`**。
2. **即时自动回复**（`0|回复`）命中则直接返回。
3. 否则确保 **Relay GUI**（`relay` / `relay gui`）已起，经 **`POST /v1/feedback`** + **`GET .../wait`** 完成一轮交互（详见 **[docs/HTTP_IPC.md](docs/HTTP_IPC.md)**）。
4. **Answer** 作为工具返回值交给 IDE。

**多标签：** 建议保持 Relay 常开。新请求新增或按 **`client_tab_id`** 合并标签；非当前标签会**闪动**提示。

---

## 功能特性

- **技术栈** — `Tauri + Rust + Vue`，支持 Windows / macOS / Linux
- **体验** — `relay mcp` ↔ 本机 HTTP ↔ 桌面多标签（见 [HTTP_IPC.md](docs/HTTP_IPC.md)）
- **使用** — **复述 / Answer** 对话流；**Enter** 发送（窗口保留）；**Shift+Enter** 换行；**⌘/Ctrl+Enter** 发送并关闭当前标签页；支持贴图
- **运维** — 可选即时自动回复（配置行形如 `0|你的回复`）；`feedback_log.txt`

---

## 仓库结构

| 路径         | 说明                                               |
| ------------ | -------------------------------------------------- |
| `src-tauri/` | Rust 后端、MCP、CLI、Tauri 窗口                    |
| `src/`       | Vue 前端（`App.vue` + `src/composables/`）         |
| `docs/`      | **[TERMINOLOGY.md](docs/TERMINOLOGY.md)** 领域用语 |
| `mcp.json`   | MCP 配置示例                                       |

### 开发

```bash
npm install
npm run lint       # 对 `src/**/*.vue` 运行 ESLint
npm run typecheck  # `vue-tsc --noEmit`
npm run tauri dev
```

### 重新生成图标

矢量源：[`src-tauri/icons/source/relay-icon.svg`](src-tauri/icons/source/relay-icon.svg)（直接交给 `tauri icon`）。

```bash
npm run icons:build
```

需 **Node**（`@tauri-apps/cli`）。会生成桌面及 **iOS、Android、Windows 商店** 等资源（见 `src-tauri/icons/`）。

---

## 构建

```bash
npm install
npm run build
```

在仓库根目录执行（无需 `cd`）：

```bash
cargo build --manifest-path src-tauri/Cargo.toml --release
```

**正式打包安装包**（安装程序 / `.app` 等）：

```bash
npm run tauri build
```

```bash
npm run tauri dev
```

---

## 隐私与数据

**Answer** 与状态**仅在本机**（含运行中的 `gui_endpoint.json` 等，见 [配置存储](#配置存储)）。**无**遥测。**无**云端上报。

---

## 可执行文件与子命令

仅一个 **`relay`**（Windows：`relay.exe`），用 **clap** 区分子命令：

| 命令                          | 作用                                                                                         |
| ----------------------------- | -------------------------------------------------------------------------------------------- |
| `relay` / `relay gui`         | 打开 Relay 窗口                                                                              |
| `relay mcp`                   | IDE 的 MCP 服务（stdio）                                                                     |
| `relay feedback --retell "…"` | **仅终端：** **Answer** 输出到 stdout（`--timeout`、`--session-title`、`--client-tab-id`）。 |
| _已移除_                      | 原 **`relay window`** 已由 **HTTP IPC** 替代；IDE 只需 **`relay mcp`**。                     |

---

## MCP 配置

`command` = **`relay` 绝对路径**，**`args`** = **`["mcp"]`**。

| 环境             | `command` 示例                                 |
| ---------------- | ---------------------------------------------- |
| **macOS**        | `/Applications/Relay.app/Contents/MacOS/relay` |
| **Windows**      | `C:\Program Files\Relay\relay.exe`             |
| **源码 release** | `…/target/release/relay`                       |

若 `.app` 在用户目录（如 `~/Applications/Relay.app`），请把路径改成对应位置。

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

- **`timeout`**：等待 **Answer** 可能较久，示例为 `600`。若未提交即被宿主判超时，请在 IDE 的 MCP 设置中提高工具等待上限。

仓库里的 [`mcp.json`](mcp.json) 请按本机路径填写。

### 工具参数（给智能体）

| 参数            | 必填       | 作用                               |
| --------------- | ---------- | ---------------------------------- |
| `retell`        | 是（非空） | **本轮**助手对用户回复的**原文**。 |
| `session_title` | 否（建议） | IDE 标签/会话标题。                |
| `client_tab_id` | 否（建议） | 每标签稳定 ID。                    |

### 窗口行为

- **即时自动回复**或 **IDE 取消**后：空草稿可能关标签；已取消/超时见界面。提交 **Answer** 后回到占位页，应用保持打开。

### 规则提示词（英文）

工具名 **`relay_interactive_feedback`**。正文为**英文**，便于模型严格执行；请在 **⚙ 设置 → 规则提示词** 选择「标准 / 严格循环 / 仅工具说明」并复制，同页附有 Cursor、Windsurf、VS Code、Claude Desktop 等粘贴说明。离线维护见源码 [`src/cursorRulesTemplates.ts`](src/cursorRulesTemplates.ts)（含英文与中文对照版）。

---

## 配置存储

首次启动会在用户数据目录自动创建配置（无需手填路径）。

| 系统    | 路径                                       |
| ------- | ------------------------------------------ |
| macOS   | `~/Library/Application Support/relay-mcp/` |
| Linux   | `~/.config/relay-mcp/`                     |
| Windows | `%APPDATA%\relay-mcp\`                     |

常见文件：`feedback_log.txt`、`ui_locale.json`、`gui_endpoint.json`（GUI 运行时）、`relay_gui_alive.marker`、自动回复规则文件等。

**可选即时自动回复**（不弹窗）：在该目录下**自行创建** **`auto_reply_oneshot.txt`** 和/或 **`auto_reply_loop.txt`**（安装时**不会**自动生成）。仅 **`0|`** 行生效（其它 `数字|` 忽略）：

```text
0|reply_text
```

---

## 命令行

`relay --help` 查看全部子命令。把含 **`relay`** 的目录加入 PATH（与此前相同）。

点击 **⚙ 设置**：**环境与 MCP**、**规则提示词**。

### 终端试用（stdout = Answer）

```bash
relay feedback --retell "工作摘要" --timeout 600
relay feedback --retell "工作摘要" --session-title "当前会话标题"
# 与 MCP 相同：同一 client-tab-id 合并到同一 Relay 标签；不同 id 则多标签
relay feedback --retell "…" --client-tab-id "终端会话甲"
```

stdout 为 **Answer**；超时或空提交为空行。IDE 只跑 **`relay mcp`**，勿用位置参数。

---

## 许可证

[MIT](LICENSE)
