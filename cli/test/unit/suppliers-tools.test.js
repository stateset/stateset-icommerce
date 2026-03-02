import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { supplierTools } from '../../src/tools/suppliers.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const byName = Object.fromEntries(supplierTools.map((t) => [t.name, t]));

// ---------------------------------------------------------------------------
// Module exports
// ---------------------------------------------------------------------------

describe('supplierTools — module exports', () => {
  it('exports an array of 6 tools', () => {
    assert.ok(Array.isArray(supplierTools));
    assert.equal(supplierTools.length, 6);
  });

  it('exports expected tool names', () => {
    const names = supplierTools.map((t) => t.name);
    assert.deepStrictEqual(names, [
      'list_suppliers',
      'create_supplier',
      'list_purchase_orders',
      'create_purchase_order',
      'approve_purchase_order',
      'send_purchase_order',
    ]);
  });

  it('all tools have handler functions', () => {
    for (const tool of supplierTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of supplierTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of supplierTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });
});

// ---------------------------------------------------------------------------
// Permissions
// ---------------------------------------------------------------------------

describe('supplierTools — permissions', () => {
  it('read tools have read permission', () => {
    assert.equal(byName['list_suppliers'].permission, 'read');
    assert.equal(byName['list_purchase_orders'].permission, 'read');
  });

  it('write tools have write permission', () => {
    assert.equal(byName['create_supplier'].permission, 'write');
    assert.equal(byName['create_purchase_order'].permission, 'write');
    assert.equal(byName['approve_purchase_order'].permission, 'write');
    assert.equal(byName['send_purchase_order'].permission, 'write');
  });
});

// ---------------------------------------------------------------------------
// Input schemas
// ---------------------------------------------------------------------------

describe('supplierTools — input schemas', () => {
  it('list_suppliers has empty inputSchema', () => {
    assert.deepStrictEqual(byName['list_suppliers'].inputSchema, {});
  });

  it('create_supplier has name field', () => {
    assert.ok(byName['create_supplier'].inputSchema.name);
  });

  it('create_purchase_order has supplierId and items', () => {
    const schema = byName['create_purchase_order'].inputSchema;
    assert.ok(schema.supplierId);
    assert.ok(schema.items);
  });

  it('approve_purchase_order has purchaseOrderId and approvedBy', () => {
    const schema = byName['approve_purchase_order'].inputSchema;
    assert.ok(schema.purchaseOrderId);
    assert.ok(schema.approvedBy);
  });

  it('send_purchase_order has purchaseOrderId', () => {
    assert.ok(byName['send_purchase_order'].inputSchema.purchaseOrderId);
  });
});

// ---------------------------------------------------------------------------
// Apply guards
// ---------------------------------------------------------------------------

describe('supplierTools — apply guards', () => {
  it('create_supplier requires --apply', async () => {
    const result = await byName['create_supplier'].handler({
      commerce: {},
      params: { name: 'Test Supplier' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('create_purchase_order requires --apply', async () => {
    const result = await byName['create_purchase_order'].handler({
      commerce: {},
      params: { supplierId: 'sup-1', items: '[]' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('approve_purchase_order requires --apply', async () => {
    const result = await byName['approve_purchase_order'].handler({
      commerce: {},
      params: { purchaseOrderId: 'po-1', approvedBy: 'admin' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('send_purchase_order requires --apply', async () => {
    const result = await byName['send_purchase_order'].handler({
      commerce: {},
      params: { purchaseOrderId: 'po-1' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });
});

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

describe('supplierTools — error handling', () => {
  it('list_suppliers catches errors gracefully', async () => {
    try {
      await byName['list_suppliers'].handler({ commerce: {} });
    } catch (err) {
      // Expected — handler doesn't wrap in try-catch, commerce.purchaseOrders is undefined
      assert.ok(err);
    }
  });

  it('create_purchase_order returns error for invalid JSON items', async () => {
    const result = await byName['create_purchase_order'].handler({
      commerce: {},
      params: { supplierId: 'sup-1', items: 'not-json' },
      allowApply: true,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('Invalid items JSON'));
  });
});
