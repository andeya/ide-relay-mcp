/**
 * Shared TypeScript types for the Relay GUI (Tauri frontend).
 */

export type CommandItem = {
  name: string;
  id: string;
  category?: string;
  description?: string;
};

/** Image/file refs returned alongside plain `human` in the tool / wait JSON. */
export type QaAttachmentRef = {
  kind: string;
  path: string;
};

export type LaunchState = {
  retell: string;
  /** Correlates with MCP HTTP wait; empty for hub preview. */
  request_id: string;
  /** Tab strip label: MM-DD HH:mm:ss from session_id. */
  title: string;
  tab_id: string;
  relay_mcp_session_id: string;
  is_preview: boolean;
  /** Commands for slash-completion in input; bound to this session. */
  commands?: CommandItem[];
  /** Skills (same shape as commands) for slash-completion in input. */
  skills?: CommandItem[];
};

/** Matches backend `ControlStatus` JSON serialization (snake_case). */
export type ControlStatus =
  | "active"
  | "idle"
  | "timed_out"
  | "cancelled"
  | null;

export type QaRound = {
  retell: string;
  reply: string;
  skipped?: boolean;
  /** HTTP wait ended by idle orphan (empty human to IDE). */
  idle_timeout?: boolean;
  submitted?: boolean;
  tab_id: string;
  relay_mcp_session_id?: string;
  /** Structured attachments for this Answer; prefer over `reply` marker parsing. */
  reply_attachments?: QaAttachmentRef[];
  /** Wall-clock when the AI retell arrived (YYYY-MM-DD HH:MM:SS local). */
  retell_at?: string;
  /** Wall-clock when the user submitted their reply (YYYY-MM-DD HH:MM:SS local). */
  reply_at?: string;
};

export type FeedbackTabsState = {
  tabs: LaunchState[];
  active_tab_id: string;
  qa_rounds?: QaRound[];
};

export type PathEnvStatus = {
  configured: boolean;
  bin_dir: string;
  platform: string;
  /** When not configured, reason for the user to fix manually. */
  reason?: string;
};

export type SettingsSegment = "setup" | "rulePrompts" | "app" | "usage";

export type CursorUsagePlanBlock = {
  enabled: boolean;
  used: number;
  limit: number;
  remaining: number;
};

export type CursorUsageOnDemandBlock = {
  enabled: boolean;
  used: number;
  limit: number;
  remaining: number;
};

export type CursorUsageSummary = {
  billingCycleStart: string;
  billingCycleEnd: string;
  membershipType: string;
  isYearlyPlan: boolean;
  onDemandAutoEnabled: boolean;
  individualUsage: {
    plan: CursorUsagePlanBlock;
    onDemand: CursorUsageOnDemandBlock;
  };
  teamUsage?: {
    onDemand: CursorUsageOnDemandBlock;
  };
};

export type CursorTokenUsage = {
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens?: number;
  cacheWriteTokens?: number;
  totalCents: number;
};

export type CursorUsageEvent = {
  timestamp: string;
  model: string;
  kind: string;
  requestsCosts?: number;
  chargedCents: number;
  isChargeable: boolean;
  tokenUsage?: CursorTokenUsage;
};

export type CursorUsageEventsPage = {
  totalUsageEventsCount: number;
  usageEventsDisplay: CursorUsageEvent[];
};

export type CursorUsageSettings = {
  /** Legacy field from older Relay builds; ignored by the UI. */
  refresh_on_new_session?: boolean;
  refresh_interval_minutes: number;
};

// ---------------------------------------------------------------------------
// IDE binding
// ---------------------------------------------------------------------------

export type IdeKind = "cursor" | "claude_code" | "windsurf" | "other";

export type IdeCapabilities = {
  supportsMcpInject: boolean;
  supportsRulePrompt: boolean;
  supportsUsage: boolean;
};

export type RelayCacheStats = {
  attachments_bytes: number;
  log_bytes: number;
  /** `qa_archive/*.jsonl`; cleared with "clear logs". */
  qa_archive_bytes: number;
  data_dir: string;
};

export type DragDropUnlisten = (() => void) | undefined;
