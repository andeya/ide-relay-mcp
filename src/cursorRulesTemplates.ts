/**
 * Agent rule prompts for relay_interactive_feedback.
 * - English blocks: intended for model-facing IDE rules (strict, non-redundant).
 * - Chinese blocks: same contract, for human readers; keep 1:1 semantics with EN.
 */

export type RulePromptMode = "mild" | "loop" | "toolOnly";

/** @deprecated Reserved for API compatibility; prompts no longer inject dynamic retell lines. */
export type RetellInlineHintLines = { line_en: string; line_zh: string };

/**
 * Single definition of `retell` — semantics + transport in one place (no duplicate bullets elsewhere).
 */
const RETELL_SPEC_EN = `#### \`retell\` (required, non-empty)
- **Semantics:** The **assistant content the user sees for this turn**, **verbatim** — i.e. the substantive reply you are presenting now, **not** a compressed recap of earlier turns or internal reasoning.
- **Transport:** \`relay mcp\` forwards the full string to the Relay desktop UI over **127.0.0.1 HTTP**. **No** shell / \`argv\` / \`ARG_MAX\` limitation applies to length.`;

const RETELL_SPEC_ZH = `#### \`retell\`（必填、非空）
- **语义：** **本轮**对用户展示的助手正文，**逐字一致**；是**当前这一步**的实质回复，**不是**对更早轮次的摘要，也**不是**内心推理过程。
- **传递：** \`relay mcp\` 经 **127.0.0.1 HTTP** 将完整字符串送达 Relay 桌面端；**不受** shell 参数长度或 \`ARG_MAX\` 限制。`;

/** Human-side composer behavior only — does not repeat `retell`. */
const RELAY_WORKFLOW_EN = `**Human gate (your Answer):** Plain text; images may use the \`<<<RELAY_FEEDBACK_JSON>>>\` attachment convention when applicable.
**Composer keys:** **Enter** → submit (never newline) · **Shift+Enter** → newline · **⌘/Ctrl+Enter** → submit and close tab.
**Pause:** If the tool result contains \`<<<RELAY_MCP_PAUSED>>>\`, the user paused Relay in Settings — **do not call** \`relay_interactive_feedback\` again until they resume.`;

const RELAY_WORKFLOW_ZH = `**人侧（Answer）：** 纯文本；附图时按约定可含 \`<<<RELAY_FEEDBACK_JSON>>>\` 等。
**快捷键：** **Enter** → 提交（不换行）· **Shift+Enter** → 换行 · **⌘/Ctrl+Enter** → 提交并关标签页。
**暂停：** 若工具返回含 \`<<<RELAY_MCP_PAUSED>>>\`，表示用户在 Relay 设置中已暂停 MCP — **不得再调用** \`relay_interactive_feedback\`，直至用户恢复。`;

const SESSION_FIELDS_EN = `**\`client_tab_id\` (required):** Stable merge key for **this** Composer/chat thread — **reuse verbatim every call** in the same thread:
1. **Workspace root path** (from user_info / workspace), normalized (trim, no trailing slash).
2. **First user message** in this thread (earliest user turn).
3. Concatenate: \`{workspace_root}\\n{first_user_message}\` (newline). If the first message is very long, use only the **first 500 characters** (same cut every time in that thread).

**Relay window title:** The GUI assigns **Chat 1**, **Chat 2**, … — **global incrementing** per Relay process. The **first time** a \`client_tab_id\` appears it gets the next number; **same id** later (or after closing that tab) **reuses** the same **Chat N**. **Omit \`session_title\`** (ignored for labels). **Caveat:** Same workspace + identical first message → same id → one Relay tab; vary the opening line to split. **docs/CLIENT_TAB_ID.md**.`;

const SESSION_FIELDS_ZH = `**\`client_tab_id\`（必填）：** 本聊天线程的**稳定合并键**，**每轮原样传入**：
1. **工作区根路径**（用户信息里的 workspace），规范化。
2. **本线程首条用户消息**。
3. 拼接：\`{根路径}\\n{首条消息}\`；首条过长则固定取**前 500 字符**。

**Relay 窗口标题：** 由 GUI **全局自增**分配 **Chat 1**、**Chat 2**…… 某个 \`client_tab_id\` **首次出现**时占用下一个序号；**同一 id** 再次请求（含关标签后再来）仍显示**同一 Chat N**。**不必传 \`session_title\`**（界面不用长标题）。**注意：** 同仓库且首条完全相同会共用一个 Relay 标签。详见 **docs/CLIENT_TAB_ID.md**。`;

