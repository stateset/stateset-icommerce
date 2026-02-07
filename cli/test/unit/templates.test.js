/**
 * Unit tests for channels/templates.js — message templates
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import {
  orderConfirmation,
  shippingUpdate,
  deliveryConfirmation,
  returnReceived,
  returnApproved,
  abandonedCartReminder,
  lowStockAlert,
  backInStock,
  welcomeMessage,
  approvalRequest,
} from '../../src/channels/templates.js';

// ===========================================================================
// Order templates
// ===========================================================================

describe('orderConfirmation', () => {
  it('builds confirmation with order details', () => {
    const msg = orderConfirmation({ orderNumber: 'ORD-42', total: 99.99, items: [1, 2, 3] });
    assert.strictEqual(msg.title, 'Order Confirmed: ORD-42');
    assert.strictEqual(msg.color, '#4CAF50');
    assert.ok(msg.fields.some((f) => f.name === 'Total' && f.value === '$99.99'));
    assert.ok(msg.fields.some((f) => f.name === 'Items' && f.value === '3'));
  });

  it('falls back to id', () => {
    const msg = orderConfirmation({ id: 'X', total: 0 });
    assert.strictEqual(msg.title, 'Order Confirmed: X');
  });
});

describe('shippingUpdate', () => {
  it('includes tracking when available', () => {
    const msg = shippingUpdate({ orderNumber: 'O1', id: '1', trackingNumber: 'TRK-ABC' });
    assert.ok(msg.fields.some((f) => f.name === 'Tracking' && f.value === 'TRK-ABC'));
    assert.ok(msg.buttons.length > 0);
  });

  it('omits tracking button when no tracking', () => {
    const msg = shippingUpdate({ orderNumber: 'O1', id: '1' });
    assert.strictEqual(msg.buttons.length, 0);
  });
});

describe('deliveryConfirmation', () => {
  it('builds delivery template', () => {
    const msg = deliveryConfirmation({ orderNumber: 'O1' });
    assert.ok(msg.title.includes('Delivered'));
    assert.ok(msg.description.includes('delivered'));
  });
});

// ===========================================================================
// Return templates
// ===========================================================================

describe('returnReceived', () => {
  it('builds return received template', () => {
    const msg = returnReceived({ id: 'RET-1', reason: 'Defective' });
    assert.ok(msg.title.includes('RET-1'));
    assert.ok(msg.fields.some((f) => f.name === 'Reason' && f.value === 'Defective'));
  });

  it('handles missing reason', () => {
    const msg = returnReceived({ id: 'RET-1' });
    assert.ok(msg.fields.some((f) => f.name === 'Reason' && f.value === 'Not specified'));
  });
});

describe('returnApproved', () => {
  it('includes refund amount', () => {
    const msg = returnApproved({ id: 'RET-1', refundAmount: 25.5 });
    assert.ok(msg.fields.some((f) => f.name === 'Refund' && f.value === '$25.50'));
  });

  it('shows Pending when no refund amount', () => {
    const msg = returnApproved({ id: 'RET-1' });
    assert.ok(msg.fields.some((f) => f.name === 'Refund' && f.value === 'Pending'));
  });
});

// ===========================================================================
// Cart templates
// ===========================================================================

describe('abandonedCartReminder', () => {
  it('builds abandoned cart reminder', () => {
    const msg = abandonedCartReminder({
      id: 'C1',
      subtotal: 79.99,
      items: [{ name: 'Widget', quantity: 2, sku: 'W1' }],
    });
    assert.ok(msg.title.includes('cart'));
    assert.ok(msg.description.includes('Widget'));
    assert.ok(msg.fields.some((f) => f.name === 'Subtotal' && f.value === '$79.99'));
    assert.ok(msg.buttons.length > 0);
  });

  it('handles empty items', () => {
    const msg = abandonedCartReminder({ id: 'C2', subtotal: 0, items: [] });
    assert.ok(msg.description.includes('items waiting'));
  });

  it('truncates items at 3', () => {
    const items = Array.from({ length: 5 }, (_, i) => ({ name: `Item ${i}`, quantity: 1 }));
    const msg = abandonedCartReminder({ id: 'C3', subtotal: 50, items });
    assert.ok(msg.description.includes('and 2 more'));
  });
});

// ===========================================================================
// Inventory templates
// ===========================================================================

describe('lowStockAlert', () => {
  it('builds low stock alert', () => {
    const msg = lowStockAlert({ sku: 'SKU-1', available: 3, reorderPoint: 10 });
    assert.ok(msg.title.includes('SKU-1'));
    assert.ok(msg.fields.some((f) => f.name === 'Available' && f.value === '3'));
    assert.ok(msg.fields.some((f) => f.name === 'Reorder Point' && f.value === '10'));
  });

  it('includes name in description', () => {
    const msg = lowStockAlert({ sku: 'SKU-1', name: 'Blue Widget', available: 3 });
    assert.ok(msg.description.includes('Blue Widget'));
  });

  it('omits description when no name', () => {
    const msg = lowStockAlert({ sku: 'SKU-1', available: 3 });
    assert.strictEqual(msg.description, undefined);
  });
});

describe('backInStock', () => {
  it('builds back in stock notification', () => {
    const msg = backInStock({ sku: 'SKU-1', name: 'Red Gadget', available: 50 });
    assert.ok(msg.title.includes('Red Gadget'));
    assert.ok(msg.description.includes('available again'));
  });

  it('falls back to sku when no name', () => {
    const msg = backInStock({ sku: 'SKU-1', available: 10 });
    assert.ok(msg.title.includes('SKU-1'));
  });
});

// ===========================================================================
// Welcome / Approval templates
// ===========================================================================

describe('welcomeMessage', () => {
  it('includes name when provided', () => {
    const msg = welcomeMessage('Alice');
    assert.ok(msg.title.includes('Alice'));
  });

  it('works without name', () => {
    const msg = welcomeMessage();
    assert.strictEqual(msg.title, 'Welcome!');
  });
});

describe('approvalRequest', () => {
  it('builds approval request', () => {
    const msg = approvalRequest({
      title: 'Large Purchase',
      amount: 5000,
      requester: 'Bob',
      domain: 'procurement',
    });
    assert.ok(msg.title.includes('Large Purchase'));
    assert.ok(msg.fields.some((f) => f.name === 'Amount' && f.value === '$5000.00'));
    assert.ok(msg.fields.some((f) => f.name === 'Requested By' && f.value === 'Bob'));
    assert.ok(msg.fields.some((f) => f.name === 'Type' && f.value === 'procurement'));
  });

  it('omits amount when not provided', () => {
    const msg = approvalRequest({ title: 'Request' });
    assert.ok(!msg.fields.some((f) => f.name === 'Amount'));
  });
});
