/** Split Answer body from `<<<RELAY_FEEDBACK_JSON>>>` image paths. */

const MARKER = "<<<RELAY_FEEDBACK_JSON>>>";

export type ParsedRelayFeedback = {
  text: string;
  imagePaths: string[];
  filePaths: string[];
};

export function parseRelayFeedbackReply(raw: string): ParsedRelayFeedback {
  const s = raw ?? "";
  const idx = s.indexOf(MARKER);
  if (idx < 0) {
    return { text: s, imagePaths: [], filePaths: [] };
  }
  const text = s.slice(0, idx).replace(/\s+$/u, "");
  const jsonPart = s.slice(idx + MARKER.length).trim();
  try {
    const j = JSON.parse(jsonPart) as {
      attachments?: { kind?: string; path?: string }[];
    };
    const att = j.attachments ?? [];
    const seenI = new Set<string>();
    const imagePaths = att
      .filter((a) => a.kind === "image" && a.path && a.path.length > 0)
      .map((a) => String(a.path).trim())
      .filter((p) => (seenI.has(p) ? false : (seenI.add(p), true)));
    const seenF = new Set<string>();
    const filePaths = att
      .filter((a) => a.kind === "file" && a.path && a.path.length > 0)
      .map((a) => String(a.path).trim())
      .filter((p) => (seenF.has(p) ? false : (seenF.add(p), true)));
    return { text, imagePaths, filePaths };
  } catch {
    return { text: s, imagePaths: [], filePaths: [] };
  }
}
