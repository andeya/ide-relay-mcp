/** Human-readable byte size for UI (binary KB/MB). */
export function formatBytes(n: number): string {
  const x = Number(n) || 0;
  if (x < 1024) return `${x} B`;
  if (x < 1048576) return `${(x / 1024).toFixed(1)} KB`;
  return `${(x / 1048576).toFixed(2)} MB`;
}
