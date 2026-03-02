/**
 * Manufacturing Tools Test Suite
 *
 * Tests for cli/src/tools/manufacturing.js
 * Covers: list_boms, get_bom, create_bom, add_bom_component, activate_bom,
 *         list_work_orders, get_work_order, create_work_order, start_work_order,
 *         complete_work_order, cancel_work_order
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { manufacturingTools } from '../../src/tools/manufacturing.js';

// ============================================================================
// Helper: find tool by name
// ============================================================================

function findTool(name) {
  const tool = manufacturingTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock factory
// ============================================================================

const mockBom = {
  id: 'bom_001',
  bomNumber: 'BOM-1001',
  name: 'Widget Assembly BOM',
  productId: 'prod_001',
  status: 'draft',
  revision: 'A',
  createdAt: '2026-02-01T00:00:00Z',
};

const mockComponent = {
  id: 'comp_001',
  name: 'Steel Rod',
  componentSku: 'STEEL-001',
  quantity: '4',
  unitOfMeasure: 'each',
  notes: null,
};

const mockWorkOrder = {
  id: 'wo_001',
  workOrderNumber: 'WO-2001',
  productId: 'prod_001',
  status: 'planned',
  priority: 'normal',
  quantityToBuild: 100,
  quantityCompleted: 0,
  scheduledStart: '2026-03-01T00:00:00Z',
  scheduledEnd: '2026-03-15T00:00:00Z',
};

function makeManufacturingCommerce(overrides = {}) {
  return {
    bom: {
      list: async () => [mockBom],
      count: async () => 1,
      get: async (_id) => mockBom,
      getComponents: async (_id) => [mockComponent],
      create: async (data) => ({
        id: 'bom_002',
        bomNumber: 'BOM-1002',
        name: data.name,
        status: 'draft',
      }),
      addComponent: async (_bomId, data) => ({ id: 'comp_002', ...data }),
      activate: async (_id) => ({ id: _id, name: mockBom.name, status: 'active' }),
      ...(overrides.bom || {}),
    },
    workOrders: {
      list: async () => [mockWorkOrder],
      count: async () => 1,
      get: async (_id) => mockWorkOrder,
      create: async (data) => ({
        id: 'wo_002',
        workOrderNumber: 'WO-2002',
        status: 'planned',
        quantityToBuild: data.quantityToBuild,
      }),
      start: async (_id) => ({
        id: _id,
        workOrderNumber: mockWorkOrder.workOrderNumber,
        status: 'in_progress',
      }),
      complete: async (_id, qty) => ({
        id: _id,
        workOrderNumber: mockWorkOrder.workOrderNumber,
        status: 'completed',
        quantityCompleted: qty,
      }),
      cancel: async (_id) => ({
        id: _id,
        workOrderNumber: mockWorkOrder.workOrderNumber,
        status: 'cancelled',
      }),
      ...(overrides.workOrders || {}),
    },
  };
}

// ============================================================================
// Module exports
// ============================================================================

describe('manufacturingTools — module exports', () => {
  it('exports an array of 11 tools', () => {
    assert.ok(Array.isArray(manufacturingTools));
    assert.equal(manufacturingTools.length, 11);
  });

  it('exports expected tool names', () => {
    const names = manufacturingTools.map((t) => t.name);
    assert.deepStrictEqual(names, [
      'list_boms',
      'get_bom',
      'create_bom',
      'add_bom_component',
      'activate_bom',
      'list_work_orders',
      'get_work_order',
      'create_work_order',
      'start_work_order',
      'complete_work_order',
      'cancel_work_order',
    ]);
  });

  it('all tools have handler functions', () => {
    for (const tool of manufacturingTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of manufacturingTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of manufacturingTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });
});

// ============================================================================
// Permission checks
// ============================================================================

describe('manufacturingTools — permission assignments', () => {
  it('read tools have read permission', () => {
    const readToolNames = ['list_boms', 'get_bom', 'list_work_orders', 'get_work_order'];
    for (const name of readToolNames) {
      const tool = findTool(name);
      assert.equal(tool.permission, 'read', `${name} should be read`);
    }
  });

  it('write tools have write permission', () => {
    const writeToolNames = [
      'create_bom',
      'add_bom_component',
      'activate_bom',
      'create_work_order',
      'start_work_order',
      'complete_work_order',
      'cancel_work_order',
    ];
    for (const name of writeToolNames) {
      const tool = findTool(name);
      assert.equal(tool.permission, 'write', `${name} should be write`);
    }
  });
});

// ============================================================================
// Input schema validation
// ============================================================================

describe('manufacturingTools — input schemas', () => {
  it('list_boms has empty inputSchema', () => {
    const schema = findTool('list_boms').inputSchema;
    assert.deepStrictEqual(schema, {});
  });

  it('get_bom has bomId field', () => {
    const schema = findTool('get_bom').inputSchema;
    assert.ok(schema.bomId, 'missing bomId field');
  });

  it('create_bom has name, productId, description, revision fields', () => {
    const schema = findTool('create_bom').inputSchema;
    assert.ok(schema.name, 'missing name field');
    assert.ok(schema.productId, 'missing productId field');
    assert.ok(schema.description, 'missing description field');
    assert.ok(schema.revision, 'missing revision field');
  });

  it('add_bom_component has bomId, name, sku, quantity, unitOfMeasure, notes fields', () => {
    const schema = findTool('add_bom_component').inputSchema;
    assert.ok(schema.bomId, 'missing bomId field');
    assert.ok(schema.name, 'missing name field');
    assert.ok(schema.sku, 'missing sku field');
    assert.ok(schema.quantity, 'missing quantity field');
    assert.ok(schema.unitOfMeasure, 'missing unitOfMeasure field');
    assert.ok(schema.notes, 'missing notes field');
  });

  it('activate_bom has bomId field', () => {
    const schema = findTool('activate_bom').inputSchema;
    assert.ok(schema.bomId, 'missing bomId field');
  });

  it('list_work_orders has empty inputSchema', () => {
    const schema = findTool('list_work_orders').inputSchema;
    assert.deepStrictEqual(schema, {});
  });

  it('get_work_order has workOrderId field', () => {
    const schema = findTool('get_work_order').inputSchema;
    assert.ok(schema.workOrderId, 'missing workOrderId field');
  });

  it('create_work_order has productId, bomId, quantityToBuild, priority, scheduledStart, scheduledEnd, notes', () => {
    const schema = findTool('create_work_order').inputSchema;
    assert.ok(schema.productId, 'missing productId field');
    assert.ok(schema.bomId, 'missing bomId field');
    assert.ok(schema.quantityToBuild, 'missing quantityToBuild field');
    assert.ok(schema.priority, 'missing priority field');
    assert.ok(schema.scheduledStart, 'missing scheduledStart field');
    assert.ok(schema.scheduledEnd, 'missing scheduledEnd field');
    assert.ok(schema.notes, 'missing notes field');
  });

  it('start_work_order has workOrderId field', () => {
    const schema = findTool('start_work_order').inputSchema;
    assert.ok(schema.workOrderId, 'missing workOrderId field');
  });

  it('complete_work_order has workOrderId and quantityCompleted fields', () => {
    const schema = findTool('complete_work_order').inputSchema;
    assert.ok(schema.workOrderId, 'missing workOrderId field');
    assert.ok(schema.quantityCompleted, 'missing quantityCompleted field');
  });

  it('cancel_work_order has workOrderId field', () => {
    const schema = findTool('cancel_work_order').inputSchema;
    assert.ok(schema.workOrderId, 'missing workOrderId field');
  });
});

// ============================================================================
// Handler: list_boms
// ============================================================================

describe('manufacturingTools — list_boms handler', () => {
  it('returns BOMs list with correct shape', async () => {
    const tool = findTool('list_boms');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: {},
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.ok(Array.isArray(result.boms));
    const bom = result.boms[0];
    assert.equal(bom.id, 'bom_001');
    assert.equal(bom.bomNumber, 'BOM-1001');
    assert.equal(bom.name, 'Widget Assembly BOM');
    assert.equal(bom.productId, 'prod_001');
    assert.equal(bom.status, 'draft');
    assert.equal(bom.revision, 'A');
  });
});

// ============================================================================
// Handler: get_bom
// ============================================================================

describe('manufacturingTools — get_bom handler', () => {
  it('returns BOM with components on success', async () => {
    const tool = findTool('get_bom');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { bomId: 'bom_001' },
    });
    assert.equal(result.success, true);
    assert.ok(result.bom);
    assert.equal(result.bom.id, 'bom_001');
    assert.ok(Array.isArray(result.bom.components));
    assert.equal(result.bom.components[0].name, 'Steel Rod');
  });

  it('returns success: false when BOM not found', async () => {
    const tool = findTool('get_bom');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce({ bom: { get: async () => null } }),
      params: { bomId: 'nonexistent' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });
});

// ============================================================================
// Handler: create_bom (write, requires --apply)
// ============================================================================

describe('manufacturingTools — create_bom handler', () => {
  it('returns apply-guard error when allowApply is false', async () => {
    const tool = findTool('create_bom');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { name: 'Test BOM', productId: 'prod_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldCreate);
  });

  it('creates BOM when allowApply is true', async () => {
    const tool = findTool('create_bom');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { name: 'New BOM', productId: 'prod_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('BOM created'));
    assert.ok(result.bom);
    assert.equal(result.bom.name, 'New BOM');
    assert.equal(result.bom.status, 'draft');
  });
});

// ============================================================================
// Handler: add_bom_component (write, requires --apply)
// ============================================================================

describe('manufacturingTools — add_bom_component handler', () => {
  it('returns apply-guard error when allowApply is false', async () => {
    const tool = findTool('add_bom_component');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { bomId: 'bom_001', name: 'Screw', quantity: 8 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldAdd);
  });

  it('adds component when allowApply is true', async () => {
    const tool = findTool('add_bom_component');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { bomId: 'bom_001', name: 'Screw', quantity: 8, unitOfMeasure: 'each' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('Component added'));
    assert.ok(result.component);
  });
});

// ============================================================================
// Handler: activate_bom (write, requires --apply)
// ============================================================================

describe('manufacturingTools — activate_bom handler', () => {
  it('returns apply-guard error when allowApply is false', async () => {
    const tool = findTool('activate_bom');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { bomId: 'bom_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldActivate);
  });

  it('activates BOM when allowApply is true', async () => {
    const tool = findTool('activate_bom');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { bomId: 'bom_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('BOM activated'));
    assert.ok(result.bom);
    assert.equal(result.bom.status, 'active');
  });
});

// ============================================================================
// Handler: list_work_orders
// ============================================================================

describe('manufacturingTools — list_work_orders handler', () => {
  it('returns work orders list with correct shape', async () => {
    const tool = findTool('list_work_orders');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: {},
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.ok(Array.isArray(result.workOrders));
    const wo = result.workOrders[0];
    assert.equal(wo.id, 'wo_001');
    assert.equal(wo.workOrderNumber, 'WO-2001');
    assert.equal(wo.productId, 'prod_001');
    assert.equal(wo.status, 'planned');
    assert.equal(wo.priority, 'normal');
    assert.equal(wo.quantityToBuild, 100);
    assert.equal(wo.quantityCompleted, 0);
  });
});

// ============================================================================
// Handler: get_work_order
// ============================================================================

describe('manufacturingTools — get_work_order handler', () => {
  it('returns work order on success', async () => {
    const tool = findTool('get_work_order');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { workOrderId: 'wo_001' },
    });
    assert.equal(result.success, true);
    assert.ok(result.workOrder);
    assert.equal(result.workOrder.id, 'wo_001');
  });

  it('returns success: false when work order not found', async () => {
    const tool = findTool('get_work_order');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce({ workOrders: { get: async () => null } }),
      params: { workOrderId: 'nonexistent' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });
});

// ============================================================================
// Handler: create_work_order (write, requires --apply)
// ============================================================================

describe('manufacturingTools — create_work_order handler', () => {
  it('returns apply-guard error when allowApply is false', async () => {
    const tool = findTool('create_work_order');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { productId: 'prod_001', quantityToBuild: 50 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldCreate);
  });

  it('creates work order when allowApply is true', async () => {
    const tool = findTool('create_work_order');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { productId: 'prod_001', quantityToBuild: 50, priority: 'high' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('Work order created'));
    assert.ok(result.workOrder);
    assert.equal(result.workOrder.quantityToBuild, 50);
  });
});

// ============================================================================
// Handler: start_work_order (write, requires --apply)
// ============================================================================

describe('manufacturingTools — start_work_order handler', () => {
  it('returns apply-guard error when allowApply is false', async () => {
    const tool = findTool('start_work_order');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { workOrderId: 'wo_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldStart);
  });

  it('starts work order when allowApply is true', async () => {
    const tool = findTool('start_work_order');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { workOrderId: 'wo_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('started'));
    assert.ok(result.workOrder);
    assert.equal(result.workOrder.status, 'in_progress');
  });
});

// ============================================================================
// Handler: complete_work_order (write, requires --apply)
// ============================================================================

describe('manufacturingTools — complete_work_order handler', () => {
  it('returns apply-guard error when allowApply is false', async () => {
    const tool = findTool('complete_work_order');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { workOrderId: 'wo_001', quantityCompleted: 95 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldComplete);
    assert.equal(result.wouldComplete.quantityCompleted, 95);
  });

  it('completes work order when allowApply is true', async () => {
    const tool = findTool('complete_work_order');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { workOrderId: 'wo_001', quantityCompleted: 95 },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('95 units produced'));
    assert.ok(result.workOrder);
    assert.equal(result.workOrder.status, 'completed');
    assert.equal(result.workOrder.quantityCompleted, 95);
  });
});

// ============================================================================
// Handler: cancel_work_order (write, requires --apply)
// ============================================================================

describe('manufacturingTools — cancel_work_order handler', () => {
  it('returns apply-guard error when allowApply is false', async () => {
    const tool = findTool('cancel_work_order');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { workOrderId: 'wo_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldCancel);
  });

  it('cancels work order when allowApply is true', async () => {
    const tool = findTool('cancel_work_order');
    const result = await tool.handler({
      commerce: makeManufacturingCommerce(),
      params: { workOrderId: 'wo_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('cancelled'));
    assert.ok(result.workOrder);
    assert.equal(result.workOrder.status, 'cancelled');
  });
});

// ============================================================================
// Error paths — commerce object missing methods
// ============================================================================

describe('manufacturingTools — error paths (empty commerce)', () => {
  const readTools = ['list_boms', 'get_bom', 'list_work_orders', 'get_work_order'];

  for (const toolName of readTools) {
    it(`${toolName} throws TypeError when commerce methods are missing`, async () => {
      const tool = findTool(toolName);
      try {
        await tool.handler({
          commerce: {},
          params: { bomId: 'bom_001', workOrderId: 'wo_001' },
        });
        assert.fail('should have thrown');
      } catch (err) {
        assert.ok(err instanceof TypeError);
      }
    });
  }

  const writeTools = [
    'create_bom',
    'add_bom_component',
    'activate_bom',
    'create_work_order',
    'start_work_order',
    'complete_work_order',
    'cancel_work_order',
  ];

  for (const toolName of writeTools) {
    it(`${toolName} throws TypeError when commerce methods are missing and allowApply is true`, async () => {
      const tool = findTool(toolName);
      try {
        await tool.handler({
          commerce: {},
          params: {
            bomId: 'bom_001',
            name: 'Test',
            productId: 'prod_001',
            quantity: 1,
            workOrderId: 'wo_001',
            quantityToBuild: 10,
            quantityCompleted: 5,
          },
          allowApply: true,
        });
        assert.fail('should have thrown');
      } catch (err) {
        assert.ok(err instanceof TypeError);
      }
    });
  }
});
