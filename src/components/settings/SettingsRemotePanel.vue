<script setup lang="ts">
/**
 * Settings → Remote IDE: manage SSH reverse-tunnel connections to remote IDE hosts.
 */
import { computed, ref, watch, onBeforeUnmount } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { RemoteConnection, IdeKind } from "../../types/relay-app";
import type { SettingsToastPayload } from "../../composables/useRelayCacheSettings";

type PushToastFn = (_payload: SettingsToastPayload) => void;

const props = defineProps<{
  strings: Record<string, string>;
  remoteSegmentActive: boolean;
  pushToast: PushToastFn;
}>();

const isActive = computed(() => props.remoteSegmentActive);
const S = computed(() => props.strings);

const connections = ref<RemoteConnection[]>([]);
const loading = ref(false);

const loadError = ref(false);

async function loadConnections() {
  loading.value = true;
  loadError.value = false;
  try {
    connections.value = await invoke<RemoteConnection[]>("remote_list_connections");
  } catch {
    connections.value = [];
    loadError.value = true;
  } finally {
    loading.value = false;
  }
}

watch(isActive, (active) => {
  if (active) void loadConnections();
}, { immediate: true });

const showAddDialog = ref(false);
const addForm = ref({
  sshTarget: "",
  sshPort: 22,
  sshKeyPath: "",
  proxyJump: "",
  ideKind: "cursor" as IdeKind,
  remoteRelayPath: "",
});
const addBusy = ref(false);

// IDE kind custom dropdown
const ideKindMenuOpen = ref(false);
const ideKindOptions: { value: IdeKind; label: string }[] = [
  { value: "cursor", label: "Cursor" },
  { value: "claude_code", label: "Claude Code" },
  { value: "windsurf", label: "Windsurf" },
  { value: "other", label: "Other" },
];

function ideKindDisplayLabel(kind: IdeKind): string {
  return ideKindOptions.find((o) => o.value === kind)?.label ?? kind;
}

function selectIdeKind(kind: IdeKind) {
  addForm.value.ideKind = kind;
  ideKindMenuOpen.value = false;
}

function closeIdeKindMenu(e: MouseEvent) {
  if (!(e.target as HTMLElement)?.closest(".remoteIdeKindField")) {
    ideKindMenuOpen.value = false;
  }
}

watch(ideKindMenuOpen, (open) => {
  if (open) {
    document.addEventListener("click", closeIdeKindMenu, { capture: true });
  } else {
    document.removeEventListener("click", closeIdeKindMenu, { capture: true });
  }
});

onBeforeUnmount(() => {
  document.removeEventListener("click", closeIdeKindMenu, { capture: true });
});

function openAddDialog() {
  addForm.value = {
    sshTarget: "",
    sshPort: 22,
    sshKeyPath: "",
    proxyJump: "",
    ideKind: "cursor",
    remoteRelayPath: "",
  };
  ideKindMenuOpen.value = false;
  showAddDialog.value = true;
}

function cancelAdd() {
  showAddDialog.value = false;
  ideKindMenuOpen.value = false;
}

async function saveConnection() {
  if (addBusy.value || !addForm.value.sshTarget.trim()) return;
  addBusy.value = true;
  try {
    await invoke("remote_add_connection", {
      sshTarget: addForm.value.sshTarget.trim(),
      sshPort: addForm.value.sshPort,
      sshKeyPath: addForm.value.sshKeyPath.trim() || null,
      proxyJump: addForm.value.proxyJump.trim() || null,
      ideKind: addForm.value.ideKind,
      remoteRelayPath: addForm.value.remoteRelayPath.trim() || null,
    });
    showAddDialog.value = false;
    await loadConnections();
  } catch {
    props.pushToast({ type: "err", text: S.value.remoteSaveErr });
  } finally {
    addBusy.value = false;
  }
}

const removingId = ref<string | null>(null);
const removeConfirmId = ref<string | null>(null);

