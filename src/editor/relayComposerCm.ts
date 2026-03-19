/**
 * CodeMirror 6 setup for the Relay feedback composer (IME-safe; slash token styling).
 */
import {
  history,
  historyKeymap,
  insertNewline,
  insertNewlineAndIndent,
} from "@codemirror/commands";
import {
  Compartment,
  EditorState,
  Extension,
  Prec,
  RangeSetBuilder,
  StateField,
} from "@codemirror/state";
import {
  Decoration,
  DecorationSet,
  EditorView,
  keymap,
  placeholder,
} from "@codemirror/view";
import { slashLineTokenRegex } from "../composables/feedbackComposerUtils";

const slashMark = Decoration.mark({ class: "cm-relay-slash" });

function buildSlashDecorations(doc: string): DecorationSet {
  const b = new RangeSetBuilder<Decoration>();
  const re = slashLineTokenRegex();
  let m: RegExpExecArray | null;
  while ((m = re.exec(doc)) !== null) {
    const tokenFrom = m.index + m[1].length;
    b.add(tokenFrom, tokenFrom + 1, slashMark);
  }
  return b.finish();
}

const slashDecorationsField = StateField.define<DecorationSet>({
  create(s) {
    return buildSlashDecorations(s.doc.toString());
  },
  update(deco, tr) {
    if (!tr.docChanged) return deco;
    return buildSlashDecorations(tr.state.doc.toString());
  },
  provide: (f) => EditorView.decorations.from(f),
});

/** @internal */
export function relayComposerBaseTheme(): Extension {
  return EditorView.theme(
    {
      "&": {
        fontSize: "0.88rem",
        lineHeight: "1.45",
        backgroundColor: "transparent",
        fontFamily: [
          "Inter",
          "ui-sans-serif",
          "PingFang SC",
          "Hiragino Sans GB",
          "Microsoft YaHei",
          "Noto Sans CJK SC",
          "system-ui",
          "sans-serif",
        ].join(", "),
      },
      ".cm-scroller": {
        fontFamily: "inherit",
        lineHeight: "inherit",
        backgroundColor: "transparent",
      },
      ".cm-content": {
        caretColor: "#e2e8f0",
        padding: "10px 12px",
        minHeight: "72px",
        minWidth: "100%",
        boxSizing: "border-box" as const,
      },
      ".cm-line": { padding: "0" },
      ".cm-cursor, .cm-dropCursor": {
        borderLeftColor: "#e2e8f0",
      },
      "&.cm-focused .cm-selectionBackground, ::selection": {
        backgroundColor: "rgba(99, 102, 241, 0.35)",
      },
      ".cm-activeLine": { backgroundColor: "transparent" },
      ".cm-relay-slash": {
        color: "#cbd5e1",
        fontWeight: "500",
      },
    },
    { dark: true },
  );
}

export type RelayComposerCmOptions = {
  readOnly: boolean;
  placeholderText: string;
  onDocChange: (_doc: string) => void;
  onPaste: (_ev: ClipboardEvent) => void;
  onCompositionStart: () => void;
  onCompositionEnd: () => void;
  onScroll?: () => void;
  /** Called when caret moves or doc changes (reliable slash `/` detection vs component ref). */
  /** Second arg is current doc (avoids Vue v-model one-tick lag vs caret). */
  onCaretHead?: (_head: number, _doc: string) => void;
  /**
   * When true, plain Enter is not a newline (parent uses it to submit). When false, allow newline.
   * Call on each keystroke so idle/active can switch without remounting CM.
   */
  getSwallowPlainEnter?: () => boolean;
};

export function createRelayComposerExtensions(
  readOnlyComp: Compartment,
  placeholderComp: Compartment,
  opts: RelayComposerCmOptions,
): Extension[] {
  return [
    Prec.highest(
      keymap.of([
        {
          key: "Enter",
          shift: insertNewlineAndIndent,
          run: (view) => {
            if (view.composing) return false;
            return opts.getSwallowPlainEnter?.() ?? true;
          },
        },
      ]),
    ),
    keymap.of([
      {
        key: "Enter",
        run: (view) => {
          if (view.composing) return false;
          if (opts.getSwallowPlainEnter?.() ?? true) return false;
          return insertNewline(view);
        },
      },
    ]),
    EditorView.lineWrapping,
    slashDecorationsField,
    relayComposerBaseTheme(),
    readOnlyComp.of(EditorState.readOnly.of(opts.readOnly)),
    placeholderComp.of(placeholder(opts.placeholderText)),
    history(),
    keymap.of([...historyKeymap]),
    EditorView.updateListener.of((u) => {
      if (u.docChanged) {
        opts.onDocChange(u.state.doc.toString());
      }
      if (u.docChanged || u.selectionSet) {
        opts.onCaretHead?.(
          u.state.selection.main.head,
          u.state.doc.toString(),
        );
      }
    }),
    EditorView.domEventHandlers({
      paste(e) {
        opts.onPaste(e);
      },
      compositionstart() {
        opts.onCompositionStart();
      },
      compositionend() {
        opts.onCompositionEnd();
      },
      scroll() {
        opts.onScroll?.();
        return false;
      },
    }),
  ];
}

export { Compartment, EditorState } from "@codemirror/state";
export { EditorView } from "@codemirror/view";
