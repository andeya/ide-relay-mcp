import type { QaRound } from "../types/relay-app";

/** Split Answer body from `<<<RELAY_FEEDBACK_JSON>>>` image paths (legacy). */

const MARKER = "<<<RELAY_FEEDBACK_JSON>>>";

export type ParsedRelayFeedback = {
  text: string;
  imagePaths: string[];
  filePaths: string[];
};

type AttachItem = { kind?: string; path?: string };

/** First top-level `{ ... }` in `src` starting at `start`, respecting strings. */
function scanBalancedObjectEnd(src: string, start: number): number | null {
  let depth = 0;
  let inStr = false;
  let esc = false;
  for (let i = start; i < src.length; i++) {
    const c = src[i];
    if (inStr) {
      if (esc) {
        esc = false;
      } else if (c === "\\") {
        esc = true;
      } else if (c === '"') {
        inStr = false;
      }
      continue;
    }
    if (c === '"') {
      inStr = true;
      continue;
    }
    if (c === "{") depth++;
    else if (c === "}") {
      depth--;
      if (depth === 0) return i;
    }
  }
  return null;
}

function collectPaths(attachments: AttachItem[]): {
  imagePaths: string[];
  filePaths: string[];
} {
  const seenI = new Set<string>();
  const imagePaths = attachments
    .filter((a) => a.kind === "image" && a.path && a.path.length > 0)
    .map((a) => String(a.path).trim())
    .filter((p) => (seenI.has(p) ? false : (seenI.add(p), true)));
  const seenF = new Set<string>();
  const filePaths = attachments
    .filter((a) => a.kind === "file" && a.path && a.path.length > 0)
    .map((a) => String(a.path).trim())
    .filter((p) => (seenF.has(p) ? false : (seenF.add(p), true)));
  return { imagePaths, filePaths };
}

/** Parse JSON block after marker; allow trailing prose after the closing `}`. */
function parseAttachmentsBlob(tail: string): {
  imagePaths: string[];
  filePaths: string[];
  restText: string;
} | null {
  const t = tail.trimStart();
  const brace = t.indexOf("{");
  if (brace >= 0) {
    const end = scanBalancedObjectEnd(t, brace);
    if (end != null) {
      const jsonStr = t.slice(brace, end + 1);
      const restText = t.slice(end + 1).trim();
      try {
        const j = JSON.parse(jsonStr) as { attachments?: AttachItem[] };
        const { imagePaths, filePaths } = collectPaths(j.attachments ?? []);
        return { imagePaths, filePaths, restText };
      } catch {
        /* fall through */
      }
    }
  }
  try {
    const j = JSON.parse(t.trim()) as { attachments?: AttachItem[] };
    const { imagePaths, filePaths } = collectPaths(j.attachments ?? []);
    return { imagePaths, filePaths, restText: "" };
  } catch {
    return null;
  }
}

/** Same trimming as GUI/Rust `strip_legacy_relay_marker_tail` before submit. */
export function stripLegacyRelayMarkerTail(s: string): string {
  const t = (s ?? "").trim();
  const i = t.indexOf(MARKER);
  return i >= 0 ? t.slice(0, i).trimEnd() : t;
}

/** Dedupe by path; `primary` order wins, then append new paths from `secondary`. */
function mergeDedupedPaths(primary: string[], secondary: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const list of [primary, secondary]) {
    for (const p of list) {
      const t = p.trim();
      if (!t || seen.has(t)) continue;
      seen.add(t);
      out.push(t);
    }
  }
  return out;
}

/**
 * Prefer `reply_attachments` from the hub; merge with legacy `<<<RELAY_FEEDBACK_JSON>>>` in `reply`
 * so invalid/empty structured rows still recover paths from the stored body.
 */
export function parsedUserReplyFromRound(round: QaRound): ParsedRelayFeedback {
  const raw = round.reply ?? "";
  const legacy = parseRelayFeedbackReply(raw);
  const att = round.reply_attachments;
  if (!att?.length) {
    return legacy;
  }
  const seenI = new Set<string>();
  const fromStructImages = att
    .filter((a) => a.kind === "image" && a.path?.trim())
    .map((a) => String(a.path).trim())
    .filter((p) => (seenI.has(p) ? false : (seenI.add(p), true)));
  const seenF = new Set<string>();
  const fromStructFiles = att
    .filter((a) => a.kind === "file" && a.path?.trim())
    .map((a) => String(a.path).trim())
    .filter((p) => (seenF.has(p) ? false : (seenF.add(p), true)));
  return {
    text: legacy.text,
    imagePaths: mergeDedupedPaths(fromStructImages, legacy.imagePaths),
    filePaths: mergeDedupedPaths(fromStructFiles, legacy.filePaths),
  };
}

/**
 * True if the user submitted bubble should render (markdown and/or attachment strip).
 * Uses the same parsing as {@link parsedUserReplyFromRound} / QaUserSubmittedBubble (e.g. attachment-only Answers).
 */
export function qaRoundHasRenderableUserContent(round: QaRound): boolean {
  const p = parsedUserReplyFromRound(round);
  return (
    p.text.trim().length > 0 ||
    p.imagePaths.length > 0 ||
    p.filePaths.length > 0
  );
}

export function parseRelayFeedbackReply(raw: string): ParsedRelayFeedback {
  const s = (raw ?? "").trim();
  const idx = s.indexOf(MARKER);
  if (idx < 0) {
    return { text: s, imagePaths: [], filePaths: [] };
  }
  let text = s.slice(0, idx).replace(/\s+$/u, "");
  const tail = s.slice(idx + MARKER.length);
  const parsed = parseAttachmentsBlob(tail);
  if (!parsed) {
    return { text, imagePaths: [], filePaths: [] };
  }
  if (parsed.restText) {
    text = [text, parsed.restText].filter(Boolean).join("\n\n");
  }
  return {
    text,
    imagePaths: parsed.imagePaths,
    filePaths: parsed.filePaths,
  };
}
