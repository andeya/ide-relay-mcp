/**
 * Multi-tab feedback: when a tab has a pending MCP request, plain Enter submits; Shift+Enter = newline;
 * ⌘/Ctrl+Enter = submit and close this tab. Idle tabs (no request_id) draft with Enter = newline.
 */
import type { ComponentPublicInstance } from "vue";
import { computed, nextTick, onBeforeUnmount, ref, shallowRef, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { locale, t } from "../i18n";
import type {
  CommandItem,
  ControlStatus,
  DragDropUnlisten,
  FeedbackTabsState,
  LaunchState,
  QaRound,
} from "../types/relay-app";
import type { RelayComposerEditorExpose } from "../types/composer-cm";
import {
  feedbackTabLabel,
  fileUrlToPath,
  filterAndSortSlashCommands,
  looksLikeSingleFilePath,
} from "./feedbackComposerUtils";
import { stripLegacyRelayMarkerTail } from "../utils/parseRelayFeedbackReply";

export type PendingImage = {
  id: string;
  previewUrl: string;
  file: File;
};

/** Path = drag-drop from disk; file = picked in file dialog (no path). */
export type PendingFileDrop =
  | { id: string; path: string; name: string; error?: string }
  | { id: string; file: File; error?: string };

async function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => {
      const s = r.result as string;
      const i = s.indexOf(",");
      resolve(i >= 0 ? s.slice(i + 1) : s);
    };
    r.onerror = () => reject(new Error("read file"));
    r.readAsDataURL(file);
  });
}

let imgIdSeq = 0;
function nextImgId() {
  imgIdSeq += 1;
  return `img_${imgIdSeq}`;
}

