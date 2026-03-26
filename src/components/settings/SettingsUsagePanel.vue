<script setup lang="ts">
/**
 * Settings → Cursor Usage: token config, refresh policy, usage preview.
 */
import { computed, ref, watch } from "vue";
import {
  useCursorUsage,
  type UsageToastPayload,
} from "../../composables/useCursorUsage";
import type { CursorUsageEvent } from "../../types/relay-app";

type PushToastFn = (_payload: UsageToastPayload) => void;

const props = defineProps<{
  strings: Record<string, string>;
  usageSegmentActive: boolean;
  pushToast: PushToastFn;
}>();

const isActive = computed(() => props.usageSegmentActive);

const {
  usageSummary,
  loading,
  error,
  settings,
  lastRefreshed,
  usageCapsuleLabel,
  planProgressPct,
  usageEvents,
  usageEventsTotal,
  usageEventsPage,
  loadingEvents,
  refreshUsage,
  loadEvents,
  saveSettings,
} = useCursorUsage(isActive, (p) => props.pushToast(p));

const S = computed(() => props.strings);

const INTERVAL_OPTIONS = [5, 10, 15, 30, 60] as const;

function onRefreshOnNewSessionChange(ev: Event) {
  const checked = (ev.target as HTMLInputElement).checked;
  settings.value = { ...settings.value, refresh_on_new_session: checked };
  void saveSettings();
}

function onRefreshIntervalChange(ev: Event) {
  const val = Number((ev.target as HTMLSelectElement).value);
  settings.value = { ...settings.value, refresh_interval_minutes: val };
  void saveSettings();
}

const lastRefreshedLabel = computed(() => {
  const d = lastRefreshed.value;
  if (!d) return S.value.usageNever;
  return d.toLocaleTimeString();
});

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

watch(isActive, (active) => {
  if (active && !usageSummary.value) {
    void refreshUsage();
  }
  if (active && usageSummary.value && !usageEvents.value.length) {
    void loadEvents(1);
  }
});
</script>

