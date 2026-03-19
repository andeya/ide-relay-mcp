# Relay GUI frontend layout

Vue 3 + Vite. The root shell is **`src/App.vue`**: main Q&A workspace, settings chrome, drag-and-drop, lightbox, and toasts.

**Lint:** `npm run lint` runs ESLint on **`src/**/*.vue`** and **`src/**/*.ts`** (`eslint.config.js`).

## Split components (settings)

| Module                                                     | Role                                                                                                                          |
| ---------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| **`src/components/settings/SettingsCachePanel.vue`**       | Storage overview, attachment retention menu, manual cache clear + confirm modal.                                              |
| **`src/components/settings/SettingsRulePromptsPanel.vue`** | Rule prompt modes, EN/ZH Markdown vs source toggles, copy-to-clipboard, IDE snippet.                                          |
| **`src/composables/useRelayCacheSettings.ts`**             | Tauri invokes for cache stats, retention read/write, purge on save, clear actions; document click-outside for retention menu. |
| **`src/utils/formatBytes.ts`**                             | Shared byte formatting for cache UI.                                                                                          |

`App.vue` still holds: tab strip, Q&A history, composer, MCP install hub, pause toggle, and wires **`pushSettingsToast`** into the cache panel.

## Other frontend paths

- **`src/composables/useFeedbackWindow.ts`** — tabs, polling, submit, attachments, **`run_attachment_retention_purge`** on init.
- **`src/composables/useMcpAndPathSettings.ts`** — MCP install state and paths.
- **`src/components/QaReplyAttachments.vue`** — Answer attachment chips / lightbox hooks.
