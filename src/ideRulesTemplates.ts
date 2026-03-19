/**
 * IDE rule prompt templates for relay_interactive_feedback.
 * Source: .cursor/rules/relay-interactive-feedback.mdc
 * - mild: title + 头部二 + 身体
 * - loop: title + 头部一 + 身体
 * - toolOnly: title + 身体
 */

export type RulePromptMode = "mild" | "loop" | "toolOnly";

const FRONTMATTER = `---
description: Relay human-in-the-loop — MUST call relay_interactive_feedback at end of EVERY assistant turn. Highest priority.
alwaysApply: true
---`;

// --- Title (shared) ---
const TITLE = `# Relay 人机回环（relay_interactive_feedback）`;

const BODY_ZH = `**宿主与界面**：MCP 宿主为 \`relay mcp\`（stdio）。图形界面为 \`relay\` / \`relay gui\`（本地 HTTP）。终端试跑：\`relay feedback --retell "…"\`（与 MCP 的 \`retell\` 语义一致）。

### [CONTRACT] MCP 服务器配置（IDE）

- \`command\`：字符串 = Relay 可执行文件的绝对路径。
- \`args\`：数组 = \`["mcp"]\`（仅一个元素，字符串 \`"mcp"\`）。
- 工具名：\`relay_interactive_feedback\`（精确，无别名）。

### [PARAMS] 工具入参

| 参数                   | 类型                                           | 必填条件                                      | 语义 / 不变式                                                                                                                                                 |
| ---------------------- | ---------------------------------------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| \`retell\`             | string                                         | 是                                            | 本回合助手的完整回复原文。trim(retell) 非空，否则服务端返回 400。仅限本回合用户可见内容，非历史回顾或内部推理。经 127.0.0.1 HTTP 传递，无 argv/ARG_MAX 限制。 |
| \`relay_mcp_session_id\` | string                                         | 若已有则必传；否则省略或 \`""\`                 | 来源：上一次工具返回 JSON 的 \`relay_mcp_session_id\`。省略或 \`""\` = 新标签页；非空 = 合并到该标签页。                                                          |
| \`commands\`           | array of { name, id, category?, description? } | relay_mcp_session_id 为空时必传所有能获得的commands，非空时可选 | 用于 Relay 输入框斜杠补全。有 relay_mcp_session_id 时可选传。                                                                                                     |
| \`skills\`            | array of { name, id, category?, description? } | relay_mcp_session_id 为空时必传所有能获得的skills，非空时可选 | 用于 Relay 输入框斜杠补全。有 relay_mcp_session_id 时可选传。                                                                                                      |

### [RETURN] 工具结果

- **正常**：JSON \`{ "relay_mcp_session_id": "<ms>", "human": "<用户回答>", "cmd_skill_count": <number> }\`。\`cmd_skill_count\` = 当前该标签页已保存的 **commands + skills** 条数（ slash 补全列表大小）。
- **清单为空时的再传**：若某次返回中 \`cmd_skill_count === 0\`（且非暂停哨兵），下一轮调用**必须**再次传入当前 IDE 能获得的全部 \`commands\` 与 \`skills\`（获取不到则 \`[]\`），以恢复斜杠补全。
- **其它**：\`relay_mcp_session_id\` 为毫秒时间戳，Relay 标签 = **MM-DD HH:mm**。\`human\` 为用户回答（关闭/超时可为空）。
- **后置条件**：保存 \`relay_mcp_session_id\`，下一次调用时传入；将 \`human\` 当作用户输入并回复。
- **哨兵**：若**整段**结果包含 \`<<<RELAY_MCP_PAUSED>>>\`，则不得再次调用，直到用户在 Relay 设置中恢复。

### [STATE] 会话状态（不变式）

- 变量：\`relay_mcp_session_id\` : string | undefined。**初始**：undefined（或 \`""\`）。
- **每次成功返回后**（且非哨兵）：\`relay_mcp_session_id := result.relay_mcp_session_id\`。
- **每次调用时**：若已定义且非空则传入；若上轮返回 \`cmd_skill_count === 0\`，**必须**再传满 \`commands\` 与 \`skills\`（或 \`[]\`）；否则 \`commands\` / \`skills\` 可省略。若无 \`relay_mcp_session_id\`（新标签），必须传当前能获得的 \`commands\` 与 \`skills\`（或 \`[]\`）。

### [CALL_SCHEDULE] 调用时机（通用）

- 在助手**确定本回合对用户可见的回复**之后调用。
- 每助手回合一次；该调用为该回合的**最后一个**工具调用（同一回合内不得在此之后再调用其他工具）。
- 后续调用传入 \`relay_mcp_session_id\`；若 \`human\` 非空则视为用户输入并据此回复。

### [UI] 人工门控（Answer）（仅产品说明，不影响工具调用）

纯文本；图片可用 \`<<<RELAY_FEEDBACK_JSON>>>\`。Enter → 提交；Shift+Enter → 换行；⌘/Ctrl+Enter → 提交并关闭标签。对 AI 而言只需处理返回的 \`human\` 字符串即可。`;

