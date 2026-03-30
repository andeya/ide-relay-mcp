/**
 * IDE rule prompt templates for relay_interactive_feedback.
 * - loop (default): title + head + body
 * - toolOnly: title + body (no call policy)
 */

export type RulePromptMode = "loop" | "toolOnly";

const FRONTMATTER = `---
description: Relay human-in-the-loop — MUST call relay_interactive_feedback at end of EVERY assistant turn. Highest priority.
alwaysApply: true
---`;

// --- Title (shared) ---
const TITLE = `# Relay 人机回环（relay_interactive_feedback）`;

const BODY_ZH = `**宿主与界面**：MCP 宿主为 \`relay mcp-<ide>\`（stdio），其中 \`<ide>\` 为 IDE 标识（如 \`cursor\`、\`claudecode\`、\`windsurf\`、\`other\`）。图形界面为 \`relay gui-<ide>\`（本地 HTTP）。终端试跑：\`relay feedback --retell "…"\`（与 MCP 的 \`retell\` 语义一致）。

### [CONTRACT] MCP 服务器配置（IDE）

- \`command\`：字符串 = Relay 可执行文件的绝对路径。
- \`args\`：数组 — 至少含 \`"mcp-<ide>"\`（如 \`["mcp-cursor"]\`）；WSL 内 Agent 使用 Windows \`relay.exe\` 时用 \`["mcp-<ide>", "--exe_in_wsl"]\` 以便附件路径改写为 \`/mnt/...\`。
- 工具名：\`relay_interactive_feedback\`（精确，无别名）。

### [PARAMS] 工具入参

| 参数                   | 类型                                           | 必填条件                                      | 语义 / 不变式                                                                                                                                                 |
| ---------------------- | ---------------------------------------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| \`retell\`             | string                                         | 是                                            | 本回合助手的完整回复原文。trim(retell) 非空，否则服务端返回 400。仅限本回合用户可见内容，非历史回顾或内部推理。经 127.0.0.1 HTTP 传递，无 argv/ARG_MAX 限制。 |
| \`relay_mcp_session_id\` | string                                         | 若已有则必传；否则省略或 \`""\`                 | 来源：上一次工具返回 JSON 的 \`relay_mcp_session_id\`。省略或 \`""\` = 新标签页；非空 = 合并到该标签页。                                                          |
| \`commands\`           | array of { name, id, category?, description? } | **无 session（新标签）**：每次调用**必须**带上本字段（数组）；内容应为当前宿主/IDE **能够枚举到的全部** commands，供 Relay 输入框斜杠补全。**仅当宿主确实无法提供任何项时**才允许 \`[]\`；**不得**在仍能拿到列表时偷懒传空。**有 session**：可选；若传入则按 \`id\` 与标签内已有列表合并去重。 |
| \`skills\`            | array of { name, id, category?, description? } | 与 \`commands\` 相同规则，对象均为当前宿主能获得的 **skills**。 |
| \`title\`             | string                                         | **仅新标签（无 session）时生效；强烈推荐传入**。≤60 字符的短描述标题，概括本次聊天上下文（如 \`"修复登录页 CSS"\`）。Relay 用它替代默认的 MM-DD HH:mm:ss 时间戳作为标签页标题。有 session 时忽略。**创建新标签时 agent 应始终传入此参数**以提供有意义的标签名。 |

### [RETURN] 工具结果

- **正常**：JSON \`{ "relay_mcp_session_id": "<ms>", "human": "<用户回答>", "cmd_skill_count": <number> [, "attachments": [{ "kind": "image"|"file", "path": "..." }] ] }\`；无附件时省略 \`attachments\`。\`path\` 在 MCP 返回中可为 Windows 路径；若 MCP 启动参数含 \`--exe_in_wsl\`（如 \`args\` 为 \`["mcp-<ide>", "--exe_in_wsl"]\`），则可能已改写为 WSL 的 \`/mnt/...\`。\`cmd_skill_count\` = 当前该标签页已保存的 **commands + skills** 条数（ slash 补全列表大小）。
- **清单为空时的再传**：若某次返回中 \`cmd_skill_count === 0\`（且非暂停哨兵），下一轮调用**必须**再次带上 \`commands\` 与 \`skills\`，并填入当前 IDE **能够枚举到的全部**项（**仅当确实无法提供任何项时**才为 \`[]\`），以恢复斜杠补全。
- **其它**：\`relay_mcp_session_id\` 为毫秒时间戳，Relay 标签 = agent 提供的 \`title\`（若有），否则 **MM-DD HH:mm:ss**。\`human\` 为用户回答（关闭/超时可为空）。
- **后置条件**：保存 \`relay_mcp_session_id\`，下一次调用时传入；若 \`human\` 非空或 \`attachments\` 非空，则当作用户输入并回复；**仅当 human 与 attachments 均为空（或 attachments 不存在）时本回合结束**。
- **哨兵**：若**整段**结果包含 \`<<<RELAY_MCP_PAUSED>>>\`，则不得再次调用，直到用户在 Relay 设置中恢复。

### [STATE] 会话状态（不变式）

- 变量：\`relay_mcp_session_id\` : string | undefined。**初始**：undefined（或 \`""\`）。
- **每次成功返回后**（且非哨兵）：\`relay_mcp_session_id := result.relay_mcp_session_id\`。
- **每次调用时**：若已有非空 \`relay_mcp_session_id\` 则必传该字段；若上轮返回 \`cmd_skill_count === 0\`，下一轮**必须**再次带上 \`commands\` 与 \`skills\`，且尽量填全 IDE 可枚举项（**仅当无可枚举项时**才为 \`[]\`）；否则有 session 时这两字段可省略。若无 \`relay_mcp_session_id\`（新标签），**必须**同时带上 \`commands\` 与 \`skills\` 两个数组，并尽量填入宿主当前能提供的全部项；**仅当宿主确实无法提供任何项时才使用 \`[]\`**。

### [CALL_SCHEDULE] 调用时机（通用）

- 在助手**确定本回合对用户可见的回复**之后调用。
- 每助手回合一次；该调用为该回合的**最后一个**工具调用（同一回合内不得在此之后再调用其他工具）。
- 后续调用传入 \`relay_mcp_session_id\`；若 \`human\` 非空或 \`attachments\` 非空，则视为用户输入并据此回复；**仅当两者均为空时本回合结束**。

### [UI] 人工门控（Answer）（仅产品说明，不影响工具调用）

纯文本 \`human\`；若有贴图/文件，结果 JSON 另有 \`attachments\` 数组（旧版把 JSON 塞进正文的做法服务端仍会剥离）。Enter → 提交；Shift+Enter → 换行；⌘/Ctrl+Enter → 提交并关闭标签。Agent 应同时读取 \`human\` 与 \`attachments\`（若有）。`;

