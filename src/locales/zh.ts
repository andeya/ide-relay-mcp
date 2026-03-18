export default {
  settingsTitle: "设置",
  appAuthorLine: "作者：andeya",
  appAuthorEmailAria: "邮箱 andeyalee@outlook.com",
  settingsBack: "返回",
  settingsCheckStatus: "刷新状态",
  settingsChecking: "刷新中…",
  ariaOpenSettings: "打开设置",
  segSetup: "环境与 MCP",
  segRulePrompts: "规则提示词",

  settingsLangAria: "界面语言",

  rulePromptsTitle: "规则提示词",
  rulePromptsLead:
    "英文为给模型使用的正文（**复制仅英文**）；中文与英文语义一致，仅供对照。**切换界面语言不会改变下方英/中文案。** MCP：relay_interactive_feedback，接入见「环境与 MCP」。",
  rulePromptsSectionPreview: "规则预览",
  rulePromptsSectionIde: "各 IDE 配置方式",
  rulePromptsModeMild: "标准（推荐）",
  rulePromptsModeMildDesc: "按需再次调用；不强制无限循环。",
  rulePromptsModeLoop: "严格循环",
  rulePromptsModeLoopDesc: "每轮结束必调；失败退避重试；慎用。",
  rulePromptsModeTool: "仅工具说明",
  rulePromptsModeToolDesc: "可并入现有规则的一段定义。",
  rulePromptsCopy: "复制英文（粘贴 IDE）",
  rulePromptsLabelEn: "English",
  rulePromptsLabelZh: "中文对照",
  rulePromptsCopied: "已复制。",
  rulePromptsCopyErr: "复制失败。",
  rulePromptsLoopRisk:
    "「严格循环」可能使对话持续到你手动停止；子 Agent（如 Task 委派）请按提示词内说明由父 Agent 负责调用。",

  rulePromptsIdeBody: `【Cursor】
Settings → Rules → User rules 或 Project rules，粘贴上方英文全文。
MCP：{cursorPath}（command 为 relay 可执行文件，args 为 [\"mcp\"]；建议 autoApprove relay_interactive_feedback）

【Windsurf】
在 Agent / MCP 相关自定义说明处粘贴同一套英文（界面以当前版本为准）。
MCP：{windsurfPath}

【VS Code】
使用 MCP 相关扩展时，将英文提示词写入工作区或用户级 Rules / 自定义指令；MCP：command 为 relay，args 为 [\"mcp\"]。

【Claude Desktop】
在应用提供的自定义指令处粘贴英文提示词；MCP：command 为 relay 绝对路径，args 为 [\"mcp\"]。

【其他 IDE】
凡支持 MCP 与系统/项目级规则：粘贴英文全文，并确保已注册工具 relay_interactive_feedback。`,

  setupTitle: "本机环境",
  setupLead:
    "若尚未配齐，可用一键安装同时写入 PATH 与 Cursor、Windsurf 的 MCP；其它编辑器请复制 JSON。已配齐时只需维护或卸载。",
  setupAllReadyLead:
    "PATH、Cursor 与 Windsurf 的 MCP 均已就绪。可复制 JSON 给其它 IDE；需还原时请一键卸载。",
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
  setupUninstallOnlyHint: "至少一项已配置时可一键卸载还原。",
  setupToolParamsTitle: "工具参数（本机）",
  setupToolParamsLead:
    "与当前机器上 MCP tools/list 一致；若说明未更新可重启 IDE。",
  setupParamSessionTitle:
    "session_title — 强烈建议：会话/标签标题，用于 Relay 窗口标签。",
  setupParamClientTabId:
    "client_tab_id — 强烈建议：每个 IDE 聊天标签的稳定 ID。",
  setupCopyTitle: "其它编辑器",
  setupCopyLead:
    "复制 JSON 到剪贴板；command 为本机 relay，args 为 [\"mcp\"]。",
  mcpCopy: "复制 MCP JSON",
  mcpCopied: "已复制到剪贴板。",
  mcpCopyErr: "复制失败。",
  setupAdvanced: "高级选项（排障或单独操作）",
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
  mcpCursorBusy: "…",
  setupJsonPreview: "查看 MCP JSON",
  mcpJsonTitle: "生成的配置",
  setupIdeGuide: "各 IDE 配置路径说明",

  mcpFullBusy: "处理中…",
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

  mainSessionBadge: "会话",
  /** i18n: badge when only generic tab title (Chat N) is shown in header */
  mainTabBadge: "标签",
  appTitle: "Relay MCP",
  brand: "Relay",
  subtitle: "面向 AI IDE 的人工反馈层",
  statusAwaiting: "等待你的回复",
  ideBlockingHint:
    "IDE 正在等待你提交 Answer（最长约 10 分钟）；提交后智能体会在同一轮继续。",
  mcpPauseTitle: "暂停 Relay MCP",
  mcpPauseHint:
    "开启后：IDE 调用本 MCP 时**不会打开 Relay 窗口**，并立刻返回固定提示；请在 Cursor 规则中约定：收到 <<<RELAY_MCP_PAUSED>>> 后**停止再次调用** relay_interactive_feedback，直到你在此关闭暂停。",
  mcpPauseSwitch: "暂停 MCP",
  mcpPauseSwitchTitle: "暂停人机回路：后续 IDE 调用不再弹窗",
  mcpPauseStatusOn: "当前：已暂停",
  mcpPauseStatusOff: "当前：正常",

  setupInstallChangesNote:
    "一键安装可能修改：用户 PATH；Cursor / Windsurf 的 MCP 配置文件（见下方路径）；Relay 应用数据目录（日志、附件、本机 HTTP 端点信息）。",
  statusIdle: "等待下一轮助手请求",
  statusTimedOut: "已超时",
  statusCancelled: "已取消",
  hint: "上：`retell` = 本轮助手回复；下：**Answer · 你的回复**。",
  mainHintPreview:
    "占位页：有 MCP 请求后可用箭头提交。Enter 提交 · ⌘/Ctrl+Enter 提交并关标签页。",
  mainSummaryReadonly: "只读 · AI：本轮回复（retell）· 你：Answer",
  tabStripAria: "反馈标签",
  tabCloseAria: "关闭此标签",
  tabCloseTitle: "关闭标签（悬停本标签后显示）",
  tabStripHub: "Hub",
  qaHistoryTitle: "对话",
  qaRetell: "本轮回复",
  qaRetellHint: "助手本轮对用户可见内容（MCP：`retell`）",
  qaAssistantTurn: "AI",
  qaUserFeedback: "Answer（你的回复）",
  composerMessage: "Answer",
  composerAnswerSub: "你的回复",
  composerHint:
    "发送按钮或 Enter 提交 · ⌘/Ctrl+Enter 提交并关标签页 · Shift+Enter 换行 · 粘贴或插入图片",
  composerImageAria: "待发送图片预览",
  composerAttach: "插入图片",
  composerThumbRemove: "移除图片",
  composerImageZoomTitle: "点击放大查看",
  imageLightboxClose: "关闭预览",
  composerSubmitIconTitle: "提交（Enter）；提交并关标签页：⌘/Ctrl+Enter。",
  composerSubmitIconAria: "提交回复",
  composerSubmitDisabledPreview: "有 MCP 请求后可点此提交（或按 Enter）。",
  composerSendShort: "提交",
  composerSendCloseShort: "提交并关标签",
  qaPendingCurrent: "在下方输入后按 Enter 或点右下角发送按钮。",
  qaPendingOther: "请在对应标签中回复…",
  qaSkipped: "已关闭（未填写回复）",
  qaEmptySubmit: "已提交（无文字）",
  feedback: "Answer",
  placeholder: "写下你的回复…",
  composerIdlePlaceholder: "等待助手下一轮消息后可继续回复…",
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
};
