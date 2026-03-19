<script setup lang="ts">
/**
 * Settings → Rule prompts: mode, EN/ZH preview, copy, IDE snippet.
 */
import { computed, ref } from "vue";
import {
  getRelayRulePrompt,
  getRelayRulePromptEn,
  getRelayRulePromptZh,
  type RulePromptMode,
} from "../../cursorRulesTemplates";
import { locale, t } from "../../i18n";
import { safeMarkdownToHtml } from "../../utils/safeMarkdown";

const props = defineProps<{
  strings: Record<string, string>;
  cursorMcpPath: string;
  windsurfMcpPath: string;
}>();

function qaMd(html: string) {
  return safeMarkdownToHtml(html);
}

const rulePromptMode = ref<RulePromptMode>("mild");
const rulesCopyToast = ref("");
const rulePromptEn = computed(() =>
  getRelayRulePromptEn(rulePromptMode.value, undefined),
);
const rulePromptZh = computed(() =>
  getRelayRulePromptZh(rulePromptMode.value, undefined),
);

const rulePromptViewEn = ref<"md" | "src">("md");
const rulePromptViewZh = ref<"md" | "src">("md");
const rulePromptEnHtml = computed(() => qaMd(rulePromptEn.value));
const rulePromptZhHtml = computed(() => qaMd(rulePromptZh.value));

function setRulePromptViewEn(mode: "md" | "src") {
  rulePromptViewEn.value = mode;
}
function setRulePromptViewZh(mode: "md" | "src") {
  rulePromptViewZh.value = mode;
}

const rulePromptsIdeHtml = computed(() => {
  void locale.value;
  return qaMd(
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
    await navigator.clipboard.writeText(
      getRelayRulePrompt(rulePromptMode.value, undefined),
    );
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
          <span class="cursorRulesModeTitle">{{
            S.rulePromptsModeMild
          }}</span>
          <span class="cursorRulesModeDesc">{{
            S.rulePromptsModeMildDesc
          }}</span>
        </button>
        <button
          type="button"
          class="cursorRulesModeBtn"
          :class="{ active: rulePromptMode === 'loop' }"
          role="radio"
          :aria-checked="rulePromptMode === 'loop'"
          @click="rulePromptMode = 'loop'"
        >
          <span class="cursorRulesModeTitle">{{
            S.rulePromptsModeLoop
          }}</span>
          <span class="cursorRulesModeDesc">{{
            S.rulePromptsModeLoopDesc
          }}</span>
        </button>
        <button
          type="button"
          class="cursorRulesModeBtn"
          :class="{ active: rulePromptMode === 'toolOnly' }"
          role="radio"
          :aria-checked="rulePromptMode === 'toolOnly'"
          @click="rulePromptMode = 'toolOnly'"
        >
          <span class="cursorRulesModeTitle">{{
            S.rulePromptsModeTool
          }}</span>
          <span class="cursorRulesModeDesc">{{
            S.rulePromptsModeToolDesc
          }}</span>
        </button>
      </div>

      <p v-if="rulePromptMode === 'loop'" class="note cursorRulesRisk">
        {{ S.rulePromptsLoopRisk }}
      </p>

      <div class="rulePromptBilingual">
        <div class="rulePromptLangRow">
          <p class="rulePromptLangLabel rulePromptLangLabel--row">
            {{ S.rulePromptsLabelEn }}
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
                :class="{ active: rulePromptViewEn === 'md' }"
                :aria-pressed="rulePromptViewEn === 'md'"
                :title="S.rulePromptsViewMd"
                @click="setRulePromptViewEn('md')"
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
                :class="{ active: rulePromptViewEn === 'src' }"
                :aria-pressed="rulePromptViewEn === 'src'"
                :title="S.rulePromptsViewSource"
                @click="setRulePromptViewEn('src')"
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
        <template v-if="rulePromptViewEn === 'md'">
          <div
            class="rulePromptMdBody qaRoundMd cursorRulesPre cursorRulesPre--prompt"
            tabindex="0"
            v-html="rulePromptEnHtml"
          />
        </template>
        <pre
          v-else
          class="cursorRulesPre cursorRulesPre--prompt"
          tabindex="0"
        >{{ rulePromptEn }}</pre>
        <div class="rulePromptLangRow rulePromptLangRow--zh">
          <p
            class="rulePromptLangLabel rulePromptLangLabel--row rulePromptLangLabel--zhHead"
          >
            {{ S.rulePromptsLabelZh }}
          </p>
          <div
            class="rulePromptViewToggles"
            role="group"
            :aria-label="S.rulePromptsToggleZhAria"
          >
            <button
              type="button"
              class="rulePromptViewIconBtn"
              :class="{ active: rulePromptViewZh === 'md' }"
              :aria-pressed="rulePromptViewZh === 'md'"
              :title="S.rulePromptsViewMd"
              @click="setRulePromptViewZh('md')"
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
              :class="{ active: rulePromptViewZh === 'src' }"
              :aria-pressed="rulePromptViewZh === 'src'"
              :title="S.rulePromptsViewSource"
              @click="setRulePromptViewZh('src')"
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
        </div>
        <template v-if="rulePromptViewZh === 'md'">
          <div
            class="rulePromptMdBody qaRoundMd cursorRulesPre cursorRulesPre--zh"
            tabindex="0"
            v-html="rulePromptZhHtml"
          />
        </template>
        <pre
          v-else
          class="cursorRulesPre cursorRulesPre--zh"
          tabindex="0"
        >{{ rulePromptZh }}</pre>
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
