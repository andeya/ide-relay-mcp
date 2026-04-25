<script setup lang="ts">
/**
 * Settings → Application: system tray behavior + storage & cache management.
 */
import {
  useRelayCacheSettings,
  type SettingsToastPayload,
} from "../../composables/useRelayCacheSettings";
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

type PushToastFn = (_payload: SettingsToastPayload) => void;

const props = defineProps<{
  strings: Record<string, string>;
  appSegmentActive: boolean;
  pushToast: PushToastFn;
}>();

const isActive = computed(() => props.appSegmentActive);

const {
  cacheStats,
  cacheLoadBusy,
  cacheActionBusy,
  cacheLogRelatedBytes,
  cacheTotalBytes,
  cacheUsageFlexAttach,
  cacheUsageFlexLog,
  attachmentRetentionStr,
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
} = useRelayCacheSettings(isActive, (p) => props.pushToast(p));

const S = computed(() => props.strings);

const closeToTray = ref(true);
const closeToTrayBusy = ref(false);
watch(
  isActive,
  async (active) => {
    if (active) {
      try {
        closeToTray.value = await invoke<boolean>("get_close_to_tray");
      } catch {
        /* keep default */
      }
    }
  },
  { immediate: true },
);
async function onCloseToTrayChange(ev: Event) {
  if (closeToTrayBusy.value) return;
  closeToTrayBusy.value = true;
  const checked = (ev.target as HTMLInputElement).checked;
  closeToTray.value = checked;
  try {
    await invoke("set_close_to_tray", { enabled: checked });
  } catch {
    closeToTray.value = !checked;
    props.pushToast({ type: "err", text: "保存失败，请重试" });
  } finally {
    closeToTrayBusy.value = false;
  }
}

const idleTimeoutMin = ref(60);
const idleTimeoutBusy = ref(false);
const APP_IDLE_MIN = 0;
const APP_IDLE_MAX = 1440;

watch(
  isActive,
  async (active) => {
    if (active) {
      try {
        const v = await invoke<number>("get_feedback_idle_timeout_minutes");
        idleTimeoutMin.value = clampIdleTimeout(v);
      } catch {
        idleTimeoutMin.value = 60;
      }
    }
  },
  { immediate: true },
);

function clampIdleTimeout(n: number): number {
  if (!Number.isFinite(n)) return 60;
  return Math.min(APP_IDLE_MAX, Math.max(APP_IDLE_MIN, Math.round(n)));
}

async function persistIdleTimeout() {
  if (idleTimeoutBusy.value) return;
  idleTimeoutBusy.value = true;
  const next = clampIdleTimeout(Number(idleTimeoutMin.value));
  idleTimeoutMin.value = next;
  try {
    await invoke("set_feedback_idle_timeout_minutes", { minutes: next });
    props.pushToast({ type: "ok", text: props.strings.appIdleTimeoutSaved });
  } catch {
    /* ignore */
  } finally {
    idleTimeoutBusy.value = false;
  }
}

function onIdleTimeoutBlur() {
  void persistIdleTimeout();
}

const enterSubmitModOnly = ref(false);
const enterSubmitBusy = ref(false);

watch(
  isActive,
  async (active) => {
    if (active) {
      try {
        enterSubmitModOnly.value = await invoke<boolean>("get_enter_submit_requires_mod");
      } catch {
        enterSubmitModOnly.value = false;
      }
    }
  },
  { immediate: true },
);

async function onEnterSubmitChange(ev: Event) {
  if (enterSubmitBusy.value) return;
  enterSubmitBusy.value = true;
  const checked = (ev.target as HTMLInputElement).checked;
  enterSubmitModOnly.value = checked;
  try {
    await invoke("set_enter_submit_requires_mod", { enabled: checked });
    props.pushToast({ type: "ok", text: props.strings.appEnterSubmitSaved });
    window.dispatchEvent(
      new CustomEvent("relay-enter-submit-changed", { detail: checked }),
    );
  } catch {
    enterSubmitModOnly.value = !checked;
  } finally {
    enterSubmitBusy.value = false;
  }
}
</script>

