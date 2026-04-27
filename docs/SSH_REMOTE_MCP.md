# SSH Remote MCP — 远程人机回环设计方案

> OpenSpec 提案 · v0.1 · 2026-04-27

## 一、概述

允许用户在 B 电脑操作 Relay GUI，而 IDE + MCP 运行在 A 电脑，通过 SSH 反向隧道实现远程人机回环。所有配置在 GUI 侧完成，MCP 侧零改动。

## 二、核心架构

```
B 电脑 (GUI, 人在这边)              A 电脑 (IDE + MCP)
┌─────────────────────┐             ┌─────────────────────┐
│  Relay GUI          │             │  Cursor IDE         │
│  ┌───────────────┐  │             │  ┌───────────────┐  │
│  │ HTTP Server   │  │◄═══SSH═════►│  │ MCP Process   │  │
│  │ 127.0.0.1:GUI │  │  反向隧道    │  │               │  │
│  └───────────────┘  │             │  └───────┬───────┘  │
│                     │             │          │          │
│  管理 SSH 连接       │             │  读取 endpoint     │
│  写入 A 的 endpoint  │             │  连接 127.0.0.1:X  │
└─────────────────────┘             └─────────────────────┘
```

**数据流**：
1. B 的 GUI 通过 SSH 连接到 A
2. 建立 SSH 反向端口转发：A 的 `127.0.0.1:TUNNEL_PORT` → B 的 `127.0.0.1:GUI_PORT`
3. B 在 A 上写入 `gui_endpoint_<ide>.json`（port = TUNNEL_PORT）
4. A 的 MCP 读取 endpoint 文件 → 连接本地 TUNNEL_PORT → 流量通过 SSH 到达 B 的 GUI
5. MCP 侧完全无感知

## 三、配对机制

### 为什么需要配对

- `gui_endpoint_<ide>.json` 是单例，被覆盖会导致已有连接断开
- 多个 B 电脑可能尝试连接同一个 A
- 需要防止非授权方劫持 endpoint

### 配对流程

```
首次连接：
  B → SSH → A: 检查 relay 二进制是否存在
  B → SSH → A: 读取 remote_pair.json（如不存在则创建）
  B → SSH → A: 注册 pair_token（B 的唯一标识 + 共享密钥）
  B → SSH → A: 写入 gui_endpoint_<ide>.json（含 remote_pair_id）
  B ← SSH ← A: 确认配对成功

重连：
  B → SSH → A: 验证 pair_token
  B → SSH → A: 更新 gui_endpoint_<ide>.json（刷新端口）
  B ← SSH ← A: 确认连接恢复
```

### 配对数据（A 端存储）

```json
// remote_pair_<ide>.json — 存储在 A 的 relay 数据目录
{
  "pair_id": "uuid-v4",
  "pair_token_hash": "sha256-of-shared-secret",
  "connected_from": "user@B_host",
  "created_at": "2026-04-27T10:00:00Z",
  "last_connected_at": "2026-04-27T11:30:00Z"
}
```

## 四、多客户端规则

### 排他性原则

每个 IDE 类型（cursor / claude_code / windsurf / other）同时只允许**一个**活跃 GUI（本地或远程）。

| 场景 | 行为 |
|------|------|
| A 有本地 GUI 运行 | 远程 B 无法连接，提示"本地 GUI 运行中" |
| B1 已连接 | B2 连接 → 提示"另一个 GUI 已连接，是否接管？" |
| B2 确认接管 | B1 断开（收到断连事件）；B2 写入新 endpoint |
| B 主动断开 | 清理 A 上的 endpoint 文件，恢复为可连接状态 |
| 一个 B 连多个 A | 支持。各连接独立，tabs 按 A 来源标记 |

### endpoint 保护

```json
// gui_endpoint_cursor.json — 增加 remote 字段
{
  "port": 39527,
  "token": "bearer-xxx",
  "pid": null,
  "remote_pair_id": "uuid-v4",
  "remote_from": "user@B_host"
}
```

- `remote_pair_id` 非空时：仅持有对应 `pair_token` 的 B 可更新此文件
- `pid` 为 null（远程 GUI 无本地进程）：MCP 跳过 PID 存活检查

## 五、GUI 设置页设计

### 远程连接列表

```
┌──────────────────────────────────────────────────┐
│ 远程 IDE 连接                                     │
├──────────────────────────────────────────────────┤
│ ● dev@192.168.1.100 — Cursor                    │
│   已连接 · 2h 15m · 3 个活跃标签                  │
│   [断开] [移除]                                   │
├──────────────────────────────────────────────────┤
│ ○ admin@work-server — Claude Code                │
│   已断开 · 上次连接 3 天前                         │
│   [连接] [移除]                                   │
├──────────────────────────────────────────────────┤
│ [+ 添加远程 IDE]                                  │
└──────────────────────────────────────────────────┘
```

### 添加对话框

