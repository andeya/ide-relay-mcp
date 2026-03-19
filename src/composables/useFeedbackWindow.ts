/**
 * Multi-tab feedback: Enter always submits (never newline); Shift+Enter = newline;
 * ⌘/Ctrl+Enter = submit and close this tab. Same in preview / idle (no stray newlines).
 */
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { locale, t } from "../i18n";
import type {
  ControlStatus,
  DragDropUnlisten,
  FeedbackTabsState,
  LaunchState,
  QaRound,
} from "../types/relay-app";

export type PendingImage = {
  id: string;
  previewUrl: string;
  file: File;
};

/** Path = drag-drop from disk; file = picked in file dialog (no path). */
export type PendingFileDrop =
  | { id: string; path: string; name: string; error?: string }
  | { id: string; file: File; error?: string };

function tabLabel(tab: LaunchState): string {
  if (tab.is_preview) {
    return "Hub";
  }
  /** Tab strip label: `title` is **Chat N** from backend; then optional `session_title`, then retell preview. */
  const w = tab.title?.trim();
  if (w) {
    return w.length > 22 ? `${w.slice(0, 20)}…` : w;
  }
  const s = tab.session_title?.trim();
  if (s) {
    return s.length > 22 ? `${s.slice(0, 20)}…` : s;
  }
  const sum = tab.retell?.trim() || "";
  if (sum) {
    const one = sum.split(/\n/)[0] ?? sum;
    return one.length > 22 ? `${one.slice(0, 20)}…` : one;
  }
  return `#${tab.tab_id.slice(-6)}`;
}

function looksLikeSingleFilePath(line: string): boolean {
  const t = line.trim();
  if (!t || t.includes("\n")) return false;
  if (t.startsWith("file:")) return true;
  if (/^[/~]/.test(t)) return true;
  if (/^[A-Za-z]:[\\/]/.test(t)) return true;
  if (t.startsWith("\\\\")) return true;
  return false;
}

