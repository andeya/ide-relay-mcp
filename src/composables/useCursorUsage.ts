import { computed, onBeforeUnmount, ref, watch, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { t } from "../i18n";
import type {
  CursorUsageSummary,
  CursorUsageEventsPage,
  CursorUsageSettings,
  CursorUsageEvent,
  FeedbackTabsState,
} from "../types/relay-app";

export type UsageToastPayload = {
  type: "ok" | "warn" | "err";
  text: string;
  durationMs?: number;
};

export type CursorUsageState = ReturnType<typeof useCursorUsage>;

/**
 * Cursor Usage composable: token state, usage summary, events, refresh logic.
 * @param isSettingsActive — true when Settings > Usage tab is visible
 * @param pushToast — parent-owned toast
 */
export function useCursorUsage(
  isSettingsActive: Ref<boolean>,
  pushToast: (_payload: UsageToastPayload) => void,
) {
  const usageSummary = ref<CursorUsageSummary | null>(null);
  const usageEvents = ref<CursorUsageEvent[]>([]);
  const usageEventsTotal = ref(0);
  const usageEventsPage = ref(1);
  const loading = ref(false);
  const loadingEvents = ref(false);
  const error = ref("");
  const settings = ref<CursorUsageSettings>({
    refresh_interval_minutes: 10,
  });
  const popoverOpen = ref(false);
  const lastRefreshed = ref<Date | null>(null);

  let refreshTimer: ReturnType<typeof setInterval> | null = null;
  let tabsUnlisten: UnlistenFn | null = null;
  let lastTabCount = -1;
  /** After first refresh attempt so we do not treat initial tab list as "new tabs". */
  let initialRefreshDone = false;
  /** Min interval between usage refreshes triggered by new Relay tabs (ms). */
  const NEW_TAB_REFRESH_MIN_MS = 60_000;
  let throttleUntil = 0;

  const usageCapsuleLabel = computed(() => {
    const s = usageSummary.value;
    if (!s) return "";
    const used = Math.round(s.individualUsage.plan.used);
    const limit = Math.round(s.individualUsage.plan.limit);
    return `${used}/${limit}`;
  });

  const usageCapsuleWarn = computed(() => {
    const s = usageSummary.value;
    if (!s) return false;
    return s.individualUsage.plan.remaining <= 0;
  });

  /** Used/limit as %; can exceed 100 when IDE-reported requests are over the plan cap. */
  const planUsagePct = computed(() => {
    const s = usageSummary.value;
    if (!s || s.individualUsage.plan.limit <= 0) return 0;
    return (s.individualUsage.plan.used / s.individualUsage.plan.limit) * 100;
  });

  /** Progress bar width only (capped so layout stays valid). */
  const planProgressPct = computed(() => Math.min(100, planUsagePct.value));

  const cycleStartDate = computed(() => {
    const s = usageSummary.value;
    if (!s?.billingCycleStart) return null;
    return new Date(s.billingCycleStart);
  });

  const cycleResetDate = computed(() => {
    const s = usageSummary.value;
    if (s?.billingCycleEnd) return new Date(s.billingCycleEnd);
    const start = cycleStartDate.value;
    if (!start) return null;
    const reset = new Date(start);
    reset.setMonth(reset.getMonth() + 1);
    return reset;
  });

  const daysUntilReset = computed(() => {
    const reset = cycleResetDate.value;
    if (!reset) return null;
    const now = new Date();
    const diff = reset.getTime() - now.getTime();
    return Math.max(0, Math.ceil(diff / (1000 * 60 * 60 * 24)));
  });

  const daysSinceCycleStart = computed(() => {
    const start = cycleStartDate.value;
    if (!start) return 0;
    const now = new Date();
    const diff = now.getTime() - start.getTime();
    return Math.max(1, Math.ceil(diff / (1000 * 60 * 60 * 24)));
  });

  const avgRequestsPerDay = computed(() => {
    const s = usageSummary.value;
    if (!s) return 0;
    const days = daysSinceCycleStart.value;
    return days > 0 ? s.individualUsage.plan.used / days : 0;
  });

  const daysUntilExhausted = computed(() => {
    const s = usageSummary.value;
    if (!s) return null;
    const avg = avgRequestsPerDay.value;
    if (avg <= 0) return null;
    const remaining = s.individualUsage.plan.remaining;
    if (remaining <= 0) return 0;
    return Math.round(remaining / avg);
  });

  const PLAN_PRICE_MONTHLY: Record<string, number | null> = {
    free: 0,
    hobby: 20,
    pro: 20,
    business: 40,
    enterprise: null,
  };

  const planPriceLabel = computed(() => {
    const s = usageSummary.value;
    if (!s) return null;
    const key = s.membershipType.toLowerCase();
    const monthly = PLAN_PRICE_MONTHLY[key];
    if (monthly === undefined) return null;
    if (monthly === null) return t("usagePlanCustom");
    if (s.isYearlyPlan) return `$${monthly * 12}/yr`;
    return `$${monthly}/mo`;
  });

  async function loadSettings() {
    try {
      settings.value = await invoke<CursorUsageSettings>(
        "get_cursor_usage_settings",
      );
    } catch {
      /* keep defaults */
    }
  }

  async function saveSettings() {
    try {
      await invoke("set_cursor_usage_settings", { settings: settings.value });
    } catch (e) {
      const detail = e instanceof Error ? e.message : String(e);
      pushToast({ type: "err", text: detail, durationMs: 4000 });
    }
    resetRefreshTimer();
  }

  let retryCount = 0;
  const MAX_AUTO_RETRIES = 2;
  let retryTimer: ReturnType<typeof setTimeout> | null = null;

  async function refreshUsage(isRetry = false) {
    if (loading.value) return;
    if (!isRetry) retryCount = 0;
    loading.value = true;
    error.value = "";
    let willRetry = false;
    try {
      usageSummary.value = await invoke<CursorUsageSummary>(
        "fetch_cursor_usage_via_ide",
      );
      lastRefreshed.value = new Date();
    } catch (e) {
      const detail = e instanceof Error ? e.message : String(e);
      error.value = detail;
      if (retryCount < MAX_AUTO_RETRIES && !usageSummary.value) {
        retryCount++;
        willRetry = true;
        retryTimer = setTimeout(() => void refreshUsage(true), 3000 * retryCount);
      }
    } finally {
      loading.value = false;
      initialRefreshDone = true;
      if (!willRetry) resetRefreshTimer();
    }
  }

  async function loadEvents(page = 1) {
    if (loadingEvents.value) return;
    loadingEvents.value = true;
    try {
      const s = usageSummary.value;
      const startDate = s?.billingCycleStart ?? "";
      const endDate = s?.billingCycleEnd ?? "";
      if (!startDate || !endDate) {
        loadingEvents.value = false;
        return;
      }
      const result = await invoke<CursorUsageEventsPage>(
        "fetch_cursor_usage_events",
        { startDate, endDate, page, pageSize: 20 },
      );
      if (page === 1) {
        usageEvents.value = result.usageEventsDisplay;
      } else {
        usageEvents.value = [
          ...usageEvents.value,
          ...result.usageEventsDisplay,
        ];
      }
      usageEventsTotal.value = result.totalUsageEventsCount;
      usageEventsPage.value = page;
    } catch {
      /* Events API may not be available for all plan types */
    } finally {
      loadingEvents.value = false;
    }
  }

  function resetRefreshTimer() {
    if (refreshTimer) {
      clearInterval(refreshTimer);
      refreshTimer = null;
    }
    const mins = settings.value.refresh_interval_minutes;
    if (mins > 0) {
      refreshTimer = setInterval(
        () => {
          if (document.visibilityState !== "hidden") {
            void refreshUsage();
          }
        },
        mins * 60 * 1000,
      );
    }
  }

  async function setupTabListener() {
    tabsUnlisten = await listen("relay_tabs_changed", async () => {
      let count = -1;
      try {
        const state = await invoke<FeedbackTabsState>("get_feedback_tabs");
        count = state.tabs.length;
      } catch {
        return;
      }
      if (lastTabCount < 0) {
        lastTabCount = count;
        return;
      }
      if (initialRefreshDone && count > lastTabCount) {
        const now = Date.now();
        if (now > throttleUntil) {
          throttleUntil = now + NEW_TAB_REFRESH_MIN_MS;
          void refreshUsage();
        }
      }
      lastTabCount = count;
    });
  }

  async function init() {
    await loadSettings();
    void refreshUsage();
    void setupTabListener();
  }

  watch(isSettingsActive, (active) => {
    if (active) void loadSettings();
  });

  void init();

  onBeforeUnmount(() => {
    if (refreshTimer) clearInterval(refreshTimer);
    if (retryTimer) clearTimeout(retryTimer);
    if (tabsUnlisten) tabsUnlisten();
  });

  return {
    usageSummary,
    usageEvents,
    usageEventsTotal,
    usageEventsPage,
    loading,
    loadingEvents,
    error,
    settings,
    popoverOpen,
    lastRefreshed,
    usageCapsuleLabel,
    usageCapsuleWarn,
    planUsagePct,
    planProgressPct,
    cycleResetDate,
    daysUntilReset,
    daysSinceCycleStart,
    avgRequestsPerDay,
    daysUntilExhausted,
    planPriceLabel,
    refreshUsage,
    loadEvents,
    saveSettings,
    resetRefreshTimer,
  };
}
