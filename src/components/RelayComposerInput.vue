<script setup lang="ts">
/**
 * Feedback composer: CodeMirror 6 (solid IME/caret vs textarea+mirror).
 */
import { Compartment, EditorState } from "@codemirror/state";
import { EditorView, placeholder as cmPlaceholder } from "@codemirror/view";
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { createRelayComposerExtensions } from "../editor/relayComposerCm";
import type { RelayComposerEditorExpose } from "../types/composer-cm";

/** Mutated ref so CM keymap reads latest Enter policy without re-creating extensions. */
const enterSwallow = { value: true };

const props = withDefaults(
  defineProps<{
    modelValue: string;
    readonly: boolean;
    placeholder: string;
    hasThumbs: boolean;
    /** When true, plain Enter is reserved for submit (not newline). */
    swallowPlainEnter: boolean;
    /** Optional: `aria-activedescendant` on CM content (slash listbox option id). */
    slashActiveDescendantId?: string | null;
    /** Optional: `aria-controls` target id when slash popup is open. */
    slashCombPopupId?: string | null;
  }>(),
  {
    hasThumbs: false,
    swallowPlainEnter: true,
    slashActiveDescendantId: null,
    slashCombPopupId: null,
  },
);

const emit = defineEmits<{
  "update:modelValue": [v: string];
  keydown: [e: KeyboardEvent];
  paste: [e: ClipboardEvent];
  compositionstart: [];
  compositionend: [];
  scroll: [];
  caretHead: [head: number, doc: string];
}>();

const host = ref<HTMLDivElement | null>(null);
const readOnlyComp = new Compartment();
const placeholderComp = new Compartment();
let view: EditorView | null = null;
let syncingFromParent = false;
let keydownCapture: ((_e: Event) => void) | null = null;

function syncSlashA11y() {
  if (!view) return;
  const popup = props.slashCombPopupId?.trim();
  if (popup) {
    view.contentDOM.setAttribute("aria-controls", popup);
    view.contentDOM.setAttribute("aria-expanded", "true");
  } else {
    view.contentDOM.removeAttribute("aria-controls");
    view.contentDOM.removeAttribute("aria-expanded");
  }
  const ad = props.slashActiveDescendantId?.trim();
  if (ad) view.contentDOM.setAttribute("aria-activedescendant", ad);
  else view.contentDOM.removeAttribute("aria-activedescendant");
}

function cmOpts() {
  return {
    readOnly: props.readonly,
    placeholderText: props.placeholder,
    onDocChange(v: string) {
      if (syncingFromParent) return;
      emit("update:modelValue", v);
    },
    onPaste(e: ClipboardEvent) {
      emit("paste", e);
    },
    onCompositionStart() {
      emit("compositionstart");
    },
    onCompositionEnd() {
      emit("compositionend");
    },
    onScroll: () => emit("scroll"),
    onCaretHead(head: number, doc: string) {
      emit("caretHead", head, doc);
    },
    getSwallowPlainEnter: () => enterSwallow.value,
  };
}

watch(
  () => props.swallowPlainEnter,
  (v) => {
    enterSwallow.value = v;
  },
  { immediate: true },
);

function mountEditor() {
  if (!host.value) return;
  view = new EditorView({
    parent: host.value,
    state: EditorState.create({
      doc: props.modelValue,
      extensions: createRelayComposerExtensions(
        readOnlyComp,
        placeholderComp,
        cmOpts(),
      ),
    }),
  });
  // Capture on contentDOM (focus target) so we run before CM's internal handlers.
  keydownCapture = (ev: Event) => {
    const e = ev as KeyboardEvent;
    emit("keydown", e);
    if (
      (e.key === "Enter" || e.code === "NumpadEnter") &&
      e.defaultPrevented
    ) {
      e.stopImmediatePropagation();
    }
  };
  view.contentDOM.addEventListener("keydown", keydownCapture, true);
  syncSlashA11y();
}

onMounted(() => {
  mountEditor();
  void nextTick(() => {
    if (view)
      emit("caretHead", view.state.selection.main.head, view.state.doc.toString());
  });
});

onBeforeUnmount(() => {
  if (view && keydownCapture) {
    view.contentDOM.removeEventListener("keydown", keydownCapture, true);
    keydownCapture = null;
  }
  view?.destroy();
  view = null;
});

watch(
  () => props.modelValue,
  (next) => {
    if (!view) return;
    const cur = view.state.doc.toString();
    if (cur === next) return;
    syncingFromParent = true;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: next },
    });
    syncingFromParent = false;
  },
);

watch(
  () => props.readonly,
  (ro) => {
    if (!view) return;
    view.dispatch({
      effects: readOnlyComp.reconfigure(EditorState.readOnly.of(ro)),
    });
  },
);

watch(
  () => props.placeholder,
  (p) => {
    if (!view) return;
    view.dispatch({
      effects: placeholderComp.reconfigure(cmPlaceholder(p)),
    });
  },
);

watch(
  () => [props.slashActiveDescendantId, props.slashCombPopupId] as const,
  () => {
    syncSlashA11y();
  },
);

defineExpose<RelayComposerEditorExpose>({
  focus: () => {
    if (!view) return;
    view.focus();
    // After parent async work (e.g. submit + reload), layout may settle one frame late.
    requestAnimationFrame(() => {
      view?.requestMeasure();
    });
  },
  getCursor: () => view?.state.selection.main.head ?? 0,
  getSelection: () => {
    const m = view?.state.selection.main;
    return m ? { from: m.from, to: m.to } : { from: 0, to: 0 };
  },
  setSelection: (anchor: number, head?: number) => {
    if (!view) return;
    view.dispatch({ selection: { anchor, head: head ?? anchor } });
  },
  getDoc: () => view?.state.doc.toString() ?? "",
});
</script>

<template>
  <div
    ref="host"
    class="relay-cm-host"
    :class="{
      'relay-cm-host--thumbs': hasThumbs,
    }"
  />
</template>
