/**
 * Inventory Tools Test Suite
 *
 * Tests for cli/src/tools/inventory.js
 * Covers: get_stock, create_inventory_item, adjust_inventory,
 *         reserve_inventory, confirm_reservation, release_reservation
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { inventoryTools } from '../../src/tools/inventory.js';

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

const mockStock = {
  sku: 'WIDGET-001',
  name: 'Widget',
  totalOnHand: 100,
  totalAllocated: 20,
  totalAvailable: 80,
};

const mockItem = {
  id: 'inv_001',
  sku: 'WIDGET-001',
  name: 'Widget',
  description: 'A standard widget',
};

const mockReservation = {
  id: 'res_001',
  quantity: 5,
  status: 'reserved',
};

function makeInventoryCommerce(overrides = {}) {
  return {
    inventory: {
      getStock: async (_sku) => mockStock,
      createItem: async (data) => ({ ...mockItem, ...data }),
      adjust: async (_sku, _qty, _reason) => undefined,
      reserve: async (_sku, _qty, _refType, _refId, _expires) => mockReservation,
      confirmReservation: async (_id) => undefined,
      releaseReservation: async (_id) => undefined,
      ...overrides,
    },
  };
}

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Inventory Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(inventoryTools));
  });

  it('has at least 6 tools', () => {
    assert.ok(inventoryTools.length >= 6, `Expected >= 6, got ${inventoryTools.length}`);
  });

  it('every tool has name, handler, and permission', () => {
    for (const tool of inventoryTools) {
      assert.ok(tool.name, 'tool missing name');
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });
});

// ============================================================================
// get_stock
// ============================================================================

describe('get_stock', () => {
  const tool = findTool(inventoryTools, 'get_stock');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns stock for valid SKU', async () => {
    const result = await tool.handler({
      commerce: makeInventoryCommerce(),
      params: { sku: 'WIDGET-001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.stock.sku, 'WIDGET-001');
    assert.equal(result.stock.totalOnHand, 100);
    assert.equal(result.stock.totalAllocated, 20);
    assert.equal(result.stock.totalAvailable, 80);
  });

  it('maps all expected fields on stock', async () => {
    const result = await tool.handler({
      commerce: makeInventoryCommerce(),
      params: { sku: 'WIDGET-001' },
    });
    const s = result.stock;
    assert.ok('sku' in s);
    assert.ok('name' in s);
    assert.ok('totalOnHand' in s);
    assert.ok('totalAllocated' in s);
    assert.ok('totalAvailable' in s);
  });

  it('returns success: false when stock not found', async () => {
    const commerce = makeInventoryCommerce({ getStock: async () => null });
    const result = await tool.handler({
      commerce,
      params: { sku: 'NONEXISTENT-SKU' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('No inventory item found'));
  });

  it('returns error when getStock throws', async () => {
    const commerce = makeInventoryCommerce({
      getStock: async () => {
        throw new Error('DB error');
      },
    });
    try {
      await tool.handler({ commerce, params: { sku: 'WIDGET-001' } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('DB error'));
    }
  });
});

// ============================================================================
// create_inventory_item
// ============================================================================

describe('create_inventory_item', () => {
  const tool = findTool(inventoryTools, 'create_inventory_item');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeInventoryCommerce(),
      params: { sku: 'NEW-SKU', name: 'New Item', initialQuantity: 50 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field');
    assert.ok(result.wouldCreate, 'expected wouldCreate preview');
  });

  it('creates inventory item with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeInventoryCommerce(),
      params: { sku: 'NEW-SKU', name: 'New Item', initialQuantity: 50 },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('created'));
    assert.ok(result.item);
    assert.ok(result.item.id);
    assert.ok(result.item.sku);
    assert.ok(result.item.name);
  });

  it('returns error when createItem throws', async () => {
    const commerce = makeInventoryCommerce({
      createItem: async () => {
        throw new Error('Duplicate SKU');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { sku: 'DUP-SKU', name: 'Dup' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Duplicate SKU'));
    }
  });
});

// ============================================================================
// adjust_inventory
// ============================================================================

describe('adjust_inventory', () => {
  const tool = findTool(inventoryTools, 'adjust_inventory');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeInventoryCommerce(),
      params: { sku: 'WIDGET-001', quantity: 10, reason: 'Received shipment' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field');
    assert.ok(result.wouldAdjust, 'expected wouldAdjust preview');
    assert.equal(result.wouldAdjust.sku, 'WIDGET-001');
    assert.equal(result.wouldAdjust.currentOnHand, 100);
    assert.equal(result.wouldAdjust.adjustment, 10);
    assert.equal(result.wouldAdjust.newOnHand, 110);
  });

  it('returns preview with zero onHand when stock not found', async () => {
    const commerce = makeInventoryCommerce({ getStock: async () => null });
    const result = await tool.handler({
      commerce,
      params: { sku: 'UNKNOWN', quantity: 5, reason: 'Initial stock' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.equal(result.wouldAdjust.currentOnHand, 0);
    assert.equal(result.wouldAdjust.newOnHand, 5);
  });

  it('adjusts inventory with positive quantity', async () => {
    const adjustedStock = { ...mockStock, totalOnHand: 110, totalAvailable: 90 };
    const commerce = makeInventoryCommerce({
      getStock: async () => adjustedStock,
    });
    const result = await tool.handler({
      commerce,
      params: { sku: 'WIDGET-001', quantity: 10, reason: 'Received shipment' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('+10'));
    assert.equal(result.stock.totalOnHand, 110);
  });

  it('adjusts inventory with negative quantity', async () => {
    const adjustedStock = { ...mockStock, totalOnHand: 95, totalAvailable: 75 };
    const commerce = makeInventoryCommerce({
      getStock: async () => adjustedStock,
    });
    const result = await tool.handler({
      commerce,
      params: { sku: 'WIDGET-001', quantity: -5, reason: 'Damaged goods' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('-5'));
    assert.equal(result.stock.totalOnHand, 95);
  });

  it('returns error when adjust throws', async () => {
    const commerce = makeInventoryCommerce({
      adjust: async () => {
        throw new Error('Insufficient stock');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { sku: 'WIDGET-001', quantity: -999, reason: 'test' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Insufficient stock'));
    }
  });
});

// ============================================================================
// reserve_inventory
// ============================================================================

describe('reserve_inventory', () => {
  const tool = findTool(inventoryTools, 'reserve_inventory');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeInventoryCommerce(),
      params: {
        sku: 'WIDGET-001',
        quantity: 5,
        referenceType: 'order',
        referenceId: 'ord_001',
      },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field');
    assert.ok(result.wouldReserve, 'expected wouldReserve preview');
  });

  it('reserves inventory with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeInventoryCommerce(),
      params: {
        sku: 'WIDGET-001',
        quantity: 5,
        referenceType: 'order',
        referenceId: 'ord_001',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('reserved'));
    assert.ok(result.reservation);
    assert.equal(result.reservation.id, 'res_001');
    assert.equal(result.reservation.quantity, 5);
    assert.equal(result.reservation.status, 'reserved');
  });

  it('passes expiresInSeconds to commerce.inventory.reserve', async () => {
    let calledWith = null;
    const commerce = makeInventoryCommerce({
      reserve: async (sku, qty, refType, refId, expires) => {
        calledWith = { sku, qty, refType, refId, expires };
        return mockReservation;
      },
    });
    await tool.handler({
      commerce,
      params: {
        sku: 'WIDGET-001',
        quantity: 5,
        referenceType: 'order',
        referenceId: 'ord_001',
        expiresInSeconds: 3600,
      },
      allowApply: true,
    });
    assert.equal(calledWith.expires, 3600);
  });

  it('returns error when reserve throws', async () => {
    const commerce = makeInventoryCommerce({
      reserve: async () => {
        throw new Error('Not enough available stock');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: {
          sku: 'WIDGET-001',
          quantity: 999,
          referenceType: 'order',
          referenceId: 'ord_001',
        },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Not enough available stock'));
    }
  });
});

// ============================================================================
// confirm_reservation
// ============================================================================

describe('confirm_reservation', () => {
  const tool = findTool(inventoryTools, 'confirm_reservation');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeInventoryCommerce(),
      params: { reservationId: 'res_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field');
    assert.ok(result.wouldConfirm);
    assert.equal(result.wouldConfirm.reservationId, 'res_001');
  });

  it('confirms reservation with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeInventoryCommerce(),
      params: { reservationId: 'res_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('confirmed'));
  });

  it('returns error when confirmReservation throws', async () => {
    const commerce = makeInventoryCommerce({
      confirmReservation: async () => {
        throw new Error('Reservation not found');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { reservationId: 'bad_res' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Reservation not found'));
    }
  });
});

// ============================================================================
// release_reservation
// ============================================================================

describe('release_reservation', () => {
  const tool = findTool(inventoryTools, 'release_reservation');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeInventoryCommerce(),
      params: { reservationId: 'res_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field');
    assert.ok(result.wouldRelease);
    assert.equal(result.wouldRelease.reservationId, 'res_001');
  });

  it('releases reservation with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeInventoryCommerce(),
      params: { reservationId: 'res_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('released'));
  });

  it('returns error when releaseReservation throws', async () => {
    const commerce = makeInventoryCommerce({
      releaseReservation: async () => {
        throw new Error('Reservation already confirmed');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { reservationId: 'res_001' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Reservation already confirmed'));
    }
  });
});
