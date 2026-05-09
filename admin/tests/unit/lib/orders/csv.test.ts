// Unit tests for the orders CSV helpers used by the bulk-orders page.
// downloadCsv touches the DOM and is excluded from these node-side tests.

import { describe, expect, it } from 'vitest';

import { ORDERS_CSV_HEADER, ordersToCsv, toCsvCell } from '@/lib/orders/csv';
import type { Order } from '@/lib/types';

const fakeOrder = (overrides: Partial<Order> = {}): Order => ({
  id: 'ORD-1',
  customerId: 'CUST-1',
  status: 'pending',
  items: [
    { productId: 'p1', sku: 'A', name: 'A', quantity: 1, unitPrice: 10, totalPrice: 10 },
    { productId: 'p2', sku: 'B', name: 'B', quantity: 2, unitPrice: 5, totalPrice: 10 },
  ],
  totalAmount: 20,
  currency: 'USD',
  createdAt: '2026-05-07T13:00:00.000Z',
  updatedAt: '2026-05-07T13:05:00.000Z',
  ...overrides,
});

describe('orders CSV · toCsvCell', () => {
  it('returns empty for null/undefined', () => {
    expect(toCsvCell(null)).toBe('');
    expect(toCsvCell(undefined)).toBe('');
  });

  it('quotes plain strings', () => {
    expect(toCsvCell('hello')).toBe('"hello"');
  });

  it('escapes embedded double-quotes by doubling them', () => {
    expect(toCsvCell('she said "hi"')).toBe('"she said ""hi"""');
  });

  it('JSON-stringifies non-strings', () => {
    expect(toCsvCell(42)).toBe('"42"');
    expect(toCsvCell(true)).toBe('"true"');
    expect(toCsvCell({ a: 1 })).toBe('"{""a"":1}"');
    expect(toCsvCell([1, 2])).toBe('"[1,2]"');
  });
});

describe('orders CSV · ordersToCsv', () => {
  it('emits the canonical header as the first line', () => {
    const csv = ordersToCsv([]);
    expect(csv).toBe(ORDERS_CSV_HEADER.join(','));
  });

  it('emits header plus one row per order', () => {
    const csv = ordersToCsv([fakeOrder({ id: 'A' }), fakeOrder({ id: 'B' })]);
    expect(csv.split('\n').length).toBe(3);
  });

  it('preserves input order in the output rows', () => {
    const csv = ordersToCsv([
      fakeOrder({ id: 'B' }),
      fakeOrder({ id: 'A' }),
      fakeOrder({ id: 'C' }),
    ]);
    const lines = csv.split('\n');
    expect(lines[1]).toContain('"B"');
    expect(lines[2]).toContain('"A"');
    expect(lines[3]).toContain('"C"');
  });

  it('renders item_count from items.length', () => {
    const csv = ordersToCsv([fakeOrder()]);
    const row = csv.split('\n')[1];
    // item_count is column 6 (0-indexed: 5)
    const cells = row.split(',');
    expect(cells[5]).toBe('"2"');
  });

  it('escapes IDs that contain commas/quotes safely', () => {
    const csv = ordersToCsv([
      fakeOrder({ id: 'weird,id', customerId: 'has "quote"' }),
    ]);
    const row = csv.split('\n')[1];
    expect(row).toContain('"weird,id"');
    expect(row).toContain('"has ""quote"""');
  });

  it('emits valid CSV that round-trips back through naive split (when free of literal newlines)', () => {
    const csv = ordersToCsv([fakeOrder({ id: 'ORD-X', customerId: 'CUST-Y' })]);
    const [_, dataRow] = csv.split('\n');
    expect(dataRow).toContain('"ORD-X"');
    expect(dataRow).toContain('"CUST-Y"');
    expect(dataRow).toContain('"USD"');
    expect(dataRow).toContain('"pending"');
  });
});
