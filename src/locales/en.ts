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
  rulePromptsSectionPreview: "Prompt preview",
  rulePromptsSectionIde: "Where to paste (by IDE)",
  rulePromptsModeMild: "Standard (recommended)",
  rulePromptsModeMildDesc:
    "Call once at end of each turn; call again as needed when you get human. Does not force an infinite loop. Suited for most use cases.",
  rulePromptsModeLoop: "Strict loop",
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
  rulePromptsLoopRisk:
    "Strict loop may run until you stop the session. Sub-agents (e.g. Task): parent owns the tool per the prompt.",

  rulePromptsIdeMd: `**How to use** **Paste in IDE** above copies the **bilingual rule** (中文 + English). Then match each editor’s **rules area** and **MCP** below (you need both).

### Cursor
- **Rules**: Settings → **Rules** → User or Project → paste the rule block **at the top** (or use \`.cursor/rules/00-relay-mcp.mdc\` so it loads first). The rule declares highest priority for relay_interactive_feedback.  
- **MCP file** \`{cursorPath}\`  
- \`command\` = local \`relay\`, \`args\` = \`["mcp"]\`; suggest \`autoApprove\`: \`relay_interactive_feedback\`

### Windsurf
- **Instructions**: paste the same rule block in Agent / MCP custom text (UI varies by version)  
- **MCP file** \`{windsurfPath}\`

### VS Code
- Put the English prompt where your MCP extension expects **Rules / custom instructions**  
- MCP: \`command\` = \`relay\` (full path recommended), \`args\` = \`["mcp"]\`

### Claude Desktop
- **Custom instructions** + MCP \`command\` = **full path** to relay, \`args\` = \`["mcp"]\`

### Other IDEs
Any MCP + rules-capable client: paste the rule block and register \`relay_interactive_feedback\`.`,

  setupTitle: "This machine",
  setupLead:
    "Not ready: Install all (PATH + Cursor & Windsurf MCP). When ready: copy MCP JSON in the green card below, or use per-IDE actions.",
  setupAllReadyLead:
    "PATH and both IDEs are set. JSON copy and per-IDE actions live in the green card; Uninstall all to revert.",
  setupStatus: "Configuration detail",
  setupChipPath: "Terminal PATH",
  setupPathExplain: "Folder containing the relay binary is on your user PATH",
  setupChipCursor: "Cursor MCP",
  setupCursorExplain: "relay-mcp is present in mcp.json",
  setupChipWindsurf: "Windsurf MCP",
  setupWindsurfExplain: "relay-mcp is present in mcp_config.json",
  setupConfigFile: "Config file",
  setupBinDir: "Binary folder",
  setupOn: "Ready",
  setupOff: "Not set",
  setupBtnInstall: "Install all",
  setupBtnUninstall: "Uninstall all",
  setupInstallHint:
    "Writes user PATH and merges relay-mcp into Cursor and Windsurf (restart both IDEs and open a new terminal afterward).",
  setupUninstallHint:
    "Removes relay-mcp from Cursor & Windsurf and undoes Relay’s PATH changes.",
  setupNoInstallNeeded: "Everything is already configured.",
  setupActionsStripNeedInstall:
    "Use Install all on the right to write PATH and Cursor / Windsurf MCP.",
  setupActionsAria: "Install and uninstall",
  setupUninstallOnlyHint: "Uninstall is available when at least one item above is configured.",
  setupToolParamsTitle: "Human-in-the-loop & MCP on this machine",
  setupToolParamsLead:
    "Copy MCP JSON here, or use the Cursor / Windsurf panels below for per-IDE install/remove.",
  mcpCopy: "Copy MCP JSON",
  mcpCopied: "Copied to clipboard.",
  mcpCopyErr: "Copy failed.",
  setupAdvanced:
    "Advanced (PATH, JSON preview… — per-IDE Cursor/Windsurf is in the green card above)",
  setupAdvPathTitle: "PATH only",
  setupAdvPathLead:
    "If full install skipped PATH (e.g. relay binary not found beside the app), add it here. Open a new terminal or fish session afterward.",
  pathEnvFolder: "Folder",
  pathEnvBtn: "Add to user PATH",
  pathEnvBusy: "Applying…",
  pathEnvDoneWin: "Done. Open a new Command Prompt or PowerShell.",
  pathEnvDoneMac: "Done. New terminal, or: source ~/.zshrc",
  pathEnvDoneLinux: "Done. New terminal, or source ~/.bashrc / ~/.profile",
  pathEnvDoneOther: "Done. Open a new terminal.",
  pathEnvAlready: "Already on user PATH.",
  pathEnvErrPrefix: "Could not update PATH:",
  setupAdvSingle: "One IDE at a time",
  mcpCursorFile: "Cursor",
  mcpInCursor: "relay-mcp present",
  mcpNotInCursor: "relay-mcp not present",
  mcpInstallCursorOnly: "Install Cursor only",
  mcpUninstallCursorOnly: "Remove from Cursor only",
  mcpWindsurfFile: "Windsurf",
  mcpInWindsurf: "relay-mcp present",
  mcpNotInWindsurf: "relay-mcp not present",
  mcpInstallWindsurfOnly: "Install Windsurf only",
  mcpUninstallWindsurfOnly: "Remove from Windsurf only",
  mcpCursorBusy: "…",
  setupJsonPreview: "Show MCP JSON",
  mcpJsonTitle: "Generated config",
  setupIdeGuide: "IDE config paths",

  mcpFullBusy: "Working…",
  mcpFullInstallOk:
    "Done. Restart Cursor & Windsurf; PATH applies in a new terminal (or new fish session).",
  mcpFullUninstallOk: "Uninstalled: MCP entries and Relay PATH changes removed.",
  mcpFullUninstallConfirm:
    "This removes relay-mcp from Cursor & Windsurf and undoes Relay user PATH changes.",
  setupUninstallConfirmBtn: "Uninstall",
  setupUninstallCancel: "Cancel",
  mcpFullErr: "Failed:",
  mcpPathSkippedNote:
    "(PATH skipped: relay not found beside app — MCP still installed. Add PATH in Advanced.)",
  mcpCursorInstallOk: "Cursor MCP updated. Restart Cursor.",
  mcpCursorUninstallOk: "Removed relay-mcp from Cursor.",
  mcpWindsurfInstallOk: "Windsurf MCP updated. Restart Windsurf.",
  mcpWindsurfUninstallOk: "Removed relay-mcp from Windsurf.",

  windowDockAria: "Window horizontal position on screen",
  windowDockLeft: "Dock left",
  windowDockCenter: "Center horizontally",
  windowDockRight: "Dock right",
  dockBtnLeft: "◀",
  dockBtnCenter: "●",
  dockBtnRight: "▶",

  mainSessionBadge: "Chat",
  appTitle: "Relay MCP",
  brand: "Relay",
  subtitle: "Human feedback layer for AI IDEs",
  statusAwaiting: "Your turn",
  statusHubWaiting: "Waiting for IDE connection",
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

  setupInstallChangesNote:
    "Install/uninstall touches: user PATH, both IDEs’ MCP config, and Relay app data (logs, attachments, local HTTP).",
  statusIdle: "Waiting for next assistant turn",
  statusTimedOut: "Timed out",
  statusCancelled: "Cancelled",
  hint: "Top: **Retell** (`retell`) = this turn's assistant reply. Bottom: **Answer**.",
  mainHintPreview:
    "When a tab is waiting for your reply, type here. Enter to send · ⌘/Ctrl+Enter to send and close the tab.",
  mainSummaryReadonly:
    "Read-only · Left: AI (retell) · Right: ME (Answer)",
  tabStripAria: "Feedback tabs",
  tabCloseAria: "Close this tab",
  tabCloseTitle: "Close tab (shown when hovering this tab)",
  tabStripHub: "Hub",
  qaHistoryTitle: "Thread",
  qaRetell: "Retell",
  qaRetellHint: "This turn's assistant reply (MCP: retell)",
  qaAssistantTurn: "AI",
  /** Thread bubble label for the user side (composer still says “Answer”). */
  qaUserTurnMe: "ME",
  qaUserFeedback: "Answer",
  qaNoRetellYet: "No assistant message for this turn yet.",
  composerMessage: "Answer",
  composerAnswerSub: "Your reply",
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
  composerSubmitDisabledPreview:
    "Open when an MCP request is active — then tap to submit (Enter).",
  composerSubmitDisabledIdle:
    "Submit is available once an assistant request is active.",
  composerSendShort: "Submit",
  composerSendCloseShort: "Submit & close tab",
  qaPendingCurrent: "Type below, then Enter or the send button.",
  qaPendingOther: "Awaiting reply on another tab…",
  qaSkipped: "Closed with no reply",
  qaEmptySubmit: "Submitted with no text",
  feedback: "Answer",
  placeholder: "Write your reply…",
  slashNoMatch: "No matching command or skill",
  slashNoCommandsForSession: "No commands or skills for this session (IDE did not provide any)",
  slashDropdownHint: "↑↓ Navigate · Enter or Tab to insert",
  slashCategoryAgentSkill: "Skill",
  noteExpired:
    "This request has already timed out or been cancelled. Your text can be reviewed locally, but it can no longer be submitted.",
  close: "Close",
  submit: "Submit (Enter)",
  submitClose: "Dismiss",
  submitCloseTab: "Submit & close tab (⌘/Ctrl+Enter)",
  loading: "Loading…",
  noLaunch: "No launch data available.",

  ideHintCursor:
    "Cursor — Full install writes:\n{cursorPath}\nOr merge JSON manually, then restart. Check Settings → MCP.",
  ideHintVscode:
    "VS Code — MCP UI varies; command = full path to relay, args = [\"mcp\"].",
  ideHintWindsurf:
    "Windsurf — Full install writes:\n{windsurfPath}\nManual MCP: same command as in JSON.",
  ideHintClaude:
    "Claude Desktop — command = full path to relay, args = [\"mcp\"]. Approve relay_interactive_feedback if prompted.",

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
};
