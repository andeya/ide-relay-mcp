/**
 * Pure helpers for the feedback hub composer (slash highlighting, tab labels, file paths).
 * Split from useFeedbackWindow for reuse and lighter composable surface.
 */
import type { CommandItem, LaunchState } from "../types/relay-app";

/**
 * Characters allowed in `/foo` tail (IDE command / skill id segment).
 * Single source for CM decorations, slash palette query validation, and related regexes.
 */
export const SLASH_CMD_CHAR_CLASS = "A-Za-z0-9_.:-";

/** New global RegExp each call — reuse of `/g` regex risks stale `lastIndex`. */
export function slashLineTokenRegex(): RegExp {
  return new RegExp(`(^|[\\n ])(\\/[${SLASH_CMD_CHAR_CLASS}]*)`, "g");
}

/** Tab strip label: `title` is MM-DD HH:mm:ss from backend; else retell preview. */
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

/**
 * Secondary line in slash palette: keep long filesystem paths readable; use full `desc` for title=.
 */
export function slashItemDetailPreview(desc: string, maxLen = 52): string {
  const s = desc.trim();
  if (!s) return "";
  if (s.length <= maxLen) return s;
  const lastSlash = s.lastIndexOf("/");
  if (lastSlash >= 0 && lastSlash < s.length - 1) {
    const base = s.slice(lastSlash + 1);
    const prefixBudget = maxLen - base.length - 2;
    if (prefixBudget >= 8) {
      const dir = s.slice(0, lastSlash);
      const tail = dir.length > prefixBudget ? `…${dir.slice(-prefixBudget)}` : dir;
      return `${tail}/${base}`;
    }
    return `…/${base}`;
  }
  return `${s.slice(0, Math.max(1, maxLen - 1))}…`;
}

/**
 * Second line in `/` palette: `description`, or `name` when no description (if not redundant with primary id).
 */
export function slashCommandSecondaryLine(cmd: CommandItem): string {
  const desc = (cmd.description ?? "").trim();
  if (desc) return desc;
  const primary = (cmd.id ?? cmd.name ?? "").trim();
  const name = (cmd.name ?? "").trim();
  if (!name || name === primary) return "";
  return name;
}

/**
 * Query slice after `/` for slash menu + mirror: IDE/skill ids (ASCII), not CJK prose
 * (Chinese has no spaces; `[^\s]+` would swallow the whole line).
 */
export function isSlashCommandQueryChars(query: string): boolean {
  return new RegExp(`^[${SLASH_CMD_CHAR_CLASS}]*$`).test(query);
}

/**
 * Match score for slash palette ordering (higher = better). No new deps; similar idea to
 * cmdk / chat UIs (prefix > substring > id segments > description > subsequence).
 */
export function slashCommandMatchScore(cmd: CommandItem, query: string): number {
  const q = query.trim().toLowerCase();
  if (!q) return 1;
  const name = (cmd.name ?? "").toLowerCase();
  const id = (cmd.id ?? "").toLowerCase();
  const desc = (cmd.description ?? "").toLowerCase();
  const cat = (cmd.category ?? "").toLowerCase();

  if (name.startsWith(q) || id.startsWith(q)) {
    return 1000 + Math.max(0, 64 - Math.min(name.length, id.length));
  }
  if (name.includes(q) || id.includes(q)) return 720;
  for (const seg of id.split(/[./_-]+/)) {
    if (seg && seg.startsWith(q)) return 650;
  }
  if (desc.includes(q)) return 420;
  if (cat.includes(q)) return 320;

  const hay = `${name}\0${id}`;
  let hi = 0;
  for (const ch of q) {
    const j = hay.indexOf(ch, hi);
    if (j < 0) return 0;
    hi = j + 1;
  }
  return 200;
}

export function filterAndSortSlashCommands(
  list: CommandItem[],
  query: string,
): CommandItem[] {
  const q = query.trim();
  if (!q) return list.slice();
  const seen = new Set<string>();
  const scored = list
    .map((c) => ({ c, s: slashCommandMatchScore(c, q) }))
    .filter((x) => x.s > 0)
    .sort(
      (a, b) =>
        b.s - a.s ||
        (a.c.id ?? a.c.name ?? "").localeCompare(
          b.c.id ?? b.c.name ?? "",
          undefined,
          { sensitivity: "base" },
        ),
    );
  return scored.map((x) => x.c).filter((c) => {
    if (seen.has(c.id)) return false;
    seen.add(c.id);
    return true;
  });
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
