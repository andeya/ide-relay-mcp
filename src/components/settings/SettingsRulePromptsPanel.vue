<script setup lang="ts">
/**
 * Settings → Rule prompts: bilingual (中英合本) preview and copy, IDE snippet.
 */
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
  getRelayRulePromptBilingual,
  type RulePromptMode,
} from "../../ideRulesTemplates";
import { locale, t } from "../../i18n";
import { safeMarkdownToHtml } from "../../utils/safeMarkdown";
import type { SettingsToastPayload } from "../../composables/useRelayCacheSettings";

const props = defineProps<{
  strings: Record<string, string>;
  cursorMcpPath: string;
  windsurfMcpPath: string;
  pushToast: (p: SettingsToastPayload) => void;
}>();

const rulePromptMode = ref<RulePromptMode>("mild");
const rulesCopyToast = ref("");
const rulePromptBilingual = computed(() =>
  getRelayRulePromptBilingual(rulePromptMode.value),
);
const rulePromptView = ref<"md" | "src">("md");
const rulePromptBilingualHtml = computed(() =>
  safeMarkdownToHtml(rulePromptBilingual.value),
);

const cursorRuleInstalled = ref(false);
const cursorRuleBusy = ref(false);

async function checkCursorRuleInstalled() {
  try {
    cursorRuleInstalled.value = await invoke<boolean>("get_cursor_rule_installed");
  } catch {
    cursorRuleInstalled.value = false;
  }
}

async function installOrUpdateCursorRule() {
  if (cursorRuleBusy.value) return;
  const wasInstalled = cursorRuleInstalled.value;
  cursorRuleBusy.value = true;
  try {
    await invoke("install_cursor_rule", { content: rulePromptBilingual.value });
    cursorRuleInstalled.value = true;
    const msg = wasInstalled ? t("rulePromptsUpdateOk") : t("rulePromptsInstallOk");
    props.pushToast({ type: "ok", text: msg, durationMs: 3000 });
  } catch (e) {
    const detail = e instanceof Error ? e.message : String(e);
    props.pushToast({ type: "err", text: `${t("rulePromptsInstallErr")} ${detail}`, durationMs: 4000 });
  } finally {
    cursorRuleBusy.value = false;
  }
}

async function removeCursorRule() {
  if (cursorRuleBusy.value) return;
  cursorRuleBusy.value = true;
  try {
    await invoke("uninstall_cursor_rule");
    cursorRuleInstalled.value = false;
    props.pushToast({ type: "ok", text: t("rulePromptsRemoveOk"), durationMs: 3000 });
  } catch (e) {
    const detail = e instanceof Error ? e.message : String(e);
    props.pushToast({ type: "err", text: `${t("rulePromptsRemoveErr")} ${detail}`, durationMs: 4000 });
  } finally {
    cursorRuleBusy.value = false;
  }
}

onMounted(() => {
  void checkCursorRuleInstalled();
});

function setRulePromptView(mode: "md" | "src") {
  rulePromptView.value = mode;
}

const rulePromptsIdeHtml = computed(() => {
  void locale.value;
  return safeMarkdownToHtml(
    t("rulePromptsIdeMd", {
      cursorPath: props.cursorMcpPath || "~/.cursor/mcp.json",
      windsurfPath: props.windsurfMcpPath || "~/.codeium/windsurf/mcp_config.json",
    }),
  );
});

const S = computed(() => props.strings);

async function copyRulePrompt() {
  rulesCopyToast.value = "";
  try {
    await navigator.clipboard.writeText(rulePromptBilingual.value);
    rulesCopyToast.value = t("rulePromptsCopied");
  } catch {
    rulesCopyToast.value = t("rulePromptsCopyErr");
  }
  setTimeout(() => {
    rulesCopyToast.value = "";
  }, 2500);
}
</script>

