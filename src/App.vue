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
import { listen } from "@tauri-apps/api/event";
import { locale, t } from "./i18n";
import { useAppStrings } from "./composables/useAppStrings";
import { useFeedbackWindow } from "./composables/useFeedbackWindow";
import { useReleaseBadge } from "./composables/useReleaseBadge";
import { useMcpAndPathSettings } from "./composables/useMcpAndPathSettings";
import type { CommandItem, CursorUsageEvent, IdeKind, SettingsSegment } from "./types/relay-app";
import type { SettingsToastPayload } from "./composables/useRelayCacheSettings";
import { useCursorUsage } from "./composables/useCursorUsage";
import { useIdeBinding } from "./composables/useIdeBinding";
import IdeSelectionPage from "./components/IdeSelectionPage.vue";
import SettingsAppPanel from "./components/settings/SettingsAppPanel.vue";
import SettingsRulePromptsPanel from "./components/settings/SettingsRulePromptsPanel.vue";
import SettingsUsagePanel from "./components/settings/SettingsUsagePanel.vue";
import relayLogoUrl from "./assets/relay-logo.svg?url";
import QaAssistantRetellMd from "./components/QaAssistantRetellMd.vue";
import QaUserSubmittedBubble from "./components/QaUserSubmittedBubble.vue";
import RelayComposerInput from "./components/RelayComposerInput.vue";
import {
  slashCommandSecondaryLine,
  slashItemDetailPreview,
} from "./composables/feedbackComposerUtils";
import { qaRoundHasRenderableUserContent } from "./utils/parseRelayFeedbackReply";

/** Extract `HH:mm` from a `YYYY-MM-DD HH:MM:SS` timestamp; empty input → empty output. */
function formatTime(ts: string | undefined): string {
  if (!ts) return "";
  const t = ts.trim();
  if (t.length >= 16) return t.slice(11, 16);
  return "";
}

const {
  ideKind,
  loaded: ideLoaded,
  supportsMcpInject: ideSupportsMcpInject,
  supportsUsage: ideSupportsUsage,
  supportsRulePrompt: ideSupportsRulePrompt,
  ideLabel,
  switchIde: ideSwitchIde,
} = useIdeBinding();

const ideNeedsSelection = computed(() => ideLoaded.value && ideKind.value === null);
const ideSwitchError = ref("");
const ideSwitchBusy = ref(false);

const showIdeSelectionOverlay = ref(false);
const lightboxSrc = ref<string | null>(null);
const windowDock = ref<"left" | "center" | "right">("left");
const dockEdgeHide = ref(false);
const windowAlwaysOnTop = ref(false);
/** Debounce tuck after pointer leaves the webview — ms from Rust `get_dock_edge_hide_ui_timing`. */
let shellLeaveTimer: ReturnType<typeof setTimeout> | null = null;
const shellLeaveDebounceMs = ref(120);
const mcpPaused = ref(false);
const mcpPauseBusy = ref(false);
const mcpPauseErr = ref("");
const summaryScrolling = ref(false);
let summaryScrollTimer: ReturnType<typeof setTimeout> | undefined;
const summaryCanScrollUp = ref(false);
const summaryCanScrollDown = ref(false);
const summaryTopFadeAlpha = ref(0);
const summaryBottomFadeAlpha = ref(0);
let summaryScrollRaf = 0;
const SUMMARY_FADE_DISTANCE = 88;
const SUMMARY_FADE_MAX_ALPHA = 0.34;

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
  enterSubmitModOnly,
  hasPendingFileDropErrors,
  status,
  submit,
  submitRelayExit,
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
  tabHue,
  tabFullTitle,
  renameTab,
} = useFeedbackWindow();

const editingTabId = ref<string | null>(null);
const editingTabTitle = ref("");
let tabClickTimer: ReturnType<typeof setTimeout> | null = null;

function onTabClick(tabId: string) {
  if (tabClickTimer !== null) clearTimeout(tabClickTimer);
  tabClickTimer = setTimeout(() => {
    tabClickTimer = null;
    void selectTab(tabId);
  }, 220);
}

function startTabRename(tab: { tab_id: string; title?: string }) {
  if (tabClickTimer !== null) {
    clearTimeout(tabClickTimer);
    tabClickTimer = null;
  }
  editingTabId.value = tab.tab_id;
  editingTabTitle.value = tab.title?.trim() || "";
}

let commitRenameGuard = false;
async function commitTabRename() {
  if (commitRenameGuard) return;
  commitRenameGuard = true;
  const tid = editingTabId.value;
  const val = editingTabTitle.value.trim();
  editingTabId.value = null;
  try {
    if (!tid || !val) return;
    await renameTab(tid, val);
  } finally {
    commitRenameGuard = false;
  }
}
function cancelTabRename() {
  editingTabId.value = null;
}

const tabTooltip = ref<{ text: string; x: number; y: number } | null>(null);
let tabTooltipTimer: ReturnType<typeof setTimeout> | null = null;
function onTabMouseEnter(ev: MouseEvent, tabId: string) {
  const tab = tabs.value.find(t => t.tab_id === tabId);
  if (!tab) { tabTooltip.value = null; return; }
  const full = tabFullTitle(tab);
  if (!full) { tabTooltip.value = null; return; }
  const el = ev.currentTarget as HTMLElement;
  const visuallyTruncated = el.scrollWidth > el.clientWidth;
  const label = tabLabel(tab);
  const jsTruncated = !label.includes(full);
  if (!visuallyTruncated && !jsTruncated) { tabTooltip.value = null; return; }
  if (tabTooltipTimer) clearTimeout(tabTooltipTimer);
  tabTooltipTimer = setTimeout(() => {
    const rect = el.getBoundingClientRect();
    tabTooltip.value = { text: full, x: rect.left + rect.width / 2, y: rect.bottom + 6 };
  }, 380);
}
function onTabMouseLeave() {
  if (tabTooltipTimer) { clearTimeout(tabTooltipTimer); tabTooltipTimer = null; }
  tabTooltip.value = null;
}

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
  const el = e.target as HTMLInputElement;
  if (el.files?.length) {
    addAttachedFilesFromPicker(el.files);
  }
  el.value = "";
}

