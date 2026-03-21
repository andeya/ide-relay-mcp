<script setup lang="ts">
/**
 * Relay GUI: Q&A (retell + Answer), composer, settings / MCP install.
 */
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { invoke } from "@tauri-apps/api/core";
import { locale, t } from "./i18n";
import { useAppStrings } from "./composables/useAppStrings";
import { useFeedbackWindow } from "./composables/useFeedbackWindow";
import { useReleaseBadge } from "./composables/useReleaseBadge";
import { useMcpAndPathSettings } from "./composables/useMcpAndPathSettings";
import type { CommandItem, SettingsSegment } from "./types/relay-app";
import type { SettingsToastPayload } from "./composables/useRelayCacheSettings";
import SettingsCachePanel from "./components/settings/SettingsCachePanel.vue";
import SettingsRulePromptsPanel from "./components/settings/SettingsRulePromptsPanel.vue";
import relayLogoUrl from "./assets/relay-logo.svg?url";
import QaUserSubmittedBubble from "./components/QaUserSubmittedBubble.vue";
import RelayComposerInput from "./components/RelayComposerInput.vue";
import {
  slashCommandSecondaryLine,
  slashItemDetailPreview,
} from "./composables/feedbackComposerUtils";
import { qaRoundHasRenderableUserContent } from "./utils/parseRelayFeedbackReply";
import { safeMarkdownToHtml } from "./utils/safeMarkdown";

const lightboxSrc = ref<string | null>(null);
const windowDock = ref<"left" | "center" | "right">("left");
const mcpPaused = ref(false);
const mcpPauseBusy = ref(false);
const mcpPauseErr = ref("");
const qaScrollEndRef = ref<HTMLElement | null>(null);

const {
  payload: releasePayload,
  loading: releaseLoading,
  openRepo,
  badgeTitle,
} = useReleaseBadge();

const releaseLabel = computed(() => {
  void locale.value;
  const p = releasePayload.value;
  if (!p) return "";
  if (p.update_available && p.latest_version) {
    return t("releaseBadgeUpdate", { latest: p.latest_version });
  }
  return t("releaseBadgeCurrent", { current: p.current_version });
});

const showReleaseBadge = computed(
  () => !releaseLoading.value && releasePayload.value !== null,
);

function openLightbox(src: string) {
  lightboxSrc.value = src;
}
function closeLightbox() {
  lightboxSrc.value = null;
}
function qaMd(html: string) {
  return safeMarkdownToHtml(html);
}

/** Show `/id` in slash menu; `name` is for IDE display elsewhere, not the palette primary line. */
function slashMenuLabel(cmd: CommandItem) {
  const n = (cmd.id ?? cmd.name ?? "").trim();
  if (!n) return "/";
  return n.startsWith("/") ? n : `/${n}`;
}

function slashMenuCategoryLabel(raw: string | undefined) {
  const c = (raw ?? "").trim();
  if (!c) return "";
  if (/^agent_skill$/i.test(c)) return t("slashCategoryAgentSkill");
  return c;
}

const {
  isHubPage,
  tabs,
  activeTabId,
  hasRealTabs,
  tabLabel,
  selectTab,
  flashingTabIds,
  feedback,
  bindRelayComposerRef,
  pendingImages,
  pendingFileDrops,
  dragActive,
  loading,
  error,
  submitting,
  expired,
  composerDrafting,
  composerSwallowPlainEnter,
  hasPendingFileDropErrors,
  status,
  statusLabel,
  statusPillUi,
  submit,
  requestCloseTab,
  onDragOver,
  onDragLeave,
  onDrop,
  onComposerPaste,
  onKeydown,
  onComposerCompositionStart,
  onComposerCompositionEnd,
  onComposerCaretHead,
  onComposerScroll,
  slashOpen,
  slashDropdownRef,
  slashSelectedIndex,
  filteredCommands,
  hasSlashList,
  insertSlashCommand,
  initAfterLocale,
  setWindowTitle,
  qaRounds,
  addAttachedFilesFromPicker,
  removePendingImage,
  removePendingFileDrop,
} = useFeedbackWindow();

/** One `slashCommandSecondaryLine` eval per row (description, or non-redundant name). */
const slashPaletteRows = computed(() =>
  filteredCommands.value.map((cmd, index) => ({
    cmd,
    index,
    secondary: slashCommandSecondaryLine(cmd),
  })),
);

const slashA11yPopupId = computed(() =>
  slashOpen.value && !expired.value && !isHubPage.value
    ? "relay-slash-listbox"
    : null,
);

const slashA11yActiveId = computed(() => {
  if (!slashOpen.value || expired.value || isHubPage.value) return null;
  if (filteredCommands.value.length === 0) return "slash-cmd-empty";
  return `slash-cmd-${slashSelectedIndex.value}`;
});

function pendingFileChipLabel(
  fd: { path: string; name: string } | { file: File },
): string {
  return "file" in fd ? fd.file.name || "file" : fd.name;
}
function pendingFileChipTitle(
  fd: { path: string; name: string } | { file: File },
): string {
  return "file" in fd ? fd.file.name || "" : fd.path;
}

const attachInputRef = ref<HTMLInputElement | null>(null);

function onAttachChange(e: Event) {
  const t = e.target as HTMLInputElement;
  if (t.files?.length) {
    addAttachedFilesFromPicker(t.files);
  }
  t.value = "";
}

const summaryScrollRef = ref<HTMLElement | null>(null);

function scrollQaToBottom() {
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      const end = qaScrollEndRef.value;
      const el = summaryScrollRef.value;
      if (end) {
        end.scrollIntoView({ block: "end", behavior: "instant" });
      } else if (el) {
        el.scrollTop = el.scrollHeight;
      }
    });
  });
}

watch(
  qaRounds,
  async () => {
    await nextTick();
    scrollQaToBottom();
  },
  { deep: true, immediate: true },
);

