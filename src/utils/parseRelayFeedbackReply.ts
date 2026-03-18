/** Split Answer body from `<<<RELAY_FEEDBACK_JSON>>>` image paths. */

const MARKER = "<<<RELAY_FEEDBACK_JSON>>>";

export type ParsedRelayFeedback = {
  text: string;
  imagePaths: string[];
};

export function parseRelayFeedbackReply(raw: string): ParsedRelayFeedback {
  const s = raw ?? "";
  const idx = s.indexOf(MARKER);
  if (idx < 0) {
    return { text: s, imagePaths: [] };
  }
  const text = s.slice(0, idx).replace(/\s+$/u, "");
  const jsonPart = s.slice(idx + MARKER.length).trim();
  try {
    const j = JSON.parse(jsonPart) as {
      attachments?: { kind?: string; path?: string }[];
    };
    const paths = (j.attachments ?? [])
      .filter((a) => a.kind === "image" && a.path && a.path.length > 0)
      .map((a) => String(a.path).trim());
    const seen = new Set<string>();
    const imagePaths = paths.filter((p) =>
      seen.has(p) ? false : (seen.add(p), true),
    );
    return { text, imagePaths };
  } catch {
    return { text: s, imagePaths: [] };
  }
}
