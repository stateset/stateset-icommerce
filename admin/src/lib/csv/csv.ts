// Generic CSV export helpers shared across admin pages.
//
// The orders bulk-export page already has its own `lib/orders/csv.ts` —
// this module generalises the pattern so customers, inventory, and any
// future entity can opt into export with a single import. The two
// implementations don't overlap: the orders module has a fixed canonical
// header; this one is column-spec driven.

/** RFC 4180-ish CSV cell escape: wrap in quotes, double inner quotes. */
export function toCsvCell(value: unknown): string {
  if (value === null || value === undefined) return '';
  const s = typeof value === 'string' ? value : JSON.stringify(value);
  return `"${s.replace(/"/g, '""')}"`;
}

/**
 * Column spec: each entry is `{ key, accessor }` where `accessor` is a
 * function from the row object to the cell value. Keeping the accessor
 * as a function lets us support derived columns (e.g. `items.length`)
 * and renamed schemas without coupling to TypeScript field names.
 */
export interface CsvColumn<T> {
  key: string;
  accessor: (row: T) => unknown;
}

/**
 * Serialize a list of rows to CSV using the given column spec.
 * Emits the header row first, then one row per input element. Preserves
 * input order so callers can sort before exporting.
 */
export function rowsToCsv<T>(rows: T[], columns: CsvColumn<T>[]): string {
  const header = columns.map((c) => c.key).join(',');
  const lines = rows.map((row) =>
    columns.map((col) => toCsvCell(col.accessor(row))).join(','),
  );
  return [header, ...lines].join('\n');
}

/** Trigger a browser download of a CSV string. Client-only (uses DOM). */
export function downloadCsv(filename: string, csv: string): void {
  const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}