const summaryScrollRef = ref<HTMLElement | null>(null);
const summaryScrollClasses = computed(() => ({
  "mainSummaryScroll--scrolling": summaryScrolling.value,
  "mainSummaryScroll--canUp": summaryCanScrollUp.value,
  "mainSummaryScroll--canDown": summaryCanScrollDown.value,
}));
const summaryScrollStyles = computed(() => ({
  "--summary-top-fade-alpha": String(summaryTopFadeAlpha.value),
  "--summary-bottom-fade-alpha": String(summaryBottomFadeAlpha.value),
}));

function updateSummaryScrollHints() {
  const el = summaryScrollRef.value;
  if (!el) {
    summaryCanScrollUp.value = false;
    summaryCanScrollDown.value = false;
    summaryTopFadeAlpha.value = 0;
    summaryBottomFadeAlpha.value = 0;
    return;
  }
  const maxScroll = el.scrollHeight - el.clientHeight;
  if (maxScroll <= 2) {
    summaryCanScrollUp.value = false;
    summaryCanScrollDown.value = false;
    summaryTopFadeAlpha.value = 0;
    summaryBottomFadeAlpha.value = 0;
    return;
  }
  summaryCanScrollUp.value = el.scrollTop > 2;
  summaryCanScrollDown.value = el.scrollTop < maxScroll - 2;
  const topRatio = Math.min(1, Math.max(0, el.scrollTop / SUMMARY_FADE_DISTANCE));
  const bottomRatio = Math.min(
    1,
    Math.max(0, (maxScroll - el.scrollTop) / SUMMARY_FADE_DISTANCE),
  );
  summaryTopFadeAlpha.value = Number((topRatio * SUMMARY_FADE_MAX_ALPHA).toFixed(3));
  summaryBottomFadeAlpha.value = Number(
    (bottomRatio * SUMMARY_FADE_MAX_ALPHA).toFixed(3),
  );
}

function onSummaryScroll() {
  if (summaryScrollRaf) return;
  summaryScrollRaf = requestAnimationFrame(() => {
    summaryScrollRaf = 0;
    updateSummaryScrollHints();
  });
  summaryScrolling.value = true;
  if (summaryScrollTimer) clearTimeout(summaryScrollTimer);
  summaryScrollTimer = setTimeout(() => {
    summaryScrolling.value = false;
    summaryScrollTimer = undefined;
  }, 720);
}

function scrollQaToBottom() {
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      const el = summaryScrollRef.value;
      if (el) {
        /* Avoid scrollIntoView: it can scroll multiple ancestors; prefer the known
         * chat scrollport only (same as scroll chaining bugs in nested layouts). */
        el.scrollTop = el.scrollHeight;
      }
    });
  });
}

watch(
  () => qaRounds.value.length,
  async () => {
    await nextTick();
    scrollQaToBottom();
    await nextTick();
    updateSummaryScrollHints();
  },
  { immediate: true },
);

watch(
  () => {
    const last = qaRounds.value[qaRounds.value.length - 1];
    if (!last) return "";
    const att = last.reply_attachments?.length ?? 0;
    return `${last.submitted ? 1 : 0}|${last.retell?.length ?? 0}|${last.reply?.length ?? 0}|${att}`;
  },
  async () => {
    await nextTick();
    scrollQaToBottom();
    await nextTick();
    updateSummaryScrollHints();
  },
);

const {
  mcpJson,
  ideMcpInstalled,
  ideRuleInstalled,
  ideMcpPath,
  hubMsg,
  hubErr,
  hubInstallBusy,
  hubUninstallBusy,
  ideInstallBusy,
  ideUninstallBusy,
  copyToast,
  pathEnv,
  pathEnvMsg,
  pathEnvErr,
  pathEnvBusy,
  ideHintsBlock,
  refreshMcpHub,
  copyMcpJson,
  doPublicInstall,
  runPublicUninstall,
  doIdeInstall,
  runIdeUninstall,
  configureRelayPath,
} = useMcpAndPathSettings(ideLabel, ideKind);

const { strings } = useAppStrings(ideLabel, ideKind, enterSubmitModOnly);

async function doSwitchIde(ide: IdeKind) {
  if (ideSwitchBusy.value) return;
  ideSwitchBusy.value = true;
  ideSwitchError.value = "";
  try {
    await ideSwitchIde(ide);
    showIdeSelectionOverlay.value = false;
    await refreshMcpHub();
  } catch (e) {
    ideSwitchError.value = e instanceof Error ? e.message : String(e);
  } finally {
    ideSwitchBusy.value = false;
  }
}

const showUninstallConfirm = ref(false);
const showIdeUninstallConfirm = ref(false);

function onPublicUninstallClick() {
  showUninstallConfirm.value = true;
}
function onIdeUninstallClick() {
  showIdeUninstallConfirm.value = true;
}

function cancelUninstallConfirm() {
  showUninstallConfirm.value = false;
}
function cancelIdeUninstallConfirm() {
  showIdeUninstallConfirm.value = false;
}

async function confirmAndRunPublicUninstall() {
  showUninstallConfirm.value = false;
  await runPublicUninstall();
}

async function confirmAndRunIdeUninstall() {
  showIdeUninstallConfirm.value = false;
  await runIdeUninstall();
}

const publicConfigured = computed(() => Boolean(pathEnv.value?.configured));
const ideConfigured = computed(() => ideMcpInstalled.value && ideRuleInstalled.value);

/** PATH + bound IDE MCP + rule OK — hide primary install. */
const setupAllConfigured = computed(() => publicConfigured.value && ideConfigured.value);

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
let unlistenIdleTimeout: (() => void) | undefined;