<template>
  <div>
    <div v-show="appSegmentActive" class="segPanel segPanel--app">
      <div class="cachePage">
        <section class="cachePolicyCard settingsCard">
          <h4 class="cacheSectionLabel">{{ S.appTrayTitle }}</h4>
          <label class="usageToggleRow">
            <span
              class="usageToggleTrack"
              :class="{ 'usageToggleTrack--on': closeToTray }"
              role="switch"
              :aria-checked="closeToTray"
            >
              <span class="usageToggleThumb" />
            </span>
            <input
              type="checkbox"
              class="usageToggleNative"
              :checked="closeToTray"
              @change="onCloseToTrayChange"
            />
            <span>{{ S.appTrayCloseToTray }}</span>
          </label>
          <p class="cachePolicyLead">{{ S.appTrayCloseToTrayHint }}</p>
        </section>

        <section class="cachePolicyCard settingsCard">
          <h4 class="cacheSectionLabel">{{ S.appMcpWaitTitle }}</h4>
          <p class="cachePolicyLead">{{ S.appIdleTimeoutHint }}</p>
          <div class="appIdleTimeoutRow">
            <label class="appIdleTimeoutLabel" for="relayIdleTimeoutMin">{{
              S.appIdleTimeoutLabel
            }}</label>
            <input
              id="relayIdleTimeoutMin"
              v-model.number="idleTimeoutMin"
              type="number"
              class="appIdleTimeoutInput"
              :min="APP_IDLE_MIN"
              :max="APP_IDLE_MAX"
              :disabled="idleTimeoutBusy"
              @blur="onIdleTimeoutBlur"
            />
            <span class="cacheDays" aria-hidden="true">min</span>
          </div>
        </section>

        <section class="cachePolicyCard settingsCard">
          <h4 class="cacheSectionLabel">{{ S.appEnterSubmitTitle }}</h4>
          <p class="cachePolicyLead">{{ S.appEnterSubmitLabel }}</p>
          <label class="usageToggleRow">
            <span
              class="usageToggleTrack"
              :class="{ 'usageToggleTrack--on': enterSubmitModOnly }"
              role="switch"
              :aria-checked="enterSubmitModOnly"
            >
              <span class="usageToggleThumb" />
            </span>
            <input
              type="checkbox"
              class="usageToggleNative"
              :checked="enterSubmitModOnly"
              :disabled="enterSubmitBusy"
              @change="onEnterSubmitChange"
            />
            <span>{{ enterSubmitModOnly ? S.appEnterSubmitModOnly : S.appEnterSubmitPlain }}</span>
          </label>
        </section>

        <header class="cachePageHero">
          <div class="cachePageHeroRow">
            <div class="cachePageHeroText">
              <h3 class="cachePageTitle">{{ S.cacheTitle }}</h3>
              <p class="cachePageSubtitle">{{ S.cacheSubtitle }}</p>
            </div>
            <div class="cachePageHeroActions">
              <button
                type="button"
                class="setupUninstallBtnCompact cacheRefreshBtn"
                :disabled="cacheLoadBusy || cacheActionBusy"
                @click="loadCacheStats"
              >
                {{ cacheLoadBusy ? S.cacheLoading : S.cacheRefresh }}
              </button>
              <button
                type="button"
                class="setupInstallBtnCompact cacheRefreshBtn"
                :disabled="cacheActionBusy"
                @click="openRelayDataFolder"
              >
                {{ S.cacheOpenFolder }}
              </button>
            </div>
          </div>
        </header>

        <p v-if="!cacheStats && !cacheLoadBusy" class="note cacheMgmtErr">
          {{ S.cacheLoadErr }}
        </p>

        <section
          v-else-if="cacheStats"
          class="cacheSection"
          :aria-label="S.cacheSectionStorage"
        >
          <h4 class="cacheSectionLabel">{{ S.cacheSectionStorage }}</h4>
          <div class="cacheBigCards">
            <article class="cacheBigCard cacheBigCard--attach">
              <span class="cacheBigCardKicker">{{ S.cacheAttachments }}</span>
              <p class="cacheBigCardValue">
                {{ formatBytes(cacheStats.attachments_bytes) }}
              </p>
            </article>
            <article class="cacheBigCard cacheBigCard--log">
              <span class="cacheBigCardKicker">{{ S.cacheLogs }}</span>
              <p class="cacheBigCardValue">
                {{ formatBytes(cacheLogRelatedBytes) }}
              </p>
            </article>
          </div>
          <div
            class="cacheUsageBar"
            role="img"
            :aria-label="`${S.cacheAttachments} / ${S.cacheLogs}`"
          >
            <div
              class="cacheUsageBarSeg cacheUsageBarSeg--attach"
              :style="{ flex: cacheUsageFlexAttach }"
            />
            <div
              class="cacheUsageBarSeg cacheUsageBarSeg--log"
              :style="{ flex: cacheUsageFlexLog }"
            />
          </div>
          <p class="cacheTotalLine">
            <span>{{ S.cacheTotal }}</span>
            <strong>{{ formatBytes(cacheTotalBytes) }}</strong>
          </p>
        </section>

        <section class="cachePolicyCard settingsCard">
          <h4 class="cacheSectionLabel">{{ S.cacheAutoTitle }}</h4>
          <p class="cachePolicyLead">{{ S.cacheAutoLead }}</p>
          <p id="cacheRetentionLabel" class="cacheRetentionLabel">
            {{ S.cacheAutoSelectLabel }}
          </p>
          <div
            ref="retentionFieldRef"
            class="cacheRetentionField"
            @keydown="onRetentionKeydown"
          >
            <button
              type="button"
              class="cacheRetentionTrigger"
              :class="{
                'cacheRetentionTrigger--busy': attachmentRetentionBusy,
              }"
              :disabled="attachmentRetentionBusy || cacheActionBusy"
              :aria-expanded="retentionMenuOpen"
              aria-haspopup="listbox"
              :aria-label="S.cacheRetentionTriggerAria"
              aria-labelledby="cacheRetentionLabel"
              :aria-busy="attachmentRetentionBusy"
              @click.stop="retentionMenuOpen = !retentionMenuOpen"
            >
              <span class="cacheRetentionTriggerText">{{
                retentionDisplayLabel
              }}</span>
              <span
                class="cacheRetentionTriggerChev"
                :class="{
                  'cacheRetentionTriggerChev--open': retentionMenuOpen,
                }"
                aria-hidden="true"
              />
              <span
                v-show="attachmentRetentionBusy"
                class="cacheRetentionTriggerSpinner"
                aria-hidden="true"
              />
            </button>
            <Transition name="cacheRetPop">
              <ul
                v-show="retentionMenuOpen"
                role="listbox"
                class="cacheRetentionMenu"
                @click.stop
              >
                <li
                  v-for="opt in retentionOptions"
                  :key="opt.value === '' ? '__off' : opt.value"
                  role="option"
                  class="cacheRetentionMenuItem"
                  :class="{
                    'cacheRetentionMenuItem--on':
                      attachmentRetentionStr === opt.value,
                  }"
                  @click.stop="pickRetentionOption(opt.value)"
                >
                  <span class="cacheRetentionMenuDot" aria-hidden="true">
                    <svg
                      v-if="attachmentRetentionStr === opt.value"
                      class="cacheRetentionMenuCheckSvg"
                      viewBox="0 0 16 16"
                      fill="none"
                      xmlns="http://www.w3.org/2000/svg"
                    >
                      <path
                        d="M3.5 8.2 6.4 11l6.1-6.5"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                      />
                    </svg>
                  </span>
                  <span class="cacheRetentionMenuLabel">{{ opt.label }}</span>
                </li>
              </ul>
            </Transition>
          </div>
        </section>

        <section class="cacheManualCard settingsCard">
          <h4 class="cacheSectionLabel">{{ S.cacheManualTitle }}</h4>
          <p class="installHubDesc cacheManualLead">{{ S.cacheLead }}</p>
          <p class="cacheDataDirLabel">{{ S.cacheDataDir }}</p>
          <pre
            class="cachePathPre cachePathPre--compact"
            tabindex="0"
          >{{ cacheStats?.data_dir ?? "—" }}</pre>
          <div class="cacheBtnRow">
            <button
              type="button"
              class="primary cacheDangerBtn"
              :disabled="cacheActionBusy"
              @click="openCacheClearConfirm('all')"
            >
              {{ cacheActionBusy ? S.cacheBusy : S.cacheClearAll }}
            </button>
            <button
              type="button"
              class="setupUninstallBtnCompact cacheDangerBtn"
              :disabled="cacheActionBusy"
              @click="openCacheClearConfirm('attachments')"
            >
              {{ S.cacheClearAttach }}
            </button>
            <button
              type="button"
              class="setupUninstallBtnCompact cacheDangerBtn"
              :disabled="cacheActionBusy"
              @click="openCacheClearConfirm('logs')"
            >
              {{ S.cacheClearLogs }}
            </button>
          </div>
        </section>
      </div>
    </div>

    <div
      v-if="cacheConfirmKind && appSegmentActive"
      class="cacheConfirmBackdrop"
      role="presentation"
      @click.self="cancelCacheClearConfirm"
    >
      <div
        class="cacheConfirmModal"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="cacheConfirmTitle"
        @click.stop
      >
        <h4 id="cacheConfirmTitle" class="cacheConfirmTitle">
          {{ S.cacheConfirmModalTitle }}
        </h4>
        <p class="cacheConfirmBodyText">{{ cacheConfirmBody }}</p>
        <div class="cacheConfirmActions">
          <button
            type="button"
            class="setupUninstallBtnCompact"
            @click="cancelCacheClearConfirm"
          >
            {{ S.cacheCancelBtn }}
          </button>
          <button
            type="button"
            class="primary cacheDangerBtn"
            @click="executeCacheClear"
          >
            {{ S.cacheConfirmBtn }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.appIdleTimeoutRow {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem 0.75rem;
  margin-top: 0.75rem;
}
.appIdleTimeoutLabel {
  flex: 1 1 12rem;
}
.appIdleTimeoutInput {
  width: 5rem;
  padding: 0.35rem 0.5rem;
  border-radius: 8px;
  border: 1px solid var(--relay-border-muted, #3a3f4a);
  background: var(--relay-surface-2, #1e222a);
  color: inherit;
}
</style>
