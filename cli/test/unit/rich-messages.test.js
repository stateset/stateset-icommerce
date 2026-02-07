/**
 * Unit tests for channels/rich-messages.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import {
  createOrderSummary,
  createOrderList,
  createInventoryCard,
  createCartSummary,
  createAnalyticsSummary,
  richMessageToPlainText,
} from '../../src/channels/rich-messages.js';

// ===========================================================================
// createOrderSummary
// ===========================================================================

describe('createOrderSummary', () => {
  it('builds summary with basic fields', () => {
    const msg = createOrderSummary({ id: 'ORD-1', status: 'shipped', total: 99.99 });
    assert.strictEqual(msg.title, 'Order ORD-1');
    assert.ok(msg.fields.some((f) => f.name === 'Status' && f.value === 'SHIPPED'));
    assert.ok(msg.fields.some((f) => f.name === 'Total' && f.value === '$99.99'));
    assert.strictEqual(msg.footer, 'StateSet Commerce');
  });

  it('uses orderNumber over id', () => {
    const msg = createOrderSummary({
      id: '1',
      orderNumber: 'ORD-100',
      status: 'pending',
      total: 0,
    });
    assert.strictEqual(msg.title, 'Order ORD-100');
  });

  it('uses snake_case order_number', () => {
    const msg = createOrderSummary({
      id: '1',
      order_number: 'ORD-SC',
      status: 'pending',
      total: 0,
    });
    assert.strictEqual(msg.title, 'Order ORD-SC');
  });

  it('assigns color by status', () => {
    assert.strictEqual(createOrderSummary({ status: 'shipped', total: 0 }).color, '#4CAF50');
    assert.strictEqual(createOrderSummary({ status: 'cancelled', total: 0 }).color, '#F44336');
    assert.strictEqual(createOrderSummary({ status: 'pending', total: 0 }).color, '#FFA500');
  });

  it('includes customer email', () => {
    const msg = createOrderSummary({ status: 'pending', total: 0, customerEmail: 'a@b.com' });
    assert.ok(msg.fields.some((f) => f.name === 'Customer' && f.value === 'a@b.com'));
  });

  it('includes items list', () => {
    const msg = createOrderSummary({
      status: 'pending',
      total: 50,
      items: [{ name: 'Widget', quantity: 2, unitPrice: 25 }],
    });
    assert.ok(msg.fields.some((f) => f.name === 'Items' && f.value.includes('Widget')));
  });

  it('truncates items at 5', () => {
    const items = Array.from({ length: 7 }, (_, i) => ({
      name: `Item ${i}`,
      quantity: 1,
      unitPrice: 1,
    }));
    const msg = createOrderSummary({ status: 'pending', total: 7, items });
    const itemField = msg.fields.find((f) => f.name === 'Items');
    assert.ok(itemField.value.includes('...and 2 more'));
  });

  it('includes tracking number', () => {
    const msg = createOrderSummary({ status: 'shipped', total: 0, trackingNumber: 'TRK123' });
    assert.ok(msg.fields.some((f) => f.name === 'Tracking' && f.value === 'TRK123'));
  });
});

// ===========================================================================
// createOrderList
// ===========================================================================

describe('createOrderList', () => {
  it('builds list from orders', () => {
    const orders = [
      { id: 'O1', status: 'shipped', total: 50 },
      { id: 'O2', status: 'pending', total: 30 },
    ];
    const msg = createOrderList(orders);
    assert.strictEqual(msg.title, 'Orders (2)');
    assert.strictEqual(msg.fields.length, 2);
  });

  it('limits to 10 entries', () => {
    const orders = Array.from({ length: 15 }, (_, i) => ({
      id: `O${i}`,
      status: 'pending',
      total: 0,
    }));
    const msg = createOrderList(orders);
    assert.strictEqual(msg.fields.length, 10);
    assert.ok(msg.footer.includes('10 of 15'));
  });
});

// ===========================================================================
// createInventoryCard
// ===========================================================================

describe('createInventoryCard', () => {
  it('green for healthy stock', () => {
    const msg = createInventoryCard('SKU-1', { available: 100, reserved: 5, reorderPoint: 10 });
    assert.strictEqual(msg.color, '#4CAF50');
    assert.strictEqual(msg.footer, 'In Stock');
  });

  it('orange for low stock', () => {
    const msg = createInventoryCard('SKU-1', { available: 5, reserved: 0, reorderPoint: 10 });
    assert.strictEqual(msg.color, '#FFA500');
    assert.strictEqual(msg.footer, 'LOW STOCK');
  });

  it('red for out of stock', () => {
    const msg = createInventoryCard('SKU-1', { available: 0, reserved: 0 });
    assert.strictEqual(msg.color, '#F44336');
    assert.strictEqual(msg.footer, 'OUT OF STOCK');
  });

  it('includes name when present', () => {
    const msg = createInventoryCard('SKU-1', { available: 10, name: 'Blue Widget' });
    assert.ok(msg.fields.some((f) => f.name === 'Name' && f.value === 'Blue Widget'));
  });
});

// ===========================================================================
// createCartSummary
// ===========================================================================

describe('createCartSummary', () => {
  it('builds cart summary', () => {
    const msg = createCartSummary({ id: 'CART-1', status: 'active', subtotal: 49.99 });
    assert.strictEqual(msg.title, 'Cart CART-1');
    assert.ok(msg.fields.some((f) => f.name === 'Subtotal' && f.value === '$49.99'));
  });

  it('includes items', () => {
    const msg = createCartSummary({
      id: 'C1',
      subtotal: 30,
      items: [{ name: 'Gadget', quantity: 1, unitPrice: 30 }],
    });
    assert.ok(msg.fields.some((f) => f.name === 'Items' && f.value.includes('Gadget')));
  });
});

// ===========================================================================
// createAnalyticsSummary
// ===========================================================================

describe('createAnalyticsSummary', () => {
  it('builds analytics summary', () => {
    const msg = createAnalyticsSummary({
      totalRevenue: 12500.5,
      orderCount: 42,
      averageOrderValue: 297.63,
      itemsSold: 150,
      uniqueCustomers: 35,
    });
    assert.strictEqual(msg.title, 'Sales Summary');
    assert.ok(msg.fields.some((f) => f.name === 'Revenue' && f.value.includes('12500.50')));
    assert.ok(msg.fields.some((f) => f.name === 'Orders' && f.value === '42'));
    assert.ok(msg.fields.some((f) => f.name === 'Items Sold' && f.value === '150'));
    assert.ok(msg.fields.some((f) => f.name === 'Customers' && f.value === '35'));
  });

  it('handles defaults', () => {
    const msg = createAnalyticsSummary({});
    assert.ok(msg.fields.some((f) => f.name === 'Revenue' && f.value === '$0.00'));
  });
});

// ===========================================================================
// richMessageToPlainText
// ===========================================================================

describe('richMessageToPlainText', () => {
  it('formats title', () => {
    const text = richMessageToPlainText({ title: 'My Title' });
    assert.ok(text.includes('*My Title*'));
  });

  it('includes description', () => {
    const text = richMessageToPlainText({ title: 'T', description: 'Some details' });
    assert.ok(text.includes('Some details'));
  });

  it('formats fields', () => {
    const text = richMessageToPlainText({
      title: 'T',
      fields: [
        { name: 'Status', value: 'ACTIVE' },
        { name: 'Count', value: '42' },
      ],
    });
    assert.ok(text.includes('Status: ACTIVE'));
    assert.ok(text.includes('Count: 42'));
  });

  it('formats buttons with URLs', () => {
    const text = richMessageToPlainText({
      title: 'T',
      buttons: [{ label: 'View', url: 'https://example.com' }],
    });
    assert.ok(text.includes('View: https://example.com'));
  });

  it('formats buttons without URLs', () => {
    const text = richMessageToPlainText({
      title: 'T',
      buttons: [{ label: 'Click Me' }],
    });
    assert.ok(text.includes('[Click Me]'));
  });

  it('includes footer', () => {
    const text = richMessageToPlainText({ title: 'T', footer: 'StateSet' });
    assert.ok(text.includes('— StateSet'));
  });
});