function confirmRemove(id: string) {
  removeConfirmId.value = id;
}

function cancelRemove() {
  removeConfirmId.value = null;
}

async function executeRemove() {
  const id = removeConfirmId.value;
  if (!id) return;
  removingId.value = id;
  removeConfirmId.value = null;
  try {
    await invoke("remote_remove_connection", { id });
    await loadConnections();
  } catch {
    props.pushToast({ type: "err", text: S.value.remoteRemoveErr });
  } finally {
    removingId.value = null;
  }
}

const testingId = ref<string | null>(null);

async function testConnection(conn: RemoteConnection) {
  testingId.value = conn.id;
  try {
    await invoke("remote_test_connection", {
      sshTarget: conn.ssh_target,
      sshPort: conn.ssh_port,
      sshKeyPath: conn.ssh_key_path ?? null,
      proxyJump: conn.proxy_jump ?? null,
    });
    props.pushToast({ type: "ok", text: S.value.remoteTestOk });
  } catch (e) {
    props.pushToast({ type: "err", text: `${S.value.remoteTestFail} ${e}` });
  } finally {
    testingId.value = null;
  }
}

// Routing preemption state per connection
const preemptedIds = ref<Set<string>>(new Set());
const preemptBusy = ref<string | null>(null);

async function togglePreempt(conn: RemoteConnection) {
  const wasPreempted = preemptedIds.value.has(conn.id);
  preemptBusy.value = conn.id;
  try {
    if (wasPreempted) {
      await invoke("remote_release_routing", { id: conn.id });
      preemptedIds.value.delete(conn.id);
    } else {
      await invoke("remote_preempt_routing", { id: conn.id });
      preemptedIds.value.add(conn.id);
    }
  } catch (e) {
    const msg = String(e);
    if (msg.includes("pinned")) {
      props.pushToast({ type: "err", text: S.value.remotePreemptPinnedErr ?? msg });
    } else {
      props.pushToast({ type: "err", text: `${S.value.remoteSaveErr} ${msg}` });
    }
  } finally {
    preemptBusy.value = null;
  }
}

function isPreempted(connId: string): boolean {
  return preemptedIds.value.has(connId);
}

// Local routing: two states + one-shot reclaim action
// locked = routing lock with prefer=local, pinned=true (remote cannot override)
// unlocked = no lock file (default: healthy local > remote)
// reclaim = one-shot: clear any remote preemption by writing prefer=local then clear
type RoutingLock = { prefer: string; set_by?: string; pinned?: boolean };
const localLocked = ref(false);
const routingBusy = ref(false);
const currentRouting = ref<RoutingLock | null>(null);
const isRemotePreempted = computed(() =>
  currentRouting.value?.prefer === "remote" && currentRouting.value?.set_by === "remote"
);

async function loadRoutingState() {
  try {
    const ide = await invoke<string | null>("get_ide_binding");
    if (!ide) return;
    const lock = await invoke<RoutingLock | null>("get_routing_lock", { ideKind: ide });
    currentRouting.value = lock;
    localLocked.value = lock?.pinned === true && lock?.set_by === "local";
  } catch {
    localLocked.value = false;
    currentRouting.value = null;
  }
}

async function toggleLocalLock() {
  if (routingBusy.value) return;
  routingBusy.value = true;
  try {
    const ide = await invoke<string | null>("get_ide_binding");
    if (!ide) return;
    if (localLocked.value) {
      await invoke("clear_routing_lock", { ideKind: ide });
      localLocked.value = false;
      currentRouting.value = null;
    } else {
      await invoke("set_routing_lock", {
        ideKind: ide,
        prefer: "local",
        setBy: "local",
        pinned: true,
      });
      localLocked.value = true;
      currentRouting.value = { prefer: "local", set_by: "local", pinned: true };
    }
  } catch {
    props.pushToast({ type: "err", text: S.value.remoteSaveErr });
  } finally {
    routingBusy.value = false;
  }
}

