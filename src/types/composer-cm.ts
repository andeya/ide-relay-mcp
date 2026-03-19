/** `defineExpose` surface for `RelayComposerInput` (CodeMirror). */
export type RelayComposerEditorExpose = {
  focus: () => void;
  getCursor: () => number;
  getSelection: () => { from: number; to: number };
  setSelection: (_a: number, _b?: number) => void;
  /** Current editor document (avoids v-model lag vs CM selection). */
  getDoc: () => string;
};