const {
  mcpJson,
  mcpCursorInstalled,
  mcpCursorReason,
  cursorMcpPath,
  mcpWindsurfInstalled,
  mcpWindsurfReason,
  windsurfMcpPath,
  mcpWindsurfBusy,
  hubMsg,
  hubErr,
  hubInstallBusy,
  hubUninstallBusy,
  mcpCursorBusy,
  copyToast,
  pathEnv,
  pathEnvMsg,
  pathEnvErr,
  pathEnvBusy,
  ideHintsBlock,
  refreshMcpHub,
  copyMcpJson,
  doFullInstall,
  runFullUninstall,
  installCursorMcpOnly,
  uninstallCursorMcpOnly,
  installWindsurfMcpOnly,
  uninstallWindsurfMcpOnly,
  configureRelayPath,
} = useMcpAndPathSettings();

const { strings } = useAppStrings();

const showUninstallConfirm = ref(false);

function onUninstallClick() {
  showUninstallConfirm.value = true;
}

function cancelUninstallConfirm() {
  showUninstallConfirm.value = false;
}

async function confirmAndRunUninstall() {
  showUninstallConfirm.value = false;
  await runFullUninstall();
}

/** PATH + Cursor MCP + Windsurf MCP all OK — hide primary install. */
const setupAllConfigured = computed(
  () =>
    Boolean(
      pathEnv.value?.configured &&
        mcpCursorInstalled.value &&
        mcpWindsurfInstalled.value,
    ),
);

const setupAnythingInstalled = computed(() =>
  Boolean(
    pathEnv.value &&
      (pathEnv.value.configured ||
        mcpCursorInstalled.value ||
        mcpWindsurfInstalled.value),
  ),
);

const uiView = ref<"main" | "settings">("main");
watch(hasRealTabs, (v) => {
  if (v) uiView.value = "main";
});
const settingsSeg = ref<SettingsSegment>("setup");
const settingsCheckBusy = ref(false);
const settingsRefreshToast = ref<{
  type: "ok" | "warn" | "err";
  text: string;
} | null>(null);
let settingsRefreshToastTimer: ReturnType<typeof setTimeout> | undefined;

function pushSettingsToast(p: SettingsToastPayload) {
  if (settingsRefreshToastTimer) clearTimeout(settingsRefreshToastTimer);
  settingsRefreshToast.value = { type: p.type, text: p.text };
  settingsRefreshToastTimer = setTimeout(() => {
    settingsRefreshToast.value = null;
    settingsRefreshToastTimer = undefined;
  }, p.durationMs ?? 4500);
}

const cacheSegmentActive = computed(() => settingsSeg.value === "cache");

async function openSettings() {
  uiView.value = "settings";
  mcpPauseErr.value = "";
  void refreshMcpPaused();
  settingsCheckBusy.value = true;
  try {
    await refreshMcpHub();
  } finally {
    settingsCheckBusy.value = false;
  }
}

async function checkInstallStatus() {
  if (settingsCheckBusy.value) return;
  const t0 = Date.now();
  settingsCheckBusy.value = true;
  if (settingsRefreshToastTimer) clearTimeout(settingsRefreshToastTimer);
  settingsRefreshToast.value = null;
  let toast: { type: "ok" | "warn" | "err"; text: string } | null = null;
  try {
    const r = await refreshMcpHub();
    if (!r.ok) {
      const detail = r.fatalError?.trim();
      toast = {
        type: "err",
        text: detail
          ? `${t("settingsRefreshFail")} ${detail}`
          : t("settingsRefreshFail"),
      };
    } else if (r.mcpConfigReadFailed) {
      toast = { type: "warn", text: t("settingsRefreshWarn") };
    } else {
      toast = { type: "ok", text: t("settingsRefreshOk") };
    }
  } catch (e) {
    const detail = e instanceof Error ? e.message : String(e);
    toast = {
      type: "err",
      text: `${t("settingsRefreshFail")} ${detail}`.trim(),
    };
  } finally {
    const minMs = 420;
    const elapsed = Date.now() - t0;
    if (elapsed < minMs) {
      await new Promise((res) => setTimeout(res, minMs - elapsed));
    }
    settingsCheckBusy.value = false;
    settingsRefreshToast.value = toast;
    settingsRefreshToastTimer = setTimeout(() => {
      settingsRefreshToast.value = null;
      settingsRefreshToastTimer = undefined;
    }, 2800);
  }
}

function closeSettings() {
  uiView.value = "main";
}

function onGlobalKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    if (lightboxSrc.value) {
      closeLightbox();
      return;
    }
    if (uiView.value === "settings") {
      closeSettings();
    }
  }
}

async function refreshMcpPaused() {
  try {
    mcpPaused.value = await invoke<boolean>("get_mcp_paused");
  } catch {
    /* ignore */
  }
}

async function onMcpPausedChange() {
  mcpPauseErr.value = "";
  const next = mcpPaused.value;
  mcpPauseBusy.value = true;
  try {
    await invoke("set_mcp_paused", { paused: next });
  } catch (err) {
    mcpPaused.value = !next;
    const detail = err instanceof Error ? err.message : String(err);
    mcpPauseErr.value = detail
      ? `${t("mcpPauseUpdateErr")} (${detail})`
      : t("mcpPauseUpdateErr");
  } finally {
    mcpPauseBusy.value = false;
  }
}