```
┌──────────────────────────────────────────────────┐
│ 添加远程 IDE 连接                                 │
├──────────────────────────────────────────────────┤
│ SSH 目标:  [user@host_________________]          │
│ SSH 端口:  [22_____]                              │
│ IDE 类型:   [Cursor ▼]                            │
│ SSH 密钥:  [~/.ssh/id_rsa_____] [浏览]           │
│                                                  │
│ 高级选项 ▸                                        │
│   ProxyJump:  [___________________]              │
│   远端 relay 路径: [自动检测_________]              │
│                                                  │
│            [取消]              [连接]             │
└──────────────────────────────────────────────────┘
```

## 六、数据模型

### Rust 结构

```rust
/// 远程连接配置（B 端持久化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConnection {
    pub id: String,
    pub ssh_target: String,         // user@host
    pub ssh_port: u16,              // default 22
    pub ssh_key_path: Option<String>,
    pub proxy_jump: Option<String>, // SSH ProxyJump
    pub ide_kind: IdeKind,
    pub pair_token: String,         // shared secret
    pub remote_relay_path: Option<String>, // relay binary on A
    pub created_at: String,
    pub last_connected_at: Option<String>,
}

/// 远程连接运行时状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConnectionStatus {
    pub id: String,
    pub state: RemoteState,
    pub tunnel_local_port: Option<u16>,
    pub connected_since: Option<String>,
    pub active_tabs: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemoteState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}
```

### TypeScript 类型

```typescript
interface RemoteConnection {
  id: string;
  sshTarget: string;
  sshPort: number;
  sshKeyPath?: string;
  proxyJump?: string;
  ideKind: IdeKind;
  createdAt: string;
  lastConnectedAt?: string;
}

interface RemoteConnectionStatus {
  id: string;
  state: 'disconnected' | 'connecting' | 'connected' | 'reconnecting' | 'error';
  tunnelLocalPort?: number;
  connectedSince?: string;
  activeTabs: number;
  error?: string;
}
```

## 七、SSH 隧道管理

### 建立隧道

```bash
# B 端执行（由 Relay GUI 管理）
ssh -o StrictHostKeyChecking=accept-new \
    -o ServerAliveInterval=30 \
    -o ServerAliveCountMax=3 \
    -i ~/.ssh/id_rsa \
    -R 0:127.0.0.1:{GUI_PORT} \
    -p {SSH_PORT} \
    {SSH_TARGET} \
    "echo TUNNEL_PORT_PLACEHOLDER && cat" # 读取动态分配的端口
```

### 健康检查

- SSH `ServerAliveInterval=30` + `ServerAliveCountMax=3` → 90s 无响应自动断开
- GUI 侧每 30s 通过 SSH 检查 endpoint 文件是否仍指向自己
- 断连后指数退避重连：5s → 10s → 20s → 40s → 60s（上限）

### 生命周期

```
GUI 启动 → 读取 remote_connections.json → 自动重连所有"上次连接"的远程
GUI 关闭 → 清理所有 SSH 进程 → 清理 A 上的 endpoint 文件
用户断开 → 关闭该 SSH 进程 → 清理 A 上的 endpoint → 更新状态
用户移除 → 断开 + 删除配置
```

## 八、安全模型

| 层 | 保障 |
|----|------|
| 传输 | SSH 原生加密（AES-256-GCM 等） |
| 认证 | SSH 密钥/密码 + pair_token 双重验证 |
| 授权 | 每个 IDE 只允许一个活跃 GUI，接管需确认 |
| 存储 | pair_token 哈希存储在 A；B 端存储明文（与 SSH 密钥同等信任级别） |
| endpoint | `remote_pair_id` 防劫持 |

## 九、与现有架构的兼容性

### MCP 侧（零改动）

- `mcp_http.rs` 的 `read_gui_endpoint()` 无需修改
- endpoint 文件格式向后兼容（新增字段有 `#[serde(default)]`）
- `pid` 为 null 时跳过存活检查（已有逻辑）

### GUI 侧（增量改动）

- `gui_http.rs`：无需改动（仍监听 `127.0.0.1`）
- 新增：`remote_ssh.rs` 模块（SSH 隧道管理）
- 新增：`remote_connection.rs` 模块（配置持久化）
- 新增：`SettingsRemotePanel.vue` 组件
- `main.rs`：注册新的 Tauri 命令

## 十、实现分期

### Phase 1：MVP（核心远程能力）
- [ ] 远程连接数据模型 + 持久化
- [ ] SSH 反向隧道建立与管理
- [ ] GUI 远程连接设置页
- [ ] endpoint 写入与保护
- [ ] 自动重连
- [ ] 排他性检查

### Phase 2：增强体验
- [ ] relay 二进制自动推送到 A
- [ ] 连接健康监测面板
- [ ] 多 A 支持 + tabs 来源标记
- [ ] B2 接管 B1 流程
- [ ] 连接统计（uptime、流量）

### Phase 3：高级功能
- [ ] `~/.ssh/config` 解析（Host 别名、ProxyJump）
- [ ] 跳板机（Bastion Host）支持
- [ ] Web GUI 远端访问（手机/平板）
- [ ] 连接分享（团队协作）
