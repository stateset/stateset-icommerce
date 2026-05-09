// CSV helpers for the orders bulk-export workflow.
//
// Pure functions — testable, no DOM dependency in `ordersToCsv`. The
// `downloadCsv` helper does touch DOM (Blob + anchor element) and is only
// called from client components.

import type { Order } from '@/lib/types';

/**
 * RFC 4180-ish CSV cell escape: wraps in quotes, doubles inner quotes.
 * Returns an empty cell for null/undefined.
 */
export function toCsvCell(value: unknown): string {
  if (value === null || value === undefined) return '';
  const s = typeof value === 'string' ? value : JSON.stringify(value);
  return `"${s.replace(/"/g, '""')}"`;
}

/** CSV header used by `ordersToCsv`. Stable across releases — exported for tests. */
export const ORDERS_CSV_HEADER = [
  'order_id',
  'customer_id',
  'status',
  'total_amount',
  'currency',
  'item_count',
  'created_at',
  'updated_at',
] as const;

/**
 * Serialize a list of Orders to CSV. Header row first, then one row per order.
 * Order matters: rows preserve the input list order so operators can sort
 * before exporting.
 */
export function ordersToCsv(orders: Order[]): string {
  const header = ORDERS_CSV_HEADER.join(',');
  const rows = orders.map((o) =>
    [
      toCsvCell(o.id),
      toCsvCell(o.customerId),
      toCsvCell(o.status),
      toCsvCell(o.totalAmount),
      toCsvCell(o.currency),
      toCsvCell(o.items.length),
      toCsvCell(o.createdAt),
      toCsvCell(o.updatedAt),
    ].join(','),
  );
  return [header, ...rows].join('\n');
}

/**
 * Trigger a browser download of a CSV string. Client-only.
 */
export function downloadCsv(filename: string, csv: string): void {
  const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}
