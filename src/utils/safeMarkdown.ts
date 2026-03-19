import DOMPurify from "dompurify";
import { marked } from "marked";

marked.setOptions({ gfm: true, breaks: true });

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
  const withLineBreaks = ensureLineBreaksForMarkdown(t);
  const raw = marked.parse(withLineBreaks, { async: false }) as string;
  return DOMPurify.sanitize(raw, {
    ALLOWED_URI_REGEXP:
      /^(?:(?:https?|mailto|data):|[^a-z]|[a-z+.-]+(?:[^a-z+.\-:]|$))/iu,
  });
}
