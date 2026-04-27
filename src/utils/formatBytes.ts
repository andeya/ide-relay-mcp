/** Human-readable byte size for UI (binary KB/MB). */
export function formatBytes(n: number): string {
  const x = Number(n) || 0;
  if (x < 1024) return `${x} B`;
  if (x < 1048576) return `${(x / 1024).toFixed(1)} KB`;
  return `${(x / 1048576).toFixed(2)} MB`;
}

/**
 * Format token count with locale-aware unit.
 * @param n - raw token count
 * @param wanUnit - localized unit for >=10k (e.g. "万tok" / "w tok")
 */
export function formatTokenUnit(n: number, wanUnit: string): string {
  if (n >= 10000) {
    const wan = n / 10000;
    return wan >= 100 ? `${Math.round(wan)}${wanUnit}` : `${wan.toFixed(1)}${wanUnit}`;
  }
  return `${n.toLocaleString()} tok`;
}
