/**
 * Shared TypeScript types for the Relay GUI (Tauri frontend).
 */

export type LaunchState = {
  retell: string;
  /** Correlates with MCP HTTP wait; empty for hub preview. */
  request_id: string;
  title: string;
  /** Chat/session title from MCP tool; empty if not passed. */
  session_title: string;
  tab_id: string;
  client_tab_id: string;
  is_preview: boolean;
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
  /** Matches `LaunchState.client_tab_id` for this chat tab. */
  client_tab_id?: string;
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
};

export type SettingsSegment = "setup" | "rulePrompts" | "cache";

export type RelayCacheStats = {
  attachments_bytes: number;
  log_bytes: number;
  data_dir: string;
};

export type DragDropUnlisten = (() => void) | undefined;