function pushSettingsToast(p: SettingsToastPayload) {
  if (settingsRefreshToastTimer) clearTimeout(settingsRefreshToastTimer);
  settingsRefreshToast.value = { type: p.type, text: p.text };
  settingsRefreshToastTimer = setTimeout(() => {
    settingsRefreshToast.value = null;
    settingsRefreshToastTimer = undefined;
  }, p.durationMs ?? 4500);
}

const appSegmentActive = computed(() => settingsSeg.value === "app");
const usageSegmentActive = computed(() => settingsSeg.value === "usage");

const cursorUsage = useCursorUsage(usageSegmentActive, pushSettingsToast);
const {
  usageSummary,
  loading: usageLoading,
  error: usageError,
  usageCapsuleLabel,
  usageCapsuleWarn,
  popoverOpen: usagePopoverOpen,
  usageEvents,
  usageEventsTotal,
  usageEventsPage,
  loadingEvents: usageLoadingEvents,
  planUsagePct,
  planProgressPct,
  cycleResetDate,
  daysUntilReset,
  avgRequestsPerDay,
  daysUntilExhausted,
  planPriceLabel,
  refreshUsage,
  loadEvents: loadUsageEvents,
} = cursorUsage;

function formatEventTime(ts: string): string {
  const n = Number(ts);
  if (!isNaN(n) && n > 1e12) {
    const d = new Date(n);
    const mm = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    const hh = String(d.getHours()).padStart(2, "0");
    const mi = String(d.getMinutes()).padStart(2, "0");
    return `${mm}-${dd} ${hh}:${mi}`;
  }
  return ts?.slice(5, 16)?.replace("T", " ") ?? "";
}

function totalTokens(u: CursorUsageEvent["tokenUsage"]): number {
  if (!u) return 0;
  return u.inputTokens + u.outputTokens + (u.cacheReadTokens ?? 0) + (u.cacheWriteTokens ?? 0);
}

function formatTokUnit(n: number): string {
  const unit = strings.value.usageTokUnit;
  if (n >= 10000) {
    const wan = n / 10000;
    return wan >= 100 ? `${Math.round(wan)}${unit}` : `${wan.toFixed(1)}${unit}`;
  }
  return `${n.toLocaleString()} tok`;
}

const hoveredEvent = ref<CursorUsageEvent | null>(null);
const hoverTooltipStyle = ref<Record<string, string>>({});

function onEventMouseEnter(ev: CursorUsageEvent, e: MouseEvent) {
  hoveredEvent.value = ev;
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  hoverTooltipStyle.value = {
    top: `${rect.bottom + 4}px`,
    left: `${rect.left}px`,
  };
}

function onEventMouseLeave() {
  hoveredEvent.value = null;
}

function openDashboard() {
  invoke("open_url", { url: "https://cursor.com/dashboard/usage" }).catch(() => {});
}

function toggleUsagePopover() {
  const opening = !usagePopoverOpen.value;
  usagePopoverOpen.value = opening;
  if (opening) {
    if (!usageEvents.value.length && usageSummary.value) {
      void loadUsageEvents(1);
    }
  }
}

function closeUsagePopover() {
  usagePopoverOpen.value = false;
}

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

function onTabStripKeydown(e: KeyboardEvent) {
  if (e.key !== "ArrowLeft" && e.key !== "ArrowRight" && e.key !== "Home" && e.key !== "End") return;
  e.preventDefault();
  const ids = tabs.value.map((t) => t.tab_id);
  if (ids.length < 2) return;
  const cur = ids.indexOf(activeTabId.value);
  let next: number;
  if (e.key === "ArrowRight") next = (cur + 1) % ids.length;
  else if (e.key === "ArrowLeft") next = (cur - 1 + ids.length) % ids.length;
  else if (e.key === "Home") next = 0;
  else next = ids.length - 1;
  const strip = e.currentTarget as HTMLElement;
  void selectTab(ids[next]);
  void nextTick(() => {
    strip.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
  });
}

