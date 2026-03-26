<script setup lang="ts">
/**
 * Settings -> Rule prompts: default rule (installable) + tool spec (copy-only).
 */
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getRelayRulePromptBilingual } from "../../ideRulesTemplates";
import { locale, t } from "../../i18n";
import { safeMarkdownToHtml } from "../../utils/safeMarkdown";
import type { SettingsToastPayload } from "../../composables/useRelayCacheSettings";
import type { IdeKind } from "../../types/relay-app";
import type { LocaleKey } from "../../i18n";

const props = defineProps<{
  strings: Record<string, string>;
  ideLabel: string;
  ideKind: IdeKind | null;
  ideMcpPath: string;
  active: boolean;
  pushToast: (_p: SettingsToastPayload) => void;
}>();

const cliId = computed(() => props.ideKind || undefined);
const defaultRule = computed(() => getRelayRulePromptBilingual("loop", cliId.value));
const defaultRuleHtml = computed(() => safeMarkdownToHtml(defaultRule.value));
const toolOnlyRule = computed(() => getRelayRulePromptBilingual("toolOnly", cliId.value));
const toolOnlyRuleHtml = computed(() => safeMarkdownToHtml(toolOnlyRule.value));

const defaultView = ref<"md" | "src">("md");
const toolOnlyView = ref<"md" | "src">("md");
const defaultCopyToast = ref("");
const toolOnlyCopyToast = ref("");

const ideRuleInstalled = ref(false);
const ideRuleBusy = ref(false);

async function checkIdeRuleInstalled() {
  try {
    ideRuleInstalled.value = await invoke<boolean>("ide_rule_installed");
  } catch {
    ideRuleInstalled.value = false;
  }
}

async function installOrUpdateIdeRule() {
  if (ideRuleBusy.value) return;
  const wasInstalled = ideRuleInstalled.value;
  ideRuleBusy.value = true;
  try {
    await invoke("ide_install_rule", { content: defaultRule.value });
    ideRuleInstalled.value = true;
    const ide = props.ideLabel;
    const msg = wasInstalled ? t("rulePromptsUpdateOk", { ide }) : t("rulePromptsInstallOk", { ide });
    props.pushToast({ type: "ok", text: msg, durationMs: 3000 });
  } catch (e) {
    const detail = e instanceof Error ? e.message : String(e);
    props.pushToast({ type: "err", text: `${t("rulePromptsInstallErr")} ${detail}`, durationMs: 4000 });
  } finally {
    ideRuleBusy.value = false;
  }
}

async function removeIdeRule() {
  if (ideRuleBusy.value) return;
  ideRuleBusy.value = true;
  try {
    await invoke("ide_uninstall_rule");
    ideRuleInstalled.value = false;
    props.pushToast({ type: "ok", text: t("rulePromptsRemoveOk", { ide: props.ideLabel }), durationMs: 3000 });
  } catch (e) {
    const detail = e instanceof Error ? e.message : String(e);
    props.pushToast({ type: "err", text: `${t("rulePromptsRemoveErr")} ${detail}`, durationMs: 4000 });
  } finally {
    ideRuleBusy.value = false;
  }
}

onMounted(() => {
  void checkIdeRuleInstalled();
});

watch(() => props.active, (active) => {
  if (active) void checkIdeRuleInstalled();
});

function showCopyToast(target: "default" | "toolOnly") {
  const ref_ = target === "default" ? defaultCopyToast : toolOnlyCopyToast;
  ref_.value = t("rulePromptsCopied");
  setTimeout(() => { ref_.value = ""; }, 2500);
}

async function copyDefault() {
  try {
    await navigator.clipboard.writeText(defaultRule.value);
    showCopyToast("default");
  } catch {
    defaultCopyToast.value = t("rulePromptsCopyErr");
    setTimeout(() => { defaultCopyToast.value = ""; }, 2500);
  }
}

async function copyToolOnly() {
  try {
    await navigator.clipboard.writeText(toolOnlyRule.value);
    showCopyToast("toolOnly");
  } catch {
    toolOnlyCopyToast.value = t("rulePromptsCopyErr");
    setTimeout(() => { toolOnlyCopyToast.value = ""; }, 2500);
  }
}

const IDE_GUIDE_KEYS: Record<IdeKind, LocaleKey> = {
  cursor: "rulePromptsIdeGuideCursor",
  claude_code: "rulePromptsIdeGuideClaude",
  windsurf: "rulePromptsIdeGuideWindsurf",
  other: "rulePromptsIdeGuideOther",
};

const rulePromptsIdeHtml = computed(() => {
  void locale.value;
  const kind = props.ideKind;
  if (!kind) return "";
  const key = IDE_GUIDE_KEYS[kind];
  return safeMarkdownToHtml(t(key, { mcpPath: props.ideMcpPath || "" }));
});

const S = computed(() => props.strings);
</script>