const PROMPTS_EN: Record<RulePromptMode, string> = {
  mild: `### Relay human-in-the-loop (recommended)

**MCP:** \`command\` → \`relay\`, \`args\` → \`["mcp"]\`. Tool: \`relay_interactive_feedback\`.

${RELAY_WORKFLOW_EN}

${RETELL_SPEC_EN}

${SESSION_FIELDS_EN}

**Call timing:** **Once** per assistant turn, **after** the user-visible reply is finalized — typically the **last** tool call in that turn.

**After non-empty Answer:** Honor it before further assistant output.`,

  loop: `### Relay human-in-the-loop (strict loop)

**MCP:** \`relay\` + \`args: ["mcp"]\`. Tool: \`relay_interactive_feedback\`.

${RELAY_WORKFLOW_EN}

${RETELL_SPEC_EN}

${SESSION_FIELDS_EN}

1. **Every** assistant turn **must** end with this tool (after the user-visible message).
2. After **each** tool result, call again until the session aborts or the host stops the agent.
3. On transport failure: backoff ~60s, retry.
4. Sub-agents: parent owns this tool.

**Risk:** Runaway loop until manual stop.`,

  toolOnly: `### Tool: \`relay_interactive_feedback\`

**Host:** \`relay mcp\` (stdio). **GUI:** \`relay\` / \`relay gui\` (local HTTP to MCP).

${RETELL_SPEC_EN}

${SESSION_FIELDS_EN}

---

${RELAY_WORKFLOW_EN}

**Returns:** Human **Answer** (text; optional \`<<<RELAY_FEEDBACK_JSON>>>\`). **Terminal tryout:** \`relay feedback --retell "…"\`.`,
};

const PROMPTS_ZH: Record<RulePromptMode, string> = {
  mild: `### Relay 人在回路（推荐）

**MCP：** \`command\` → \`relay\`，\`args\` → \`["mcp"]\`，工具 \`relay_interactive_feedback\`。

${RELAY_WORKFLOW_ZH}

${RETELL_SPEC_ZH}

${SESSION_FIELDS_ZH}

**调用时机：** 每轮助手对用户可见回复**定稿之后**调用 **一次**，一般为该轮**最后**一个工具调用。

**若非空 Answer：** 须先落实再继续输出。`,

  loop: `### Relay 人在回路（严格循环）

**MCP：** \`relay\` + \`args: ["mcp"]\`，工具 \`relay_interactive_feedback\`。

${RELAY_WORKFLOW_ZH}

${RETELL_SPEC_ZH}

${SESSION_FIELDS_ZH}

1. 每轮助手输出 **必须** 以本工具收尾（在对用户可见消息之后）。
2. 每次工具返回后 **须再调**，直至会话中止或宿主停止。
3. 传输失败：约 60s 退避后重试。
4. 子 Agent：由父级负责调用。

**风险：** 可能循环至人工停止。`,

  toolOnly: `### 工具：\`relay_interactive_feedback\`

**宿主：** \`relay mcp\`（stdio）。**界面：** \`relay\` / \`relay gui\`（与 MCP 经本机 HTTP 通信）。

${RETELL_SPEC_ZH}

${SESSION_FIELDS_ZH}

---

${RELAY_WORKFLOW_ZH}

**返回值：** 人提交的 **Answer**（文本；可选 \`<<<RELAY_FEEDBACK_JSON>>>\`）。**终端试用：** \`relay feedback --retell "…"\`。`,
};

export function applyRetellInlinePlaceholders(
  text: string,
  _hint?: RetellInlineHintLines | null,
): string {
  return text;
}

export function getRelayRulePromptEn(
  mode: RulePromptMode,
  _hint?: RetellInlineHintLines | null,
): string {
  return PROMPTS_EN[mode];
}

export function getRelayRulePromptZh(
  mode: RulePromptMode,
  _hint?: RetellInlineHintLines | null,
): string {
  return PROMPTS_ZH[mode];
}

/** English only — clipboard / IDE. */
export function getRelayRulePrompt(
  mode: RulePromptMode,
  hint?: RetellInlineHintLines | null,
): string {
  return getRelayRulePromptEn(mode, hint);
}
