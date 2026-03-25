<script setup lang="ts">
/**
 * Settings → Rule prompts: bilingual (中英合本) preview and copy, IDE snippet.
 */
import { computed, ref } from "vue";
import {
  getRelayRulePromptBilingual,
  type RulePromptMode,
} from "../../ideRulesTemplates";
import { locale, t } from "../../i18n";
import { safeMarkdownToHtml } from "../../utils/safeMarkdown";

const props = defineProps<{
  strings: Record<string, string>;
  cursorMcpPath: string;
  windsurfMcpPath: string;
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
