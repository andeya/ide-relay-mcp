export default {
  settingsTitle: "设置",
  appAuthorLine: "作者：andeya",
  appAuthorEmailAria: "邮箱 andeyalee@outlook.com",
  settingsBack: "返回",
  settingsCheckStatus: "刷新状态",
  settingsChecking: "刷新中…",
  settingsRefreshOk: "状态已刷新。",
  settingsRefreshWarn: "已刷新，但读取 MCP 配置失败。",
  settingsRefreshFail: "刷新失败：",
  ariaOpenSettings: "打开设置",
  segSetup: "环境与 MCP",
  segRulePrompts: "规则提示词",
  segCache: "缓存管理",

  settingsLangAria: "界面语言",

  rulePromptsTitle: "规则提示词",
  rulePromptsLead:
    "relay_interactive_feedback 的规则为**中英合本**。复制会粘贴**整段**内容；请将规则**置于顶部**或使用优先加载的规则文件（如 `00-relay-mcp.mdc`）。MCP：relay_interactive_feedback，接入见「环境与 MCP」。",
  rulePromptsSectionPreview: "规则预览",
  rulePromptsSectionIde: "各 IDE 配置方式",
  rulePromptsModeMild: "标准（推荐）",
  rulePromptsModeMildDesc:
    "每回合结束时调用一次；收到 human 后按需再次调用。不强制无限循环，适合多数场景。",
  rulePromptsModeLoop: "严格循环",
  rulePromptsModeLoopDesc:
    "每轮结束必调一次；传输失败时退避后重试。确保每轮都不遗漏调用。",
  rulePromptsModeTool: "仅工具说明",
  rulePromptsModeToolDesc:
    "仅工具约定，不含调用策略；可并入已有规则作为纯工具说明使用。",
  rulePromptsCopy: "粘贴到IDE",
  rulePromptsViewMd: "Markdown 预览",
  rulePromptsViewSource: "原文",
  rulePromptsToggleEnAria: "规则显示方式（Markdown 或原文）",
  rulePromptsLabelBilingual: "规则（中文 + English）",
  rulePromptsCopied: "已复制。",
  rulePromptsCopyErr: "复制失败。",
  rulePromptsLoopRisk:
    "「严格循环」可能使对话持续到你手动停止；子 Agent（如 Task 委派）请按提示词内说明由父 Agent 负责调用。",

  rulePromptsIdeMd: `**怎么用** 上方 **粘贴到IDE** 复制的是**中英合本规则**；再按下表把各编辑器的 **规则区** 与 **MCP** 配齐（两件事都要做）。

### Cursor
- **规则**：Settings → **Rules** → User rules 或 Project rules → 将规则全文**粘贴到规则顶部**（或使用 \`.cursor/rules/00-relay-mcp.mdc\` 以优先加载）。该规则声明 relay_interactive_feedback 最高优先级。  
- **MCP 文件** \`{cursorPath}\`  
- \`command\` = 本机 \`relay\`，\`args\` = \`["mcp"]\`；建议 \`autoApprove\`：\`relay_interactive_feedback\`

### Windsurf
- **说明区**：Agent / MCP 相关自定义说明里粘贴**同一套**规则（界面随版本可能不同）  
- **MCP 文件** \`{windsurfPath}\`

### VS Code
- 在 MCP 扩展要求的 **Rules / 自定义指令** 中写入英文提示词  
- MCP：\`command\` = \`relay\`（建议绝对路径），\`args\` = \`["mcp"]\`

### Claude Desktop
- **自定义指令** 贴英文；MCP 的 \`command\` 须为 relay **绝对路径**，\`args\`：\`["mcp"]\`

### 其他 IDE
支持 MCP + 系统/项目级规则即可：贴规则全文，并注册工具 \`relay_interactive_feedback\`。`,

  setupTitle: "本机环境",
  setupLead:
    "未就绪：点右侧「一键安装」（PATH + Cursor / Windsurf MCP）。就绪后：下方绿框可复制 MCP JSON，或只动某一个 IDE。",
  setupAllReadyLead:
    "PATH 与两编辑器 MCP 已就绪。绿框内可复制 JSON、做单 IDE 操作；要还原请点「一键卸载」。",
  setupStatus: "配置详情",
  setupChipPath: "终端 PATH",
  setupPathExplain: "relay 可执行文件所在目录已加入当前用户 PATH",
  setupChipCursor: "Cursor MCP",
  setupCursorExplain: "mcp.json 内已包含 relay-mcp",
  setupChipWindsurf: "Windsurf MCP",
  setupWindsurfExplain: "mcp_config.json 内已包含 relay-mcp",
  setupConfigFile: "配置文件",
  setupBinDir: "可执行目录",
  setupOn: "已配置",
  setupOff: "未配置",
  setupBtnInstall: "一键安装",
  setupBtnUninstall: "一键卸载",
  setupInstallHint:
    "写入用户 PATH，并向 Cursor、Windsurf 的 MCP 配置合并 relay-mcp（完成后请重启两 IDE，并新开终端）。",
  setupUninstallHint:
    "从 Cursor、Windsurf 移除 relay-mcp，并撤销 Relay 写入的 PATH。",
  setupNoInstallNeeded: "已全部配置，无需再安装。",
  setupActionsStripNeedInstall:
    "点击右侧「一键安装」写入 PATH 与 Cursor / Windsurf 的 MCP。",
  setupActionsAria: "安装与卸载",
  setupUninstallOnlyHint: "至少一项已配置时可一键卸载还原。",
  setupToolParamsTitle: "人机回路与本机 MCP",
  setupToolParamsLead:
    "本页：可复制的 MCP JSON、以及仅改 Cursor 或 Windsurf 时的快捷操作。",
  mcpCopy: "复制 MCP JSON",
  mcpCopied: "已复制到剪贴板。",
  mcpCopyErr: "复制失败。",
  setupAdvanced:
    "高级选项（PATH / JSON 等排障；Cursor·Windsurf 单 IDE 已在上方「人机回路」绿框内）",
  setupAdvPathTitle: "只补写 PATH",
  setupAdvPathLead:
    "若一键安装时 PATH 未成功（例如未找到 relay 可执行文件），可单独写入。新开终端或 fish 新会话后生效。",
  pathEnvFolder: "目录",
  pathEnvBtn: "写入用户 PATH",
  pathEnvBusy: "正在写入…",
  pathEnvDoneWin:
    "已完成。请重新打开命令提示符或 PowerShell。",
  pathEnvDoneMac: "已完成。请新开终端，或执行 source ~/.zshrc",
  pathEnvDoneLinux: "已完成。请新开终端，或 source ~/.bashrc / ~/.profile",
  pathEnvDoneOther: "已完成。请新开终端。",
  pathEnvAlready: "已在用户 PATH 中。",
  pathEnvErrPrefix: "无法写入 PATH：",
  setupAdvSingle: "只操作某一个 IDE",
  mcpCursorFile: "Cursor",
  mcpInCursor: "已配置 relay-mcp",
  mcpNotInCursor: "未配置 relay-mcp",
  mcpInstallCursorOnly: "仅写入 Cursor",
  mcpUninstallCursorOnly: "仅从 Cursor 移除",
  mcpWindsurfFile: "Windsurf",
  mcpInWindsurf: "已配置 relay-mcp",
  mcpNotInWindsurf: "未配置 relay-mcp",
  mcpInstallWindsurfOnly: "仅写入 Windsurf",
  mcpUninstallWindsurfOnly: "仅从 Windsurf 移除",
  mcpBusyInstallingAll: "正在安装…",
  mcpBusyUninstallingAll: "正在卸载…",
  mcpBusyCursorMcp: "正在保存 Cursor MCP…",
  mcpBusyWindsurfMcp: "正在保存 Windsurf MCP…",
  setupJsonPreview: "查看 MCP JSON",
  mcpJsonTitle: "生成的配置",
  setupIdeGuide: "各 IDE 配置路径说明",
  mcpFullInstallOk:
    "已完成。请重启 Cursor、Windsurf；PATH 需新开终端（或 fish 新会话）后生效。",
  mcpFullUninstallOk: "已卸载：MCP 与 Relay 写入的 PATH 均已撤销。",
  mcpFullUninstallConfirm:
    "将移除 Cursor、Windsurf 中的 relay-mcp，并撤销 Relay 写入的用户 PATH。",
  setupUninstallConfirmBtn: "确认卸载",
  setupUninstallCancel: "取消",
  mcpFullErr: "失败：",
  mcpPathSkippedNote:
    "（PATH 未写入：未在应用旁找到 relay；MCP 仍已写入。可在高级中单独补 PATH。）",
  mcpCursorInstallOk: "已更新 Cursor MCP，请重启 Cursor。",
  mcpCursorUninstallOk: "已从 Cursor 移除 relay-mcp。",
  mcpWindsurfInstallOk: "已更新 Windsurf MCP，请重启 Windsurf。",
  mcpWindsurfUninstallOk: "已从 Windsurf 移除 relay-mcp。",

  windowDockAria: "窗口在屏幕上的水平位置",
  windowDockLeft: "靠左",
  windowDockCenter: "居中",
  windowDockRight: "靠右",
  dockBtnLeft: "◀",
  dockBtnCenter: "●",
  dockBtnRight: "▶",

  releaseBadgeAria: "在 GitHub 打开 Relay 仓库",
  releaseBadgeUpdate: "v{latest} 新版本",
  releaseBadgeCurrent: "v{current}",

  mainSessionBadge: "Chat",
  appTitle: "Relay MCP",
  brand: "Relay",
  subtitle: "面向 AI IDE 的人工反馈层",
  statusAwaiting: "ME 回合",
  statusHubWaiting: "Hub · 未连IDE",
  ideBlockingHint:
    "IDE 正在等待你提交 Answer；提交后智能体会在同一轮继续。",
  mcpPauseTitle: "暂停 Relay MCP",
  mcpPauseHint:
    "开启后：IDE 调用本 MCP 时**不会打开 Relay 窗口**，并立刻返回固定提示；请在 Cursor 规则中约定：收到 <<<RELAY_MCP_PAUSED>>> 后**停止再次调用** relay_interactive_feedback，直到你在此关闭暂停。",
  mcpPauseSwitch: "暂停 MCP",
  mcpPauseSwitchTitle: "暂停人机回路：后续 IDE 调用不再弹窗",
  mcpPauseStatusOn: "当前：已暂停",
  mcpPauseStatusOff: "当前：正常",
  mcpPauseUpdateErr: "无法更新暂停状态，请检查权限后重试。",

  setupInstallChangesNote:
    "上述安装/卸载会动到：用户 PATH、两 IDE 的 MCP 配置、以及 Relay 数据目录（日志、附件、本机 HTTP）。",
  statusIdle: "AI 回合",
  statusTimedOut: "已超时",
  statusCancelled: "已取消",
  hint: "上：`retell` = 本轮AI回复；下：**Answer · 你的回复**。",
  mainHintPreview:
    "有待回复的标签时在此输入。Enter 发送 · ⌘/Ctrl+Enter 发送并关闭该标签。",
  mainSummaryReadonly: "只读 · 左：AI（retell）· 右：我（Answer）",
  tabStripAria: "反馈标签",
  tabCloseAria: "关闭此标签",
  tabCloseTitle: "关闭标签（悬停本标签后显示）",
  tabStripHub: "Hub",
  qaHistoryTitle: "对话",
  qaRetell: "本轮回复",
  qaRetellHint: "本轮AI对用户可见内容（MCP：`retell`）",
  qaAssistantTurn: "AI",
  qaUserTurnMe: "我",
  qaUserFeedback: "Answer（你的回复）",
  qaNoRetellYet: "本轮尚无AI消息。",
  composerMessage: "Answer",
  composerAnswerSub: "你的回复",
  composerAriaRegion: "你的回复输入区",
  composerHint:
    "Enter 提交 · Shift+Enter 换行 · ⌘/Ctrl+Enter 提交并关标签页 · 粘贴或附加图片/文件",
  composerHintDraft:
    "可先起草；请求到达后 Enter 提交 · 等待期间仅 Shift+Enter 换行 · 可粘贴或附加图片/文件",
  composerImageAria: "待发送图片预览",
  composerAttach: "附加图片或文件",
  composerThumbRemove: "移除图片",
  composerFileDropAria: "待发送文件",
  composerFileDropRemove: "移除文件",
  composerFilePathNotAFile: "不是文件（不支持文件夹）",
  composerFilePathTooLarge: "文件过大（最大 50MB）",
  composerFileReadFailed: "无法读取该文件",
  composerSubmitBlockedFileError: "请先移除或修正标红附件后再提交。",
  composerImageZoomTitle: "点击放大查看",
  imageLightboxClose: "关闭预览",
  composerSubmitIconTitle: "提交（Enter）；提交并关标签页：⌘/Ctrl+Enter。",
  composerSubmitIconAria: "提交回复",
  composerSubmitting: "正在提交…",
  composerSubmitDisabledPreview: "有 MCP 请求后可点此提交（或按 Enter）。",
  composerSubmitDisabledIdle: "等待AI请求到达后可提交。",
  composerSendShort: "提交",
  composerSendCloseShort: "提交并关标签",
  qaPendingCurrent: "在下方输入后按 Enter 或点右下角发送按钮。",
  qaPendingOther: "请在对应标签中回复…",
  qaSkipped: "已关闭（未填写回复）",
  qaEmptySubmit: "已提交（无文字）",
  feedback: "Answer",
  placeholder: "写下你的回复…",
  slashNoMatch: "无匹配的命令或技能",
  slashNoCommandsForSession: "本会话暂无命令或技能（IDE 未传入）",
  slashDropdownHint: "↑↓ 选择 · Enter 或 Tab 插入",
  slashCategoryAgentSkill: "技能",
  noteExpired: "该请求已超时或已被取消。内容仅可本地查看，无法再提交。",
  close: "关闭",
  submit: "提交（Enter）",
  submitClose: "关闭此条",
  submitCloseTab: "提交并关标签页（⌘/Ctrl+Enter）",
  loading: "加载中…",
  noLaunch: "无启动数据。",

  ideHintCursor:
    "Cursor — 一键安装会写入：\n{cursorPath}\n也可手动合并 JSON 后重启，在设置 → MCP 查看 relay-mcp。",
  ideHintVscode:
    "VS Code — MCP 入口因扩展而异；command 为 relay 绝对路径，args 为 [\"mcp\"]。",
  ideHintWindsurf:
    "Windsurf — 一键安装写入：\n{windsurfPath}\n手动添加时 command 与 JSON 一致。",
  ideHintClaude:
    "Claude Desktop — command 为 relay 全路径，args 为 [\"mcp\"]；允许 relay_interactive_feedback。",

  cacheTitle: "存储与缓存",
  cacheSubtitle:
    "附件与日志仅保存在本机。常见做法：仅对附件做「按天数自动清理」；日志仍建议手动清空。",
  cacheLead:
    "以下为 Relay 用户数据目录中「附件目录」与「反馈日志」占用的空间。清空附件后，历史对话里旧 Answer 的缩略图将无法再显示。",
  cacheDataDir: "数据目录",
  cacheOpenFolder: "打开数据文件夹",
  cacheOpenFolderErr: "无法打开文件夹。",
  cacheLoading: "正在统计…",
  cacheLoadErr: "无法读取缓存占用。",
  cacheTotal: "合计（附件 + 日志与会话归档）",
  cacheAttachments: "附件缓存",
  cacheLogs: "日志与会话归档",
  cacheRefresh: "刷新",
  cacheClearAll: "全部清空缓存",
  cacheClearAttach: "仅清空附件缓存",
  cacheClearLogs: "仅清空日志缓存",
  cacheBusy: "处理中…",
  cacheClearedOk: "已清空。",
  cacheClearErr: "清空失败。",
  cacheConfirmClearAll:
    "确定删除全部已保存的反馈附件，并清空 feedback_log.txt 与 qa_archive 下全部 jsonl？",
  cacheConfirmClearAttach:
    "确定删除附件目录下所有文件？历史里旧回复的图片/文件预览将失效。",
  cacheConfirmClearLogs:
    "确定清空 feedback_log.txt 并删除 qa_archive/*.jsonl？依赖磁盘恢复的历史气泡将丢失。",
  cacheConfirmModalTitle: "确认清空缓存",
  cacheConfirmBtn: "确定清空",
  cacheCancelBtn: "取消",
  cacheClearing: "正在清空…",
  cacheAutoTitle: "自动清理（附件与会话归档）",
  cacheAutoLead:
    "默认删除超过 30 天的已保存附件和 qa_archive 下早于该天数的 jsonl（启动或修改下方选项时生效）；选「关闭（全部保留）」可停用。不自动截断 feedback_log.txt（可在下方手动清空日志）。",
  cacheAutoSelectLabel: "删除早于以下时间的附件与 qa_archive 归档行",
  cacheRetentionOff: "关闭（全部保留）",
  cacheDays: "天",
  cacheMonths3: "90 天（约 3 个月）",
  cacheMonths6: "180 天（约 6 个月）",
  cacheYear1: "365 天（1 年）",
  cacheManualTitle: "手动清理",
  cachePurgeFreed: "已释放约 {n} 的旧附件。",
  cacheSectionStorage: "占用概览",
  cacheRetentionTriggerAria: "附件保留策略",
};
