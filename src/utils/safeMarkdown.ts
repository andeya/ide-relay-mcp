import DOMPurify from "dompurify";
import { marked, type Token } from "marked";

marked.setOptions({ gfm: true, breaks: true });

/** For title= on non-navigating link spans (hover shows target URL). */
function escapeHtmlAttr(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

type ParserThis = { parser: { parseInline: (_tokens: Token[]) => string } };

/* Markdown links + GFM autolinks: render as <span> so clicks never navigate away from Relay. */
marked.use({
  renderer: {
    link(this: ParserThis, { href, title, tokens }) {
      const inner = this.parser.parseInline(tokens);
      const tip = escapeHtmlAttr((title ?? "").trim() || href);
      return `<span class="qaMdNoLink" title="${tip}">${inner}</span>`;
    },
  },
});

/** Escape for safe static HTML when markdown/sanitize fails (no v-html raw user string). */
function escapeHtmlPlain(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Matches a Markdown list item: optional leading space, then "1." or "-"/"*"/"+" and a space. */
const LIST_ITEM = /^\s*(\d+\.|[-*+])\s/;

/** Matches a GFM table row: line starts with optional space and | and contains at least one more |. */
const TABLE_ROW = /^\s*\|.+\|/;

/** Ensure each line becomes a separate block in Markdown (avoid single newlines collapsing).
 *  Consecutive newlines collapse to one; then use \n\n between lines except:
 *  - between two list items (keep \n so lists don't break);
 *  - between two table rows (keep \n so GFM table parses as one block). */
function ensureLineBreaksForMarkdown(text: string): string {
  const lines = text.split(/\r?\n+/);
  if (lines.length <= 1) return text;
  let out = lines[0];
  for (let i = 1; i < lines.length; i++) {
    const prevList = LIST_ITEM.test(lines[i - 1]);
    const currList = LIST_ITEM.test(lines[i]);
    const prevTable = TABLE_ROW.test(lines[i - 1]);
    const currTable = TABLE_ROW.test(lines[i]);
    const useSingleNewline = (prevList && currList) || (prevTable && currTable);
    out += useSingleNewline ? "\n" : "\n\n";
    out += lines[i];
  }
  return out;
}

/** Render Markdown to sanitized HTML for Q&A panels. */
export function safeMarkdownToHtml(src: string): string {
  const t = (src ?? "").trim();
  if (!t) return "";
  try {
    const withLineBreaks = ensureLineBreaksForMarkdown(t);
    const raw = marked.parse(withLineBreaks, { async: false }) as string;
    return DOMPurify.sanitize(raw, {
      FORBID_TAGS: ["a"],
      ALLOWED_URI_REGEXP:
        /^(?:(?:https?|mailto|data):|[^a-z]|[a-z+.-]+(?:[^a-z+.\-:]|$))/iu,
    });
  } catch {
    /* marked/DOMPurify edge cases must not break the chat subtree — fall back to escaped text */
    return `<p class="qaRoundMdFallback">${escapeHtmlPlain(t)}</p>`;
  }
}
