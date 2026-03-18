<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";

type LaunchState = {
  summary: string;
  result_file: string;
  control_file: string;
  title: string;
};

type ControlStatus = "active" | "timed_out" | "cancelled" | null;
type DragDropUnlisten = (() => void) | undefined;

const launch = ref<LaunchState | null>(null);
const feedback = ref("");
const status = ref<ControlStatus>(null);
const dragActive = ref(false);
const loading = ref(true);
const error = ref("");
let pollTimer: number | undefined;
let unlistenDragDrop: DragDropUnlisten;
let closing = false;

const isMac = navigator.platform.toUpperCase().includes("MAC");
const submitShortcut = computed(() => (isMac ? "Cmd+Enter" : "Ctrl+Enter"));
const expired = computed(() => status.value === "timed_out" || status.value === "cancelled");
const statusLabel = computed(() => {
  if (status.value === "timed_out") {
    return "Timed out";
  }
  if (status.value === "cancelled") {
    return "Cancelled";
  }
  return "Awaiting feedback";
});
const submitLabel = computed(() => (expired.value ? "Close" : `Submit feedback (${submitShortcut.value})`));

async function setWindowTitle() {
  const window = getCurrentWindow();
  if (!launch.value) {
    await window.setTitle("Relay MCP");
    return;
  }

  if (status.value === "timed_out") {
    await window.setTitle("Relay MCP [Timed out]");
    return;
  }

  if (status.value === "cancelled") {
    await window.setTitle("Relay MCP [Cancelled]");
    return;
  }

  await window.setTitle(launch.value.title);
}

async function loadLaunchState() {
  const state = await invoke<LaunchState>("get_launch_state");
  launch.value = state;
  feedback.value = "";
  loading.value = false;
  error.value = "";
  await setWindowTitle();
}

async function refreshStatus() {
  if (!launch.value || closing) {
    return;
  }

  try {
    const next = await invoke<ControlStatus>("read_feedback_status");
    if (!next || next === "active" || next === status.value) {
      return;
    }

    status.value = next;
    dragActive.value = false;
    await setWindowTitle();

    if (!feedback.value.trim()) {
      await closeWindow();
    }
  } catch {
    // Ignore transient status read failures while the request is active.
  }
}

async function closeWindow() {
  if (closing) {
    return;
  }
  closing = true;
  clearInterval(pollTimer);
  unlistenDragDrop?.();
  await getCurrentWindow().close();
}

async function submit() {
  if (!launch.value || closing) {
    return;
  }

  if (expired.value) {
    await closeWindow();
    return;
  }

  try {
    await invoke("submit_feedback", { feedback: feedback.value });
    await closeWindow();
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
}

function insertPaths(paths: string[]) {
  if (!paths.length) {
    return;
  }

  const block = paths.join("\n");
  if (!feedback.value) {
    feedback.value = block;
    return;
  }

  if (!feedback.value.endsWith("\n")) {
    feedback.value += "\n";
  }
  feedback.value += block;
}

function fileUrlToPath(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  try {
    const url = new URL(trimmed);
    if (url.protocol !== "file:") {
      return null;
    }

    let pathname = decodeURIComponent(url.pathname);
    if (/^\/[A-Za-z]:/.test(pathname)) {
      pathname = pathname.slice(1);
    }
    return pathname;
  } catch {
    return null;
  }
}

function extractClipboardPaths(event: ClipboardEvent): string[] {
  const data = event.clipboardData;
  if (!data) {
    return [];
  }

  const uriList = data
    .getData("text/uri-list")
    .split(/\r?\n/)
    .map(fileUrlToPath)
    .filter((value): value is string => Boolean(value));
  if (uriList.length > 0) {
    return uriList;
  }

  const plainText = data.getData("text/plain").trim();
  if (plainText) {
    const plainPath = fileUrlToPath(plainText);
    if (plainPath) {
      return [plainPath];
    }

    return plainText
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);
  }

  return Array.from(data.files)
    .map((file) => file.name)
    .filter(Boolean);
}

function onDragOver(event: DragEvent) {
  if (expired.value || closing) {
    return;
  }
  event.preventDefault();
  dragActive.value = true;
}

function onDragLeave(event: DragEvent) {
  if (expired.value || closing) {
    return;
  }
  event.preventDefault();
  dragActive.value = false;
}

function onDrop(event: DragEvent) {
  if (expired.value || closing) {
    return;
  }
  event.preventDefault();
  dragActive.value = false;
}

function onPaste(event: ClipboardEvent) {
  if (expired.value || closing) {
    return;
  }
  const paths = extractClipboardPaths(event);
  if (paths.length > 0) {
    event.preventDefault();
    insertPaths(paths);
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    void submit();
  }
}

onMounted(async () => {
  try {
    await loadLaunchState();
    unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
      if (expired.value || closing) {
        return;
      }

      if (event.payload.type === "over") {
        dragActive.value = true;
        return;
      }

      dragActive.value = false;
      if (event.payload.type === "drop") {
        insertPaths(event.payload.paths);
      }
    });
    pollTimer = window.setInterval(() => {
      void refreshStatus();
    }, 500);
    window.addEventListener("paste", onPaste);
    await refreshStatus();
  } catch (err) {
    loading.value = false;
    error.value = err instanceof Error ? err.message : String(err);
  }
});

onBeforeUnmount(() => {
  clearInterval(pollTimer);
  unlistenDragDrop?.();
  window.removeEventListener("paste", onPaste);
});
</script>

<template>
  <main
    class="shell"
    :class="{ dragActive }"
    @dragover="onDragOver"
    @dragleave="onDragLeave"
    @drop="onDrop"
  >
    <section class="panel">
      <header class="hero">
        <div>
          <p class="eyebrow">Relay</p>
          <h1>Relay MCP</h1>
          <p class="subtitle">Human feedback layer for AI IDEs</p>
        </div>
        <span class="statusPill">{{ statusLabel }}</span>
      </header>

      <p class="hint">
        Summaries stay read-only. Feedback supports <kbd>{{ submitShortcut }}</kbd>, file drag, and paste.
      </p>

      <label class="field">
        <span>Summary</span>
        <textarea v-if="launch" :value="launch.summary" readonly rows="7" />
        <textarea v-else readonly rows="7" :value="loading ? 'Loading…' : 'No launch data available.'" />
        <p v-if="error" class="error">{{ error }}</p>
      </label>

      <label class="field">
        <span>Feedback</span>
        <textarea
          v-model="feedback"
          rows="8"
          :readonly="expired"
          placeholder="Write concise, actionable feedback..."
          @keydown="onKeydown"
        />
        <p v-if="expired" class="note">This request has already timed out or been cancelled. Your text can be reviewed locally, but it can no longer be submitted.</p>
      </label>

      <div class="footer">
        <p class="meta">
          <strong>{{ statusLabel }}</strong>
          <span v-if="launch"> · {{ launch.result_file }}</span>
        </p>
        <div class="actions">
          <button class="secondary" type="button" @click="closeWindow">Close</button>
          <button class="primary" type="button" @click="submit">{{ submitLabel }}</button>
        </div>
      </div>
    </section>
  </main>
</template>

