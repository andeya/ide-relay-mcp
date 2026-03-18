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
**Composer keys:** **Enter** → submit · **⌘/Ctrl+Enter** → submit and close tab · **Shift+Enter** → newline.
**Pause:** If the tool result contains \`<<<RELAY_MCP_PAUSED>>>\`, the user paused Relay in Settings — **do not call** \`relay_interactive_feedback\` again until they resume.`;

const RELAY_WORKFLOW_ZH = `**人侧（Answer）：** 纯文本；附图时按约定可含 \`<<<RELAY_FEEDBACK_JSON>>>\` 等。
**快捷键：** **Enter** → 提交 · **⌘/Ctrl+Enter** → 提交并关标签页 · **Shift+Enter** → 换行。
**暂停：** 若工具返回含 \`<<<RELAY_MCP_PAUSED>>>\`，表示用户在 Relay 设置中已暂停 MCP — **不得再调用** \`relay_interactive_feedback\`，直至用户恢复。`;

const SESSION_FIELDS_EN = `**\`session_title\`:** Optional fixed window/tab label. If omitted, each new tab gets **\`Chat N\`** with **N** from a **global counter** (1, 2, 3… per Relay process, no reuse after a tab closes). Merging via the same \`client_tab_id\` **keeps** the title when \`session_title\` is still omitted.
**\`client_tab_id\`:** Strongly recommended — **stable** id per IDE chat tab so Relay merges into one tab and **newest round stays at the bottom**.`;

const SESSION_FIELDS_ZH = `**\`session_title\`：** 可选固定窗口/标签名。省略时新标签为 **\`Chat N\`**，**N** 为**进程内全局递增序号**（从 1 起，关标签也不回收）；同一 \`client_tab_id\` 且仍省略时**保留**原标题。
**\`client_tab_id\`：** 强烈建议 — **每个 IDE 会话标签固定** id，合并为单标签且**最新一轮在列表底部**。`;

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