<template>
  <div class="segPanel">
    <div class="installHubCard settingsCard rulePromptsCard">
      <h3 class="installHubTitle">{{ S.rulePromptsTitle }}</h3>
      <p class="installHubDesc">{{ S.rulePromptsLead }}</p>

      <h4 class="rulePromptsSubhead">{{ S.rulePromptsSectionPreview }}</h4>

      <div
        class="cursorRulesModeGrid"
        role="radiogroup"
        :aria-label="S.rulePromptsSectionPreview"
      >
        <button
          type="button"
          class="cursorRulesModeBtn"
          :class="{ active: rulePromptMode === 'mild' }"
          role="radio"
          :aria-checked="rulePromptMode === 'mild'"
          @click="rulePromptMode = 'mild'"
        >
          <span class="cursorRulesModeTitle">{{ S.rulePromptsModeMild }}</span>
          <span class="cursorRulesModeDesc">{{ S.rulePromptsModeMildDesc }}</span>
        </button>
        <button
          type="button"
          class="cursorRulesModeBtn"
          :class="{ active: rulePromptMode === 'loop' }"
          role="radio"
          :aria-checked="rulePromptMode === 'loop'"
          @click="rulePromptMode = 'loop'"
        >
          <span class="cursorRulesModeTitle">{{ S.rulePromptsModeLoop }}</span>
          <span class="cursorRulesModeDesc">{{ S.rulePromptsModeLoopDesc }}</span>
        </button>
        <button
          type="button"
          class="cursorRulesModeBtn"
          :class="{ active: rulePromptMode === 'toolOnly' }"
          role="radio"
          :aria-checked="rulePromptMode === 'toolOnly'"
          @click="rulePromptMode = 'toolOnly'"
        >
          <span class="cursorRulesModeTitle">{{ S.rulePromptsModeTool }}</span>
          <span class="cursorRulesModeDesc">{{ S.rulePromptsModeToolDesc }}</span>
        </button>
      </div>

      <p v-if="rulePromptMode === 'loop'" class="note cursorRulesRisk">
        {{ S.rulePromptsLoopRisk }}
      </p>

      <div class="rulePromptBilingual">
        <div class="rulePromptLangRow">
          <p class="rulePromptLangLabel rulePromptLangLabel--row">
            {{ S.rulePromptsLabelBilingual }}
          </p>
          <div class="rulePromptRowTools">
            <div
              class="rulePromptViewToggles"
              role="group"
              :aria-label="S.rulePromptsToggleEnAria"
            >
              <button
                type="button"
                class="rulePromptViewIconBtn"
                :class="{ active: rulePromptView === 'md' }"
                :aria-pressed="rulePromptView === 'md'"
                :title="S.rulePromptsViewMd"
                @click="setRulePromptView('md')"
              >
                <svg
                  class="rulePromptViewIconSvg"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  aria-hidden="true"
                >
                  <path
                    d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"
                  />
                  <path d="M14 2v6h6M16 13H8M16 17H8M10 9H8" />
                </svg>
              </button>
              <button
                type="button"
                class="rulePromptViewIconBtn"
                :class="{ active: rulePromptView === 'src' }"
                :aria-pressed="rulePromptView === 'src'"
                :title="S.rulePromptsViewSource"
                @click="setRulePromptView('src')"
              >
                <svg
                  class="rulePromptViewIconSvg"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  aria-hidden="true"
                >
                  <polyline points="16 18 22 12 16 6" />
                  <polyline points="8 6 2 12 8 18" />
                </svg>
              </button>
            </div>
            <div class="rulePromptCopyActions">
              <span
                v-if="rulesCopyToast"
                class="copyToast rulePromptCopyToast"
                >{{ rulesCopyToast }}</span
              >
              <button
                type="button"
                class="secondary rulePromptsCopyBtn"
                @click="copyRulePrompt"
              >
                {{ S.rulePromptsCopy }}
              </button>
              <button
                type="button"
                class="usageBtn usageBtn--primary rulePromptsCursorBtn"
                :disabled="cursorRuleBusy"
                @click="installOrUpdateCursorRule"
              >
                {{ cursorRuleInstalled ? S.rulePromptsUpdateCursor : S.rulePromptsInstallCursor }}
              </button>
              <button
                v-if="cursorRuleInstalled"
                type="button"
                class="usageBtn usageBtn--ghost rulePromptsCursorBtn"
                :disabled="cursorRuleBusy"
                @click="removeCursorRule"
              >
                {{ S.rulePromptsRemoveCursor }}
              </button>
              <span
                v-if="cursorRuleInstalled"
                class="rulePromptsInstalledBadge"
              >{{ S.rulePromptsInstalledBadge }}</span>
            </div>
          </div>
        </div>
        <template v-if="rulePromptView === 'md'">
          <div
            class="rulePromptMdBody qaRoundMd cursorRulesPre cursorRulesPre--prompt"
            tabindex="0"
            v-html="rulePromptBilingualHtml"
          />
        </template>
        <pre
          v-else
          class="cursorRulesPre cursorRulesPre--prompt"
          tabindex="0"
        >{{ rulePromptBilingual }}</pre>
      </div>

      <h4 class="rulePromptsSubhead rulePromptsSubhead--spaced">
        {{ S.rulePromptsSectionIde }}
      </h4>
      <div
        class="rulePromptsIdePanel"
        tabindex="0"
        role="region"
        :aria-label="S.rulePromptsSectionIde"
      >
        <div
          class="rulePromptsIdeMd qaRoundMd"
          v-html="rulePromptsIdeHtml"
        />
      </div>
    </div>
  </div>
</template>