const BODY_EN = `**Host & UI**: Host is \`relay mcp-<ide>\` (stdio), where \`<ide>\` is the IDE identifier (e.g. \`cursor\`, \`claudecode\`, \`windsurf\`, \`other\`). GUI: \`relay gui-<ide>\` (local HTTP). Terminal tryout: \`relay feedback --retell "…"\` (same semantics as MCP \`retell\`).

### [CONTRACT] MCP server config (IDE)

- \`command\`: string = absolute path to Relay binary.
- \`args\`: array — at minimum \`["mcp-<ide>"]\` (e.g. \`["mcp-cursor"]\`); for WSL-hosted agents with Windows \`relay.exe\`, use \`["mcp-<ide>", "--exe_in_wsl"]\` so attachment paths rewrite to \`/mnt/...\`.
- Tool name: \`relay_interactive_feedback\` (exact; no alias).

### [PARAMS] Tool input

| Parameter              | Type                                           | Required                                         | Semantics / invariant                                                                                                                                        |
| ---------------------- | ---------------------------------------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| \`retell\`             | string                                         | YES                                              | This turn's full assistant reply, verbatim. trim(retell) non-empty; else 400. Content user sees this turn only; not recap or internal reasoning. Over 127.0.0.1 HTTP; no argv/ARG_MAX limit. |
| \`relay_mcp_session_id\` | string                                         | If you have it: YES; else omit or \`""\`           | From previous tool return JSON. Omit or \`""\` = new tab; non-empty = merge into that tab.                                                                     |
| \`commands\`           | array of { name, id, category?, description? } | **No session (new tab)**: **must** include this property every call; array **must list every IDE/host command you can obtain** for Relay slash-completion. Use \`[]\` **only if** the host truly exposes none — **do not** send empty when you could populate. **With session**: optional; if sent, merged into that tab (dedupe by \`id\`). |
| \`skills\`             | array of { name, id, category?, description? } | Same rules as \`commands\`, for **skills** the host can expose. |
| \`title\`              | string                                         | **New tab only (no session); strongly recommended.** ≤60-char short title summarising the chat context (e.g. \`"Fix login page CSS"\`). Relay displays it instead of the default MM-DD HH:mm:ss timestamp. Ignored when session already exists. **Agents should always provide this when creating a new tab** to give tabs meaningful labels. |

### [RETURN] Tool result

- **Normal**: JSON \`{ "relay_mcp_session_id": "<ms>", "human": "<Answer text>", "cmd_skill_count": <number> [, "attachments": [{ "kind": "image"|"file", "path": "..." }] ] }\`; omit \`attachments\` when none. \`path\` is Windows-local from the GUI; if the MCP process was started with \`--exe_in_wsl\` (e.g. \`args\` = \`["mcp-<ide>", "--exe_in_wsl"]\`), MCP may rewrite paths to WSL \`/mnt/...\` before the IDE sees the tool result. \`cmd_skill_count\` = number of \`commands\` + \`skills\` currently stored on that Relay tab (slash-completion list size).
- **Re-list when zero**: If \`cmd_skill_count === 0\` on a return (and not the pause sentinel), the **next** call **must** again include \`commands\` and \`skills\` filled with every item the IDE **can** enumerate — use \`[]\` **only** when the host truly provides none.
- **Also**: \`relay_mcp_session_id\`: ms timestamp. Tab label = agent-provided \`title\` if given, else **MM-DD HH:mm:ss**. \`human\`: Answer (empty on dismiss/timeout).
- **Postcondition**: Store \`relay_mcp_session_id\`; pass it on the **next** call. If \`human\` is non-empty or \`attachments\` is non-empty, treat as user input and reply; **the turn ends only when both are empty** (or attachments absent).
- **Sentinel**: If the **entire** result contains \`<<<RELAY_MCP_PAUSED>>>\`, do not call again until the user resumes in Relay Settings.

### [STATE] Session state (invariant)

- Variable: \`relay_mcp_session_id\` : string | undefined. **Initial**: undefined (or \`""\`).
- **After each successful return** (and not sentinel): \`relay_mcp_session_id := result.relay_mcp_session_id\`.
- **On each call**: if \`relay_mcp_session_id\` is defined and non-empty, pass it. If the **last** result had \`cmd_skill_count === 0\`, the **next** call **must** include \`commands\` and \`skills\` again, populated with everything the IDE can list — \`[]\` only when there is nothing to list; otherwise, with a session, you may omit the two arrays. If you have **no** session yet (new tab), you **must** include **both** arrays and fill them with everything the host can provide; **\`[]\` only when the host truly exposes no items**.

### [CALL_SCHEDULE] When to call (generic)

- Call after the assistant has finalized the **user-visible reply** for this turn.
- Exactly once per assistant turn; this call MUST be the **last** tool call of that turn (no tool calls after it in the same turn).
- On subsequent calls, pass \`relay_mcp_session_id\`; if \`human\` is non-empty or \`attachments\` is non-empty, treat as user input and reply; **turn ends only when both are empty**.

### [UI] Human gate (Answer) (product context only; does not affect tool usage)

Plain \`human\` text; images/files appear in the optional \`attachments\` array on the tool result (legacy marker-in-\`human\` is stripped server-side). Enter → submit; Shift+Enter → newline; ⌘/Ctrl+Enter → submit and close tab. Agents should read both \`human\` and \`attachments\` when present.`;

