<script setup lang="ts">
/**
 * Settings → Remote IDE: manage SSH reverse-tunnel connections to remote IDE hosts.
 */
import { computed, ref, watch } from "vue";
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

async function loadConnections() {
  loading.value = true;
  try {
    connections.value = await invoke<RemoteConnection[]>("remote_list_connections");
  } catch {
    connections.value = [];
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

function openAddDialog() {
  addForm.value = {
    sshTarget: "",
    sshPort: 22,
    sshKeyPath: "",
    proxyJump: "",
    ideKind: "cursor",
    remoteRelayPath: "",
  };
  showAddDialog.value = true;
}

function cancelAdd() {
  showAddDialog.value = false;
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

function ideLabel(kind: IdeKind): string {
  const map: Record<IdeKind, string> = {
    cursor: "Cursor",
    claude_code: "Claude Code",
    windsurf: "Windsurf",
    other: "Other",
  };
  return map[kind] ?? kind;
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
            <button
              type="button"
              class="setupInstallBtnCompact remoteAddBtn"
              @click="openAddDialog"
            >
              {{ S.remoteAddBtn }}
            </button>
          </div>
        </header>

        <p v-if="!connections.length && !loading" class="remoteEmptyHint">
          {{ S.remoteEmptyHint }}
        </p>

        <div v-for="conn in connections" :key="conn.id" class="remoteCard settingsCard">
          <div class="remoteCardHeader">
            <div class="remoteCardMeta">
              <span class="remoteCardTarget">{{ conn.ssh_target }}</span>
              <span class="remoteCardBadge">{{ ideLabel(conn.ide_kind) }}</span>
              <span class="remoteCardBadge remoteCardBadge--status remoteCardBadge--disconnected">
                {{ S.remoteStatusDisconnected }}
              </span>
            </div>
            <div class="remoteCardActions">
              <button
                type="button"
                class="setupInstallBtnCompact remoteActionBtn"
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
            <span v-if="conn.ssh_port !== 22" class="remoteDetailChip">Port {{ conn.ssh_port }}</span>
            <span v-if="conn.ssh_key_path" class="remoteDetailChip">Key: {{ conn.ssh_key_path }}</span>
            <span v-if="conn.proxy_jump" class="remoteDetailChip">Jump: {{ conn.proxy_jump }}</span>
            <span v-if="conn.last_connected_at" class="remoteDetailChip">Last: {{ conn.last_connected_at }}</span>
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
          <label class="remoteFormLabel remoteFormLabel--half">
            {{ S.remoteIdeKind }}
            <select v-model="addForm.ideKind" class="remoteFormInput">
              <option value="cursor">Cursor</option>
              <option value="claude_code">Claude Code</option>
              <option value="windsurf">Windsurf</option>
              <option value="other">Other</option>
            </select>
          </label>
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
            class="setupInstallBtnCompact"
            :disabled="addBusy || !addForm.sshTarget.trim()"
            @click="saveConnection"
          >
            {{ addBusy ? "…" : S.remoteSaveBtn }}
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

select.remoteFormInput {
  appearance: none;
  cursor: pointer;
}

.remoteAddModalActions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 20px;
}
</style>
