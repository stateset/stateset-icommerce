// Unit tests for the generic CSV helpers. downloadCsv is DOM-only and
// covered by the orders bulk-export integration.

import { describe, expect, it } from 'vitest';

import { rowsToCsv, toCsvCell, type CsvColumn } from '@/lib/csv/csv';
import {
  CUSTOMER_CSV_COLUMNS,
  INVENTORY_CSV_COLUMNS,
  ORDER_CSV_COLUMNS,
} from '@/lib/csv/specs';
import type { Customer, InventoryItem, Order } from '@/lib/types';

describe('csv · toCsvCell', () => {
  it('returns empty for null/undefined', () => {
    expect(toCsvCell(null)).toBe('');
    expect(toCsvCell(undefined)).toBe('');
  });

  it('quotes plain strings', () => {
    expect(toCsvCell('hello')).toBe('"hello"');
  });

  it('escapes embedded double-quotes', () => {
    expect(toCsvCell('she said "hi"')).toBe('"she said ""hi"""');
  });

  it('JSON-stringifies non-strings', () => {
    expect(toCsvCell(42)).toBe('"42"');
    expect(toCsvCell(true)).toBe('"true"');
    expect(toCsvCell({ a: 1 })).toBe('"{""a"":1}"');
  });
});

describe('csv · rowsToCsv', () => {
  interface Row {
    name: string;
    age: number;
  }

  const cols: CsvColumn<Row>[] = [
    { key: 'name', accessor: (r) => r.name },
    { key: 'age', accessor: (r) => r.age },
    { key: 'name_length', accessor: (r) => r.name.length },
  ];

  it('emits header + one row per input', () => {
    const csv = rowsToCsv([{ name: 'Ada', age: 200 }, { name: 'Alan', age: 100 }], cols);
    const lines = csv.split('\n');
    expect(lines.length).toBe(3);
    expect(lines[0]).toBe('name,age,name_length');
  });

  it('emits header only on empty input', () => {
    expect(rowsToCsv([], cols)).toBe('name,age,name_length');
  });

  it('preserves input order', () => {
    const csv = rowsToCsv([{ name: 'B', age: 2 }, { name: 'A', age: 1 }], cols);
    const lines = csv.split('\n');
    expect(lines[1]).toContain('"B"');
    expect(lines[2]).toContain('"A"');
  });

  it('supports derived columns via accessor', () => {
    const csv = rowsToCsv([{ name: 'hello', age: 0 }], cols);
    const cells = csv.split('\n')[1].split(',');
    expect(cells[2]).toBe('"5"'); // name_length derived from "hello".length
  });

  it('escapes commas and quotes inside cell values', () => {
    const csv = rowsToCsv([{ name: 'has,comma', age: 0 }], cols);
    expect(csv).toContain('"has,comma"');
  });
});

describe('csv · CUSTOMER_CSV_COLUMNS', () => {
  const customer: Customer = {
    id: 'CUST-1',
    email: 'ada@example.com',
    firstName: 'Ada',
    lastName: 'Lovelace',
    phone: '+44',
    addresses: [],
    tags: ['vip', 'beta'],
    totalOrders: 3,
    totalSpent: 199.99,
    averageOrderValue: 66.66,
    lastOrderDate: '2026-04-01',
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-04-01T00:00:00.000Z',
  };

  it('exposes the expected canonical column keys', () => {
    expect(CUSTOMER_CSV_COLUMNS.map((c) => c.key)).toEqual([
      'customer_id',
      'email',
      'first_name',
      'last_name',
      'phone',
      'total_orders',
      'total_spent',
      'avg_order_value',
      'last_order_date',
      'tags',
    ]);
  });

  it('falls back to empty string for missing optional fields', () => {
    const minimal: Customer = {
      id: 'C',
      email: 'x@y',
      addresses: [],
      tags: [],
      totalOrders: 0,
      totalSpent: 0,
      averageOrderValue: 0,
      createdAt: '2026-01-01T00:00:00.000Z',
      updatedAt: '2026-01-01T00:00:00.000Z',
    };
    const csv = rowsToCsv([minimal], CUSTOMER_CSV_COLUMNS);
    const dataRow = csv.split('\n')[1];
    // first_name, last_name, phone, last_order_date should all be empty cells
    expect(dataRow).toMatch(/^"C","x@y","","","","0","0","0","",""$/);
  });

  it('joins tags with semicolons', () => {
    const csv = rowsToCsv([customer], CUSTOMER_CSV_COLUMNS);
    expect(csv).toContain('"vip;beta"');
  });
});

describe('csv · INVENTORY_CSV_COLUMNS', () => {
  const item: InventoryItem = {
    id: 'INV-1',
    sku: 'WIDGET-001',
    productId: 'PROD-1',
    productName: 'Widget',
    quantity: 100,
    reservedQuantity: 10,
    availableQuantity: 90,
    reorderPoint: 25,
    reorderQuantity: 50,
    warehouseId: 'WH-MAIN',
    location: 'A1',
    lastRestocked: '2026-04-01',
    updatedAt: '2026-04-01T00:00:00.000Z',
  };

  it('exposes the expected canonical column keys', () => {
    expect(INVENTORY_CSV_COLUMNS.map((c) => c.key)).toEqual([
      'sku',
      'product_id',
      'product_name',
      'quantity',
      'reserved',
      'available',
      'reorder_point',
      'reorder_quantity',
      'warehouse_id',
      'location',
      'last_restocked',
    ]);
  });

  it('serializes a full inventory row', () => {
    const csv = rowsToCsv([item], INVENTORY_CSV_COLUMNS);
    const row = csv.split('\n')[1];
    expect(row).toContain('"WIDGET-001"');
    expect(row).toContain('"100"');
    expect(row).toContain('"WH-MAIN"');
  });
});

describe('csv · ORDER_CSV_COLUMNS', () => {
  const order: Order = {
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
  };

  it('item_count derives from items.length', () => {
    const csv = rowsToCsv([order], ORDER_CSV_COLUMNS);
    const cells = csv.split('\n')[1].split(',');
    // item_count is at column index 5
    expect(cells[5]).toBe('"2"');
  });

  it('matches the canonical orders header used elsewhere', () => {
    expect(ORDER_CSV_COLUMNS.map((c) => c.key)).toEqual([
      'order_id',
      'customer_id',
      'status',
      'total_amount',
      'currency',
      'item_count',
      'created_at',
      'updated_at',
    ]);
  });
});