<template>
  <div>
    <div v-show="usageSegmentActive" class="segPanel segPanel--usage">
      <div class="usagePage">
        <header class="usagePageHero">
          <div class="usagePageHeroRow">
            <div class="usagePageHeroText">
              <h3 class="usagePageTitle">{{ S.usageTitle }}</h3>
              <p class="usagePageSubtitle">{{ S.usageLead }}</p>
            </div>
            <div class="usagePageHeroActions">
              <button
                type="button"
                class="usageBtn usageBtn--accent"
                :disabled="loading"
                @click="refreshUsage"
              >
                {{ loading ? S.usageRefreshing : S.usageRefreshBtn }}
              </button>
            </div>
          </div>
        </header>

        <!-- Auth status card -->
        <section class="usageSettingsCard setupMcpPauseCard">
          <h4 class="usageSettingsCardTitle">{{ S.usageSettingsTokenTitle }}</h4>
          <p class="usageSettingsHint">{{ S.usageSettingsAutoHint }}</p>

          <div v-if="error" class="usageTokenStatus">
            <span class="usageTokenDot usageTokenDot--empty" />
            <span>{{ S.usageSettingsIdeLoginHint }}</span>
          </div>
          <div v-else class="usageTokenStatus">
            <span class="usageTokenDot usageTokenDot--ok" />
            <span>{{ S.usageSettingsTokenConfigured }}</span>
          </div>
        </section>

        <!-- Refresh policy card -->
        <section class="usageSettingsCard setupMcpPauseCard">
          <h4 class="usageSettingsCardTitle">
            {{ S.usageSettingsRefreshTitle }}
          </h4>

          <label class="usageToggleRow">
            <span
              class="usageToggleTrack"
              :class="{ 'usageToggleTrack--on': settings.refresh_on_new_session }"
              role="switch"
              :aria-checked="settings.refresh_on_new_session"
            >
              <span class="usageToggleThumb" />
            </span>
            <input
              type="checkbox"
              class="usageToggleNative"
              :checked="settings.refresh_on_new_session"
              @change="onRefreshOnNewSessionChange"
            />
            <span>{{ S.usageSettingsRefreshOnNewSession }}</span>
          </label>

          <div class="usageIntervalRow">
            <span>{{ S.usageSettingsRefreshInterval }}</span>
            <div class="usageSelectWrap">
              <select
                :value="settings.refresh_interval_minutes"
                class="usageIntervalSelect"
                @change="onRefreshIntervalChange"
              >
                <option
                  v-for="opt in INTERVAL_OPTIONS"
                  :key="opt"
                  :value="opt"
                >
                  {{ opt }} {{ S.usageSettingsRefreshIntervalUnit }}
                </option>
              </select>
              <span class="usageSelectChevron" aria-hidden="true">&#x25BE;</span>
            </div>
          </div>
        </section>

        <!-- Usage preview -->
        <section
          v-if="usageSummary"
          class="usageSettingsCard usagePreviewCard"
        >
          <h4 class="usageSettingsCardTitle">{{ S.usageMonthSummary }}</h4>
          <div class="usagePreviewGrid">
            <div class="usagePreviewItem">
              <span class="usagePreviewLabel">{{ S.usageMembership }}</span>
              <span class="usagePreviewValue">{{
                usageSummary.membershipType
              }}</span>
            </div>
            <div class="usagePreviewItem">
              <span class="usagePreviewLabel">{{ S.usagePlanUsed }}</span>
              <span class="usagePreviewValue">{{ usageCapsuleLabel }}</span>
            </div>
            <div class="usagePreviewItem">
              <span class="usagePreviewLabel">{{ S.usageBillingCycle }}</span>
              <span class="usagePreviewValue"
                >{{ usageSummary.billingCycleStart?.slice(0, 10) }} →
                {{ usageSummary.billingCycleEnd?.slice(0, 10) }}</span
              >
            </div>
            <div class="usagePreviewItem">
              <span class="usagePreviewLabel">{{ S.usageOnDemandUsed }}</span>
              <span class="usagePreviewValue">
                ${{ (usageSummary.individualUsage.onDemand.used / 100).toFixed(2) }}
                <template v-if="usageSummary.individualUsage.onDemand.limit > 0">
                  / ${{ (usageSummary.individualUsage.onDemand.limit / 100).toFixed(2) }}
                </template>
              </span>
            </div>
            <div class="usagePreviewItem">
              <span class="usagePreviewLabel">{{ S.usageOnDemandCap }}</span>
              <span class="usagePreviewValue">
                <template v-if="usageSummary.individualUsage.onDemand.limit > 0">
                  ${{ (usageSummary.individualUsage.onDemand.limit / 100).toFixed(2) }}
                </template>
                <template v-else-if="usageSummary.onDemandAutoEnabled">
                  {{ S.usageOnDemandNoLimit }}
                </template>
                <template v-else>
                  {{ S.usageOnDemandDisabled }}
                </template>
              </span>
            </div>
            <div v-if="usageSummary.teamUsage" class="usagePreviewItem">
              <span class="usagePreviewLabel">{{ S.usageTeamOnDemand }}</span>
              <span class="usagePreviewValue">${{ (usageSummary.teamUsage.onDemand.used / 100).toFixed(2) }}</span>
            </div>
          </div>
          <div class="usageProgressBar">
            <div
              class="usageProgressBarFill"
              :style="{ width: planProgressPct + '%' }"
            />
          </div>
          <p class="usagePreviewFooter">
            {{ S.usageLastRefreshed }}: {{ lastRefreshedLabel }}
          </p>
        </section>

        <!-- Recent usage events -->
        <section v-if="usageSummary" class="usageSettingsCard setupMcpPauseCard">
          <h4 class="usageSettingsCardTitle">{{ S.usageRecentTitle }}</h4>
          <div v-if="usageEvents.length" class="usageEventsTable">
            <div
              v-for="(ev, idx) in usageEvents"
              :key="idx"
              class="usageSettingsEventRow"
              @mouseenter="onEventMouseEnter(ev, $event)"
              @mouseleave="onEventMouseLeave"
            >
              <span class="usageEventCell">{{ formatEventTime(ev.timestamp) }}</span>
              <span class="usageEventCell">{{ ev.model }}</span>
              <span class="usageEventCell">
                {{ ev.tokenUsage ? `${Math.round(ev.tokenUsage.inputTokens + ev.tokenUsage.outputTokens)} tok` : '' }}
              </span>
              <span class="usageEventCell">
                {{ ev.chargedCents > 0 ? `$${(ev.chargedCents / 100).toFixed(3)}` : ev.requestsCosts ? `${ev.requestsCosts} req` : '—' }}
              </span>
            </div>
          </div>
          <p v-else class="usagePopoverNoEvents">{{ S.usageNoEvents }}</p>
          <button
            v-if="usageEvents.length < usageEventsTotal"
            type="button"
            class="usagePopoverLoadMore"
            :disabled="loadingEvents"
            @click="loadEvents(usageEventsPage + 1)"
          >
            {{ loadingEvents ? S.usageRefreshing : S.usageLoadMore }}
          </button>
        </section>

        <p v-if="error" class="note usageErr">
          {{ S.usageRefreshErr }} {{ error }}
        </p>
      </div>
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
          <div class="eventTooltipRow"><span class="eventTooltipLabel">Input</span><span>{{ hoveredEvent.tokenUsage.inputTokens.toLocaleString() }} tok</span></div>
          <div class="eventTooltipRow"><span class="eventTooltipLabel">Output</span><span>{{ hoveredEvent.tokenUsage.outputTokens.toLocaleString() }} tok</span></div>
          <div v-if="hoveredEvent.tokenUsage.cacheReadTokens" class="eventTooltipRow"><span class="eventTooltipLabel">Cache read</span><span>{{ hoveredEvent.tokenUsage.cacheReadTokens.toLocaleString() }} tok</span></div>
          <div v-if="hoveredEvent.tokenUsage.cacheWriteTokens" class="eventTooltipRow"><span class="eventTooltipLabel">Cache write</span><span>{{ hoveredEvent.tokenUsage.cacheWriteTokens.toLocaleString() }} tok</span></div>
          <div class="eventTooltipRow"><span class="eventTooltipLabel">Cost</span><span>${{ (hoveredEvent.tokenUsage.totalCents / 100).toFixed(4) }}</span></div>
        </template>
        <div v-if="hoveredEvent.chargedCents > 0" class="eventTooltipRow eventTooltipRow--highlight"><span class="eventTooltipLabel">Charged</span><span>${{ (hoveredEvent.chargedCents / 100).toFixed(3) }}</span></div>
      </div>
    </Teleport>
  </div>
</template>