function onGlobalKeydown(e: KeyboardEvent) {
  if (
    dockEdgeHide.value &&
    e.shiftKey &&
    (e.metaKey || e.ctrlKey) &&
    (e.key === "e" || e.key === "E")
  ) {
    e.preventDefault();
    void invoke("dock_edge_force_expand").catch(() => {
      /* ignore */
    });
    return;
  }
  if (e.key === "Escape") {
    if (lightboxSrc.value) {
      closeLightbox();
      return;
    }
    if (usagePopoverOpen.value) {
      closeUsagePopover();
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

let dockBusy = false;
async function applyWindowDock(d: "left" | "center" | "right") {
  if (dockBusy) return;
  dockBusy = true;
  try {
    await invoke("set_window_dock", { dock: d });
    windowDock.value = d;
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    dockBusy = false;
  }
}

let edgeHideBusy = false;
async function toggleDockEdgeHide() {
  if (edgeHideBusy) return;
  edgeHideBusy = true;
  const next = !dockEdgeHide.value;
  try {
    await invoke("set_dock_edge_hide", { enabled: next });
    dockEdgeHide.value = next;
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    edgeHideBusy = false;
  }
}

let aotBusy = false;
async function toggleWindowAlwaysOnTop() {
  if (aotBusy) return;
  aotBusy = true;
  const next = !windowAlwaysOnTop.value;
  try {
    await invoke("set_window_always_on_top", { enabled: next });
    windowAlwaysOnTop.value = next;
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    aotBusy = false;
  }
}

function onShellMouseEnter() {
  if (shellLeaveTimer !== null) {
    clearTimeout(shellLeaveTimer);
    shellLeaveTimer = null;
  }
}

function onShellMouseLeave(ev: MouseEvent) {
  if (!dockEdgeHide.value) return;
  // Leaving through the top of the webview — pointer is headed to the native title bar / window
  // controls. Whether to tuck is decided in Rust using Tauri cursor + outer window rect.
  if (ev.clientY <= 16) return;
  if (shellLeaveTimer !== null) clearTimeout(shellLeaveTimer);
  shellLeaveTimer = setTimeout(() => {
    shellLeaveTimer = null;
    void invoke("dock_edge_hide_after_leave").catch(() => {
      /* ignore */
    });
  }, shellLeaveDebounceMs.value);
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

    // Fire all independent Rust queries in parallel while initAfterLocale loads tabs.
    const [, dockResult, edgeResult, aotResult, timingResult] = await Promise.all([
      initAfterLocale(),
      invoke<string>("get_window_dock").catch(() => "left" as string),
      invoke<boolean>("get_dock_edge_hide").catch(() => false),
      invoke<boolean>("get_window_always_on_top").catch(() => false),
      invoke<{ shellLeaveDebounceMs: number; suppressAfterPeekMs: number }>(
        "get_dock_edge_hide_ui_timing",
      ).catch(() => null),
    ]);

    const d = dockResult;
    windowDock.value = d === "center" || d === "right" ? d : "left";
    dockEdgeHide.value = edgeResult;
    windowAlwaysOnTop.value = aotResult;
    if (timingResult) shellLeaveDebounceMs.value = timingResult.shellLeaveDebounceMs;

    void refreshMcpPaused();
    window.addEventListener("keydown", onGlobalKeydown);
    unlistenIdleTimeout = await listen("relay_idle_timeout", () => {
      pushSettingsToast({
        type: "warn",
        text: t("idleTimeoutToast"),
      });
    });
    await nextTick();
    updateSummaryScrollHints();
  } catch (err) {
    loading.value = false;
    error.value = err instanceof Error ? err.message : String(err);
  }
});

onBeforeUnmount(() => {
  unlistenIdleTimeout?.();
  unlistenIdleTimeout = undefined;
  window.removeEventListener("keydown", onGlobalKeydown);
  if (settingsRefreshToastTimer) clearTimeout(settingsRefreshToastTimer);
  if (shellLeaveTimer !== null) clearTimeout(shellLeaveTimer);
  if (summaryScrollTimer) clearTimeout(summaryScrollTimer);
  if (summaryScrollRaf) cancelAnimationFrame(summaryScrollRaf);
});
</script>

<template>
  <div v-if="!ideLoaded" class="ideBootScreen" role="status" aria-live="polite">
    <div class="ideBootSpinner" aria-hidden="true" />
    <p class="ideBootText">{{ strings.loading }}</p>
  </div>
  <IdeSelectionPage
    v-else-if="ideNeedsSelection"
    :strings="strings"
    :error="ideSwitchError"
    :busy="ideSwitchBusy"
    @select="doSwitchIde"
  />
  <main
    v-else
    class="shell"
    :class="{
      dragActive,
      settingsOpen: uiView === 'settings',
      shellMainFill: uiView === 'main',
    }"
    @dragover="onDragOver"
    @dragleave="onDragLeave"
    @drop="onDrop"
    @mouseenter="onShellMouseEnter"
    @mouseleave="onShellMouseLeave"
  >
    <!-- Main: feedback-first layout -->
    <section v-show="uiView === 'main'" class="panel panelMain mainWork">
      <header class="mainTopBar">
        <div class="mainTopBarLeft">
          <div class="mainBrandCluster">
            <div class="mainBrandCapsule">
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
            <button
              v-if="ideSupportsUsage && usageSummary"
              type="button"
              class="usageCapsule"
              :class="{ 'usageCapsule--warn': usageCapsuleWarn, 'usageCapsule--error': !!usageError }"
              :title="usageError || strings.usageCapsuleTitle"
              @click="toggleUsagePopover"
            >
              <span class="usageCapsuleDot usageCapsuleDot--plan" />
              {{ usageCapsuleLabel }}
            </button>
            <span
              v-else-if="ideSupportsUsage && usageLoading"
              class="usageCapsule usageCapsule--loading"
              :title="strings.usageCapsuleTitle"
            >
              <span class="usageCapsuleSpinner" />
            </span>
            <button
              v-else-if="ideSupportsUsage && usageError"
              type="button"
              class="usageCapsule usageCapsule--error"
              :title="usageError"
              @click="refreshUsage()"
            >
              ⚠
            </button>
          </div>
        </div>
        <div class="mainTopBarRight">
          <div class="mainTopBarToolbar">
            <div class="mainTopBarDockRow">
              <div
                class="dockSeg"
                role="group"
                :aria-label="strings.windowDockAria"
              >
              <button
                type="button"
                class="dockSegBtn"
                :aria-pressed="windowDock === 'left'"
                :title="strings.windowDockLeft"
                @click="applyWindowDock('left')"
              >
                {{ strings.dockBtnLeft }}
              </button>
              <button
                type="button"
                class="dockSegBtn"
                :aria-pressed="windowDock === 'center'"
                :title="strings.windowDockCenter"
                @click="applyWindowDock('center')"
              >
                {{ strings.dockBtnCenter }}
              </button>
              <button
                type="button"
                class="dockSegBtn"
                :aria-pressed="windowDock === 'right'"
                :title="strings.windowDockRight"
                @click="applyWindowDock('right')"
              >
                {{ strings.dockBtnRight }}
              </button>
            </div>
            <button
              type="button"
              class="iconDockEdge"
              :class="{ 'iconDockEdge--on': windowAlwaysOnTop }"
              :title="strings.windowAlwaysOnTopTitle"
              :aria-label="strings.windowAlwaysOnTopAria"
              :aria-pressed="windowAlwaysOnTop"
              @click="toggleWindowAlwaysOnTop"
            >
              <svg
                class="iconDockEdgeSvg"
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
                  d="M12 3l3 3h-2v6h-2V6H9l3-3zM6 12h12v7H6z"
                />
              </svg>
            </button>
            <button
              type="button"
              class="iconDockEdge"
              :class="{ 'iconDockEdge--on': dockEdgeHide }"
              :title="strings.dockEdgeHideTitle"
              :aria-label="strings.dockEdgeHideAria"
              :aria-pressed="dockEdgeHide"
              @click="toggleDockEdgeHide"
            >
              <svg
                class="iconDockEdgeSvg"
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
                  d="M4.5 5.25h4.5v13.5H4.5a.75.75 0 01-.75-.75V6a.75.75 0 01.75-.75zm6 0h9a.75.75 0 01.75.75v10.5a.75.75 0 01-.75.75h-9V5.25z"
                />
              </svg>
            </button>
            </div>
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
          </div>
        </div>
      </header>

      <div
        v-if="tabs.length > 0"
        class="tabStrip"
        role="tablist"
        :aria-label="strings.tabStripAria"
        @keydown="onTabStripKeydown"
      >
        <div
          v-for="tab in tabs"
          :key="tab.tab_id"
          class="tabStripCell"
        >
          <input
            v-if="editingTabId === tab.tab_id"
            :ref="(el: any) => { if (el) nextTick(() => (el as HTMLInputElement).focus()); }"
            v-model="editingTabTitle"
            class="tabRenameInput"
            maxlength="60"
            @blur="commitTabRename"
            @keydown.enter.prevent="commitTabRename"
            @keydown.escape.prevent="cancelTabRename"
            @click.stop
          />
          <button
            v-else
            type="button"
            role="tab"
            class="tabBtn"
            :class="{
              active: tab.tab_id === activeTabId,
              tabBtnFlash: flashingTabIds.has(tab.tab_id),
              ['tabBtn--hue-' + tabHue(tab)]: !flashingTabIds.has(tab.tab_id) && tabHue(tab) !== 'default',
            }"
            :aria-selected="tab.tab_id === activeTabId"
            @click="onTabClick(tab.tab_id)"
            @dblclick.stop="startTabRename(tab)"
            @mouseenter="onTabMouseEnter($event, tab.tab_id)"
            @mouseleave="onTabMouseLeave"
          >
            {{ tabLabel(tab) }}
          </button>
          <button
            v-show="editingTabId !== tab.tab_id"
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
            :class="summaryScrollClasses"
            :style="summaryScrollStyles"
            tabindex="0"
            role="region"
            :aria-label="strings.qaHistoryTitle"
            @scroll="onSummaryScroll"
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
                  }}<span v-if="formatTime(round.retell_at)" class="qaChatTimestamp">{{ formatTime(round.retell_at) }}</span></span>
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
                      <QaAssistantRetellMd :retell="round.retell" />
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
                  <span class="qaChatRole qaChatRole--me"><span v-if="formatTime(round.reply_at)" class="qaChatTimestamp">{{ formatTime(round.reply_at) }}</span>{{
                    strings.qaUserTurnMe
                  }}</span>
                  <div class="qaChatBubble qaChatBubble--me qaChatBubble--meMuted">
                    <p class="qaRoundMuted">
                      {{
                        round.idle_timeout
                          ? strings.qaSkippedIdle
                          : strings.qaSkipped
                      }}
                    </p>
                  </div>
                </div>
              </div>
              <div
                v-else-if="round.submitted && qaRoundHasRenderableUserContent(round)"
                class="qaChatRow qaChatRow--me"
              >
                <div class="qaChatStack qaChatStack--me">
                  <span class="qaChatRole qaChatRole--me"><span v-if="formatTime(round.reply_at)" class="qaChatTimestamp">{{ formatTime(round.reply_at) }}</span>{{
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
                  <span class="qaChatRole qaChatRole--me"><span v-if="formatTime(round.reply_at)" class="qaChatTimestamp">{{ formatTime(round.reply_at) }}</span>{{
                    strings.qaUserTurnMe
                  }}</span>
                  <div class="qaChatBubble qaChatBubble--me qaChatBubble--meMuted">
                    <p class="qaRoundMuted">{{ strings.qaEmptySubmit }}</p>
                  </div>
                </div>
              </div>
            </article>
            <div class="qaScrollEndAnchor" aria-hidden="true" />
          </div>
          <div
            v-else
            class="mainSummaryScroll mainSummaryScroll--empty"
            :class="summaryScrollClasses"
            :style="summaryScrollStyles"
            @scroll="onSummaryScroll"
          >
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
                    :aria-label="strings.slashListboxAria"
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
                      class="composerIconBtn composerIconBtnExit"
                      :title="
                        submitting
                          ? strings.composerSubmitting
                          : isHubPage
                            ? strings.composerSubmitDisabledPreview
                            : composerDrafting
                              ? strings.composerSubmitDisabledIdle
                              : strings.composerRelayExitTitle
                      "
                      :aria-label="
                        submitting
                          ? strings.composerSubmitting
                          : isHubPage
                            ? strings.composerSubmitDisabledPreview
                            : composerDrafting
                              ? strings.composerSubmitDisabledIdle
                              : strings.composerRelayExitAria
                      "
                      :disabled="isHubPage || (!isHubPage && composerDrafting) || submitting"
                      @click="void submitRelayExit()"
                    >
                      <span
                        v-if="submitting"
                        class="composerSendSpinner composerSendSpinner--exit"
                        aria-hidden="true"
                      />
                      <svg
                        v-else
                        class="composerIconSvg"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        aria-hidden="true"
                      >
                        <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
                        <polyline points="16 17 21 12 16 7" />
                        <line x1="21" y1="12" x2="9" y2="12" />
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
            class="langBtn ideModeBtnHeader"
            :title="strings.ideSettingsChangeBtn"
            @click="showIdeSelectionOverlay = true"
          >
            {{ ideLabel || strings.ideSelectPlaceholder }}
          </button>
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
          v-if="ideSupportsRulePrompt"
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
          :class="{ active: settingsSeg === 'app' }"
          :aria-selected="settingsSeg === 'app'"
          @click="settingsSeg = 'app'"
        >
          {{ strings.segCache }}
        </button>
        <button
          v-if="ideSupportsUsage"
          type="button"
          role="tab"
          class="segTab"
          :class="{ active: settingsSeg === 'usage' }"
          :aria-selected="settingsSeg === 'usage'"
          @click="settingsSeg = 'usage'"
        >
          {{ strings.segUsage }}
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

            <!-- Public config (PATH) -->
            <section
              v-if="pathEnv"
              class="setupActionsStrip"
              :aria-label="strings.setupSectionPublic"
            >
              <div class="setupActionsStripHead">
                <div class="setupActionsStripCopy">
                  <p class="setupActionsStripLead" :class="{ 'setupActionsStripLead--ok': publicConfigured }">
                    {{ strings.setupSectionPublic }}
                  </p>
                </div>
                <div class="setupActionsStripBtns">
                  <button
                    v-if="!publicConfigured"
                    type="button"
                    class="primary setupInstallBtnCompact"
                    :class="{ btnWithWait: hubInstallBusy }"
                    :disabled="hubInstallBusy"
                    :aria-busy="hubInstallBusy"
                    @click="doPublicInstall"
                  >
                    <span v-if="hubInstallBusy" class="btnInlineSpinner" aria-hidden="true" />
                    {{ strings.setupBtnPublicInstall }}
                  </button>
                  <button
                    v-if="publicConfigured"
                    type="button"
                    class="setupUninstallBtnCompact"
                    :class="{ btnWithWait: hubUninstallBusy }"
                    :disabled="hubUninstallBusy || showUninstallConfirm"
                    :aria-busy="hubUninstallBusy"
                    @click="onPublicUninstallClick"
                  >
                    <span v-if="hubUninstallBusy" class="btnInlineSpinner" aria-hidden="true" />
                    {{ strings.setupBtnPublicUninstall }}
                  </button>
                </div>
              </div>
              <div
                v-if="showUninstallConfirm"
                class="uninstallConfirmBar"
                role="dialog"
                aria-modal="true"
                :aria-label="strings.setupBtnPublicUninstall"
              >
                <p class="uninstallConfirmText">{{ strings.publicUninstallConfirm }}</p>
                <div class="uninstallConfirmBtns">
                  <button type="button" class="secondary" @click="cancelUninstallConfirm">{{ strings.setupUninstallCancel }}</button>
                  <button type="button" class="primary btnDanger" @click="confirmAndRunPublicUninstall">{{ strings.setupUninstallConfirmBtn }}</button>
                </div>
              </div>
            </section>

            <!-- IDE-specific config (MCP + Rule) -->
            <section
              v-if="pathEnv && ideSupportsMcpInject"
              class="setupActionsStrip"
              :aria-label="strings.setupSectionIde"
            >
              <div class="setupActionsStripHead">
                <div class="setupActionsStripCopy">
                  <p class="setupActionsStripLead" :class="{ 'setupActionsStripLead--ok': ideConfigured }">
                    {{ strings.setupSectionIde }}
                  </p>
                </div>
                <div class="setupActionsStripBtns">
                  <button
                    v-if="!ideConfigured"
                    type="button"
                    class="primary setupInstallBtnCompact"
                    :class="{ btnWithWait: ideInstallBusy }"
                    :disabled="ideInstallBusy"
                    :aria-busy="ideInstallBusy"
                    @click="doIdeInstall"
                  >
                    <span v-if="ideInstallBusy" class="btnInlineSpinner" aria-hidden="true" />
                    {{ strings.setupBtnIdeInstall }}
                  </button>
                  <button
                    v-if="ideConfigured"
                    type="button"
                    class="setupUninstallBtnCompact"
                    :class="{ btnWithWait: ideUninstallBusy }"
                    :disabled="ideUninstallBusy || showIdeUninstallConfirm"
                    :aria-busy="ideUninstallBusy"
                    @click="onIdeUninstallClick"
                  >
                    <span v-if="ideUninstallBusy" class="btnInlineSpinner" aria-hidden="true" />
                    {{ strings.setupBtnIdeUninstall }}
                  </button>
                </div>
              </div>
              <div
                v-if="showIdeUninstallConfirm"
                class="uninstallConfirmBar"
                role="dialog"
                aria-modal="true"
                :aria-label="strings.setupBtnIdeUninstall"
              >
                <p class="uninstallConfirmText">{{ strings.ideUninstallConfirm }}</p>
                <div class="uninstallConfirmBtns">
                  <button type="button" class="secondary" @click="cancelIdeUninstallConfirm">{{ strings.setupUninstallCancel }}</button>
                  <button type="button" class="primary btnDanger" @click="confirmAndRunIdeUninstall">{{ strings.setupUninstallConfirmBtn }}</button>
                </div>
              </div>
              <p v-if="hubMsg" class="note setupHubMsg">{{ hubMsg }}</p>
              <p v-if="hubErr" class="error setupHubErr">{{ hubErr }}</p>
            </section>

            <section
              v-if="ideSupportsMcpInject"
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

            <section v-if="ideSupportsMcpInject" class="setupStatus" :aria-label="strings.setupStatus">
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
                    <span class="setupStatusItemTitle">{{ ideLabel }} MCP</span>
                    <span
                      class="setupStatusBadge"
                      :class="{
                        'setupStatusBadge--ok': ideMcpInstalled,
                      }"
                      >{{
                        ideMcpInstalled ? strings.setupOn : strings.setupOff
                      }}</span>
                  </div>
                  <p class="setupStatusExplain">
                    {{ strings.setupMcpExplain }}
                  </p>
                  <p v-if="ideMcpPath" class="setupStatusMeta">
                    <span class="setupStatusMetaKey">{{
                      strings.setupConfigFile
                    }}</span>
                    <code class="setupStatusCode">{{ ideMcpPath }}</code>
                  </p>
                </li>
                <li class="setupStatusItem">
                  <div class="setupStatusItemTop">
                    <span class="setupStatusItemTitle">{{ ideLabel }} Rule</span>
                    <span
                      class="setupStatusBadge"
                      :class="{
                        'setupStatusBadge--ok': ideRuleInstalled,
                      }"
                      >{{
                        ideRuleInstalled ? strings.setupOn : strings.setupOff
                      }}</span>
                  </div>
                  <p class="setupStatusExplain">
                    {{ strings.setupRuleExplain }}
                  </p>
                </li>
              </ul>
              <p v-else class="note">{{ strings.loading }}</p>
            </section>

            <section
              class="setupConfigFrame setupConfigFrame--tools"
              :aria-label="strings.setupToolParamsTitle"
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

                <details v-if="ideHintsBlock" class="setupNested">
                  <summary>{{ strings.setupIdeGuide }}</summary>
                  <pre class="ideHintsPre ideHintsAdv">{{ ideHintsBlock }}</pre>
                </details>
              </div>
            </details>
          </div>
        </div>

        <SettingsRulePromptsPanel
          v-if="ideSupportsRulePrompt"
          v-show="settingsSeg === 'rulePrompts'"
          :strings="strings"
          :ide-label="ideLabel"
          :ide-kind="ideKind"
          :ide-mcp-path="ideMcpPath"
          :active="settingsSeg === 'rulePrompts'"
          :push-toast="pushSettingsToast"
        />

        <SettingsAppPanel
          :app-segment-active="appSegmentActive"
          :strings="strings"
          :push-toast="pushSettingsToast"
        />

        <SettingsUsagePanel
          v-if="ideSupportsUsage"
          :usage-segment-active="usageSegmentActive"
          :strings="strings"
          :usage="cursorUsage"
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

    <!-- Usage detail popover -->
    <div
      v-if="usagePopoverOpen"
      class="usagePopoverBackdrop"
      @click.self="closeUsagePopover"
    >
      <aside class="usagePopover" role="dialog" :aria-label="strings.usagePopoverTitle">
        <header class="usagePopoverHeader">
          <h3 class="usagePopoverTitle">{{ strings.usagePopoverTitle }}</h3>
          <button
            type="button"
            class="usagePopoverClose"
            :aria-label="strings.settingsBack"
            @click="closeUsagePopover"
          >
            ✕
          </button>
        </header>
        <div v-if="usageSummary" class="usagePopoverBody">
          <!-- Progress bar -->
          <div class="usageProgressSection">
            <div class="usageProgressLabelRow">
              <span class="usageProgressLabel">
                {{ Math.round(usageSummary.individualUsage.plan.used) }} / {{ Math.round(usageSummary.individualUsage.plan.limit) }}
                <span class="usageProgressPct">({{ planUsagePct.toFixed(1) }}%)</span>
              </span>
              <span class="usageProgressRemaining">
                {{ Math.round(usageSummary.individualUsage.plan.remaining) }} {{ strings.usagePlanRemaining }}
              </span>
            </div>
            <div class="usageProgressTrack">
              <div
                class="usageProgressBar"
                :class="{
                  'usageProgressBar--warn': planUsagePct > 80,
                  'usageProgressBar--danger': planUsagePct > 95,
                }"
                :style="{ width: planProgressPct + '%' }"
              />
            </div>
          </div>

          <!-- Insights row -->
          <div class="usageInsights">
            <div v-if="daysUntilReset !== null" class="usageInsightItem">
              <span class="usageInsightIcon">⏱</span>
              <span>{{ strings.usageResetsIn }} <strong>{{ daysUntilReset }}</strong> {{ strings.usageDays }}</span>
              <span v-if="cycleResetDate" class="usageInsightSub">({{ cycleResetDate.toISOString().slice(0, 10) }})</span>
            </div>
            <div class="usageInsightItem">
              <span class="usageInsightIcon">📊</span>
              <span>{{ strings.usageDailyAvg }}: <strong>{{ avgRequestsPerDay.toFixed(1) }}</strong> {{ strings.usageReqPerDay }}</span>
            </div>
            <div v-if="daysUntilExhausted !== null && daysUntilExhausted <= (daysUntilReset ?? 999)" class="usageInsightItem usageInsightItem--warn">
              <span class="usageInsightIcon">⚠️</span>
              <span>{{ strings.usageExhaustedIn }} <strong>~{{ daysUntilExhausted }}</strong> {{ strings.usageExhaustedDays }}</span>
            </div>
          </div>

          <!-- Details card -->
          <div class="usagePopoverSummaryCard">
            <div class="usagePopoverRow">
              <span class="usagePopoverRowLabel">{{ strings.usageMembership }}</span>
              <span class="usagePopoverRowValue usagePopoverRowValue--badge">
                {{ usageSummary.membershipType }}
              </span>
            </div>
            <div v-if="planPriceLabel" class="usagePopoverRow">
              <span class="usagePopoverRowLabel">{{ strings.usagePlanCost }}</span>
              <span class="usagePopoverRowValue">{{ planPriceLabel }}</span>
            </div>
            <div class="usagePopoverRow">
              <span class="usagePopoverRowLabel">{{ strings.usageBillingCycle }}</span>
              <span class="usagePopoverRowValue">
                {{ usageSummary.billingCycleStart?.slice(0, 10) }}
                <template v-if="usageSummary.billingCycleEnd"> → {{ usageSummary.billingCycleEnd.slice(0, 10) }}</template>
              </span>
            </div>
            <div class="usagePopoverRow">
              <span class="usagePopoverRowLabel">{{ strings.usageOnDemandUsed }}</span>
              <span class="usagePopoverRowValue">
                ${{ (usageSummary.individualUsage.onDemand.used / 100).toFixed(2) }}
                <template v-if="usageSummary.individualUsage.onDemand.limit > 0">
                  / ${{ (usageSummary.individualUsage.onDemand.limit / 100).toFixed(2) }}
                </template>
              </span>
            </div>
            <div class="usagePopoverRow">
              <span class="usagePopoverRowLabel">{{ strings.usageOnDemandCap }}</span>
              <span class="usagePopoverRowValue">
                <template v-if="usageSummary.individualUsage.onDemand.limit > 0">
                  ${{ (usageSummary.individualUsage.onDemand.limit / 100).toFixed(2) }}
                </template>
                <template v-else-if="usageSummary.onDemandAutoEnabled">
                  <a
                    href="#"
                    class="usagePopoverLink"
                    @click.prevent="openDashboard"
                  >{{ strings.usageOnDemandViewDashboard }}</a>
                </template>
                <template v-else>
                  {{ strings.usageOnDemandDisabled }}
                </template>
              </span>
            </div>
            <div v-if="usageSummary.teamUsage" class="usagePopoverRow">
              <span class="usagePopoverRowLabel">{{ strings.usageTeamOnDemand }}</span>
              <span class="usagePopoverRowValue">${{ (usageSummary.teamUsage.onDemand.used / 100).toFixed(2) }}</span>
            </div>
          </div>
          <div class="usagePopoverEventsSection">
            <h4 class="usagePopoverEventsTitle">{{ strings.usageRecentTitle }}</h4>
            <div v-if="usageEvents.length" class="usagePopoverEventsTable">
              <div
                v-for="(ev, idx) in usageEvents"
                :key="idx"
                class="usagePopoverEventRow"
                @mouseenter="onEventMouseEnter(ev, $event)"
                @mouseleave="onEventMouseLeave"
              >
                <span class="usagePopoverEventTime">{{ formatEventTime(ev.timestamp) }}</span>
                <span class="usagePopoverEventModel">{{ ev.model }}</span>
                <span class="usagePopoverEventKind">{{ ev.kind }}</span>
                <span class="usagePopoverEventCost">
                  {{ ev.tokenUsage ? formatTokUnit(totalTokens(ev.tokenUsage)) : '' }}
                </span>
                <span class="usagePopoverEventCents">
                  {{ ev.chargedCents > 0 ? `$${(ev.chargedCents / 100).toFixed(3)}` : ev.requestsCosts ? `${ev.requestsCosts} req` : '—' }}
                </span>
              </div>
            </div>
            <p v-else class="usagePopoverNoEvents">{{ strings.usageNoEvents }}</p>
            <button
              v-if="usageEvents.length < usageEventsTotal"
              type="button"
              class="usagePopoverLoadMore"
              :disabled="usageLoadingEvents"
              @click="loadUsageEvents(usageEventsPage + 1)"
            >
              {{ usageLoadingEvents ? strings.usageRefreshing : strings.usageLoadMore }}
            </button>
          </div>
        </div>
        <footer class="usagePopoverFooter">
          <button
            type="button"
            class="usagePopoverRefreshBtn"
            :disabled="usageLoading"
            @click="refreshUsage()"
          >
            {{ usageLoading ? strings.usageRefreshing : strings.usageRefreshBtn }}
          </button>
        </footer>
      </aside>
    </div>

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

    <Teleport to="body">
      <div
        v-if="hoveredEvent"
        class="eventTooltip"
        :style="hoverTooltipStyle"
      >
        <div class="eventTooltipRow"><span class="eventTooltipLabel">Time</span><span>{{ formatEventTime(hoveredEvent.timestamp) }}</span></div>
        <div class="eventTooltipRow"><span class="eventTooltipLabel">Model</span><span>{{ hoveredEvent.model }}</span></div>
        <div class="eventTooltipRow"><span class="eventTooltipLabel">Kind</span><span>{{ hoveredEvent.kind.replace('USAGE_EVENT_KIND_', '') }}</span></div>
        <div v-if="hoveredEvent.requestsCosts" class="eventTooltipRow"><span class="eventTooltipLabel">Requests</span><span>{{ hoveredEvent.requestsCosts }}</span></div>
        <template v-if="hoveredEvent.tokenUsage">
          <div class="eventTooltipRow"><span class="eventTooltipLabel">Input</span><span>{{ formatTokUnit(hoveredEvent.tokenUsage.inputTokens) }}</span></div>
          <div class="eventTooltipRow"><span class="eventTooltipLabel">Output</span><span>{{ formatTokUnit(hoveredEvent.tokenUsage.outputTokens) }}</span></div>
          <div v-if="hoveredEvent.tokenUsage.cacheReadTokens" class="eventTooltipRow"><span class="eventTooltipLabel">Cache read</span><span>{{ formatTokUnit(hoveredEvent.tokenUsage.cacheReadTokens) }}</span></div>
          <div v-if="hoveredEvent.tokenUsage.cacheWriteTokens" class="eventTooltipRow"><span class="eventTooltipLabel">Cache write</span><span>{{ formatTokUnit(hoveredEvent.tokenUsage.cacheWriteTokens) }}</span></div>
          <div class="eventTooltipRow"><span class="eventTooltipLabel">Cost</span><span>${{ (hoveredEvent.tokenUsage.totalCents / 100).toFixed(4) }}</span></div>
        </template>
        <div v-if="hoveredEvent.chargedCents > 0" class="eventTooltipRow eventTooltipRow--highlight"><span class="eventTooltipLabel">Charged</span><span>${{ (hoveredEvent.chargedCents / 100).toFixed(3) }}</span></div>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="showIdeSelectionOverlay" class="ideOverlayBackdrop" @click.self="showIdeSelectionOverlay = false">
        <IdeSelectionPage
          :strings="strings"
          :error="ideSwitchError"
          :busy="ideSwitchBusy"
          @select="doSwitchIde"
        />
      </div>
    </Teleport>

    <!-- Custom tab title tooltip -->
    <div
      v-if="tabTooltip"
      class="tabTooltip"
      :style="{ left: tabTooltip.x + 'px', top: tabTooltip.y + 'px' }"
    >
      {{ tabTooltip.text }}
    </div>
  </main>
</template>
