/**
 * Pure helpers for the feedback hub composer (slash highlighting, tab labels, file paths).
 * Split from useFeedbackWindow for reuse and lighter composable surface.
 */
import type { LaunchState } from "../types/relay-app";

/** Tab strip label: `title` is MM-DD HH:mm from backend; else retell preview. */
export function feedbackTabLabel(tab: LaunchState): string {
  if (tab.is_preview) {
    return "Hub";
  }
  const w = tab.title?.trim();
  if (w) {
    return w.length > 22 ? `${w.slice(0, 20)}…` : w;
  }
  const sum = tab.retell?.trim() || "";
  if (sum) {
    const one = sum.split(/\n/)[0] ?? sum;
    return one.length > 22 ? `${one.slice(0, 20)}…` : one;
  }
  return `#${tab.tab_id.slice(-6)}`;
}

export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

/** Mark `/name` tokens: keep `/` as plain text; wrap only the name in a pill (mirror layer). */
export function highlightComposerSlashTags(text: string): string {
  if (!text) return "";
  const re = /(^|[\n ])(\/[^\s]+)/g;
  let out = "";
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    out += escapeHtml(text.slice(last, m.index));
    const token = m[2];
    const body = token.startsWith("/") ? token.slice(1) : token;
    out +=
      escapeHtml(m[1]) +
      `<span class="composerSlashToken"><span class="composerSlashMark">/</span><span class="composerSlashTag">${escapeHtml(body)}</span></span>`;
    last = re.lastIndex;
  }
  out += escapeHtml(text.slice(last));
  return out;
}

export function looksLikeSingleFilePath(line: string): boolean {
  const t = line.trim();
  if (!t || t.includes("\n")) return false;
  if (t.startsWith("file:")) return true;
  if (/^[/~]/.test(t)) return true;
  if (/^[A-Za-z]:[\\/]/.test(t)) return true;
  if (t.startsWith("\\\\")) return true;
  return false;
}

export function fileUrlToPath(value: string): string | null {
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
