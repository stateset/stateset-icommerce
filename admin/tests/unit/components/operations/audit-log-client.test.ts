// Unit tests for the pure helpers in the audit-log feed:
//  - eventMatchesFilter (exact, prefix wildcard, glob fallback, substring)
//  - bufferToCsv (CSV escaping + ordering)

import { describe, expect, it } from 'vitest';

import {
  type AuditEvent,
  bufferToCsv,
  eventMatchesFilter,
} from '@/components/operations/audit-log-client';

describe('eventMatchesFilter', () => {
  it('matches everything when the filter is empty', () => {
    expect(eventMatchesFilter('order_created', '')).toBe(true);
    expect(eventMatchesFilter('inventory_adjusted', '   ')).toBe(true);
  });

  it('matches an exact event type', () => {
    expect(eventMatchesFilter('order_created', 'order_created')).toBe(true);
  });

  it('matches a prefix.* wildcard against dotted children', () => {
    expect(eventMatchesFilter('order.created', 'order.*')).toBe(true);
    expect(eventMatchesFilter('order.shipped', 'order.*')).toBe(true);
    expect(eventMatchesFilter('inventory.adjusted', 'order.*')).toBe(false);
  });

  it('matches a prefix.* wildcard against snake_case children', () => {
    expect(eventMatchesFilter('order_created', 'order.*')).toBe(true);
    expect(eventMatchesFilter('order_shipped', 'order.*')).toBe(true);
    expect(eventMatchesFilter('inventory_adjusted', 'order.*')).toBe(false);
  });

  it('matches the prefix itself for prefix.*', () => {
    expect(eventMatchesFilter('order', 'order.*')).toBe(true);
  });

  it('handles arbitrary glob patterns', () => {
    expect(eventMatchesFilter('order_created', 'or*ed')).toBe(true);
    expect(eventMatchesFilter('returns_processed', 'or*ed')).toBe(false);
  });

  it('falls back to substring match for non-glob patterns', () => {
    expect(eventMatchesFilter('cart_payment_set', 'payment')).toBe(true);
    expect(eventMatchesFilter('order_created', 'payment')).toBe(false);
  });

  it('trims whitespace around the filter', () => {
    expect(eventMatchesFilter('order_created', '  order.*  ')).toBe(true);
  });
});

describe('bufferToCsv', () => {
  const events: AuditEvent[] = [
    {
      id: '1',
      receivedAt: '2026-05-07T13:00:00.000Z',
      type: 'order_created',
      data: { id: 'ORD-1', total: 99.99 },
    },
    {
      id: '2',
      receivedAt: '2026-05-07T13:00:01.000Z',
      type: 'inventory_adjusted',
      data: { sku: 'WIDGET-1', delta: -2 },
    },
  ];

  it('emits a header row plus one row per event', () => {
    const csv = bufferToCsv(events);
    const lines = csv.split('\n');
    expect(lines.length).toBe(3);
    expect(lines[0]).toBe('received_at,type,data');
  });

  it('quotes every field', () => {
    const csv = bufferToCsv(events);
    const lines = csv.split('\n');
    // Each non-empty data field becomes a quoted JSON blob.
    expect(lines[1].startsWith('"2026-05-07T13:00:00.000Z","order_created",')).toBe(true);
  });

  it('escapes embedded double-quotes by doubling them', () => {
    const out = bufferToCsv([
      {
        id: 'q',
        receivedAt: '2026-05-07T13:00:00.000Z',
        type: 'note',
        data: 'she said "hi"',
      },
    ]);
    // The data string `she said "hi"` becomes `"she said ""hi"""` in CSV.
    expect(out).toContain('"she said ""hi"""');
  });

  it('renders an empty buffer as a header only', () => {
    expect(bufferToCsv([])).toBe('received_at,type,data');
  });

  it('serializes complex JSON data without losing structure', () => {
    const csv = bufferToCsv([
      {
        id: 'n',
        receivedAt: '2026-05-07T13:00:00.000Z',
        type: 'order.created',
        data: { items: [{ sku: 'A', qty: 2 }], total: 19.98 },
      },
    ]);
    // The escaped JSON should round-trip via JSON.parse after CSV-unescape
    // (doubled-quote → single-quote).
    const dataField = csv.split('\n')[1].split(',').slice(2).join(',');
    const unquoted = dataField.slice(1, -1).replace(/""/g, '"');
    const parsed = JSON.parse(unquoted);
    expect(parsed.items[0].sku).toBe('A');
    expect(parsed.total).toBe(19.98);
  });
});