const HEAD_LOOP_ZH = `## 中文版（严格循环）

- **定位**：严格循环 —— 每轮结束必调一次；传输失败时退避约 60 秒后重试。
- **完成定义**：在未调用 \`relay_interactive_feedback\` 之前，当前助手回合视为**未完成**。
- **自检**：在认为自己「已经回复完用户」之前，必须自问：**本轮是否已调用过 \`relay_interactive_feedback\`？** 若未调用，则**必须立即调用**，否则不得结束本回合。
- **无例外**：无论回复长短、是否出错、是否在解释规则，只要对用户输出了助手回复，本回合结束前都必须调用该工具。
- **失败**：传输失败时约 60 秒后退避再重试。仅父 Agent 调用，子 Agent 不调用。
- **提醒**：若你已写出对用户的回复且尚未调用 \`relay_interactive_feedback\`，则本回合未结束 — 现在就去调用。`;

const HEAD_LOOP_EN = `## English Version (Strict loop)

- **Positioning**: Strict loop — call exactly once at end of every turn; on transport failure, back off ~60s then retry.
- **Definition of "done"**: An assistant turn is **not complete** until \`relay_interactive_feedback\` has been called.
- **Self-check**: Before considering your reply to the user "finished", you MUST ask: **Have I called \`relay_interactive_feedback\` this turn?** If not, you **must call it now**; you may not end the turn without calling it.
- **No exceptions**: Short or long reply, error, or rule explanation — if you produced an assistant reply visible to the user, you must call this tool before ending the turn.
- **Failure**: Back off ~60s then retry. Only the parent agent calls this tool; sub-agents do not.
- **Reminder**: If you have written a reply to the user and have not yet called \`relay_interactive_feedback\`, your turn is not complete — call it now.`;


