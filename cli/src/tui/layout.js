// Keep a physical column and row free so terminal wrapping never scrolls the UI.
export function terminalLayout(columns = 80, rows = 24) {
  const w = Math.max(1, Math.min(220, columns - 1));
  const h = Math.max(1, Math.min(100, rows - 1));
  const compact = columns < 40 || rows < 14;
  const sidebar = !compact && w >= 110;
  return {
    w,
    h,
    compact,
    sidebar,
    usable: columns >= 20 && rows >= 8,
    size: Math.max(1, w - (sidebar ? 22 : 2)),
    available: Math.max(1, h - (compact ? 4 : 8)),
  };
}