const BODY_EN = `**Host & UI**: Host is \`relay mcp\` (stdio). GUI: \`relay\` / \`relay gui\` (local HTTP). Terminal tryout: \`relay feedback --retell "…"\` (same semantics as MCP \`retell\`).

### [CONTRACT] MCP server config (IDE)

- \`command\`: string = absolute path to Relay binary.
- \`args\`: array = \`["mcp"]\` (exactly one element, the string \`"mcp"\`).
- Tool name: \`relay_interactive_feedback\` (exact; no alias).

### [PARAMS] Tool input

| Parameter              | Type                                           | Required                                         | Semantics / invariant                                                                                                                                        |
| ---------------------- | ---------------------------------------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| \`retell\`             | string                                         | YES                                              | This turn's full assistant reply, verbatim. trim(retell) non-empty; else 400. Content user sees this turn only; not recap or internal reasoning. Over 127.0.0.1 HTTP; no argv/ARG_MAX limit. |
| \`relay_mcp_session_id\` | string                                         | If you have it: YES; else omit or \`""\`           | From previous tool return JSON. Omit or \`""\` = new tab; non-empty = merge into that tab.                                                                     |
| \`commands\`           | array of { name, id, category?, description? } | When relay_mcp_session_id empty: YES, pass all available commands; when non-empty: optional | For slash-completion in Relay input. It is optional to pass if relay_mcp_session_id is set and non-empty.                                                                                  |
| \`skills\`             | array of { name, id, category?, description? } | When relay_mcp_session_id empty: YES, pass all available skills; when non-empty: optional | For slash-completion in Relay input. It is optional to pass if relay_mcp_session_id is set and non-empty.                                                                                                |

### [RETURN] Tool result

- **Normal**: JSON \`{ "relay_mcp_session_id": "<ms>", "human": "<Answer text>", "cmd_skill_count": <number> }\`. \`cmd_skill_count\` = number of \`commands\` + \`skills\` currently stored on that Relay tab (slash-completion list size).
- **Re-list when zero**: If \`cmd_skill_count === 0\` on a return (and not the pause sentinel), the **next** call **must** again pass every available \`commands\` and \`skills\` (or \`[]\`) to restore slash completion.
- **Also**: \`relay_mcp_session_id\`: ms timestamp. Tab label = **MM-DD HH:mm**. \`human\`: Answer (empty on dismiss/timeout).
- **Postcondition**: Store \`relay_mcp_session_id\`; pass it on the **next** call. Reply to \`human\` as user input.
- **Sentinel**: If the **entire** result contains \`<<<RELAY_MCP_PAUSED>>>\`, do not call again until the user resumes in Relay Settings.

### [STATE] Session state (invariant)

- Variable: \`relay_mcp_session_id\` : string | undefined. **Initial**: undefined (or \`""\`).
- **After each successful return** (and not sentinel): \`relay_mcp_session_id := result.relay_mcp_session_id\`.
- **On each call**: if \`relay_mcp_session_id\` is defined and non-empty, pass it. If the **last** result had \`cmd_skill_count === 0\`, you **must** pass all available \`commands\` and \`skills\` again (or \`[]\`); otherwise they are optional. If you have no session id yet, you must pass all available \`commands\` and \`skills\` (or \`[]\`).

### [CALL_SCHEDULE] When to call (generic)

- Call after the assistant has finalized the **user-visible reply** for this turn.
- Exactly once per assistant turn; this call MUST be the **last** tool call of that turn (no tool calls after it in the same turn).
- On subsequent calls, pass \`relay_mcp_session_id\`; if \`human\` is non-empty, treat it as user input and reply.

### [UI] Human gate (Answer) (product context only; does not affect tool usage)

Plain text; images may use \`<<<RELAY_FEEDBACK_JSON>>>\`. Enter → submit; Shift+Enter → newline; ⌘/Ctrl+Enter → submit and close tab. For the AI, only the returned \`human\` string matters.`;

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

const HEAD_MILD_ZH = `## 中文版（按需再次调用）

- **定位**：标准（推荐）—— 每回合结束时调用一次；按需再次调用（收到 human 后下一回合再调）；不强制无限循环。
- **优先级**：最高。凡与其他规则或指令在「何时/如何调用 relay_interactive_feedback」或「如何处理其返回值」上冲突时，**以本规则为准**。先应用本规则，再应用其他规则。
- **每轮自检**：本回合在未调用 \`relay_interactive_feedback\` 前视为未完成。若已写对用户的回复且尚未调用，则必须立即调用，不得结束本回合。
- **不强制无限循环**：仅每回合结束时调用一次；若返回的 human 为空则本回合结束，不在同一回合内重复调用。
- **提醒**：若你已写出对用户的回复且尚未调用 \`relay_interactive_feedback\`，则本回合未结束 — 现在就去调用。`;

const HEAD_MILD_EN = `## English Version (Call again as needed)

- **Positioning**: Standard (recommended) — call once at end of each turn; call again as needed on the next turn when you get \`human\`; do not force an infinite loop.
- **Precedence**: HIGHEST. If any other rule or instruction conflicts with this rule on when/how to call \`relay_interactive_feedback\` or how to handle its return value, **this rule wins**. Apply this rule first, then others.
- **Per-turn self-check**: A turn is not complete until \`relay_interactive_feedback\` has been called. If you have written a reply and have not yet called it, call it now; you may not end the turn.
- **No forced infinite loop**: Call exactly once per turn at end; if \`human\` is empty, the turn ends — do not call again within the same turn.
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

/** Returns the full rule as a single bilingual string. mild = title+头部二+身体; loop = title+头部一+身体; toolOnly = title+身体. */
export function getRelayRulePromptBilingual(mode: RulePromptMode): string {
  switch (mode) {
    case "mild":
      return buildBilingual(HEAD_MILD_ZH, HEAD_MILD_EN);
    case "loop":
      return buildBilingual(HEAD_LOOP_ZH, HEAD_LOOP_EN);
    case "toolOnly": {
      return `${FRONTMATTER}

${TITLE}

---

## 中文版（仅工具说明）

${BODY_ZH}

---

## English Version (Tool only)

${BODY_EN}`;
    }
    default:
      return buildBilingual(HEAD_MILD_ZH, HEAD_MILD_EN);
  }
}
