/**
 * Shipment Tools — Comprehensive Test Suite
 *
 * Tests every tool exported from src/tools/shipments.js:
 *   list_shipments, create_shipment, deliver_shipment
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { shipmentTools } from '../../src/tools/shipments.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findTool(name) {
  const tool = shipmentTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found in shipmentTools`);
  return tool;
}

function makeShipment(overrides = {}) {
  return {
    id: 'ship_001',
    orderId: 'ord_001',
    carrier: 'FedEx',
    service: 'Ground',
    trackingNumber: 'FDX-123456789',
    status: 'in_transit',
    createdAt: '2026-02-20T00:00:00Z',
    ...overrides,
  };
}

function makeCommerce(overrides = {}) {
  return {
    shipments: {
      list: async () => [makeShipment()],
      count: async () => 1,
      create: async (data) => makeShipment({ id: 'ship_new', ...data }),
      deliver: async (id) => makeShipment({ id, status: 'delivered' }),
      ...overrides,
    },
  };
}

// ---------------------------------------------------------------------------
// Structure tests
// ---------------------------------------------------------------------------

describe('Shipment Tools — structure', () => {
  it('exports an array of 3 tools', () => {
    assert.ok(Array.isArray(shipmentTools));
    assert.strictEqual(shipmentTools.length, 3);
  });

  it('every tool has name, handler, permission, and inputSchema', () => {
    for (const tool of shipmentTools) {
      assert.ok(typeof tool.name === 'string', `Missing name`);
      assert.ok(typeof tool.handler === 'function', `${tool.name}: handler not a function`);
      assert.ok(typeof tool.permission === 'string', `${tool.name}: missing permission`);
      assert.ok(typeof tool.inputSchema === 'object', `${tool.name}: missing inputSchema`);
    }
  });

  it('tool names are unique', () => {
    const names = shipmentTools.map((t) => t.name);
    assert.strictEqual(new Set(names).size, names.length);
  });

  it('has the expected tool names', () => {
    const names = shipmentTools.map((t) => t.name).sort();
    assert.deepStrictEqual(names, ['create_shipment', 'deliver_shipment', 'list_shipments']);
  });
});

// ---------------------------------------------------------------------------
// list_shipments
// ---------------------------------------------------------------------------

describe('list_shipments', () => {
  const tool = findTool('list_shipments');

  it('has read permission', () => {
    assert.strictEqual(tool.permission, 'read');
  });

  it('returns success with shipments and count', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: {} });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.count, 1);
    assert.ok(Array.isArray(result.shipments));
    assert.strictEqual(result.shipments.length, 1);
  });

  it('returns shipment data from commerce', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params: {} });
    const ship = result.shipments[0];
    assert.strictEqual(ship.id, 'ship_001');
    assert.strictEqual(ship.orderId, 'ord_001');
    assert.strictEqual(ship.carrier, 'FedEx');
  });

  it('returns empty list when no shipments', async () => {
    const commerce = makeCommerce({ list: async () => [], count: async () => 0 });
    const result = await tool.handler({ commerce, params: {} });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.count, 0);
    assert.strictEqual(result.shipments.length, 0);
  });

  it('propagates commerce errors', async () => {
    const commerce = makeCommerce({ list: async () => { throw new Error('DB error'); } });
    await assert.rejects(() => tool.handler({ commerce, params: {} }), /DB error/);
  });
});

// ---------------------------------------------------------------------------
// create_shipment
// ---------------------------------------------------------------------------

describe('create_shipment', () => {
  const tool = findTool('create_shipment');
  const params = { orderId: 'ord_001', carrier: 'FedEx', service: 'Ground' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview from applyRequired when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
    // applyRequired puts preview data in wouldDo
    assert.ok(result.wouldDo);
    assert.strictEqual(result.wouldDo.orderId, 'ord_001');
  });

  it('creates shipment when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('created'));
    assert.ok(result.shipment);
    assert.strictEqual(result.shipment.orderId, 'ord_001');
  });

  it('passes carrier and service to commerce', async () => {
    let calledWith = {};
    const commerce = makeCommerce({
      create: async (data) => { calledWith = data; return makeShipment(data); },
    });
    await tool.handler({ commerce, params, allowApply: true });
    assert.strictEqual(calledWith.orderId, 'ord_001');
    assert.strictEqual(calledWith.carrier, 'FedEx');
    assert.strictEqual(calledWith.service, 'Ground');
  });

  it('propagates commerce errors', async () => {
    const commerce = makeCommerce({ create: async () => { throw new Error('Order not found'); } });
    await assert.rejects(() => tool.handler({ commerce, params, allowApply: true }), /Order not found/);
  });
});

// ---------------------------------------------------------------------------
// deliver_shipment
// ---------------------------------------------------------------------------

describe('deliver_shipment', () => {
  const tool = findTool('deliver_shipment');
  const params = { shipmentId: 'ship_001' };

  it('has write permission', () => {
    assert.strictEqual(tool.permission, 'write');
  });

  it('returns preview from applyRequired when allowApply is false', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: false });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
    assert.ok(result.wouldDo);
    assert.strictEqual(result.wouldDo.shipmentId, 'ship_001');
  });

  it('delivers shipment when allowApply is true', async () => {
    const result = await tool.handler({ commerce: makeCommerce(), params, allowApply: true });
    assert.strictEqual(result.success, true);
    assert.ok(result.message.includes('delivered'));
    assert.ok(result.shipment);
    assert.strictEqual(result.shipment.status, 'delivered');
  });

  it('propagates commerce errors', async () => {
    const commerce = makeCommerce({ deliver: async () => { throw new Error('Already delivered'); } });
    await assert.rejects(() => tool.handler({ commerce, params, allowApply: true }), /Already delivered/);
  });
});