async function reclaimLocal() {
  if (routingBusy.value) return;
  routingBusy.value = true;
  try {
    const ide = await invoke<string | null>("get_ide_binding");
    if (!ide) return;
    await invoke("clear_routing_lock", { ideKind: ide });
    currentRouting.value = null;
    props.pushToast({ type: "ok", text: S.value.remoteReclaimOk });
  } catch {
    props.pushToast({ type: "err", text: S.value.remoteSaveErr });
  } finally {
    routingBusy.value = false;
  }
}

watch(isActive, (active) => {
  if (active) void loadRoutingState();
}, { immediate: true });

function ideLabel(kind: IdeKind): string {
  return ideKindDisplayLabel(kind);
}
</script>

<template>
  <div>
    <div v-show="remoteSegmentActive" class="segPanel segPanel--remote">
      <div class="remotePage">
        <header class="remoteHero">
          <div class="remoteHeroRow">
            <div class="remoteHeroText">
              <h3 class="remotePageTitle">{{ S.remoteTitle }}</h3>
              <p class="remotePageSubtitle">{{ S.remoteLead }}</p>
            </div>
            <div class="remoteHeroActions">
              <!-- Lock toggle -->
              <div class="remoteLocalLock">
                <span class="remoteLocalLockLabel">{{ S.remoteLocalPinLabel }}</span>
                <button
                  type="button"
                  class="remoteLocalLockToggle"
                  :class="{ 'remoteLocalLockToggle--on': localLocked }"
                  :disabled="routingBusy"
                  :aria-label="S.remoteLocalPinLabel"
                  :aria-checked="localLocked"
                  role="switch"
                  :title="S.remoteLocalPinHint"
                  @click="toggleLocalLock"
                >
                  <span class="remoteLocalLockKnob" />
                </button>
              </div>
              <!-- Reclaim button (visible only when remote has preempted) -->
              <button
                v-if="isRemotePreempted && !localLocked"
                type="button"
                class="remoteReclaimBtn"
                :disabled="routingBusy"
                @click="reclaimLocal"
              >
                {{ S.remoteReclaimBtn }}
              </button>
              <button
                type="button"
                class="primary setupInstallBtnCompact remoteAddBtn"
                @click="openAddDialog"
              >
                {{ S.remoteAddBtn }}
              </button>
            </div>
          </div>
        </header>

        <div v-if="loading" class="remoteLoadingRow">
          <span class="remoteSpinner" />
          <span class="remoteLoadingText">{{ S.remoteLoadingText }}</span>
        </div>

        <div v-else-if="loadError" class="remoteErrorRow">
          <span class="remoteErrorText">{{ S.remoteLoadError }}</span>
          <button type="button" class="primary setupInstallBtnCompact" @click="loadConnections">
            {{ S.remoteRetryBtn }}
          </button>
        </div>

        <p v-else-if="!connections.length" class="remoteEmptyHint">
          {{ S.remoteEmptyHint }}
        </p>

        <div v-for="conn in connections" :key="conn.id" class="remoteCard settingsCard">
          <div class="remoteCardHeader">
            <div class="remoteCardMeta">
              <span class="remoteCardTarget">{{ conn.ssh_target }}</span>
              <span class="remoteCardBadge">{{ ideLabel(conn.ide_kind) }}</span>
            </div>
            <div class="remoteCardActions">
              <button
                type="button"
                class="remotePreemptBtn remoteActionBtn"
                :class="{
                  'remotePreemptBtn--active': isPreempted(conn.id),
                }"
                :disabled="preemptBusy === conn.id"
                @click="togglePreempt(conn)"
              >
                {{ isPreempted(conn.id) ? S.remoteReleaseBtn : S.remotePreemptBtn }}
              </button>
              <button
                type="button"
                class="primary setupInstallBtnCompact remoteActionBtn"
                :disabled="testingId === conn.id"
                @click="testConnection(conn)"
              >
                {{ testingId === conn.id ? S.remoteTestingBtn : S.remoteTestBtn }}
              </button>
              <button
                type="button"
                class="setupUninstallBtnCompact remoteActionBtn"
                :disabled="removingId === conn.id"
                @click="confirmRemove(conn.id)"
              >
                {{ S.remoteRemoveBtn }}
              </button>
            </div>
          </div>
          <div class="remoteCardDetails">
            <span v-if="conn.ssh_port !== 22" class="remoteDetailChip">{{ S.remoteDetailPort }} {{ conn.ssh_port }}</span>
            <span v-if="conn.ssh_key_path" class="remoteDetailChip">{{ S.remoteDetailKey }} {{ conn.ssh_key_path }}</span>
            <span v-if="conn.proxy_jump" class="remoteDetailChip">{{ S.remoteDetailJump }} {{ conn.proxy_jump }}</span>
            <span v-if="conn.last_connected_at" class="remoteDetailChip">{{ S.remoteDetailLast }} {{ conn.last_connected_at }}</span>
            <span
              v-if="isPreempted(conn.id)"
              class="remoteDetailChip remoteDetailChip--routing"
            >{{ S.remoteRoutingRemote }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Add dialog -->
    <div
      v-if="showAddDialog && remoteSegmentActive"
      class="cacheConfirmBackdrop"
      role="presentation"
      @click.self="cancelAdd"
    >
      <div
        class="remoteAddModal"
        role="dialog"
        aria-modal="true"
        @click.stop
        @keydown.escape.stop="cancelAdd"
      >
        <h4 class="remoteAddModalTitle">{{ S.remoteAddTitle }}</h4>

        <label class="remoteFormLabel">
          {{ S.remoteSshTarget }}
          <input
            v-model="addForm.sshTarget"
            type="text"
            class="remoteFormInput"
            placeholder="user@192.168.1.100"
          />
        </label>

        <div class="remoteFormRow">
          <label class="remoteFormLabel remoteFormLabel--half">
            {{ S.remoteSshPort }}
            <input
              v-model.number="addForm.sshPort"
              type="number"
              class="remoteFormInput"
              min="1"
              max="65535"
            />
          </label>
          <div class="remoteFormLabel remoteFormLabel--half">
            <span class="remoteFormLabelText">{{ S.remoteIdeKind }}</span>
            <div class="remoteIdeKindField">
              <button
                type="button"
                class="remoteIdeKindTrigger"
                :aria-expanded="ideKindMenuOpen"
                aria-haspopup="listbox"
                @click.stop="ideKindMenuOpen = !ideKindMenuOpen"
              >
                <span class="remoteIdeKindTriggerText">{{ ideKindDisplayLabel(addForm.ideKind) }}</span>
                <span
                  class="remoteIdeKindChev"
                  :class="{ 'remoteIdeKindChev--open': ideKindMenuOpen }"
                  aria-hidden="true"
                />
              </button>
              <Transition name="remoteIdePop">
                <ul
                  v-show="ideKindMenuOpen"
                  role="listbox"
                  class="remoteIdeKindMenu"
                  @click.stop
                >
                  <li
                    v-for="opt in ideKindOptions"
                    :key="opt.value"
                    role="option"
                    class="remoteIdeKindMenuItem"
                    :class="{ 'remoteIdeKindMenuItem--on': addForm.ideKind === opt.value }"
                    :aria-selected="addForm.ideKind === opt.value"
                    @click="selectIdeKind(opt.value)"
                  >
                    <span class="remoteIdeKindMenuDot">
                      <svg
                        v-if="addForm.ideKind === opt.value"
                        class="remoteIdeKindMenuCheckSvg"
                        viewBox="0 0 12 12"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                      >
                        <polyline points="2.5 6 5.5 9 10 3" />
                      </svg>
                    </span>
                    <span class="remoteIdeKindMenuLabel">{{ opt.label }}</span>
                  </li>
                </ul>
              </Transition>
            </div>
          </div>
        </div>

        <label class="remoteFormLabel">
          {{ S.remoteSshKey }}
          <input
            v-model="addForm.sshKeyPath"
            type="text"
            class="remoteFormInput"
            placeholder="~/.ssh/id_rsa"
          />
        </label>

        <label class="remoteFormLabel">
          {{ S.remoteProxyJump }}
          <input
            v-model="addForm.proxyJump"
            type="text"
            class="remoteFormInput"
            placeholder="bastion@jump.example.com"
          />
        </label>

        <label class="remoteFormLabel">
          {{ S.remoteRelayPath }}
          <input
            v-model="addForm.remoteRelayPath"
            type="text"
            class="remoteFormInput"
            placeholder="auto-detect"
          />
        </label>

        <div class="remoteAddModalActions">
          <button
            type="button"
            class="setupUninstallBtnCompact"
            @click="cancelAdd"
          >
            {{ S.remoteCancelBtn }}
          </button>
          <button
            type="button"
            class="primary setupInstallBtnCompact"
            :disabled="addBusy || !addForm.sshTarget.trim()"
            @click="saveConnection"
          >
            {{ addBusy ? S.remoteSavingBtn : S.remoteSaveBtn }}
          </button>
        </div>
      </div>
    </div>

    <!-- Remove confirm -->
    <div
      v-if="removeConfirmId && remoteSegmentActive"
      class="cacheConfirmBackdrop"
      role="presentation"
      @click.self="cancelRemove"
    >
      <div
        class="cacheConfirmModal"
        role="alertdialog"
        aria-modal="true"
        @click.stop
      >
        <h4 class="cacheConfirmTitle">{{ S.remoteRemoveBtn }}</h4>
        <p class="cacheConfirmBodyText">{{ S.remoteRemoveConfirm }}</p>
        <div class="cacheConfirmActions">
          <button
            type="button"
            class="setupUninstallBtnCompact"
            @click="cancelRemove"
          >
            {{ S.remoteCancelBtn }}
          </button>
          <button
            type="button"
            class="primary cacheDangerBtn"
            @click="executeRemove"
          >
            {{ S.remoteRemoveConfirmBtn }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.remotePage {
  padding: 0;
}

.remoteHero {
  margin-bottom: 16px;
}

.remoteHeroRow {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.remotePageTitle {
  font-size: 1.125rem;
  font-weight: 700;
  color: #e2e8f0;
  margin: 0 0 4px;
}

.remotePageSubtitle {
  font-size: 0.8125rem;
  color: #94a3b8;
  margin: 0;
  line-height: 1.5;
}

.remoteAddBtn {
  white-space: nowrap;
  flex-shrink: 0;
}

/* Hero actions row */
.remoteHeroActions {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
}

/* Local lock toggle (compact) */
.remoteLocalLock {
  display: flex;
  align-items: center;
  gap: 6px;
}
.remoteLocalLockLabel {
  font-size: 0.6875rem;
  font-weight: 600;
  color: #94a3b8;
  white-space: nowrap;
}
.remoteLocalLockToggle {
  position: relative;
  flex-shrink: 0;
  width: 36px;
  height: 20px;
  border-radius: 10px;
  border: none;
  padding: 0;
  cursor: pointer;
  background: rgba(148, 163, 184, 0.2);
  transition: background 0.2s ease;
}
.remoteLocalLockToggle--on {
  background: rgba(34, 197, 94, 0.5);
}
.remoteLocalLockToggle:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.remoteLocalLockKnob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #e2e8f0;
  transition: transform 0.2s ease;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
}
.remoteLocalLockToggle--on .remoteLocalLockKnob {
  transform: translateX(16px);
}

/* Reclaim button */
.remoteReclaimBtn {
  padding: 4px 10px;
  font-size: 0.6875rem;
  font-weight: 600;
  color: #f59e0b;
  background: rgba(245, 158, 11, 0.12);
  border: 1px solid rgba(245, 158, 11, 0.25);
  border-radius: 6px;
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.15s ease;
}
.remoteReclaimBtn:hover:not(:disabled) {
  background: rgba(245, 158, 11, 0.2);
}
.remoteReclaimBtn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}


.remoteEmptyHint {
  text-align: center;
  color: #64748b;
  font-size: 0.8125rem;
  padding: 32px 0;
}

.remoteCard {
  margin-bottom: 12px;
  padding: 14px 16px;
  border-radius: 12px;
  background: rgba(15, 23, 42, 0.45);
  border: 1px solid rgba(148, 163, 184, 0.1);
}

.remoteCardHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.remoteCardMeta {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.remoteCardTarget {
  font-size: 0.9375rem;
  font-weight: 600;
  color: #e2e8f0;
}

.remoteCardBadge {
  font-size: 0.6875rem;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 6px;
  background: rgba(148, 163, 184, 0.15);
  color: #94a3b8;
}

.remoteCardBadge--status {
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.remoteCardBadge--connected {
  background: rgba(34, 197, 94, 0.15);
  color: #22c55e;
}

.remoteCardBadge--disconnected {
  background: rgba(148, 163, 184, 0.1);
  color: #64748b;
}

.remoteCardBadge--error {
  background: rgba(239, 68, 68, 0.15);
  color: #ef4444;
}

.remoteCardActions {
  display: flex;
  gap: 8px;
}

.remoteActionBtn {
  font-size: 0.75rem !important;
  padding: 4px 10px !important;
}

.remoteCardDetails {
  display: flex;
  gap: 6px;
  margin-top: 8px;
  flex-wrap: wrap;
}

.remoteDetailChip {
  font-size: 0.6875rem;
  color: #64748b;
  background: rgba(148, 163, 184, 0.08);
  padding: 2px 6px;
  border-radius: 4px;
}

.remoteDetailChip--routing {
  background: rgba(251, 146, 60, 0.12);
  color: #fb923c;
  font-weight: 600;
}

/* Preempt toggle button */
.remotePreemptBtn {
  padding: 4px 10px !important;
  font-size: 0.75rem !important;
  font-weight: 600;
  border-radius: 8px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  background: rgba(15, 23, 42, 0.65);
  color: #94a3b8;
  cursor: pointer;
  transition: all 0.15s ease;
}

.remotePreemptBtn:hover {
  border-color: rgba(251, 146, 60, 0.45);
  color: #fb923c;
  background: rgba(251, 146, 60, 0.08);
}

.remotePreemptBtn--active {
  border-color: rgba(251, 146, 60, 0.45);
  background: rgba(251, 146, 60, 0.12);
  color: #fb923c;
}

.remotePreemptBtn--active:hover {
  background: rgba(251, 146, 60, 0.06);
  color: #94a3b8;
  border-color: rgba(148, 163, 184, 0.22);
}

/* Add dialog */
.remoteAddModal {
  background: #0f172a;
  border: 1px solid rgba(148, 163, 184, 0.15);
  border-radius: 16px;
  padding: 24px;
  max-width: 480px;
  width: 90vw;
  box-shadow: 0 24px 48px rgba(0, 0, 0, 0.4);
}

.remoteAddModalTitle {
  font-size: 1rem;
  font-weight: 700;
  color: #e2e8f0;
  margin: 0 0 16px;
}

.remoteFormLabel {
  display: block;
  font-size: 0.8125rem;
  font-weight: 500;
  color: #94a3b8;
  margin-bottom: 12px;
}

.remoteFormLabel--half {
  flex: 1 1 0;
  min-width: 0;
}

.remoteFormRow {
  display: flex;
  gap: 12px;
}

.remoteFormInput {
  display: block;
  width: 100%;
  margin-top: 4px;
  padding: 8px 12px;
  font-size: 0.875rem;
  border-radius: 10px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  background: rgba(15, 23, 42, 0.65);
  color: #e2e8f0;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
  box-sizing: border-box;
}

.remoteFormInput:focus-visible {
  outline: none;
  border-color: rgba(34, 211, 238, 0.55);
  box-shadow: 0 0 0 3px rgba(34, 211, 238, 0.15);
}

.remoteFormLabelText {
  display: block;
}

/* IDE kind custom dropdown */
.remoteIdeKindField {
  position: relative;
  width: 100%;
  margin-top: 4px;
  z-index: 2;
}

.remoteIdeKindTrigger {
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;
  padding: 8px 12px;
  font-size: 0.875rem;
  border-radius: 10px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  background: rgba(15, 23, 42, 0.65);
  color: #e2e8f0;
  cursor: pointer;
  text-align: left;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.remoteIdeKindTrigger:hover:not(:disabled) {
  border-color: rgba(34, 211, 238, 0.45);
}

.remoteIdeKindTrigger:focus-visible {
  outline: none;
  border-color: rgba(34, 211, 238, 0.7);
  box-shadow: 0 0 0 3px rgba(34, 211, 238, 0.2);
}

.remoteIdeKindTriggerText {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.remoteIdeKindChev {
  flex-shrink: 0;
  width: 8px;
  height: 8px;
  margin-left: 8px;
  border-right: 2px solid rgba(148, 163, 184, 0.6);
  border-bottom: 2px solid rgba(148, 163, 184, 0.6);
  transform: rotate(45deg) translateY(-2px);
  transition: transform 0.22s ease, border-color 0.18s ease;
}

.remoteIdeKindChev--open {
  transform: rotate(225deg) translateY(2px);
  border-color: #22d3ee;
}

.remoteIdeKindMenu {
  position: absolute;
  left: 0;
  right: 0;
  top: calc(100% + 6px);
  margin: 0;
  padding: 6px;
  list-style: none;
  background: #0f172a;
  border: 1px solid rgba(148, 163, 184, 0.15);
  border-radius: 12px;
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.5);
  overflow-y: auto;
  max-height: 240px;
}

.remoteIdeKindMenuItem {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 8px;
  font-size: 0.8125rem;
  color: #94a3b8;
  cursor: pointer;
  transition: background 0.12s ease, color 0.12s ease;
}

.remoteIdeKindMenuItem:hover {
  background: rgba(34, 211, 238, 0.08);
  color: #f8fafc;
}

.remoteIdeKindMenuItem--on {
  background: rgba(34, 211, 238, 0.1);
  color: #a5f3fc;
}

.remoteIdeKindMenuItem--on:hover {
  background: rgba(34, 211, 238, 0.14);
}

.remoteIdeKindMenuDot {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 1.5px solid rgba(148, 163, 184, 0.25);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: border-color 0.15s ease, background 0.15s ease;
}

.remoteIdeKindMenuItem--on .remoteIdeKindMenuDot {
  border-color: rgba(34, 211, 238, 0.65);
  background: rgba(34, 211, 238, 0.12);
  color: #22d3ee;
}

.remoteIdeKindMenuCheckSvg {
  width: 10px;
  height: 10px;
}

.remoteIdeKindMenuLabel {
  flex: 1;
  min-width: 0;
}

.remoteIdePop-enter-active,
.remoteIdePop-leave-active {
  transition: opacity 0.18s ease, transform 0.18s ease;
}

.remoteIdePop-enter-from,
.remoteIdePop-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

.remoteAddModalActions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 20px;
}

/* ─── Loading / Error rows ─── */
.remoteLoadingRow,
.remoteErrorRow {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 0;
}
.remoteSpinner {
  width: 18px;
  height: 18px;
  border: 2px solid rgba(255, 255, 255, 0.15);
  border-top-color: #4ade80;
  border-radius: 50%;
  animation: remoteSpin 0.7s linear infinite;
}
@keyframes remoteSpin {
  to { transform: rotate(360deg); }
}
.remoteLoadingText {
  color: #94a3b8;
  font-size: 0.85rem;
}
.remoteErrorText {
  color: #f87171;
  font-size: 0.85rem;
}
</style>
