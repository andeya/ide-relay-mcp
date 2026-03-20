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

/** Prefer `reply_attachments` from the hub; fall back to marker-in-`reply` for old rounds. */
export function parsedUserReplyFromRound(round: QaRound): ParsedRelayFeedback {
  const att = round.reply_attachments;
  if (att?.length) {
    const seenI = new Set<string>();
    const imagePaths = att
      .filter((a) => a.kind === "image" && a.path?.trim())
      .map((a) => String(a.path).trim())
      .filter((p) => (seenI.has(p) ? false : (seenI.add(p), true)));
    const seenF = new Set<string>();
    const filePaths = att
      .filter((a) => a.kind === "file" && a.path?.trim())
      .map((a) => String(a.path).trim())
      .filter((p) => (seenF.has(p) ? false : (seenF.add(p), true)));
    return { text: (round.reply ?? "").trim(), imagePaths, filePaths };
  }
  return parseRelayFeedbackReply(round.reply ?? "");
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