<template>
  <div class="segPanel">
    <div class="installHubCard settingsCard rulePromptsCard">
      <h3 class="installHubTitle">{{ S.rulePromptsTitle }}</h3>
      <p class="installHubDesc">{{ S.rulePromptsLead }}</p>

      <!-- Default rule (loop) — installable -->
      <h4 class="rulePromptsSubhead">
        {{ S.rulePromptsModeLoop }}
      </h4>
      <p class="rulePromptsSectionDesc">{{ S.rulePromptsModeLoopDesc }}</p>

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
                :class="{ active: defaultView === 'md' }"
                :aria-pressed="defaultView === 'md'"
                :title="S.rulePromptsViewMd"
                @click="defaultView = 'md'"
              >
                <svg class="rulePromptViewIconSvg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                  <path d="M14 2v6h6M16 13H8M16 17H8M10 9H8" />
                </svg>
              </button>
              <button
                type="button"
                class="rulePromptViewIconBtn"
                :class="{ active: defaultView === 'src' }"
                :aria-pressed="defaultView === 'src'"
                :title="S.rulePromptsViewSource"
                @click="defaultView = 'src'"
              >
                <svg class="rulePromptViewIconSvg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <polyline points="16 18 22 12 16 6" />
                  <polyline points="8 6 2 12 8 18" />
                </svg>
              </button>
            </div>
            <div class="rulePromptCopyActions">
              <span v-if="defaultCopyToast" class="copyToast rulePromptCopyToast">{{ defaultCopyToast }}</span>
              <button type="button" class="secondary rulePromptsCopyBtn" @click="copyDefault">
                {{ S.rulePromptsCopy }}
              </button>
              <button
                type="button"
                class="usageBtn usageBtn--primary rulePromptsCursorBtn"
                :disabled="ideRuleBusy"
                @click="installOrUpdateIdeRule"
              >
                {{ ideRuleInstalled ? S.rulePromptsUpdateCursor : S.rulePromptsInstallCursor }}
              </button>
              <button
                v-if="ideRuleInstalled"
                type="button"
                class="usageBtn usageBtn--ghost rulePromptsCursorBtn"
                :disabled="ideRuleBusy"
                @click="removeIdeRule"
              >
                {{ S.rulePromptsRemoveCursor }}
              </button>
              <span v-if="ideRuleInstalled" class="rulePromptsInstalledBadge">{{ S.rulePromptsInstalledBadge }}</span>
            </div>
          </div>
        </div>
        <template v-if="defaultView === 'md'">
          <div
            class="rulePromptMdBody qaRoundMd cursorRulesPre cursorRulesPre--prompt"
            tabindex="0"
            v-html="defaultRuleHtml"
          />
        </template>
        <pre v-else class="cursorRulesPre cursorRulesPre--prompt" tabindex="0">{{ defaultRule }}</pre>
      </div>

      <!-- Tool spec only — copy/preview only -->
      <h4 class="rulePromptsSubhead rulePromptsSubhead--spaced">
        {{ S.rulePromptsModeTool }}
      </h4>
      <p class="rulePromptsSectionDesc">{{ S.rulePromptsModeToolDesc }}</p>

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
                :class="{ active: toolOnlyView === 'md' }"
                :aria-pressed="toolOnlyView === 'md'"
                :title="S.rulePromptsViewMd"
                @click="toolOnlyView = 'md'"
              >
                <svg class="rulePromptViewIconSvg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                  <path d="M14 2v6h6M16 13H8M16 17H8M10 9H8" />
                </svg>
              </button>
              <button
                type="button"
                class="rulePromptViewIconBtn"
                :class="{ active: toolOnlyView === 'src' }"
                :aria-pressed="toolOnlyView === 'src'"
                :title="S.rulePromptsViewSource"
                @click="toolOnlyView = 'src'"
              >
                <svg class="rulePromptViewIconSvg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <polyline points="16 18 22 12 16 6" />
                  <polyline points="8 6 2 12 8 18" />
                </svg>
              </button>
            </div>
            <div class="rulePromptCopyActions">
              <span v-if="toolOnlyCopyToast" class="copyToast rulePromptCopyToast">{{ toolOnlyCopyToast }}</span>
              <button type="button" class="secondary rulePromptsCopyBtn" @click="copyToolOnly">
                {{ S.rulePromptsCopy }}
              </button>
            </div>
          </div>
        </div>
        <template v-if="toolOnlyView === 'md'">
          <div
            class="rulePromptMdBody qaRoundMd cursorRulesPre cursorRulesPre--prompt"
            tabindex="0"
            v-html="toolOnlyRuleHtml"
          />
        </template>
        <pre v-else class="cursorRulesPre cursorRulesPre--prompt" tabindex="0">{{ toolOnlyRule }}</pre>
      </div>

      <h4 v-if="rulePromptsIdeHtml" class="rulePromptsSubhead rulePromptsSubhead--spaced">
        {{ S.rulePromptsSectionIde }}
      </h4>
      <div
        v-if="rulePromptsIdeHtml"
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