async function applyWindowDock(d: "left" | "center" | "right") {
  try {
    await invoke("set_window_dock", { dock: d });
    windowDock.value = d;
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
}

async function setLang(code: "en" | "zh") {
  if (locale.value === code) return;
  locale.value = code;
  try {
    await invoke("set_ui_locale", { lang: code });
  } catch {
    /* Keep UI locale if disk write fails. */
  }
  await setWindowTitle();
}

onMounted(async () => {
  try {
    const saved = await invoke<string>("get_ui_locale");
    if (saved === "zh") {
      locale.value = "zh";
    }
    await initAfterLocale();
    try {
      const d = await invoke<string>("get_window_dock");
      if (d === "center" || d === "right") windowDock.value = d;
      else windowDock.value = "left";
    } catch {
      /* ignore */
    }
    await refreshMcpPaused();
    window.addEventListener("keydown", onGlobalKeydown);
  } catch (err) {
    loading.value = false;
    error.value = err instanceof Error ? err.message : String(err);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onGlobalKeydown);
  if (settingsRefreshToastTimer) clearTimeout(settingsRefreshToastTimer);
});
</script>

<template>
  <main
    class="shell"
    :class="{
      dragActive,
      settingsOpen: uiView === 'settings',
      shellMainFill: uiView === 'main',
    }"
    @dragover="onDragOver"
    @dragleave="onDragLeave"
    @drop="onDrop"
  >
    <!-- Main: feedback-first layout -->
    <section v-show="uiView === 'main'" class="panel panelMain mainWork">
      <header class="mainTopBar">
        <div class="mainTopBarLeft">
          <div class="mainBrandCluster">
            <span class="mainBrandLogoWrap">
              <img
                class="mainBrandLogo"
                :src="relayLogoUrl"
                width="24"
                height="24"
                alt=""
                aria-hidden="true"
              />
            </span>
            <h1 class="mainTitle">{{ strings.appTitle }}</h1>
          </div>
        </div>
        <div class="mainTopBarMid">
          <span
            class="statusPill mainTopBarStatusPill"
            :class="[
              { 'mainTopBarStatusPill--waiting': statusPillUi.indeterminate },
              statusPillUi.hue !== 'default'
                ? 'mainTopBarStatusPill--hue-' + statusPillUi.hue
                : '',
            ]"
            role="status"
            :aria-busy="statusPillUi.indeterminate"
            :title="statusLabel"
          >
            <span
              v-if="statusPillUi.indeterminate && statusPillUi.hue !== 'hub'"
              class="statusPillWaitRing"
              aria-hidden="true"
            />
            <span class="mainTopBarStatusPillText">{{ statusLabel }}</span>
          </span>
        </div>
        <div class="mainTopBarRight">
          <div
            class="dockSeg"
            role="group"
            :aria-label="strings.windowDockAria"
          >
            <button
              type="button"
              class="dockSegBtn"
              :class="{ active: windowDock === 'left' }"
              :aria-pressed="windowDock === 'left'"
              :title="strings.windowDockLeft"
              @click="applyWindowDock('left')"
            >
              {{ strings.dockBtnLeft }}
            </button>
            <button
              type="button"
              class="dockSegBtn"
              :class="{ active: windowDock === 'center' }"
              :aria-pressed="windowDock === 'center'"
              :title="strings.windowDockCenter"
              @click="applyWindowDock('center')"
            >
              {{ strings.dockBtnCenter }}
            </button>
            <button
              type="button"
              class="dockSegBtn"
              :class="{ active: windowDock === 'right' }"
              :aria-pressed="windowDock === 'right'"
              :title="strings.windowDockRight"
              @click="applyWindowDock('right')"
            >
              {{ strings.dockBtnRight }}
            </button>
          </div>
          <button
            v-if="showReleaseBadge"
            type="button"
            class="releaseBadge"
            :class="{
              'releaseBadge--update': releasePayload?.update_available,
            }"
            :title="badgeTitle"
            :aria-label="t('releaseBadgeAria')"
            @click="openRepo"
          >
            <span class="releaseBadgeDot" aria-hidden="true" />
            {{ releaseLabel }}
          </button>
          <button
            type="button"
            class="iconGear"
            :aria-label="strings.ariaOpenSettings"
            :title="strings.ariaOpenSettings"
            @click="openSettings"
          >
            <svg
              class="iconGearSvg"
              xmlns="http://www.w3.org/2000/svg"
              fill="none"
              viewBox="0 0 24 24"
              stroke-width="1.5"
              stroke="currentColor"
              aria-hidden="true"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 010 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.37.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.37-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z"
              />
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
              />
            </svg>
          </button>
        </div>
      </header>

      <div
        v-if="tabs.length > 0"
        class="tabStrip"
        role="tablist"
        :aria-label="strings.tabStripAria"
      >
        <div
          v-for="tab in tabs"
          :key="tab.tab_id"
          class="tabStripCell"
        >
          <button
            type="button"
            role="tab"
            class="tabBtn"
            :class="{
              active: tab.tab_id === activeTabId,
              tabBtnFlash: flashingTabIds.has(tab.tab_id),
            }"
            :aria-selected="tab.tab_id === activeTabId"
            @click="selectTab(tab.tab_id)"
          >
            {{ tabLabel(tab) }}
          </button>
          <button
            type="button"
            class="tabCloseBadge"
            :aria-label="strings.tabCloseAria"
            :title="strings.tabCloseTitle"
            @click.stop="requestCloseTab(tab)"
          >
            <svg
              class="tabCloseIcon"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
              stroke-linecap="round"
              aria-hidden="true"
            >
              <path d="M18 6L6 18M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>

      <div class="mainColumn mainColumnAgent">
        <div class="mainContextZone mainContextZoneScroll">
          <div
            v-if="qaRounds.length > 0"
            ref="summaryScrollRef"
            class="mainSummaryScroll"
            tabindex="0"
            role="region"
            :aria-label="strings.qaHistoryTitle"
          >
            <article
              v-for="(round, idx) in qaRounds"
              :key="'q' + idx"
              class="qaRoundCard"
            >
              <div class="qaChatRow qaChatRow--ai">
                <div class="qaChatStack qaChatStack--ai">
                  <span class="qaChatRole qaChatRole--ai">{{
                    strings.qaAssistantTurn
                  }}</span>
                  <div
                    class="qaChatBubble qaChatBubble--ai"
                    :class="{
                      'qaChatBubble--aiPlaceholder':
                        isHubPage && idx === 0 && qaRounds.length === 1,
                    }"
                  >
                    <div
                      v-if="round.retell?.trim()"
                      class="qaRoundAgentScroll qaRoundAgentScroll--bubble"
                    >
                      <div
                        class="qaRoundMd qaRoundMd--agent"
                        v-html="qaMd(round.retell)"
                      />
                    </div>
                    <p v-else class="qaChatBubblePlaceholder">
                      {{ strings.qaNoRetellYet }}
                    </p>
                  </div>
                </div>
              </div>

              <div
                v-if="round.submitted && round.skipped"
                class="qaChatRow qaChatRow--me"
              >
                <div class="qaChatStack qaChatStack--me">
                  <span class="qaChatRole qaChatRole--me">{{
                    strings.qaUserTurnMe
                  }}</span>
                  <div class="qaChatBubble qaChatBubble--me qaChatBubble--meMuted">
                    <p class="qaRoundMuted">{{ strings.qaSkipped }}</p>
                  </div>
                </div>
              </div>
              <div
                v-else-if="round.submitted && qaRoundHasRenderableUserContent(round)"
                class="qaChatRow qaChatRow--me"
              >
                <div class="qaChatStack qaChatStack--me">
                  <span class="qaChatRole qaChatRole--me">{{
                    strings.qaUserTurnMe
                  }}</span>
                  <div class="qaChatBubble qaChatBubble--me">
                    <QaUserSubmittedBubble
                      :round="round"
                      :zoom-title="strings.composerImageZoomTitle"
                      @preview="openLightbox"
                    />
                  </div>
                </div>
              </div>
              <div
                v-else-if="round.submitted"
                class="qaChatRow qaChatRow--me"
              >
                <div class="qaChatStack qaChatStack--me">
                  <span class="qaChatRole qaChatRole--me">{{
                    strings.qaUserTurnMe
                  }}</span>
                  <div class="qaChatBubble qaChatBubble--me qaChatBubble--meMuted">
                    <p class="qaRoundMuted">{{ strings.qaEmptySubmit }}</p>
                  </div>
                </div>
              </div>
            </article>
            <div
              ref="qaScrollEndRef"
              class="qaScrollEndAnchor"
              aria-hidden="true"
            />
          </div>
          <div v-else class="mainSummaryScroll mainSummaryScroll--empty">
            <p class="mainSummaryPlaceholder">{{ loading ? strings.loading : strings.noLaunch }}</p>
          </div>
        </div>

        <div class="mainFooterFixed">
          <div class="mainFeedbackZone">
            <div
              class="composerShell"
              :class="{ composerShellDrag: dragActive }"
            >
              <div
                class="composerCard"
                role="region"
                :aria-label="strings.composerAriaRegion"
              >
                <div
                  v-if="
                    (pendingImages.length || pendingFileDrops.length) && !expired
                  "
                  class="composerThumbRow"
                  :aria-label="strings.composerImageAria"
                >
                  <div
                    v-for="img in pendingImages"
                    :key="img.id"
                    class="composerThumbWrap"
                  >
                    <img
                      class="composerThumb composerThumb--zoom"
                      :src="img.previewUrl"
                      alt=""
                      :title="strings.composerImageZoomTitle"
                      @click="openLightbox(img.previewUrl)"
                    />
                    <button
                      type="button"
                      class="composerThumbRemove"
                      :aria-label="strings.composerThumbRemove"
                      @click.stop="removePendingImage(img.id)"
                    >
                      <svg
                        class="composerThumbRemoveSvg"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.5"
                        stroke-linecap="round"
                        aria-hidden="true"
                      >
                        <path d="M18 6 6 18M6 6l12 12" />
                      </svg>
                    </button>
                  </div>
                  <div
                    v-for="fd in pendingFileDrops"
                    :key="fd.id"
                    class="composerThumbWrap composerFileDropWrap"
                    :title="pendingFileChipTitle(fd)"
                  >
                    <div
                      class="composerFileDropChip"
                      :class="{
                        'composerFileDropChip--error': 'error' in fd && fd.error,
                      }"
                      :aria-label="strings.composerFileDropAria"
                    >
                      <svg
                        class="composerFileDropIcon"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.75"
                        aria-hidden="true"
                      >
                        <path
                          d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"
                        />
                        <path d="M14 2v6h6" />
                      </svg>
                      <span class="composerFileDropName">{{
                        pendingFileChipLabel(fd)
                      }}</span>
                      <span
                        v-if="'error' in fd && fd.error"
                        class="composerFileDropErr"
                        >{{ fd.error }}</span
                      >
                    </div>
                    <button
                      type="button"
                      class="composerThumbRemove"
                      :aria-label="strings.composerFileDropRemove"
                      @click.stop="removePendingFileDrop(fd.id)"
                    >
                      <svg
                        class="composerThumbRemoveSvg"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.5"
                        stroke-linecap="round"
                        aria-hidden="true"
                      >
                        <path d="M18 6 6 18M6 6l12 12" />
                      </svg>
                    </button>
                  </div>
                </div>
                <div class="composerTextareaWrap">
                  <RelayComposerInput
                    :ref="bindRelayComposerRef"
                    v-model="feedback"
                    :readonly="expired || isHubPage"
                    :placeholder="strings.placeholder"
                    :swallow-plain-enter="composerSwallowPlainEnter"
                    :slash-active-descendant-id="slashA11yActiveId"
                    :slash-comb-popup-id="slashA11yPopupId"
                    :has-thumbs="
                      !!(pendingImages.length || pendingFileDrops.length) &&
                      !expired
                    "
                    @paste="onComposerPaste"
                    @keydown="onKeydown"
                    @scroll="onComposerScroll"
                    @compositionstart="onComposerCompositionStart"
                    @compositionend="onComposerCompositionEnd"
                    @caret-head="onComposerCaretHead"
                  />
                  <div
                    v-if="slashOpen && !expired && !isHubPage"
                    id="relay-slash-listbox"
                    class="slashDropdown"
                    role="listbox"
                    aria-label="Commands"
                  >
                    <div ref="slashDropdownRef" class="slashDropdownList">
                      <div
                        v-for="row in slashPaletteRows"
                        :id="'slash-cmd-' + row.index"
                        :key="row.cmd.id || 'slash-' + row.index"
                        role="option"
                        :aria-selected="row.index === slashSelectedIndex"
                        class="slashDropdownItem"
                        :class="{
                          slashDropdownItemActive:
                            row.index === slashSelectedIndex,
                        }"
                        @mousedown.prevent
                        @click="insertSlashCommand(row.cmd)"
                      >
                        <div class="slashDropdownItemHead">
                          <span
                            v-if="slashMenuCategoryLabel(row.cmd.category)"
                            class="slashDropdownItemCategory"
                          >{{
                            slashMenuCategoryLabel(row.cmd.category)
                          }}</span>
                          <span class="slashDropdownItemName">{{
                            slashMenuLabel(row.cmd)
                          }}</span>
                        </div>
                        <span
                          v-if="row.secondary"
                          class="slashDropdownItemDesc"
                          :title="row.secondary"
                        >
                          {{ slashItemDetailPreview(row.secondary) }}
                        </span>
                      </div>
                      <div
                        v-if="filteredCommands.length === 0"
                        id="slash-cmd-empty"
                        class="slashDropdownItem slashDropdownItem--empty slashDropdownEmptyState"
                        role="option"
                        aria-disabled="true"
                      >
                        <span class="slashDropdownEmptyText">{{
                          hasSlashList ? strings.slashNoMatch : strings.slashNoCommandsForSession
                        }}</span>
                      </div>
                    </div>
                    <div class="slashDropdownFooter">
                      <span class="slashDropdownHint">{{ strings.slashDropdownHint }}</span>
                    </div>
                  </div>
                </div>
                <div
                  v-if="!expired"
                  class="composerFooterBar composerFooterBar--chat"
                >
                  <div class="composerFooterHintCol">
                    <p class="composerFooterHintMain">
                      {{
                        isHubPage
                          ? strings.mainHintPreview
                          : composerDrafting
                            ? strings.composerHintDraft
                            : strings.composerHint
                      }}
                    </p>
                    <p
                      v-if="!isHubPage && status === 'active'"
                      class="composerFooterHintIde"
                    >
                      {{ strings.ideBlockingHint }}
                    </p>
                  </div>
                  <div class="composerFooterActions">
                    <input
                      ref="attachInputRef"
                      type="file"
                      class="srOnly"
                      multiple
                      @change="onAttachChange"
                    />
                    <button
                      type="button"
                      class="composerIconBtn composerIconBtnAttach"
                      :title="strings.composerAttach"
                      :aria-label="strings.composerAttach"
                      :disabled="isHubPage"
                      @click="attachInputRef?.click()"
                    >
                      <svg
                        class="composerIconSvg"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        aria-hidden="true"
                      >
                        <path
                          d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66L9.64 16.76a2 2 0 0 1-2.83-2.83l8.49-8.49"
                        />
                      </svg>
                    </button>
                    <button
                      type="button"
                      class="composerIconBtn composerIconBtnSend"
                      :class="{ 'composerIconBtnSend--busy': submitting }"
                      :title="
                        submitting
                          ? strings.composerSubmitting
                          : hasPendingFileDropErrors
                            ? strings.composerSubmitBlockedFileError
                            : isHubPage
                              ? strings.composerSubmitDisabledPreview
                              : composerDrafting
                                ? strings.composerSubmitDisabledIdle
                                : strings.composerSubmitIconTitle
                      "
                      :aria-label="
                        submitting
                          ? strings.composerSubmitting
                          : hasPendingFileDropErrors
                            ? strings.composerSubmitBlockedFileError
                            : isHubPage
                              ? strings.composerSubmitDisabledPreview
                              : composerDrafting
                                ? strings.composerSubmitDisabledIdle
                                : strings.composerSubmitIconAria
                      "
                      :disabled="
                        isHubPage ||
                        (!isHubPage && composerDrafting) ||
                        hasPendingFileDropErrors
                      "
                      :aria-busy="submitting"
                      @click="submit(false)"
                    >
                      <span
                        v-if="submitting"
                        class="composerSendSpinner"
                        aria-hidden="true"
                      />
                      <svg
                        v-else
                        class="composerIconSvg composerIconSvg--send"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.25"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        aria-hidden="true"
                      >
                        <path d="M12 5v14M5 12l7-7 7 7" />
                      </svg>
                    </button>
                  </div>
                </div>
              </div>
            </div>
            <p v-if="expired" class="note mainNote">{{ strings.noteExpired }}</p>
            <p v-if="error" class="error mainError">{{ error }}</p>
          </div>
        </div>
      </div>
    </section>

    <!-- Settings: segmented -->
    <section v-show="uiView === 'settings'" class="panel panelSettings">
      <header class="settingsTop">
        <button type="button" class="settingsBackBtn" @click="closeSettings">
          ← {{ strings.settingsBack }}
        </button>
        <h2 class="settingsPageTitle">{{ strings.settingsTitle }}</h2>
        <div class="settingsTopActions">
          <div
            class="langToggle langToggleHeader"
            role="group"
            :aria-label="strings.settingsLangAria"
          >
            <button
              type="button"
              class="langBtn"
              :class="{ active: locale === 'en' }"
              @click="setLang('en')"
            >
              EN
            </button>
            <button
              type="button"
              class="langBtn"
              :class="{ active: locale === 'zh' }"
              @click="setLang('zh')"
            >
              中文
            </button>
          </div>
          <button
            type="button"
            class="secondary settingsCheckBtn settingsCheckBtn--icon"
            :class="{ 'settingsCheckBtn--busy': settingsCheckBusy }"
            :disabled="settingsCheckBusy"
            :title="strings.settingsCheckStatus"
            :aria-label="strings.settingsCheckStatus"
            :aria-busy="settingsCheckBusy"
            @click="checkInstallStatus"
          >
            <span class="settingsCheckIcon" aria-hidden="true">↻</span>
            <span class="srOnly">{{
              settingsCheckBusy ? strings.settingsChecking : strings.settingsCheckStatus
            }}</span>
          </button>
        </div>
      </header>

      <nav class="segBar" role="tablist" :aria-label="strings.settingsTitle">
        <button
          type="button"
          role="tab"
          class="segTab"
          :class="{ active: settingsSeg === 'setup' }"
          :aria-selected="settingsSeg === 'setup'"
          @click="settingsSeg = 'setup'"
        >
          {{ strings.segSetup }}
        </button>
        <button
          type="button"
          role="tab"
          class="segTab"
          :class="{ active: settingsSeg === 'rulePrompts' }"
          :aria-selected="settingsSeg === 'rulePrompts'"
          @click="
            settingsSeg = 'rulePrompts';
            refreshMcpHub();
          "
        >
          {{ strings.segRulePrompts }}
        </button>
        <button
          type="button"
          role="tab"
          class="segTab"
          :class="{ active: settingsSeg === 'cache' }"
          :aria-selected="settingsSeg === 'cache'"
          @click="settingsSeg = 'cache'"
        >
          {{ strings.segCache }}
        </button>
      </nav>

      <div class="settingsPane">
        <div v-show="settingsSeg === 'setup'" class="segPanel">
          <div class="setupPage">
            <header class="setupHero">
              <h3 class="setupHeroTitle">{{ strings.setupTitle }}</h3>
              <p
                class="setupHeroLead"
                :class="{ 'setupHeroLead--ok': setupAllConfigured }"
              >
                {{
                  setupAllConfigured
                    ? strings.setupAllReadyLead
                    : strings.setupLead
                }}
              </p>
            </header>

            <section
              v-if="pathEnv"
              class="setupActionsStrip"
              :aria-label="strings.setupActionsAria"
            >
              <div class="setupActionsStripHead">
                <div class="setupActionsStripCopy">
                  <p
                    v-if="setupAllConfigured"
                    class="setupActionsStripLead setupActionsStripLead--ok"
                  >
                    {{ strings.setupNoInstallNeeded }}
                  </p>
                  <template v-else>
                    <p class="setupActionsStripLead">
                      {{ strings.setupActionsStripNeedInstall }}
                    </p>
                    <p
                      v-if="!setupAnythingInstalled"
                      class="setupActionsStripSub"
                    >
                      {{ strings.setupUninstallOnlyHint }}
                    </p>
                  </template>
                </div>
                <div class="setupActionsStripBtns">
                  <button
                    v-if="!setupAllConfigured"
                    type="button"
                    class="primary setupInstallBtnCompact"
                    :class="{ btnWithWait: hubInstallBusy }"
                    :disabled="hubInstallBusy || hubUninstallBusy"
                    :aria-busy="hubInstallBusy"
                    @click="doFullInstall"
                  >
                    <span
                      v-if="hubInstallBusy"
                      class="btnInlineSpinner"
                      aria-hidden="true"
                    />
                    {{
                      hubInstallBusy
                        ? strings.mcpBusyInstallingAll
                        : strings.setupBtnInstall
                    }}
                  </button>
                  <button
                    v-if="setupAnythingInstalled"
                    type="button"
                    class="setupUninstallBtnCompact"
                    :class="{ btnWithWait: hubUninstallBusy }"
                    :disabled="
                      hubInstallBusy ||
                      hubUninstallBusy ||
                      showUninstallConfirm
                    "
                    :aria-busy="hubUninstallBusy"
                    @click="onUninstallClick"
                  >
                    <span
                      v-if="hubUninstallBusy"
                      class="btnInlineSpinner"
                      aria-hidden="true"
                    />
                    {{
                      hubUninstallBusy
                        ? strings.mcpBusyUninstallingAll
                        : strings.setupBtnUninstall
                    }}
                  </button>
                </div>
              </div>
              <div
                v-if="showUninstallConfirm"
                class="uninstallConfirmBar"
                role="dialog"
                aria-modal="true"
                :aria-label="strings.setupBtnUninstall"
              >
                <p class="uninstallConfirmText">
                  {{ strings.mcpFullUninstallConfirm }}
                </p>
                <div class="uninstallConfirmBtns">
                  <button
                    type="button"
                    class="secondary"
                    @click="cancelUninstallConfirm"
                  >
                    {{ strings.setupUninstallCancel }}
                  </button>
                  <button
                    type="button"
                    class="primary btnDanger"
                    @click="confirmAndRunUninstall"
                  >
                    {{ strings.setupUninstallConfirmBtn }}
                  </button>
                </div>
              </div>
              <p
                v-if="pathEnv && !setupAllConfigured"
                class="setupActionsFoot"
              >
                {{ strings.setupInstallHint }}
              </p>
              <p
                v-if="pathEnv && setupAnythingInstalled"
                class="setupActionsFoot setupActionsFoot--muted"
              >
                {{ strings.setupUninstallHint }}
              </p>
              <p v-if="hubMsg" class="note setupHubMsg">{{ hubMsg }}</p>
              <p v-if="hubErr" class="error setupHubErr">{{ hubErr }}</p>
              <p class="setupInstallChangesNote setupInstallChangesNote--inStrip">
                {{ strings.setupInstallChangesNote }}
              </p>
            </section>

            <section
              class="setupMcpPauseCard"
              :aria-label="strings.mcpPauseTitle"
            >
              <div class="setupMcpPauseHead">
                <h3 class="setupMcpPauseTitle">{{ strings.mcpPauseTitle }}</h3>
                <span
                  class="setupMcpPauseBadge"
                  :class="{ 'setupMcpPauseBadge--on': mcpPaused }"
                  >{{
                    mcpPaused
                      ? strings.mcpPauseStatusOn
                      : strings.mcpPauseStatusOff
                  }}</span
                >
              </div>
              <p class="setupMcpPauseHint">{{ strings.mcpPauseHint }}</p>
              <label
                class="setupMcpPauseSwitch"
                :title="strings.mcpPauseSwitchTitle"
              >
                <input
                  v-model="mcpPaused"
                  type="checkbox"
                  class="setupMcpPauseInput"
                  :disabled="mcpPauseBusy"
                  :aria-label="strings.mcpPauseSwitchTitle"
                  @change="onMcpPausedChange"
                />
                <span>{{ strings.mcpPauseSwitch }}</span>
              </label>
              <p v-if="mcpPauseErr" class="error setupMcpPauseErr" role="alert">
                {{ mcpPauseErr }}
              </p>
            </section>

            <section class="setupStatus" :aria-label="strings.setupStatus">
              <span class="setupStatusLabel">{{ strings.setupStatus }}</span>
              <ul v-if="pathEnv" class="setupStatusList">
                <li class="setupStatusItem">
                  <div class="setupStatusItemTop">
                    <span class="setupStatusItemTitle">{{
                      strings.setupChipPath
                    }}</span>
                    <span
                      class="setupStatusBadge"
                      :class="{
                        'setupStatusBadge--ok': pathEnv.configured,
                      }"
                      >{{
                        pathEnv.configured ? strings.setupOn : strings.setupOff
                      }}</span>
                  </div>
                  <p class="setupStatusExplain">{{ strings.setupPathExplain }}</p>
                  <p v-if="!pathEnv.configured && pathEnv.reason" class="setupStatusReason">
                    {{ pathEnv.reason }}
                  </p>
                  <p class="setupStatusMeta">
                    <span class="setupStatusMetaKey">{{
                      strings.setupBinDir
                    }}</span>
                    <code class="setupStatusCode">{{ pathEnv.bin_dir }}</code>
                  </p>
                </li>
                <li class="setupStatusItem">
                  <div class="setupStatusItemTop">
                    <span class="setupStatusItemTitle">{{
                      strings.setupChipCursor
                    }}</span>
                    <span
                      class="setupStatusBadge"
                      :class="{
                        'setupStatusBadge--ok': mcpCursorInstalled,
                      }"
                      >{{
                        mcpCursorInstalled ? strings.setupOn : strings.setupOff
                      }}</span>
                  </div>
                  <p class="setupStatusExplain">
                    {{ strings.setupCursorExplain }}
                  </p>
                  <p v-if="!mcpCursorInstalled && mcpCursorReason" class="setupStatusReason">
                    {{ mcpCursorReason }}
                  </p>
                  <p class="setupStatusMeta">
                    <span class="setupStatusMetaKey">{{
                      strings.setupConfigFile
                    }}</span>
                    <code class="setupStatusCode">{{ cursorMcpPath }}</code>
                  </p>
                </li>
                <li class="setupStatusItem">
                  <div class="setupStatusItemTop">
                    <span class="setupStatusItemTitle">{{
                      strings.setupChipWindsurf
                    }}</span>
                    <span
                      class="setupStatusBadge"
                      :class="{
                        'setupStatusBadge--ok': mcpWindsurfInstalled,
                      }"
                      >{{
                        mcpWindsurfInstalled ? strings.setupOn : strings.setupOff
                      }}</span>
                  </div>
                  <p class="setupStatusExplain">
                    {{ strings.setupWindsurfExplain }}
                  </p>
                  <p v-if="!mcpWindsurfInstalled && mcpWindsurfReason" class="setupStatusReason">
                    {{ mcpWindsurfReason }}
                  </p>
                  <p class="setupStatusMeta">
                    <span class="setupStatusMetaKey">{{
                      strings.setupConfigFile
                    }}</span>
                    <code class="setupStatusCode">{{ windsurfMcpPath }}</code>
                  </p>
                </li>
              </ul>
              <p v-else class="note">{{ strings.loading }}</p>
            </section>

            <section
              class="setupConfigFrame setupConfigFrame--tools"
              :aria-label="`${strings.setupToolParamsTitle} · ${strings.setupAdvSingle}`"
            >
              <div class="setupConfigFrameBody setupConfigFrameBody--center">
                <h4 class="setupConfigFrameTitle">{{ strings.setupToolParamsTitle }}</h4>
                <p class="setupConfigFrameLead">{{ strings.setupToolParamsLead }}</p>
                <div class="setupToolsJsonEmbed">
                  <div class="setupConfigFrameBar setupToolsJsonEmbedBar">
                    <div class="setupConfigFrameBarActions">
                      <span v-if="copyToast" class="copyToast">{{ copyToast }}</span>
                      <button
                        type="button"
                        class="secondary setupConfigFrameBtn"
                        @click="copyMcpJson"
                      >
                        {{ strings.mcpCopy }}
                      </button>
                    </div>
                  </div>
                  <div class="setupToolsJsonEmbedBody">
                    <pre
                      class="mcpPreview mcpPreviewToolsEmbed"
                      tabindex="0"
                    >{{ mcpJson }}</pre>
                  </div>
                </div>
                <div class="setupToolsIdeEmbed">
                  <h5 class="setupToolsIdeEmbedTitle">{{ strings.setupAdvSingle }}</h5>
                  <div class="advIdeGrid advIdeGrid--embed">
                    <div class="setupConfigFrame advIdeFrame">
                      <div class="setupConfigFrameBar">
                        <div class="setupConfigFrameBarActions">
                          <button
                            v-if="!mcpCursorInstalled"
                            type="button"
                            class="secondary setupConfigFrameBtn"
                            :class="{ btnWithWait: mcpCursorBusy }"
                            :disabled="mcpCursorBusy || mcpWindsurfBusy"
                            :aria-busy="mcpCursorBusy"
                            @click="installCursorMcpOnly"
                          >
                            <span
                              v-if="mcpCursorBusy"
                              class="btnInlineSpinner"
                              aria-hidden="true"
                            />
                            {{
                              mcpCursorBusy
                                ? strings.mcpBusyCursorMcp
                                : strings.mcpInstallCursorOnly
                            }}
                          </button>
                          <button
                            v-if="mcpCursorInstalled"
                            type="button"
                            class="secondary setupConfigFrameBtn"
                            :class="{ btnWithWait: mcpCursorBusy }"
                            :disabled="mcpCursorBusy || mcpWindsurfBusy"
                            :aria-busy="mcpCursorBusy"
                            @click="uninstallCursorMcpOnly"
                          >
                            <span
                              v-if="mcpCursorBusy"
                              class="btnInlineSpinner"
                              aria-hidden="true"
                            />
                            {{
                              mcpCursorBusy
                                ? strings.mcpBusyCursorMcp
                                : strings.mcpUninstallCursorOnly
                            }}
                          </button>
                        </div>
                      </div>
                      <div class="setupConfigFrameBody setupConfigFrameBody--ideCard">
                        <p class="advIdeLine">
                          <span class="advIdeProduct">{{
                            strings.mcpCursorFile
                          }}</span>
                          <span
                            class="advIdeState"
                            :class="{ ok: mcpCursorInstalled }"
                          >
                            {{
                              mcpCursorInstalled
                                ? strings.mcpInCursor
                                : strings.mcpNotInCursor
                            }}
                          </span>
                        </p>
                        <code class="advIdePath advIdePath--inCard">{{
                          cursorMcpPath
                        }}</code>
                      </div>
                    </div>
                    <div class="setupConfigFrame advIdeFrame">
                      <div class="setupConfigFrameBar">
                        <div class="setupConfigFrameBarActions">
                          <button
                            v-if="!mcpWindsurfInstalled"
                            type="button"
                            class="secondary setupConfigFrameBtn"
                            :class="{ btnWithWait: mcpWindsurfBusy }"
                            :disabled="mcpWindsurfBusy || mcpCursorBusy"
                            :aria-busy="mcpWindsurfBusy"
                            @click="installWindsurfMcpOnly"
                          >
                            <span
                              v-if="mcpWindsurfBusy"
                              class="btnInlineSpinner"
                              aria-hidden="true"
                            />
                            {{
                              mcpWindsurfBusy
                                ? strings.mcpBusyWindsurfMcp
                                : strings.mcpInstallWindsurfOnly
                            }}
                          </button>
                          <button
                            v-if="mcpWindsurfInstalled"
                            type="button"
                            class="secondary setupConfigFrameBtn"
                            :class="{ btnWithWait: mcpWindsurfBusy }"
                            :disabled="mcpWindsurfBusy || mcpCursorBusy"
                            :aria-busy="mcpWindsurfBusy"
                            @click="uninstallWindsurfMcpOnly"
                          >
                            <span
                              v-if="mcpWindsurfBusy"
                              class="btnInlineSpinner"
                              aria-hidden="true"
                            />
                            {{
                              mcpWindsurfBusy
                                ? strings.mcpBusyWindsurfMcp
                                : strings.mcpUninstallWindsurfOnly
                            }}
                          </button>
                        </div>
                      </div>
                      <div class="setupConfigFrameBody setupConfigFrameBody--ideCard">
                        <p class="advIdeLine">
                          <span class="advIdeProduct">{{
                            strings.mcpWindsurfFile
                          }}</span>
                          <span
                            class="advIdeState"
                            :class="{ ok: mcpWindsurfInstalled }"
                          >
                            {{
                              mcpWindsurfInstalled
                                ? strings.mcpInWindsurf
                                : strings.mcpNotInWindsurf
                            }}
                          </span>
                        </p>
                        <code class="advIdePath advIdePath--inCard">{{
                          windsurfMcpPath
                        }}</code>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </section>

            <details class="setupAdvanced">
              <summary class="setupAdvancedSummary">{{ strings.setupAdvanced }}</summary>
              <div class="setupAdvancedInner">
                <div v-if="pathEnv" class="setupConfigFrame setupConfigFrame--path">
                  <div class="setupConfigFrameBar">
                    <div class="setupConfigFrameBarActions">
                      <button
                        v-if="!pathEnv.configured"
                        type="button"
                        class="secondary setupConfigFrameBtn"
                        :class="{ btnWithWait: pathEnvBusy }"
                        :disabled="pathEnvBusy"
                        :aria-busy="pathEnvBusy"
                        @click="configureRelayPath"
                      >
                        <span
                          v-if="pathEnvBusy"
                          class="btnInlineSpinner"
                          aria-hidden="true"
                        />
                        {{ pathEnvBusy ? strings.pathEnvBusy : strings.pathEnvBtn }}
                      </button>
                    </div>
                  </div>
                  <div class="setupConfigFrameBody setupConfigFrameBody--center">
                    <h5 class="setupConfigFrameTitle setupConfigFrameTitle--sm">
                      {{ strings.setupAdvPathTitle }}
                    </h5>
                    <p class="setupConfigFrameLead setupConfigFrameLead--sm">
                      {{ strings.setupAdvPathLead }}
                    </p>
                    <code class="advPathCode advPathCode--frame">{{ pathEnv.bin_dir }}</code>
                  </div>
                  <p v-if="pathEnvMsg" class="note advNote setupConfigFrameFoot">
                    {{ pathEnvMsg }}
                  </p>
                  <p v-if="pathEnvErr" class="error advNote setupConfigFrameFoot">
                    {{ pathEnvErr }}
                  </p>
                </div>

                <details class="setupNested">
                  <summary>{{ strings.setupJsonPreview }}</summary>
                  <p class="advJsonCaption">{{ strings.mcpJsonTitle }}</p>
                  <pre class="mcpPreview mcpPreviewAdv" tabindex="0">{{
                    mcpJson
                  }}</pre>
                </details>
                <details class="setupNested">
                  <summary>{{ strings.setupIdeGuide }}</summary>
                  <pre class="ideHintsPre ideHintsAdv">{{ ideHintsBlock }}</pre>
                </details>
              </div>
            </details>
          </div>
        </div>

        <SettingsRulePromptsPanel
          v-show="settingsSeg === 'rulePrompts'"
          :strings="strings"
          :cursor-mcp-path="cursorMcpPath || ''"
          :windsurf-mcp-path="windsurfMcpPath || ''"
        />

        <SettingsCachePanel
          :cache-segment-active="cacheSegmentActive"
          :strings="strings"
          :push-toast="pushSettingsToast"
        />

        <footer class="settingsAppFooter">
          <p class="settingsAppMeta">
            {{ strings.appAuthorLine }}
            ·
            <a
              class="settingsAppMail"
              href="mailto:andeyalee@outlook.com"
              :aria-label="strings.appAuthorEmailAria"
              >andeyalee@outlook.com</a>
          </p>
        </footer>
      </div>
    </section>

    <div
      v-if="lightboxSrc"
      class="imageLightbox"
      role="dialog"
      aria-modal="true"
      :aria-label="strings.imageLightboxClose"
      @click.self="closeLightbox"
    >
      <button
        type="button"
        class="imageLightboxClose"
        :aria-label="strings.imageLightboxClose"
        :title="strings.imageLightboxClose"
        @click="closeLightbox"
      >
        <svg
          class="imageLightboxCloseSvg"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.25"
          stroke-linecap="round"
          aria-hidden="true"
        >
          <path d="M18 6 6 18M6 6l12 12" />
        </svg>
      </button>
      <img
        class="imageLightboxImg"
        :src="lightboxSrc"
        alt=""
        @click.stop
      />
    </div>

    <div
      v-if="settingsRefreshToast"
      class="settingsRefreshToast settingsRefreshToast--floating"
      :class="'settingsRefreshToast--' + settingsRefreshToast.type"
      role="status"
      aria-live="polite"
    >
      {{ settingsRefreshToast.text }}
    </div>
  </main>
</template>
