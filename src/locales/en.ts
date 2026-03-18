export default {
  settingsTitle: "Settings",
  appAuthorLine: "Author: andeya",
  appAuthorEmailAria: "Email andeyalee@outlook.com",
  settingsBack: "Back",
  settingsCheckStatus: "Refresh",
  settingsChecking: "Refreshing…",
  ariaOpenSettings: "Open settings",
  segSetup: "Environment & MCP",
  segRulePrompts: "Rule prompts",

  settingsLangAria: "Interface language",

  rulePromptsTitle: "Rule prompts",
  rulePromptsLead:
    "English is the text to paste into IDE rules (copy is English-only). Chinese is a fixed human-facing mirror of the same rules. **Switching UI language does not change the English or Chinese prompt blocks below.** Tool: relay_interactive_feedback — wire MCP under Environment & MCP.",
  rulePromptsSectionPreview: "Prompt preview",
  rulePromptsSectionIde: "Where to paste (by IDE)",
  rulePromptsModeMild: "Standard (recommended)",
  rulePromptsModeMildDesc: "Call again only when needed; no forced loop.",
  rulePromptsModeLoop: "Strict loop",
  rulePromptsModeLoopDesc: "Every turn ends with the tool; backoff on errors.",
  rulePromptsModeTool: "Tool spec only",
  rulePromptsModeToolDesc: "Single block to merge into existing rules.",
  rulePromptsCopy: "Copy English (for IDE)",
  rulePromptsLabelEn: "English",
  rulePromptsLabelZh: "Chinese reference",
  rulePromptsCopied: "Copied.",
  rulePromptsCopyErr: "Copy failed.",
  rulePromptsLoopRisk:
    "Strict loop may run until you stop the session. Sub-agents (e.g. Task): parent owns the tool per the prompt.",

  rulePromptsIdeBody: `[Cursor]
Settings → Rules → User or Project rules — paste the English block above.
MCP file: {cursorPath} — command = relay binary, args = ["mcp"]; recommend autoApprove relay_interactive_feedback

[Windsurf]
Paste the same English block into Agent / MCP custom instructions (UI may vary by version).
MCP: {windsurfPath}

[VS Code]
With an MCP extension, paste into workspace or user Rules / custom instructions; MCP command = relay, args = ["mcp"].

[Claude Desktop]
Paste into custom instructions; MCP command = full path to relay, args = ["mcp"].

[Other IDEs]
Any client with MCP + system/project rules: paste the English block and register relay_interactive_feedback.`,

  setupTitle: "This machine",
  setupLead:
    "If anything is missing, use Install all to set PATH plus Cursor and Windsurf MCP. Other editors: copy JSON. When everything is ready, you only need uninstall or copy.",
  setupAllReadyLead:
    "PATH, Cursor MCP, and Windsurf MCP are all set. Copy JSON for other IDEs, or use Uninstall all to revert.",
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
  setupUninstallOnlyHint: "Uninstall is available when at least one item above is configured.",
  setupToolParamsTitle: "Tool parameters (this machine)",
  setupToolParamsLead:
    "Matches MCP tools/list on this host. Restart the IDE if tool descriptions look stale.",
  setupParamSessionTitle:
    "session_title — strongly recommended: chat/tab title for Relay window label.",
  setupParamClientTabId:
    "client_tab_id — strongly recommended: stable id per IDE chat tab.",
  setupCopyTitle: "Other editors",
  setupCopyLead:
    "Copy JSON to the clipboard and paste into VS Code, Claude Desktop, etc.; command = relay, args = [\"mcp\"].",
  mcpCopy: "Copy MCP JSON",
  mcpCopied: "Copied to clipboard.",
  mcpCopyErr: "Copy failed.",
  setupAdvanced: "Advanced (single-step or troubleshooting)",
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

  mainSessionBadge: "Session",
  mainTabBadge: "Tab",
  appTitle: "Relay MCP",
  brand: "Relay",
  subtitle: "Human feedback layer for AI IDEs",
  statusAwaiting: "Awaiting your reply",
  ideBlockingHint:
    "The IDE is waiting for your Answer (up to ~10 min). After you submit, the agent continues in the same turn.",
  mcpPauseTitle: "Pause Relay MCP",
  mcpPauseHint:
    "When on: IDE calls to this MCP **do not open Relay** and return a fixed message immediately. Add a Cursor rule: if the tool result contains <<<RELAY_MCP_PAUSED>>>, **stop calling** relay_interactive_feedback until you turn this off here.",
  mcpPauseSwitch: "Pause MCP",
  mcpPauseSwitchTitle: "Pause human-in-the-loop — IDE calls won’t open Relay",
  mcpPauseStatusOn: "Status: paused",
  mcpPauseStatusOff: "Status: active",

  setupInstallChangesNote:
    "Full install may change: your user PATH; Cursor and Windsurf MCP JSON (paths below); Relay app data (logs, attachments, local HTTP token file).",
  statusIdle: "Waiting for next assistant turn",
  statusTimedOut: "Timed out",
  statusCancelled: "Cancelled",
  hint: "Top: **Retell** (`retell`) = this turn's assistant reply. Bottom: **Answer**.",
  mainHintPreview:
    "Hub: arrow button submits when an MCP tab is open. Enter = submit · ⌘/Ctrl+Enter = submit & close tab.",
  mainSummaryReadonly: "Read-only · AI: this turn (retell) · You: Answer",
  tabStripAria: "Feedback tabs",
  tabCloseAria: "Close this tab",
  tabCloseTitle: "Close tab (shown when hovering this tab)",
  tabStripHub: "Hub",
  qaHistoryTitle: "Thread",
  qaRetell: "Retell",
  qaRetellHint: "This turn's assistant reply (MCP: retell)",
  qaAssistantTurn: "AI",
  qaUserFeedback: "Answer",
  composerMessage: "Answer",
  composerAnswerSub: "Your reply",
  composerHint:
    "Send button or Enter to submit · ⌘/Ctrl+Enter: submit & close tab · Shift+Enter: newline · paste or attach images",
  composerImageAria: "Image attachments preview",
  composerAttach: "Attach image",
  composerThumbRemove: "Remove image",
  composerImageZoomTitle: "Click to enlarge",
  imageLightboxClose: "Close preview",
  composerSubmitIconTitle:
    "Submit (Enter). Submit and close tab: ⌘ or Ctrl + Enter.",
  composerSubmitIconAria: "Submit answer",
  composerSubmitDisabledPreview:
    "Open when an MCP request is active — then tap to submit (Enter).",
  composerSendShort: "Submit",
  composerSendCloseShort: "Submit & close tab",
  qaPendingCurrent: "Type below, then Enter or the send button.",
  qaPendingOther: "Awaiting reply on another tab…",
  qaSkipped: "Closed with no reply",
  qaEmptySubmit: "Submitted with no text",
  feedback: "Answer",
  placeholder: "Write your reply…",
  composerIdlePlaceholder: "Waiting for the next assistant message…",
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
};
