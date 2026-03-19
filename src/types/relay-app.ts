/**
 * Shared TypeScript types for the Relay GUI (Tauri frontend).
 */

export type CommandItem = {
  name: string;
  id: string;
  category?: string;
  description?: string;
};

/** MCP / HTTP wait payload for one relay_interactive_feedback round. */
export type RelayFeedbackToolResult = {
  relay_mcp_session_id: string;
  human: string;
  cmd_skill_count: number;
};

export type LaunchState = {
  retell: string;
  /** Correlates with MCP HTTP wait; empty for hub preview. */
  request_id: string;
  /** Tab strip label: MM-DD HH:mm from session_id. */
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
  submitted?: boolean;
  tab_id: string;
  relay_mcp_session_id?: string;
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

export type McpStatus = {
  installed: boolean;
  reason?: string;
};

export type SettingsSegment = "setup" | "rulePrompts" | "cache";

export type RelayCacheStats = {
  attachments_bytes: number;
  log_bytes: number;
  data_dir: string;
};

export type DragDropUnlisten = (() => void) | undefined;
