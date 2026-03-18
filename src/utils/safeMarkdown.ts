import DOMPurify from "dompurify";
import { marked } from "marked";

marked.setOptions({ gfm: true, breaks: true });

/** Render Markdown to sanitized HTML for Q&A panels. */
export function safeMarkdownToHtml(src: string): string {
  const t = (src ?? "").trim();
  if (!t) return "";
  const raw = marked.parse(t, { async: false }) as string;
  return DOMPurify.sanitize(raw, {
    ALLOWED_URI_REGEXP:
      /^(?:(?:https?|mailto|data):|[^a-z]|[a-z+.-]+(?:[^a-z+.\-:]|$))/iu,
  });
}