/** Builds bilingual (中英合本) rule: 一、中文（head + body）--- 二、English（head + body）. */
function buildBilingual(headZh: string, headEn: string): string {
  return `${FRONTMATTER}

${TITLE}

---

${headZh}

---

${BODY_ZH}

---

${headEn}

---

${BODY_EN}`;
}

/** Replace `<ide>` placeholders with the actual IDE CLI id for concrete examples. */
function renderForIde(text: string, ideCliId?: string): string {
  if (!ideCliId) return text;
  return text.replace(/<ide>/g, ideCliId);
}

/** Returns the full rule as a single bilingual string. loop (default) = title+head+body; toolOnly = title+body.
 *  When `ideCliId` is provided (e.g. "cursor", "claude_code"), `<ide>` in the text is replaced. */
export function getRelayRulePromptBilingual(mode: RulePromptMode = "loop", ideCliId?: string): string {
  let result: string;
  switch (mode) {
    case "loop":
      result = buildBilingual(HEAD_LOOP_ZH, HEAD_LOOP_EN);
      break;
    case "toolOnly": {
      result = `${FRONTMATTER}

${TITLE}

---

## 中文版（仅工具说明）

${BODY_ZH}

---

## English Version (Tool only)

${BODY_EN}`;
      break;
    }
    default:
      result = buildBilingual(HEAD_LOOP_ZH, HEAD_LOOP_EN);
      break;
  }
  return renderForIde(result, ideCliId);
}