function fileUrlToPath(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  try {
    const url = new URL(trimmed);
    if (url.protocol !== "file:") return null;
    let pathname = decodeURIComponent(url.pathname);
    if (/^\/[A-Za-z]:/.test(pathname)) {
      pathname = pathname.slice(1);
    }
    return pathname;
  } catch {
    return null;
  }
}

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
  /** Draft images per client_tab_id (or tab_id); survives reloadTabs when same session. */
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
  const feedbackTextareaRef = ref<HTMLTextAreaElement | null>(null);
  const pendingImages = ref<PendingImage[]>([]);
  const status = ref<ControlStatus>(null);
  const dragActive = ref(false);
  const loading = ref(true);
  const error = ref("");
  const flashingTabIds = ref<Set<string>>(new Set());
  /** Prevents double submit (rapid Enter / double-click send). */
  const submitting = ref(false);
  let pollTimer: ReturnType<typeof setInterval> | undefined;
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

  const qaRounds = computed((): QaRound[] => {
    const raw = tabsState.value?.qa_rounds;
    const tab = tabsState.value?.tabs.find((x) => x.tab_id === activeTabId.value);
    if (!tab) return [];

    const cid = (tab.client_tab_id || "").trim();
    let list: QaRound[] = [];
    if (Array.isArray(raw) && raw.length > 0) {
      if (cid) {
        list = raw.filter((r) => (r.client_tab_id || "").trim() === cid);
      } else if (tab.is_preview) {
        list = [];
      } else {
        list = raw.filter(
          (r) =>
            !(r.client_tab_id || "").trim() && r.tab_id === tab.tab_id,
        );
      }
    }

    const s = tab.retell?.trim();
    if (list.length === 0 && s) {
      return [
        {
          retell: s,
          reply: "",
          skipped: false,
          submitted: false,
          tab_id: tab.tab_id,
          client_tab_id: tab.client_tab_id || "",
        },
      ];
    }
    return list;
  });

  const hasRealTabs = computed(
    () => tabs.value.some((x) => !x.is_preview),
  );

  const expired = computed(
    () => status.value === "timed_out" || status.value === "cancelled",
  );
  const composerIdle = computed(() => status.value === "idle");
  const hasPendingFileDropErrors = computed(() =>
    pendingFileDrops.value.some((fd) => Boolean(fd.error)),
  );
  const statusLabel = computed(() => {
    void locale.value;
    if (status.value === "timed_out") return t("statusTimedOut");
    if (status.value === "cancelled") return t("statusCancelled");
    if (status.value === "idle") return t("statusIdle");
    return t("statusAwaiting");
  });
  const submitLabel = computed(() => {
    void locale.value;
    return expired.value ? t("submitClose") : t("submit");
  });
  const submitCloseTabLabel = computed(() => t("submitCloseTab"));

  function revokeAllPreviews() {
    for (const p of pendingImages.value) {
      URL.revokeObjectURL(p.previewUrl);
    }
    pendingImages.value = [];
  }

  function insertPathsFixed(paths: string[]) {
    if (!paths.length) return;
    const block = paths.join("\n");
    const el = feedbackTextareaRef.value;
    if (!el) {
      if (!feedback.value) feedback.value = block;
      else feedback.value = feedback.value.replace(/\s*$/, "") + "\n" + block;
      return;
    }
    const start = el.selectionStart ?? 0;
    const prefix =
      start > 0 && feedback.value.slice(0, start).trim().length > 0
        ? (feedback.value[start - 1] === "\n" ? "" : "\n") + block
        : block;
    const v = feedback.value;
    const end = el.selectionEnd ?? 0;
    feedback.value = v.slice(0, start) + prefix + v.slice(end);
    void nextTick(() => {
      const pos = start + prefix.length;
      el.selectionStart = el.selectionEnd = pos;
      el.focus();
    });
  }

  function draftKeyForTab(tab: LaunchState | null | undefined, tabId: string) {
    if (!tab) return tabId;
    const c = tab.client_tab_id?.trim();
    return c || tabId;
  }

  watch(activeTabId, (id) => {
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
    const defaultTitle = "Chat";
    const tab = launch.value;
    const head = tab?.title?.trim() || defaultTitle;
    await window.setTitle(head);
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
    if (!id || closing || !launch.value) return;
    try {
      const next = await invoke<ControlStatus | null>("read_tab_status", {
        tabId: id,
      });
      if (next === "active") {
        if (status.value !== "active") {
          status.value = "active";
          await setWindowTitle();
        }
        return;
      }
      if (next === "idle") {
        if (status.value !== "idle") {
          status.value = "idle";
          await setWindowTitle();
        }
        return;
      }
      if (next === undefined || next === null) return;
      if (next === status.value) return;
      status.value = next;
      dragActive.value = false;
      await setWindowTitle();
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

  async function buildFeedbackPayload(): Promise<string | null> {
    let body = feedback.value;
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
    if (attachments.length > 0) {
      const text = body.trim();
      const meta = { version: 1, attachments };
      body =
        (text ? text + "\n\n" : "") +
        "<<<RELAY_FEEDBACK_JSON>>>\n" +
        JSON.stringify(meta);
    }
    return body;
  }

  async function submit(closeTabAfter = false) {
    if (submitting.value) return;
    const tab = launch.value;
    const id = activeTabId.value;
    if (!tab || !id || closing) return;

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
          feedback: payload,
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
  }

  function onKeydown(event: KeyboardEvent) {
    if (
      event.isComposing ||
      imeComposing.value ||
      (event as KeyboardEvent & { keyCode?: number }).keyCode === 229
    ) {
      return;
    }
    const isEnter = event.key === "Enter" || event.code === "NumpadEnter";
    if (!isEnter) return;

    // Shift+Enter always inserts newline; plain Enter never does when it would not submit.
    if (event.shiftKey) return;

    const L = launch.value;
    /** Same idea as send button disabled: no-op (no newline), not “silent newline”. */
    const enterWouldNotSubmit =
      submitting.value ||
      hasPendingFileDropErrors.value ||
      Boolean(L?.is_preview) ||
      (!L?.is_preview && status.value === "idle");

    if (enterWouldNotSubmit) {
      event.preventDefault();
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
    pollTimer = window.setInterval(() => {
      void pollCycle();
    }, 2000);
    await pollCycle();
    try {
      await invoke<number>("run_attachment_retention_purge");
    } catch {
      /* optional auto-purge; ignore if not in Tauri */
    }
  }

  onBeforeUnmount(() => {
    if (pollTimer !== undefined) clearInterval(pollTimer);
    unlistenTabs?.();
    unlistenDragDrop?.();
    revokeAllPreviews();
    pendingFileDrops.value = [];
  });

  function bindTextareaRef(el: unknown) {
    feedbackTextareaRef.value =
      el instanceof HTMLTextAreaElement ? el : null;
  }

  return {
    launch,
    tabs,
    activeTabId,
    hasRealTabs,
    tabLabel,
    selectTab,
    flashingTabIds,
    feedback,
    feedbackTextareaRef,
    bindTextareaRef,
    pendingImages,
    pendingFileDrops,
    status,
    dragActive,
    loading,
    error,
    submitting,
    expired,
    composerIdle,
    hasPendingFileDropErrors,
    statusLabel,
    submitLabel,
    submitCloseTabLabel,
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
    initAfterLocale,
    reloadTabs,
    qaRounds,
    addImageFiles,
    addAttachedFilesFromPicker,
    removePendingImage,
    removePendingFileDrop,
  };
}
