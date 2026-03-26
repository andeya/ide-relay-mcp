export default {
  settingsTitle: "Settings",
  appAuthorLine: "Author: andeya",
  appAuthorEmailAria: "Email andeyalee@outlook.com",
  settingsBack: "Back",
  settingsCheckStatus: "Refresh",
  settingsChecking: "Refreshing…",
  settingsRefreshOk: "Status refreshed.",
  settingsRefreshWarn: "Refreshed, but MCP config could not be read.",
  settingsRefreshFail: "Refresh failed:",
  ariaOpenSettings: "Open settings",
  segSetup: "Environment & MCP",
  segRulePrompts: "Rule prompts",
  segCache: "Cache",

  settingsLangAria: "Interface language",

  rulePromptsTitle: "Rule prompts",
  rulePromptsLead:
    "Bilingual (中英合本) rule for relay_interactive_feedback. Copy pastes the **whole** block; paste it **at the top** of IDE rules or use a rule file that loads first (e.g. `00-relay-mcp.mdc`). Tool: relay_interactive_feedback — wire MCP under Environment & MCP.",
  rulePromptsSectionIde: "How to configure in {ide}",
  rulePromptsModeLoop: "Default (strict loop)",
  rulePromptsModeLoopDesc:
    "Every turn must end with a call; back off and retry on transport failure. Ensures the tool is never skipped.",
  rulePromptsModeTool: "Tool spec only",
  rulePromptsModeToolDesc:
    "Tool contract only: no call policy. Merge into your existing rules as a pure tool spec.",
  rulePromptsCopy: "Paste in IDE",
  rulePromptsViewMd: "Preview",
  rulePromptsViewSource: "Source",
  rulePromptsToggleEnAria: "Rule prompt display (Markdown or source)",
  rulePromptsLabelBilingual: "Rule (中文 + English)",
  rulePromptsCopied: "Copied.",
  rulePromptsCopyErr: "Copy failed.",
  rulePromptsInstallCursor: "Install to {ide}",
  rulePromptsUpdateCursor: "Update in {ide}",
  rulePromptsRemoveCursor: "Remove from {ide}",
  rulePromptsInstallOk: "Rule installed to {ide}.",
  rulePromptsUpdateOk: "Rule updated in {ide}.",
  rulePromptsRemoveOk: "Rule removed from {ide}.",
  rulePromptsInstallErr: "Install failed:",
  rulePromptsRemoveErr: "Remove failed:",
  rulePromptsInstalledBadge: "Installed",

  rulePromptsIdeGuideCursor: `**How to use in Cursor**

- **Rules**: Settings \u2192 **Rules** \u2192 User or Project \u2192 paste the rule block **at the top** (or use \`.cursor/rules/00-relay-mcp.mdc\` so it loads first). The rule declares highest priority for relay_interactive_feedback.
- **MCP file** \`{mcpPath}\`
- \`command\` = local \`relay\` (absolute path); \`args\` = \`["mcp-cursor"]\`; suggest \`autoApprove\`: \`relay_interactive_feedback\`
- **WSL**: If the IDE/agent runs **inside WSL** and \`command\` points at **Windows** \`relay.exe\`, set \`args\` to \`["mcp-cursor", "--exe_in_wsl"]\` so attachment paths become \`/mnt/...\`.`,
  rulePromptsIdeGuideWindsurf: `**How to use in Windsurf**

- **Instructions**: paste the rule block in Agent / MCP custom text (UI varies by version)
- **MCP file** \`{mcpPath}\`
- \`command\` = local \`relay\` (absolute path); \`args\` = \`["mcp-windsurf"]\`
- **WSL**: add \`"--exe_in_wsl"\` to \`args\` if running inside WSL with Windows relay.exe.`,
  rulePromptsIdeGuideClaude: `**How to use in Claude Code / Claude Desktop**

- **Custom instructions**: paste the rule block
- MCP \`command\` = **full path** to relay; \`args\` = \`["mcp-claude_code"]\`
- **WSL**: add \`"--exe_in_wsl"\` to \`args\` if running inside WSL with Windows relay.exe. Approve \`relay_interactive_feedback\` if prompted.`,
  rulePromptsIdeGuideOther: `**How to use in other IDEs**

Any MCP + rules-capable client: paste the rule block and register \`relay_interactive_feedback\`.
- MCP: \`command\` = \`relay\` (full path recommended); \`args\` = \`["mcp-other"]\`
- **WSL**: add \`"--exe_in_wsl"\` to \`args\` if running inside WSL with Windows relay.exe.`,

  setupTitle: "This machine",
  setupLead:
    "Not ready: Install all (PATH + {ide} MCP). When ready: copy MCP JSON in the green card below; Uninstall all to revert.",
  setupAllReadyLead:
    "PATH and {ide} MCP are set. JSON copy lives in the green card; Uninstall all to revert.",
  setupStatus: "Configuration detail",
  setupChipPath: "Terminal PATH",
  setupPathExplain: "Folder containing the relay binary is on your user PATH",
  setupMcpExplain: "relay-mcp is present in {ide} MCP config",
  setupRuleExplain: "Rule prompt installed in {ide} rules directory",
  setupConfigFile: "Config file",
  setupBinDir: "Binary folder",
  setupOn: "Ready",
  setupOff: "Not set",
  setupBtnPublicInstall: "Install PATH",
  setupBtnPublicUninstall: "Remove PATH",
  setupBtnIdeInstall: "Install {ide} config",
  setupBtnIdeUninstall: "Remove {ide} config",
  setupSectionPublic: "Public config (PATH)",
  setupSectionIde: "IDE config (MCP + Rule)",
  setupToolParamsTitle: "Human-in-the-loop & MCP on this machine",
  setupToolParamsLead:
    "Green card: merged MCP JSON for {ide} (ready to copy). Default `args` is `[\"mcp-{ideCliId}\"]`. For **WSL-hosted IDE + Windows relay.exe**, change `args` to `[\"mcp-{ideCliId}\", \"--exe_in_wsl\"]` before saving/merging.",
  mcpCopy: "Copy MCP JSON",
  mcpCopied: "Copied to clipboard.",
  mcpCopyErr: "Copy failed.",
  setupAdvanced:
    "Advanced (PATH, JSON preview…)",
  setupAdvPathTitle: "PATH only",
  setupAdvPathLead:
    "If full install skipped PATH (e.g. relay binary not found beside the app), add it here. Open a new terminal or fish session afterward.",
  pathEnvBtn: "Add to user PATH",
  pathEnvBusy: "Applying…",
  pathEnvDoneWin: "Done. Open a new Command Prompt or PowerShell.",
  pathEnvDoneMac: "Done. New terminal, or: source ~/.zshrc",
  pathEnvDoneLinux: "Done. New terminal, or source ~/.bashrc / ~/.profile",
  pathEnvDoneOther: "Done. Open a new terminal.",
  pathEnvAlready: "Already on user PATH.",
  pathEnvErrPrefix: "Could not update PATH:",
  setupIdeGuide: "IDE config guide",
  publicInstallOk: "PATH configured. Open a new terminal for it to take effect.",
  publicUninstallOk: "PATH configuration removed.",
  ideInstallOk: "MCP config and rule prompt installed for {ide}. Please restart {ide}.",
  ideUninstallOk: "MCP config and rule prompt removed from {ide}.",
  publicUninstallConfirm: "This removes Relay from your shell PATH.",
  ideUninstallConfirm:
    "This removes relay-mcp config and rule prompt from {ide}.",
  setupUninstallConfirmBtn: "Uninstall",
  setupUninstallCancel: "Cancel",
  mcpFullErr: "Failed:",
  windowDockAria: "Window horizontal position on screen",
  windowDockLeft: "Dock left",
  windowDockCenter: "Center horizontally",
  windowDockRight: "Dock right",
  dockBtnLeft: "◀",
  dockBtnCenter: "●",
  dockBtnRight: "▶",
  dockEdgeHideAria: "Edge tuck",
  dockEdgeHideTitle:
    "When docked left or right: after the pointer leaves the window, tuck to the screen edge (a thin strip remains). Move the pointer onto that strip to expand — like classic QQ panels. The window also expands when focused, and when a new MCP message arrives (Relay is raised to the front). If tuck/expand feels stuck: ⌘⇧E (Ctrl+Shift+E on Windows/Linux) forces expand.",
  windowAlwaysOnTopAria: "Always on top",
  windowAlwaysOnTopTitle: "Always on top (keep window above others)",

  releaseBadgeAria: "Open Relay repository on GitHub",
  releaseBadgeUpdate: "v{latest} · New",
  releaseBadgeCurrent: "v{current}",

  appTitle: "Relay MCP",
  statusAwaiting: "ME turn",
  statusHubWaiting: "Hub · no IDE",
  ideBlockingHint:
    "The IDE is waiting for your Answer. After you submit, the agent continues in the same turn.",
  mcpPauseTitle: "Pause Relay MCP",
  mcpPauseHint:
    "When on: IDE calls to this MCP **do not open Relay** and return a fixed message immediately. Add a Cursor rule: if the tool result contains <<<RELAY_MCP_PAUSED>>>, **stop calling** relay_interactive_feedback until you turn this off here.",
  mcpPauseSwitch: "Pause MCP",
  mcpPauseSwitchTitle: "Pause human-in-the-loop — IDE calls won’t open Relay",
  mcpPauseStatusOn: "Status: paused",
  mcpPauseStatusOff: "Status: active",
  mcpPauseUpdateErr: "Could not update pause. Check permissions and try again.",

  statusIdle: "AI turn",
  statusTimedOut: "Timed out",
  statusCancelled: "Cancelled",
  mainHintPreview:
    "When a tab is waiting for your reply, type here. Enter to send · ⌘/Ctrl+Enter to send and close the tab.",
  tabStripAria: "Feedback tabs",
  tabCloseAria: "Close this tab",
  tabCloseTitle: "Close tab (shown when hovering this tab)",
  qaHistoryTitle: "Thread",
  qaAssistantTurn: "AI",
  /** Thread bubble label for the user side (composer still says “Answer”). */
  qaUserTurnMe: "ME",
  qaNoRetellYet: "No AI message for this turn yet.",
  /** a11y: composer region after removing visible “Answer” heading */
  composerAriaRegion: "Your reply",
  composerHint:
    "Enter submits · Shift+Enter: new line · ⌘/Ctrl+Enter: submit & close tab · paste or attach images/files",
  composerHintDraft:
    "Draft while waiting; when a request arrives, Enter submits · while waiting, only Shift+Enter adds a line · paste or attach images/files",
  composerImageAria: "Image attachments preview",
  composerAttach: "Attach images or files",
  composerThumbRemove: "Remove image",
  composerFileDropAria: "Pending file attachment",
  composerFileDropRemove: "Remove file",
  composerFilePathNotAFile: "Not a file (folders are not supported)",
  composerFilePathTooLarge: "File too large (max 50MB)",
  composerFileReadFailed: "Could not read file",
  composerSubmitBlockedFileError:
    "Fix or remove attachments marked in red before submitting.",
  composerImageZoomTitle: "Click to enlarge",
  imageLightboxClose: "Close preview",
  composerSubmitIconTitle:
    "Submit (Enter). Submit and close tab: ⌘ or Ctrl + Enter.",
  composerSubmitIconAria: "Submit answer",
  composerSubmitting: "Submitting…",
  composerSubmitDisabledPreview:
    "Open when an MCP request is active — then tap to submit (Enter).",
  composerSubmitDisabledIdle:
    "Submit is available once an AI request is active.",
  qaSkipped: "Closed with no reply",
  qaEmptySubmit: "Submitted with no text",
  placeholder: "Write your reply…",
  slashListboxAria: "Commands",
  slashNoMatch: "No matching command or skill",
  slashNoCommandsForSession: "No commands or skills for this session (IDE did not provide any)",
  slashDropdownHint: "↑↓ Navigate · Enter or Tab to insert",
  slashCategoryAgentSkill: "Skill",
  noteExpired:
    "This request has already timed out or been cancelled. Your text can be reviewed locally, but it can no longer be submitted.",
  loading: "Loading…",
  noLaunch: "No launch data available.",

  ideHintCursor:
    "Cursor — Full install writes:\n{cursorPath}\nOr merge JSON manually, then restart. Settings → MCP. WSL + Windows relay: include \"--exe_in_wsl\" in args.",
  ideHintVscode:
    "VS Code — MCP UI varies; command = full path to relay; args at least [\"mcp\"]; WSL + Windows relay: [\"mcp\", \"--exe_in_wsl\"].",
  ideHintWindsurf:
    "Windsurf — Full install writes:\n{windsurfPath}\nManual MCP: match the green JSON; WSL + Windows relay: add \"--exe_in_wsl\" to args.",
  ideHintClaude:
    "Claude Desktop — command = full path to relay; args at least [\"mcp\"]; WSL + Windows relay: [\"mcp\", \"--exe_in_wsl\"]. Approve relay_interactive_feedback if prompted.",

  cacheTitle: "Storage & cache",
  cacheSubtitle:
    "Attachments and logs stay on this device. Industry norm: optional age-based cleanup for attachments only; logs stay manual.",
  cacheLead:
    "Relay data directory (attachments + feedback log). Clearing attachments breaks thumbnails for past Answers in history until you send new ones.",
  cacheDataDir: "Data folder",
  cacheOpenFolder: "Open folder",
  cacheOpenFolderErr: "Could not open folder.",
  cacheLoading: "Calculating…",
  cacheLoadErr: "Could not read cache size.",
  cacheTotal: "Total (attachments + log & archive)",
  cacheAttachments: "Attachments cache",
  cacheLogs: "Log & session archive",
  cacheRefresh: "Refresh",
  cacheClearAll: "Clear all cache",
  cacheClearAttach: "Clear attachments only",
  cacheClearLogs: "Clear log only",
  cacheBusy: "Working…",
  cacheClearedOk: "Cache cleared.",
  cacheClearErr: "Could not clear cache.",
  cacheConfirmClearAll:
    "Delete all saved feedback attachments, empty feedback_log.txt, and remove qa_archive/*.jsonl?",
  cacheConfirmClearAttach:
    "Delete all files in the attachments folder? Past Answer thumbnails in history will break.",
  cacheConfirmClearLogs:
    "Empty feedback_log.txt and delete qa_archive/*.jsonl? On-disk chat replay for past sessions will be reset.",
  cacheConfirmModalTitle: "Clear cache",
  cacheConfirmBtn: "Clear",
  cacheCancelBtn: "Cancel",
  cacheClearing: "Clearing…",
  cacheAutoTitle: "Auto-clean (attachments & archive)",
  cacheAutoLead:
    "Default: remove attachment files and qa_archive/*.jsonl older than 30 days when Relay opens (and when you change this). Choose Off (keep all) to disable. Does not rotate feedback_log.txt (manual clear or “clear log” in Storage).",
  cacheAutoSelectLabel: "Delete attachments and old qa_archive jsonl older than",
  cacheRetentionOff: "Off (keep all)",
  cacheDays: "days",
  cacheMonths3: "90 days (~3 mo)",
  cacheMonths6: "180 days (~6 mo)",
  cacheYear1: "365 days (1 yr)",
  cacheManualTitle: "Manual cleanup",
  cachePurgeFreed: "Freed {n} of old attachments.",
  cacheSectionStorage: "Usage",
  cacheRetentionTriggerAria: "Attachment retention",

  segUsage: "Cursor Usage",
  usageTitle: "Cursor Usage",
  usageLead:
    "Monitor your Cursor plan consumption and on-demand spending. Configure your session token below to enable usage tracking.",
  usageCapsuleTitle: "Cursor usage — click for details",
  usagePopoverTitle: "Cursor Usage This Cycle",
  usageMonthSummary: "Billing cycle summary",
  usagePlanUsed: "Plan requests",
  usagePlanRemaining: "Remaining",
  usageOnDemandUsed: "On-demand used",
  usageTeamOnDemand: "Team on-demand",
  usageMembership: "Plan",
  usagePlanCost: "Plan cost",
  usageOnDemandCap: "Spending cap",
  usageOnDemandNoLimit: "No limit (auto-billing on)",
  usageOnDemandViewDashboard: "View on cursor.com →",
  usageOnDemandDisabled: "Disabled",
  usageBillingCycle: "Billing cycle",
  usageResetsIn: "Resets in",
  usageDays: "days",
  usageDailyAvg: "Daily avg",
  usageReqPerDay: "req/day",
  usageExhaustedIn: "At current rate, exhausted in",
  usageExhaustedDays: "days",
  usageRecentTitle: "Recent usage events",
  usageNoEvents: "No events yet",
  usageLoadMore: "Load more",
  usageSettingsTokenTitle: "Authentication",
  usageSettingsAutoHint:
    "Usage data is automatically read from your local Cursor IDE session. No manual configuration needed.",
  usageSettingsIdeLoginHint: "Please log in to Cursor IDE to enable usage tracking.",
  usageSettingsTokenConfigured: "Connected to Cursor IDE",
  usageSettingsRefreshTitle: "Refresh policy",
  usageSettingsRefreshOnNewSession: "Refresh when a new Relay session starts",
  usageSettingsRefreshInterval: "Auto-refresh interval",
  usageSettingsRefreshIntervalUnit: "min",
  usageRefreshing: "Refreshing…",
  usageRefreshErr: "Failed to refresh usage:",
  usageRefreshBtn: "Refresh now",
  usagePlanCustom: "Custom",
  usageLastRefreshed: "Last refreshed",
  usageNever: "Never",

  // IDE selection
  ideSelectionTitle: "Select IDE Mode",
  ideSelectionSubtitle: "Select an IDE to enter and enable its features",
  ideCursor: "Cursor",
  ideCursorDesc: "MCP injection, rule prompts, usage monitoring",
  ideClaudeCode: "Claude Code",
  ideClaudeCodeDesc: "MCP injection, rule prompts",
  ideWindsurf: "Windsurf",
  ideWindsurfDesc: "MCP injection",
  ideOther: "Other",
  ideOtherDesc: "Manual MCP configuration",
  ideSettingsChangeBtn: "Switch IDE",
  ideSelectPlaceholder: "Select IDE",
};
