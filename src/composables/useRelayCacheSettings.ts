import { computed, nextTick, onBeforeUnmount, ref, watch, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { t, locale } from "../i18n";
import type { RelayCacheStats } from "../types/relay-app";
import { formatBytes } from "../utils/formatBytes";

export type SettingsToastPayload = {
  type: "ok" | "warn" | "err";
  text: string;
  durationMs?: number;
};

const RETENTION_PRESETS = [7, 15, 30, 90, 180, 365] as const;

function isRetentionPreset(n: number): boolean {
  return (RETENTION_PRESETS as readonly number[]).includes(n);
}

/**
 * Cache stats, attachment retention, and clear-cache flows for Settings → Storage.
 * @param isActive — e.g. computed(() => settingsSeg === 'cache')
 * @param pushToast — parent-owned toast (e.g. settings bar)
 */
export function useRelayCacheSettings(
  isActive: Ref<boolean>,
  pushToast: (_payload: SettingsToastPayload) => void,
) {
  const cacheStats = ref<RelayCacheStats | null>(null);
  const cacheLoadBusy = ref(false);
  const cacheActionBusy = ref(false);
  const attachmentRetentionStr = ref("");
  const attachmentRetentionCustom = ref<number | null>(null);
  const attachmentRetentionBusy = ref(false);
  const retentionMenuOpen = ref(false);
  const retentionFieldRef = ref<HTMLElement | null>(null);
  const cacheConfirmKind = ref<"all" | "attachments" | "logs" | null>(null);

  const cacheLogRelatedBytes = computed(() => {
    const s = cacheStats.value;
    if (!s) return 0;
    return s.log_bytes + (s.qa_archive_bytes ?? 0);
  });

  const cacheTotalBytes = computed(() => {
    const s = cacheStats.value;
    if (!s) return 0;
    return s.attachments_bytes + cacheLogRelatedBytes.value;
  });

  const cacheUsageFlexAttach = computed(() => {
    const s = cacheStats.value;
    if (!s) return 1;
    const tot = s.attachments_bytes + cacheLogRelatedBytes.value;
    if (tot <= 0) return 1;
    return Math.max(s.attachments_bytes, 0.001 * tot);
  });

  const cacheUsageFlexLog = computed(() => {
    const s = cacheStats.value;
    if (!s) return 1;
    const tot = s.attachments_bytes + cacheLogRelatedBytes.value;
    if (tot <= 0) return 1;
    return Math.max(cacheLogRelatedBytes.value, 0.001 * tot);
  });

  const retentionOptions = computed(() => {
    void locale.value;
    const c = attachmentRetentionCustom.value;
    const opts: { value: string; label: string }[] = [
      { value: "", label: t("cacheRetentionOff") },
      { value: "7", label: `7 ${t("cacheDays")}` },
      { value: "15", label: `15 ${t("cacheDays")}` },
      { value: "30", label: `30 ${t("cacheDays")}` },
      { value: "90", label: t("cacheMonths3") },
      { value: "180", label: t("cacheMonths6") },
      { value: "365", label: t("cacheYear1") },
    ];
    if (c != null && !isRetentionPreset(c)) {
      const vs = String(c);
      if (!opts.some((o) => o.value === vs)) {
        opts.push({ value: vs, label: `${c} ${t("cacheDays")}` });
      }
    }
    return opts;
  });

  const retentionDisplayLabel = computed(() => {
    const v = attachmentRetentionStr.value;
    return retentionOptions.value.find((o) => o.value === v)?.label ?? v;
  });

  const cacheConfirmBody = computed(() => {
    void locale.value;
    const k = cacheConfirmKind.value;
    if (!k) return "";
    if (k === "all") return t("cacheConfirmClearAll");
    if (k === "attachments") return t("cacheConfirmClearAttach");
    return t("cacheConfirmClearLogs");
  });

  async function loadCacheStats() {
    cacheLoadBusy.value = true;
    try {
      cacheStats.value = await invoke<RelayCacheStats>("get_relay_cache_stats");
    } catch {
      cacheStats.value = null;
    } finally {
      cacheLoadBusy.value = false;
    }
  }

  async function loadAttachmentRetention() {
    try {
      const d = await invoke<number | undefined>("get_attachment_retention_days");
      if (d == null || d <= 0) {
        attachmentRetentionStr.value = "";
        attachmentRetentionCustom.value = null;
        return;
      }
      const presets = RETENTION_PRESETS as readonly number[];
      if (presets.includes(d)) {
        attachmentRetentionStr.value = String(d);
        attachmentRetentionCustom.value = null;
      } else {
        attachmentRetentionStr.value = String(d);
        attachmentRetentionCustom.value = d;
      }
    } catch {
      attachmentRetentionStr.value = "";
      attachmentRetentionCustom.value = null;
    }
  }

  async function onAttachmentRetentionChange() {
    if (attachmentRetentionBusy.value) return;
    attachmentRetentionBusy.value = true;
    try {
      const v = attachmentRetentionStr.value;
      const days: number | null =
        v === "" || v === "0" ? null : Math.min(3660, Math.max(1, Number(v)));
      const freed = await invoke<number>("set_attachment_retention_days", {
        days,
      });
      attachmentRetentionCustom.value = null;
      if (days != null && !isRetentionPreset(days)) {
        attachmentRetentionCustom.value = days;
      }
      await loadCacheStats();
      if (freed > 0) {
        pushToast({
          type: "ok",
          text: t("cachePurgeFreed").replace("{n}", formatBytes(freed)),
          durationMs: 4500,
        });
      }
    } catch (e) {
      const detail = e instanceof Error ? e.message : String(e);
      pushToast({
        type: "err",
        text: `${t("cacheClearErr")} ${detail}`.trim(),
        durationMs: 5000,
      });
      await loadAttachmentRetention();
    } finally {
      attachmentRetentionBusy.value = false;
    }
  }

  async function pickRetentionOption(value: string) {
    retentionMenuOpen.value = false;
    if (attachmentRetentionStr.value === value) return;
    attachmentRetentionStr.value = value;
    await onAttachmentRetentionChange();
  }

  async function openRelayDataFolder() {
    void locale.value;
    try {
      await invoke("open_relay_data_folder");
    } catch (e) {
      const detail = e instanceof Error ? e.message : String(e);
      pushToast({
        type: "err",
        text: `${t("cacheOpenFolderErr")} ${detail}`.trim(),
        durationMs: 4000,
      });
    }
  }

  function openCacheClearConfirm(kind: "all" | "attachments" | "logs") {
    cacheConfirmKind.value = kind;
  }

  function cancelCacheClearConfirm() {
    cacheConfirmKind.value = null;
  }

  async function executeCacheClear() {
    const kind = cacheConfirmKind.value;
    if (!kind || cacheActionBusy.value) return;
    cacheConfirmKind.value = null;
    cacheActionBusy.value = true;
    pushToast({ type: "ok", text: t("cacheClearing"), durationMs: 8000 });
    try {
      if (kind === "all") await invoke("clear_relay_cache_all");
      else if (kind === "attachments")
        await invoke("clear_relay_cache_attachments");
      else await invoke("clear_relay_cache_logs");
      await loadCacheStats();
      pushToast({ type: "ok", text: t("cacheClearedOk"), durationMs: 5000 });
    } catch (e) {
      const detail = e instanceof Error ? e.message : String(e);
      pushToast({
        type: "err",
        text: `${t("cacheClearErr")} ${detail}`.trim(),
        durationMs: 5000,
      });
    } finally {
      cacheActionBusy.value = false;
    }
  }

  function onRetentionKeydown(ev: KeyboardEvent) {
    if (ev.key === "Escape") retentionMenuOpen.value = false;
  }

  watch(isActive, (active) => {
    if (active) {
      void loadCacheStats();
      void loadAttachmentRetention();
    } else {
      cacheConfirmKind.value = null;
      retentionMenuOpen.value = false;
    }
  });

  let docClickCapture: ((_ev: MouseEvent) => void) | null = null;
  watch(retentionMenuOpen, (open) => {
    if (open) {
      void nextTick(() => {
        docClickCapture = (e: MouseEvent) => {
          const el = retentionFieldRef.value;
          if (el && !el.contains(e.target as Node)) {
            retentionMenuOpen.value = false;
          }
        };
        document.addEventListener("click", docClickCapture, true);
      });
    } else if (docClickCapture) {
      document.removeEventListener("click", docClickCapture, true);
      docClickCapture = null;
    }
  });

  onBeforeUnmount(() => {
    if (docClickCapture) {
      document.removeEventListener("click", docClickCapture, true);
      docClickCapture = null;
    }
  });

  return {
    cacheStats,
    cacheLoadBusy,
    cacheActionBusy,
    cacheLogRelatedBytes,
    cacheTotalBytes,
    cacheUsageFlexAttach,
    cacheUsageFlexLog,
    attachmentRetentionStr,
    attachmentRetentionCustom,
    attachmentRetentionBusy,
    retentionMenuOpen,
    retentionFieldRef,
    retentionOptions,
    retentionDisplayLabel,
    cacheConfirmKind,
    cacheConfirmBody,
    loadCacheStats,
    openRelayDataFolder,
    openCacheClearConfirm,
    cancelCacheClearConfirm,
    executeCacheClear,
    pickRetentionOption,
    onRetentionKeydown,
    formatBytes,
  };
}