export function useFeedbackWindow() {
  const tabsState = ref<FeedbackTabsState | null>(null);
  const feedbackByTab = ref<Record<string, string>>({});
  /** Draft images per relay_mcp_session_id (or tab_id); survives reloadTabs when same session. */
  const pendingImagesByTab = ref<Record<string, { id: string; file: File }[]>>(
    {},
  );
  const pendingFileDrops = ref<PendingFileDrop[]>([]);
  const pendingFileDropsByTab = ref<Record<string, PendingFileDrop[]>>({});
  let fileDropSeq = 0;
  function nextFileDropId() {
    fileDropSeq += 1;
    return `f_${fileDropSeq}`;
  }
  const feedback = ref("");
  /** IME composition (e.g. CJK input); ignore Enter until composition ends. */
  const imeComposing = ref(false);
  const relayComposerRef = shallowRef<RelayComposerEditorExpose | null>(null);
  /** Caret index from CodeMirror updateListener (avoids ref/expose timing for `/` menu). */
  const composerCaretHead = ref(0);
  /** Slash commands: open state, query after "/", anchor index of "/", selected index in list. */
  const slashOpen = ref(false);
  const slashQuery = ref("");
  const slashAnchorStart = ref(0);
  const slashSelectedIndex = ref(0);
  const slashDropdownRef = ref<HTMLElement | null>(null);
  const pendingImages = ref<PendingImage[]>([]);
  const status = ref<ControlStatus>(null);
  const dragActive = ref(false);
  const loading = ref(true);
  const error = ref("");
  const flashingTabIds = ref<Set<string>>(new Set());
  /** Prevents double submit (rapid Enter / double-click send). */
  const submitting = ref(false);
  let pollTimer: ReturnType<typeof setInterval> | undefined;
  /** Clears and re-registers interval when tab visibility changes (lower CPU when backgrounded). */
  let pollVisibilityHandler: (() => void) | undefined;
  let unlistenDragDrop: DragDropUnlisten;
  let closing = false;

  const activeTabId = computed(
    () => tabsState.value?.active_tab_id ?? "",
  );
  const tabs = computed(() => tabsState.value?.tabs ?? []);
  const launch = computed<LaunchState | null>(() => {
    const id = activeTabId.value;
    if (!id || !tabsState.value) return null;
    return tabsState.value.tabs.find((x) => x.tab_id === id) ?? null;
  });

  const filteredCommands = computed((): CommandItem[] => {
    const commands = launch.value?.commands ?? [];
    const skills = launch.value?.skills ?? [];
    const list = [...commands, ...skills];
    return filterAndSortSlashCommands(list, slashQuery.value);
  });

  /** True when this session has any commands or skills (so empty dropdown = no match). False = IDE sent none, show slashNoCommandsForSession. */
  const hasSlashList = computed(
    () =>
      ((launch.value?.commands?.length ?? 0) +
        (launch.value?.skills?.length ?? 0)) >
      0,
  );

  function closeSlash() {
    slashOpen.value = false;
    slashQuery.value = "";
    slashAnchorStart.value = 0;
    slashSelectedIndex.value = 0;
  }

  /** @param textSnapshot — pass CM doc from caret event to avoid v-model lag. */
  function updateSlashFromInput(textSnapshot?: string) {
    // Do not skip while imeComposing: on some systems `/` triggers composition; skipping
    // blocks opening the menu, and compositionstart used to closeSlash and race with input.
    if (expired.value || closing) return;
    const L = launch.value;
    if (L?.is_preview) {
      closeSlash();
      return;
    }
    const text = textSnapshot ?? feedback.value;
    const pos = composerCaretHead.value;
    if (pos === 0) {
      closeSlash();
      return;
    }
    let i = pos - 1;
    while (i >= 0 && text[i] !== "\n" && text[i] !== " ") {
      i -= 1;
    }
    i += 1;
    if (i >= pos || text[i] !== "/") {
      closeSlash();
      return;
    }
    const anchor = i;
    const query = text.slice(anchor + 1, pos);
    slashOpen.value = true;
    slashAnchorStart.value = anchor;
    slashQuery.value = query;
    const len = filteredCommands.value.length;
    slashSelectedIndex.value =
      len > 0 ? Math.min(slashSelectedIndex.value, len - 1) : 0;
  }

  function insertSlashCommand(cmd: CommandItem) {
    const surface = relayComposerRef.value;
    if (!surface || !slashOpen.value) return;
    const start = slashAnchorStart.value;
    const end = surface.getSelection().to;
    const token = (cmd.id ?? cmd.name ?? "").trim();
    if (!token) return;
    const v = surface.getDoc();
    const replacement = token.startsWith("/") ? `${token} ` : `/${token} `;
    feedback.value = v.slice(0, start) + replacement + v.slice(end);
    closeSlash();
    void nextTick(() => {
      const pos = start + replacement.length;
      surface.setSelection(pos, pos);
      surface.focus();
    });
  }

  watch(
    [slashOpen, slashSelectedIndex],
    () => {
      if (!slashOpen.value) return;
      void nextTick(() => {
        const el = slashDropdownRef.value;
        if (!el) return;
        const child = el.children[slashSelectedIndex.value] as
          | HTMLElement
          | undefined;
        child?.scrollIntoView({ block: "nearest" });
      });
    },
    { flush: "post" },
  );

  const qaRounds = computed((): QaRound[] => {
    const raw = tabsState.value?.qa_rounds;
    const tab = tabsState.value?.tabs.find((x) => x.tab_id === activeTabId.value);
    if (!tab) return [];

    const cid = (tab.relay_mcp_session_id || "").trim();
    let list: QaRound[] = [];
    if (Array.isArray(raw) && raw.length > 0) {
      if (cid) {
        list = raw.filter((r) => (r.relay_mcp_session_id || "").trim() === cid);
      } else if (tab.is_preview) {
        list = [];
      } else {
        list = raw.filter(
          (r) =>
            !(r.relay_mcp_session_id || "").trim() && r.tab_id === tab.tab_id,
        );
      }
    }

    const s = tab.retell?.trim();
    if (list.length === 0 && s) {
      let retellAt = "";
      const sid = (tab.relay_mcp_session_id || "").trim();
      if (sid) {
        const ms = Number(sid);
        if (ms > 0) {
          const d = new Date(ms);
          retellAt = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}:${String(d.getSeconds()).padStart(2, "0")}`;
        }
      }
      return [
        {
          retell: s,
          reply: "",
          skipped: false,
          submitted: false,
          tab_id: tab.tab_id,
          relay_mcp_session_id: tab.relay_mcp_session_id || "",
          retell_at: retellAt,
        },
      ];
    }
    return list;
  });

  const hasRealTabs = computed(
    () => tabs.value.some((x) => !x.is_preview),
  );

  /** True when current tab is Hub (preview). Use this for all Hub-only UI; do not branch on launch?.is_preview elsewhere. */
  const isHubPage = computed(() => !!launch.value?.is_preview);

  const expired = computed(
    () => status.value === "timed_out" || status.value === "cancelled",
  );
  /**
   * True while there is no answerable MCP slot: no `request_id` or server reports `idle`.
   * Aligns footer hints + send disabled state with `composerSwallowPlainEnter` (avoids `status === null`
   * briefly showing the wrong hint when `request_id` is already empty).
   */
  const composerDrafting = computed(() => {
    if (launch.value?.is_preview) return false;
    if (!launch.value?.request_id?.trim()) return true;
    if (status.value === "idle") return true;
    return false;
  });
  const hasPendingFileDropErrors = computed(() =>
    pendingFileDrops.value.some((fd) => Boolean(fd.error)),
  );
  /**
   * Plain Enter = submit only when `submit()` would run (not idle drafting). Must match `submit()`:
   * `submit` returns early only for `status === "idle"`, but `status` is often `null` until
   * `read_tab_status` resolves — swallow must still be true whenever `request_id` is set, or Enter
   * inserts a newline while submit would have succeeded.
   *
   * Hub (`is_preview`): composer is read-only; swallow stays true so plain Enter does not insert.
   * `submit(false)` returns immediately on preview — Enter has no visible effect (⌘/Ctrl+Enter closes via `submit(true)`).
   */
  const composerSwallowPlainEnter = computed(() => {
    if (submitting.value) return true;
    if (hasPendingFileDropErrors.value) return true;
    if (launch.value?.is_preview) return true;
    if (!launch.value?.request_id?.trim()) return false;
    if (status.value === "idle") return false;
    return true;
  });
  const tabStatuses = ref<Map<string, ControlStatus | null>>(new Map());

  async function refreshAllTabStatuses() {
    const ts = tabs.value;
    if (!ts.length) {
      tabStatuses.value = new Map();
      return;
    }
    const next = new Map<string, ControlStatus | null>();
    for (const tab of ts) {
      if (tab.tab_id === activeTabId.value) {
        next.set(tab.tab_id, status.value);
        continue;
      }
      try {
        const st = await invoke<ControlStatus | null>("read_tab_status", { tabId: tab.tab_id });
        next.set(tab.tab_id, st ?? null);
      } catch {
        next.set(tab.tab_id, null);
      }
    }
    tabStatuses.value = next;
  }

  function tabHue(tab: LaunchState): "hub" | "active" | "idle" | "expired" | "loading" | "default" {
    if (tab.is_preview) return "hub";
    const st = tab.tab_id === activeTabId.value ? status.value : (tabStatuses.value.get(tab.tab_id) ?? null);
    if (st === "timed_out" || st === "cancelled") return "expired";
    if (st === "idle") return "idle";
    if (st === "active") return "active";
    if (st === null && tab.tab_id) return "loading";
    return "default";
  }

  function revokeAllPreviews() {
    for (const p of pendingImages.value) {
      URL.revokeObjectURL(p.previewUrl);
    }
    pendingImages.value = [];
  }

  function insertPathsFixed(paths: string[]) {
    if (!paths.length) return;
    const block = paths.join("\n");
    const surface = relayComposerRef.value;
    if (!surface) {
      if (!feedback.value) feedback.value = block;
      else feedback.value = feedback.value.replace(/\s*$/, "") + "\n" + block;
      return;
    }
    const doc = surface.getDoc();
    const start = surface.getSelection().from;
    const prefix =
      start > 0 && doc.slice(0, start).trim().length > 0
        ? (doc[start - 1] === "\n" ? "" : "\n") + block
        : block;
    const v = doc;
    const end = surface.getSelection().to;
    feedback.value = v.slice(0, start) + prefix + v.slice(end);
    void nextTick(() => {
      const pos = start + prefix.length;
      surface.setSelection(pos, pos);
      surface.focus();
    });
  }

  function onComposerScroll() {
    /* CodeMirror manages its own scroller; no mirror layer. */
  }

  function draftKeyForTab(tab: LaunchState | null | undefined, tabId: string) {
    if (!tab) return tabId;
    const c = tab.relay_mcp_session_id?.trim();
    return c || tabId;
  }

  function onComposerCaretHead(head: number, doc: string) {
    composerCaretHead.value = head;
    updateSlashFromInput(doc);
  }

  watch(activeTabId, (id) => {
    composerCaretHead.value = 0;
    closeSlash();
    if (id) {
      const s = new Set(flashingTabIds.value);
      s.delete(id);
      flashingTabIds.value = s;
      void refreshStatus();
    }
  });

  function startFlash(tabId: string) {
    if (tabId === activeTabId.value) return;
    const s = new Set(flashingTabIds.value);
    s.add(tabId);
    flashingTabIds.value = s;
  }

  async function setWindowTitle() {
    const window = getCurrentWindow();
    try {
      const title = await invoke<string>("get_window_title");
      await window.setTitle(title);
    } catch {
      await window.setTitle("Relay");
    }
  }

  async function reloadTabs(depth = 0) {
    const preTab = launch.value;
    const preId = activeTabId.value;
    const prevDraftKey =
      preTab && preId ? draftKeyForTab(preTab, preId) : null;

    if (preTab && preId) {
      const k = draftKeyForTab(preTab, preId);
      feedbackByTab.value = { ...feedbackByTab.value, [k]: feedback.value };
      const nextImg = { ...pendingImagesByTab.value };
      if (pendingImages.value.length > 0) {
        nextImg[k] = pendingImages.value.map(({ id: pid, file }) => ({
          id: pid,
          file,
        }));
      } else {
        delete nextImg[k];
      }
      pendingImagesByTab.value = nextImg;
      const nextFd = { ...pendingFileDropsByTab.value };
      if (pendingFileDrops.value.length > 0) {
        nextFd[k] = pendingFileDrops.value.map((f) => ({ ...f }));
      } else {
        delete nextFd[k];
      }
      pendingFileDropsByTab.value = nextFd;
    }

    const state = await invoke<FeedbackTabsState>("get_feedback_tabs");
    tabsState.value = state;
    if (state.tabs.length === 0) {
      loading.value = false;
      return;
    }
    if (
      depth < 4 &&
      !state.tabs.some((x) => x.tab_id === state.active_tab_id)
    ) {
      await invoke("set_active_tab", {
        tabId: state.tabs[0].tab_id,
      });
      await reloadTabs(depth + 1);
      return;
    }
    const id = state.active_tab_id;
    const tab = state.tabs.find((x) => x.tab_id === id);
    const loadKey = draftKeyForTab(tab, id);
    feedback.value = feedbackByTab.value[loadKey] ?? "";

    const sameSession =
      prevDraftKey !== null && loadKey === prevDraftKey;
    if (!sameSession) {
      revokeAllPreviews();
      pendingFileDrops.value = [
        ...(pendingFileDropsByTab.value[loadKey] ?? []),
      ];
      const stored = pendingImagesByTab.value[loadKey];
      if (stored && stored.length > 0) {
        pendingImages.value = stored.map((p) => ({
          id: p.id,
          file: p.file,
          previewUrl: URL.createObjectURL(p.file),
        }));
      }
    }

    loading.value = false;
    error.value = "";
    status.value = null;
    await refreshStatus();
    await setWindowTitle();
  }

  async function selectTab(tabId: string) {
    await invoke("set_active_tab", { tabId });
    await reloadTabs();
  }

  async function refreshStatus() {
    const id = activeTabId.value;
    if (!id || closing) return;
    try {
      const next = await invoke<ControlStatus | null>("read_tab_status", {
        tabId: id,
      });
      if (next === "active") {
        if (status.value !== "active") {
          status.value = "active";
          await setWindowTitle();
        }
        void refreshAllTabStatuses();
        return;
      }
      if (next === "idle") {
        if (status.value !== "idle") {
          status.value = "idle";
          await setWindowTitle();
        }
        void refreshAllTabStatuses();
        return;
      }
      if (next === undefined || next === null) {
        void refreshAllTabStatuses();
        return;
      }
      if (next === status.value) {
        void refreshAllTabStatuses();
        return;
      }
      status.value = next;
      dragActive.value = false;
      await setWindowTitle();
      void refreshAllTabStatuses();
    } catch {
      /* ignore */
    }
  }

  async function closeWindow() {
    if (closing) return;
    closing = true;
    revokeAllPreviews();
    pendingFileDrops.value = [];
    if (pollTimer !== undefined) {
      clearInterval(pollTimer);
      pollTimer = undefined;
    }
    if (pollVisibilityHandler) {
      document.removeEventListener("visibilitychange", pollVisibilityHandler);
      pollVisibilityHandler = undefined;
    }
    unlistenDragDrop?.();
    await getCurrentWindow().close();
  }

  function localizePathError(raw: string): string {
    const s = String(raw).trim();
    if (s === "not a file") return t("composerFilePathNotAFile");
    if (s.includes("too large") || s.includes("50MB"))
      return t("composerFilePathTooLarge");
    return s;
  }

  function setFileDropError(id: string, err: string) {
    pendingFileDrops.value = pendingFileDrops.value.map((x) =>
      x.id === id ? { ...x, error: err } : x,
    );
  }

  async function buildFeedbackPayload(): Promise<{
    human: string;
    attachments: { kind: "image" | "file"; path: string }[];
  } | null> {
    const attachments: { kind: "image" | "file"; path: string }[] = [];
    for (const img of pendingImages.value) {
      const b64 = await fileToBase64(img.file);
      const path = await invoke<string>("save_feedback_attachment", {
        name: img.file.name || "paste.png",
        bytesB64: b64,
      });
      attachments.push({ kind: "image", path });
    }
    for (const fd of pendingFileDrops.value) {
      let b64: string;
      let name: string;
      if ("file" in fd) {
        try {
          b64 = await fileToBase64(fd.file);
        } catch {
          setFileDropError(fd.id, t("composerFileReadFailed"));
          return null;
        }
        name = fd.file.name || "attachment";
      } else {
        try {
          b64 = await invoke<string>("read_local_file_bytes_b64", {
            path: fd.path,
          });
        } catch (e) {
          setFileDropError(
            fd.id,
            localizePathError(e instanceof Error ? e.message : String(e)),
          );
          return null;
        }
        name = fd.name;
      }
      try {
        const path = await invoke<string>("save_feedback_attachment", {
          name,
          bytesB64: b64,
        });
        attachments.push({ kind: "file", path });
      } catch (e) {
        setFileDropError(
          fd.id,
          e instanceof Error ? e.message : String(e),
        );
        return null;
      }
    }
    const human = stripLegacyRelayMarkerTail(feedback.value);
    return { human, attachments };
  }

  async function submit(closeTabAfter = false) {
    if (submitting.value) return;
    const tab = launch.value;
    const id = activeTabId.value;
    if (!tab || !id || closing) return;

    // Hub tab: read-only composer; only ⌘/Ctrl+Enter (closeTabAfter) does work. Plain Enter is swallowed in CM.
    if (tab.is_preview) {
      if (!closeTabAfter) return;
      submitting.value = true;
      try {
        try {
          await invoke("close_feedback_tab", { tabId: id });
          await reloadTabs();
        } catch {
          /* window may close from Rust */
        }
        if (!tabsState.value?.tabs.length) await closeWindow();
      } finally {
        submitting.value = false;
      }
      return;
    }
    if (!tab.request_id?.trim()) {
      return;
    }
    /** Drafting: no submit until MCP request is active */
    if (status.value === "idle") {
      return;
    }
    if (hasPendingFileDropErrors.value) {
      return;
    }

    submitting.value = true;
    try {
      if (expired.value) {
        try {
          const draftKey = draftKeyForTab(tab, id);
          await invoke("dismiss_feedback_tab", { tabId: id });
          revokeAllPreviews();
          pendingFileDrops.value = [];
          feedback.value = "";
          delete feedbackByTab.value[draftKey];
          const rest = { ...pendingImagesByTab.value };
          delete rest[draftKey];
          pendingImagesByTab.value = rest;
          const restFd = { ...pendingFileDropsByTab.value };
          delete restFd[draftKey];
          pendingFileDropsByTab.value = restFd;
          await reloadTabs();
          if (!tabsState.value?.tabs.length) {
            await closeWindow();
          }
        } catch {
          await closeWindow();
        }
        return;
      }
      try {
        const payload = await buildFeedbackPayload();
        if (payload === null) {
          return;
        }
        const draftKey = draftKeyForTab(tab, id);
        await invoke("submit_tab_feedback", {
          tabId: id,
          human: payload.human,
          attachments: payload.attachments,
        });
        revokeAllPreviews();
        pendingFileDrops.value = [];
        feedback.value = "";
        delete feedbackByTab.value[draftKey];
        const restSubmit = { ...pendingImagesByTab.value };
        delete restSubmit[draftKey];
        pendingImagesByTab.value = restSubmit;
        const restFdSub = { ...pendingFileDropsByTab.value };
        delete restFdSub[draftKey];
        pendingFileDropsByTab.value = restFdSub;
        if (closeTabAfter) {
          try {
            await invoke("close_feedback_tab", { tabId: id });
          } catch (e) {
            error.value = e instanceof Error ? e.message : String(e);
          }
        }
        try {
          await reloadTabs();
        } catch {
          return;
        }
        if (!tabsState.value?.tabs.length) {
          await closeWindow();
        } else {
          status.value = null;
          await refreshStatus();
          void nextTick(() => relayComposerRef.value?.focus());
        }
      } catch (err) {
        error.value = err instanceof Error ? err.message : String(err);
      }
    } finally {
      submitting.value = false;
    }
  }

  /** Tab strip close: no confirm; hover-only close button reduces mis-clicks. */
  async function requestCloseTab(tab: LaunchState) {
    const tabId = tab.tab_id;
    const wasActive = tabId === activeTabId.value;
    try {
      if (tab.is_preview) {
        await invoke("close_feedback_tab", { tabId });
        try {
          await reloadTabs();
        } catch {
          /* Native side may have closed the window; invoke can fail */
        }
        if (!tabsState.value?.tabs.length) {
          await closeWindow();
        }
        return;
      } else {
        const st = await invoke<ControlStatus | null>("read_tab_status", {
          tabId,
        });
        if (st === "timed_out" || st === "cancelled") {
          await invoke("dismiss_feedback_tab", { tabId });
        } else {
          await invoke("close_feedback_tab", { tabId });
        }
      }
      const k = draftKeyForTab(tab, tabId);
      delete feedbackByTab.value[k];
      const restClose = { ...pendingImagesByTab.value };
      delete restClose[k];
      pendingImagesByTab.value = restClose;
      const restFdCl = { ...pendingFileDropsByTab.value };
      delete restFdCl[k];
      pendingFileDropsByTab.value = restFdCl;
      if (wasActive) {
        revokeAllPreviews();
        pendingFileDrops.value = [];
        feedback.value = "";
      }
      await reloadTabs();
      if (!tabsState.value?.tabs.length) await closeWindow();
      else if (wasActive) {
        status.value = null;
        await refreshStatus();
      }
    } catch (err) {
      if (tab.is_preview) {
        await closeWindow().catch(() => {});
      }
      error.value = err instanceof Error ? err.message : String(err);
    }
  }

  async function closeTabOrWindow() {
    const tab = launch.value;
    const id = activeTabId.value;
    if (!tab || !id) {
      await closeWindow();
      return;
    }
    if (tab.is_preview) {
      try {
        await invoke("close_feedback_tab", { tabId: id });
        await reloadTabs();
        if (!tabsState.value?.tabs.length) await closeWindow();
      } catch {
        await closeWindow();
      }
      return;
    }
    try {
      if (expired.value) {
        await invoke("dismiss_feedback_tab", { tabId: id });
      } else {
        await invoke("close_feedback_tab", { tabId: id });
      }
      revokeAllPreviews();
      pendingFileDrops.value = [];
      feedback.value = "";
      const dk = draftKeyForTab(tab, id);
      delete feedbackByTab.value[dk];
      const restWin = { ...pendingImagesByTab.value };
      delete restWin[dk];
      pendingImagesByTab.value = restWin;
      const restFdW = { ...pendingFileDropsByTab.value };
      delete restFdW[dk];
      pendingFileDropsByTab.value = restFdW;
      await reloadTabs();
      if (!tabsState.value?.tabs.length) await closeWindow();
      else {
        status.value = null;
        await refreshStatus();
      }
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
    }
  }

  function extractClipboardPaths(data: DataTransfer | null): string[] {
    if (!data) return [];
    const uriList = data
      .getData("text/uri-list")
      .split(/\r?\n/)
      .map(fileUrlToPath)
      .filter((value): value is string => Boolean(value));
    if (uriList.length > 0) return uriList;
    const plain = data.getData("text/plain");
    if (!plain?.trim()) return [];
    const lines = plain.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);
    if (lines.length === 1 && looksLikeSingleFilePath(lines[0])) {
      const p = fileUrlToPath(lines[0]) ?? lines[0];
      return [p];
    }
    return [];
  }

  function onComposerPaste(event: ClipboardEvent) {
    if (expired.value || closing) return;
    const dt = event.clipboardData;
    if (!dt) return;

    const files = Array.from(dt.files ?? []).filter((f) =>
      f.type.startsWith("image/"),
    );
    if (files.length > 0) {
      event.preventDefault();
      for (const file of files) {
        pendingImages.value = [
          ...pendingImages.value,
          {
            id: nextImgId(),
            previewUrl: URL.createObjectURL(file),
            file,
          },
        ];
      }
      return;
    }

    const items = dt.items;
    if (items) {
      for (let i = 0; i < items.length; i++) {
        const it = items[i];
        if (it.kind === "file" && it.type.startsWith("image/")) {
          const f = it.getAsFile();
          if (f) {
            event.preventDefault();
            pendingImages.value = [
              ...pendingImages.value,
              {
                id: nextImgId(),
                previewUrl: URL.createObjectURL(f),
                file: f,
              },
            ];
            return;
          }
        }
      }
    }

    const paths = extractClipboardPaths(dt);
    if (paths.length > 0) {
      event.preventDefault();
      insertPathsFixed(paths);
    }
  }

  function addImageFiles(files: FileList | File[]) {
    const list = Array.from(files).filter((f) => f.type.startsWith("image/"));
    for (const file of list) {
      pendingImages.value = [
        ...pendingImages.value,
        {
          id: nextImgId(),
          previewUrl: URL.createObjectURL(file),
          file,
        },
      ];
    }
  }

  /** File dialog: images → thumbnails; other files → file chips (submit via base64). */
  function addAttachedFilesFromPicker(files: FileList | File[]) {
    for (const file of Array.from(files)) {
      if (file.type.startsWith("image/")) {
        addImageFiles([file]);
      } else {
        pendingFileDrops.value = [
          ...pendingFileDrops.value,
          { id: nextFileDropId(), file },
        ];
      }
    }
  }

  /** Drag-drop: images → thumbnails; any other file path → file chip (submit copies bytes). */
  async function handleDroppedPaths(paths: string[]) {
    for (const path of paths) {
      const trimmed = path.trim();
      if (!trimmed) continue;
      const extImg = /\.(png|jpe?g|gif|webp)$/i.test(trimmed);
      if (extImg) {
        try {
          const data = await invoke<{
            data_base64: string;
            name: string;
            mime: string;
          }>("read_dragged_image_preview", { path: trimmed });
          const raw = atob(data.data_base64);
          const arr = new Uint8Array(raw.length);
          for (let i = 0; i < raw.length; i++) arr[i] = raw.charCodeAt(i);
          const file = new File([arr], data.name || "image.png", {
            type: data.mime || "image/png",
          });
          addImageFiles([file]);
          continue;
        } catch {
          /* fall through: show as generic file */
        }
      }
      const name =
        trimmed.split(/[/\\]/).pop() ||
        (trimmed.length > 32 ? `…${trimmed.slice(-28)}` : trimmed);
      const id = nextFileDropId();
      let pathErr: string | undefined;
      try {
        await invoke("validate_feedback_attachment_path", { path: trimmed });
      } catch (e) {
        pathErr = localizePathError(
          e instanceof Error ? e.message : String(e),
        );
      }
      pendingFileDrops.value = [
        ...pendingFileDrops.value,
        pathErr
          ? { id, path: trimmed, name, error: pathErr }
          : { id, path: trimmed, name },
      ];
    }
  }

  function removePendingFileDrop(id: string) {
    pendingFileDrops.value = pendingFileDrops.value.filter((x) => x.id !== id);
  }

  function removePendingImage(id: string) {
    const idx = pendingImages.value.findIndex((p) => p.id === id);
    if (idx < 0) return;
    const p = pendingImages.value[idx];
    URL.revokeObjectURL(p.previewUrl);
    pendingImages.value = pendingImages.value.filter((x) => x.id !== id);
  }

  function onDragOver(event: DragEvent) {
    if (expired.value || closing) return;
    event.preventDefault();
    dragActive.value = true;
  }

  function onDragLeave(event: DragEvent) {
    if (expired.value || closing) return;
    event.preventDefault();
    dragActive.value = false;
  }

  function onDrop(event: DragEvent) {
    if (expired.value || closing) return;
    event.preventDefault();
    dragActive.value = false;
  }

  function onComposerCompositionStart() {
    imeComposing.value = true;
  }
  function onComposerCompositionEnd() {
    imeComposing.value = false;
    void nextTick(() =>
      updateSlashFromInput(relayComposerRef.value?.getDoc()),
    );
  }

  function onKeydown(event: KeyboardEvent) {
    if (
      event.isComposing ||
      imeComposing.value ||
      (event as KeyboardEvent & { keyCode?: number }).keyCode === 229
    ) {
      return;
    }
    if (slashOpen.value) {
      const list = filteredCommands.value;
      if (event.key === "Escape") {
        event.preventDefault();
        closeSlash();
        return;
      }
      if (event.key === "ArrowDown") {
        if (list.length === 0) {
          closeSlash();
          return;
        }
        event.preventDefault();
        slashSelectedIndex.value = Math.min(
          slashSelectedIndex.value + 1,
          Math.max(0, list.length - 1),
        );
        return;
      }
      if (event.key === "ArrowUp") {
        if (list.length === 0) {
          closeSlash();
          return;
        }
        event.preventDefault();
        slashSelectedIndex.value = Math.max(0, slashSelectedIndex.value - 1);
        return;
      }
      if (event.key === "Enter" || event.code === "NumpadEnter") {
        const cmd = list[slashSelectedIndex.value];
        if (cmd) {
          event.preventDefault();
          insertSlashCommand(cmd);
          return;
        }
        event.preventDefault();
        closeSlash();
        return;
      } else if (event.key === "Tab") {
        const cmd = list[slashSelectedIndex.value];
        if (cmd) {
          event.preventDefault();
          insertSlashCommand(cmd);
        }
        return;
      }
    }
    const isEnter = event.key === "Enter" || event.code === "NumpadEnter";
    if (!isEnter) return;

    if (event.shiftKey) return;

    if (submitting.value || hasPendingFileDropErrors.value) {
      event.preventDefault();
      return;
    }

    if (!composerSwallowPlainEnter.value) {
      return;
    }

    event.preventDefault();
    const submitAndCloseTab = event.metaKey || event.ctrlKey;
    void submit(submitAndCloseTab);
  }

  async function pollCycle() {
    if (closing) return;
    try {
      await refreshStatus();
    } catch {
      /* ignore */
    }
  }

  const STATUS_POLL_MS_VISIBLE = 5_000;
  // Shorter when hidden to reduce Windows suspending WebView2 child process (~1h idle → unresponsive)
  const STATUS_POLL_MS_HIDDEN = 15_000;

  function scheduleStatusPoll() {
    if (closing) return;
    if (pollTimer !== undefined) {
      clearInterval(pollTimer);
      pollTimer = undefined;
    }
    const ms =
      typeof document !== "undefined" &&
      document.visibilityState === "hidden"
        ? STATUS_POLL_MS_HIDDEN
        : STATUS_POLL_MS_VISIBLE;
    pollTimer = window.setInterval(() => {
      void pollCycle();
    }, ms);
  }

  let unlistenTabs: (() => void) | undefined;

  async function initAfterLocale(): Promise<void> {
    await reloadTabs();
    unlistenTabs = await listen("relay_tabs_changed", async () => {
      const before = new Set(
        tabsState.value?.tabs.map((t) => t.tab_id) ?? [],
      );
      await reloadTabs();
      const cur = activeTabId.value;
      for (const id of tabsState.value?.tabs.map((t) => t.tab_id) ?? []) {
        if (!before.has(id) && id !== cur) startFlash(id);
      }
      void refreshAllTabStatuses();
    });
    unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
      if (expired.value || closing) return;
      if (event.payload.type === "over") {
        dragActive.value = true;
        return;
      }
      dragActive.value = false;
      if (event.payload.type === "drop") {
        void handleDroppedPaths(event.payload.paths);
      }
    });
    pollVisibilityHandler = () => {
      scheduleStatusPoll();
    };
    document.addEventListener("visibilitychange", pollVisibilityHandler);
    scheduleStatusPoll();
    void pollCycle();
    void invoke<number>("run_attachment_retention_purge").catch(() => {});
  }

  onBeforeUnmount(() => {
    if (pollTimer !== undefined) clearInterval(pollTimer);
    if (pollVisibilityHandler) {
      document.removeEventListener("visibilitychange", pollVisibilityHandler);
      pollVisibilityHandler = undefined;
    }
    unlistenTabs?.();
    unlistenDragDrop?.();
    revokeAllPreviews();
    pendingFileDrops.value = [];
  });

  function bindRelayComposerRef(el: unknown) {
    if (el == null) {
      relayComposerRef.value = null;
      return;
    }
    const inst = el as ComponentPublicInstance & Partial<RelayComposerEditorExpose>;
    if (typeof inst.getCursor === "function") {
      relayComposerRef.value = inst as RelayComposerEditorExpose;
      return;
    }
    const exposed = (inst as unknown as { $?: { exposed?: RelayComposerEditorExpose } })
      .$?.exposed;
    relayComposerRef.value =
      exposed && typeof exposed.getCursor === "function" ? exposed : null;
  }

  return {
    isHubPage,
    tabs,
    activeTabId,
    hasRealTabs,
    selectTab,
    flashingTabIds,
    feedback,
    bindRelayComposerRef,
    pendingImages,
    pendingFileDrops,
    status,
    dragActive,
    loading,
    error,
    submitting,
    expired,
    composerDrafting,
    composerSwallowPlainEnter,
    hasPendingFileDropErrors,
    setWindowTitle,
    closeWindow: closeTabOrWindow,
    requestCloseTab,
    submit,
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
    qaRounds,
    addAttachedFilesFromPicker,
    removePendingImage,
    removePendingFileDrop,
    tabStatuses,
    tabHue,
    tabLabel: (tab: LaunchState) => {
      void locale.value;
      return feedbackTabLabel(tab);
    },
  };
}
