/**
 * Order Tools Test Suite
 *
 * Tests for cli/src/tools/orders.js
 * Covers: list_orders, get_order, create_order, update_order_status,
 *         ship_order, cancel_order
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { orderTools } from '../../src/tools/orders.js';

// ============================================================================
// Helper: find tool by name
// ============================================================================

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock factory
// ============================================================================

const mockOrder = {
  id: 'ord_001',
  orderNumber: 'ORD-1001',
  customerId: 'cust_001',
  status: 'pending',
  totalAmount: '59.98',
  currency: 'USD',
  paymentStatus: 'unpaid',
  fulfillmentStatus: 'unfulfilled',
  trackingNumber: null,
  items: [
    {
      id: 'item_001',
      sku: 'WIDGET-001',
      name: 'Widget',
      quantity: 2,
      unitPrice: '29.99',
      total: '59.98',
    },
  ],
  createdAt: '2026-02-21T00:00:00Z',
  updatedAt: '2026-02-21T00:00:00Z',
};

function makeOrderCommerce(overrides = {}) {
  return {
    orders: {
      list: async () => [mockOrder],
      count: async () => 1,
      get: async (_id) => mockOrder,
      create: async (data) => ({ ...mockOrder, ...data }),
      updateStatus: async (_id, status) => ({ ...mockOrder, status }),
      ship: async (_id, trackingNumber) => ({
        ...mockOrder,
        status: 'shipped',
        trackingNumber: trackingNumber || 'TRACK-001',
      }),
      cancel: async (_id) => ({ ...mockOrder, status: 'cancelled' }),
      ...overrides,
    },
  };
}

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Order Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(orderTools));
  });

  it('has at least 6 tools', () => {
    assert.ok(orderTools.length >= 6, `Expected >= 6, got ${orderTools.length}`);
  });

  it('every tool has name, handler, and permission', () => {
    for (const tool of orderTools) {
      assert.ok(tool.name, 'tool missing name');
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });
});

// ============================================================================
// list_orders
// ============================================================================

describe('list_orders', () => {
  const tool = findTool(orderTools, 'list_orders');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns list with totalCount and returned', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: { limit: 50 },
    });
    assert.equal(result.success, true);
    assert.equal(result.totalCount, 1);
    assert.equal(result.returned, 1);
    assert.equal(result.orders.length, 1);
    assert.equal(result.orders[0].id, 'ord_001');
  });

  it('maps all expected fields on each order', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: { limit: 50 },
    });
    const o = result.orders[0];
    assert.ok('id' in o);
    assert.ok('orderNumber' in o);
    assert.ok('customerId' in o);
    assert.ok('status' in o);
    assert.ok('totalAmount' in o);
    assert.ok('currency' in o);
    assert.ok('paymentStatus' in o);
    assert.ok('fulfillmentStatus' in o);
    assert.ok('itemCount' in o);
    assert.ok('createdAt' in o);
  });

  it('computes itemCount from items array', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: { limit: 50 },
    });
    assert.equal(result.orders[0].itemCount, 1);
  });

  it('respects limit parameter', async () => {
    const manyOrders = Array.from({ length: 10 }, (_, i) => ({
      ...mockOrder,
      id: `ord_${i}`,
    }));
    const commerce = makeOrderCommerce({
      list: async () => manyOrders,
      count: async () => 10,
    });
    const result = await tool.handler({
      commerce,
      params: { limit: 3 },
    });
    assert.equal(result.totalCount, 10);
    assert.equal(result.returned, 3);
    assert.equal(result.orders.length, 3);
  });

  it('returns error when list throws', async () => {
    const commerce = makeOrderCommerce({
      list: async () => {
        throw new Error('DB error');
      },
    });
    try {
      await tool.handler({ commerce, params: { limit: 50 } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('DB error'));
    }
  });
});

// ============================================================================
// get_order
// ============================================================================

describe('get_order', () => {
  const tool = findTool(orderTools, 'get_order');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns order for valid identifier', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: { identifier: 'ord_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.order.id, 'ord_001');
    assert.equal(result.order.orderNumber, 'ORD-1001');
    assert.equal(result.order.status, 'pending');
  });

  it('maps items array on the returned order', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: { identifier: 'ord_001' },
    });
    assert.ok(Array.isArray(result.order.items));
    assert.equal(result.order.items.length, 1);
    assert.equal(result.order.items[0].sku, 'WIDGET-001');
    assert.equal(result.order.items[0].quantity, 2);
  });

  it('returns success: false when order not found', async () => {
    const commerce = makeOrderCommerce({ get: async () => null });
    const result = await tool.handler({
      commerce,
      params: { identifier: 'NONEXISTENT' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when get throws', async () => {
    const commerce = makeOrderCommerce({
      get: async () => {
        throw new Error('lookup failed');
      },
    });
    try {
      await tool.handler({ commerce, params: { identifier: 'ord_001' } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('lookup failed'));
    }
  });
});

// ============================================================================
// create_order
// ============================================================================

describe('create_order', () => {
  const tool = findTool(orderTools, 'create_order');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: {
        customerId: 'cust_001',
        items: [{ sku: 'W-001', name: 'Widget', quantity: 2, unitPrice: 29.99 }],
        currency: 'USD',
      },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field');
    assert.ok(result.wouldCreate, 'expected wouldCreate preview');
    assert.equal(result.wouldCreate.itemCount, 1);
    assert.equal(result.wouldCreate.estimatedTotal, 59.98);
  });

  it('creates order with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: {
        customerId: 'cust_001',
        items: [{ sku: 'W-001', name: 'Widget', quantity: 2, unitPrice: 29.99 }],
        currency: 'USD',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('created'));
    assert.ok(result.order);
    assert.equal(result.order.id, 'ord_001');
  });

  it('calls autoIndexEntity when provided', async () => {
    let indexed = null;
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: {
        customerId: 'cust_001',
        items: [{ sku: 'W-001', name: 'Widget', quantity: 1, unitPrice: 10 }],
      },
      allowApply: true,
      autoIndexEntity: (type, entity) => {
        indexed = { type, entity };
      },
    });
    assert.equal(result.success, true);
    assert.equal(indexed.type, 'order');
    assert.ok(indexed.entity);
  });

  it('returns error when commerce.orders.create throws', async () => {
    const commerce = makeOrderCommerce({
      create: async () => {
        throw new Error('DB write failed');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: {
          customerId: 'cust_001',
          items: [{ sku: 'W-001', name: 'Widget', quantity: 1, unitPrice: 10 }],
        },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('DB write failed'));
    }
  });
});

// ============================================================================
// update_order_status
// ============================================================================

describe('update_order_status', () => {
  const tool = findTool(orderTools, 'update_order_status');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: { orderId: 'ord_001', status: 'confirmed' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field');
    assert.ok(result.wouldUpdate);
    assert.equal(result.wouldUpdate.orderId, 'ord_001');
    assert.equal(result.wouldUpdate.newStatus, 'confirmed');
  });

  it('updates status with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: { orderId: 'ord_001', status: 'confirmed' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('confirmed'));
    assert.equal(result.order.status, 'confirmed');
  });

  it('returns error when updateStatus throws', async () => {
    const commerce = makeOrderCommerce({
      updateStatus: async () => {
        throw new Error('Invalid status transition');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { orderId: 'ord_001', status: 'shipped' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Invalid status transition'));
    }
  });
});

// ============================================================================
// ship_order
// ============================================================================

describe('ship_order', () => {
  const tool = findTool(orderTools, 'ship_order');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: { orderId: 'ord_001', trackingNumber: 'FEDEX-123' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field');
    assert.ok(result.wouldShip);
    assert.equal(result.wouldShip.orderId, 'ord_001');
    assert.equal(result.wouldShip.trackingNumber, 'FEDEX-123');
  });

  it('ships order with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: { orderId: 'ord_001', trackingNumber: 'FEDEX-123' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('shipped'));
    assert.equal(result.order.status, 'shipped');
    assert.equal(result.order.trackingNumber, 'FEDEX-123');
  });

  it('ships order without tracking number', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: { orderId: 'ord_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.order.status, 'shipped');
  });

  it('returns error when ship throws', async () => {
    const commerce = makeOrderCommerce({
      ship: async () => {
        throw new Error('Order already shipped');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { orderId: 'ord_001' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Order already shipped'));
    }
  });
});

// ============================================================================
// cancel_order
// ============================================================================

describe('cancel_order', () => {
  const tool = findTool(orderTools, 'cancel_order');

  it('is a delete tool', () => {
    assert.equal(tool.permission, 'delete');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: { orderId: 'ord_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field');
    assert.ok(result.wouldCancel);
    assert.equal(result.wouldCancel.orderId, 'ord_001');
  });

  it('cancels order with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeOrderCommerce(),
      params: { orderId: 'ord_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('cancelled'));
    assert.equal(result.order.status, 'cancelled');
  });

  it('returns error when cancel throws', async () => {
    const commerce = makeOrderCommerce({
      cancel: async () => {
        throw new Error('Order cannot be cancelled');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { orderId: 'ord_001' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Order cannot be cancelled'));
    }
  });
});
