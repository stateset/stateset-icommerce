/**
 * Tests for custom-objects.js tool module.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { customObjectTools } from '../../src/tools/custom-objects.js';

function findTool(name) {
  const tool = customObjectTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

const mockType = {
  id: 'type-1',
  handle: 'warranty_registration',
  displayName: 'Warranty Registration',
  description: 'Product warranty registrations',
  fields: [
    { key: 'serial_number', fieldType: 'string', required: true },
    { key: 'purchase_date', fieldType: 'date_time', required: false },
  ],
};

const mockObject = {
  id: 'obj-1',
  typeHandle: 'warranty_registration',
  handle: 'wr-001',
  ownerType: 'customer',
  ownerId: 'cust-1',
  valuesJson: '{"serial_number":"SN123","purchase_date":"2026-01-15"}',
};

function makeCommerce(overrides = {}) {
  return {
    customObjects: {
      listTypes: async () => [mockType],
      getType: async (id) => (id === 'type-1' ? mockType : null),
      getTypeByHandle: async (handle) => (handle === 'warranty_registration' ? mockType : null),
      createType: async (data) => ({ id: 'type-2', ...data }),
      updateType: async (id, data) => ({ id, ...mockType, ...data }),
      deleteType: async () => {},
      listObjects: async () => [mockObject],
      getObject: async (id) => (id === 'obj-1' ? mockObject : null),
      getObjectByHandle: async (typeHandle, objHandle) =>
        typeHandle === 'warranty_registration' && objHandle === 'wr-001' ? mockObject : null,
      createObject: async (data) => ({ id: 'obj-2', ...data }),
      updateObject: async (id, data) => ({ id, ...mockObject, ...data }),
      deleteObject: async () => {},
      ...overrides,
    },
  };
}

// ============================================================================
// Type operations
// ============================================================================

describe('Custom Object Type tools', () => {
  describe('list_custom_object_types', () => {
    const tool = findTool('list_custom_object_types');

    it('lists types', async () => {
      const result = await tool.handler({ commerce: makeCommerce(), params: {} });
      assert.equal(result.success, true);
      assert.equal(result.types.length, 1);
    });

    it('passes search filter', async () => {
      let calledFilter;
      const commerce = makeCommerce({
        listTypes: async (filter) => {
          calledFilter = filter;
          return [];
        },
      });
      await tool.handler({ commerce, params: { search: 'warranty', limit: 10 } });
      assert.equal(calledFilter.search, 'warranty');
      assert.equal(calledFilter.limit, 10);
    });
  });

  describe('get_custom_object_type', () => {
    const tool = findTool('get_custom_object_type');

    it('returns type by ID', async () => {
      const result = await tool.handler({ commerce: makeCommerce(), params: { id: 'type-1' } });
      assert.equal(result.success, true);
      assert.equal(result.type.handle, 'warranty_registration');
    });

    it('returns error for missing type', async () => {
      const result = await tool.handler({ commerce: makeCommerce(), params: { id: 'nope' } });
      assert.ok(result.error);
    });
  });

  describe('get_custom_object_type_by_handle', () => {
    const tool = findTool('get_custom_object_type_by_handle');

    it('returns type by handle', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { handle: 'warranty_registration' },
      });
      assert.equal(result.success, true);
    });

    it('returns error for missing handle', async () => {
      const result = await tool.handler({ commerce: makeCommerce(), params: { handle: 'nope' } });
      assert.ok(result.error);
    });
  });

  describe('create_custom_object_type', () => {
    const tool = findTool('create_custom_object_type');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { handle: 'new_type', displayName: 'New Type' },
        allowApply: false,
      });
      assert.ok(result.error);
      assert.ok(result.wouldCreate);
      assert.equal(result.wouldCreate.handle, 'new_type');
    });

    it('creates type with --apply', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: {
          handle: 'test_type',
          displayName: 'Test Type',
          fields: [{ key: 'name', fieldType: 'string', required: true }],
        },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.ok(result.type);
    });
  });

  describe('update_custom_object_type', () => {
    const tool = findTool('update_custom_object_type');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { id: 'type-1', displayName: 'Updated' },
        allowApply: false,
      });
      assert.ok(result.error);
      assert.ok(result.wouldUpdate);
    });

    it('updates with --apply', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { id: 'type-1', displayName: 'Updated Name' },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });

  describe('delete_custom_object_type', () => {
    const tool = findTool('delete_custom_object_type');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { id: 'type-1' },
        allowApply: false,
      });
      assert.ok(result.error);
      assert.ok(result.wouldDelete);
    });

    it('deletes with --apply', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { id: 'type-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });
});

// ============================================================================
// Record operations
// ============================================================================

describe('Custom Object Record tools', () => {
  describe('list_custom_objects', () => {
    const tool = findTool('list_custom_objects');

    it('lists objects', async () => {
      const result = await tool.handler({ commerce: makeCommerce(), params: {} });
      assert.equal(result.success, true);
      assert.equal(result.objects.length, 1);
    });

    it('passes filters', async () => {
      let calledFilter;
      const commerce = makeCommerce({
        listObjects: async (filter) => {
          calledFilter = filter;
          return [];
        },
      });
      await tool.handler({ commerce, params: { typeHandle: 'warranty_registration', ownerId: 'cust-1' } });
      assert.equal(calledFilter.typeHandle, 'warranty_registration');
      assert.equal(calledFilter.ownerId, 'cust-1');
    });
  });

  describe('get_custom_object', () => {
    const tool = findTool('get_custom_object');

    it('returns object by ID', async () => {
      const result = await tool.handler({ commerce: makeCommerce(), params: { id: 'obj-1' } });
      assert.equal(result.success, true);
      assert.equal(result.object.handle, 'wr-001');
    });

    it('returns error for missing', async () => {
      const result = await tool.handler({ commerce: makeCommerce(), params: { id: 'nope' } });
      assert.ok(result.error);
    });
  });

  describe('get_custom_object_by_handle', () => {
    const tool = findTool('get_custom_object_by_handle');

    it('returns object by type+handle', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { typeHandle: 'warranty_registration', objectHandle: 'wr-001' },
      });
      assert.equal(result.success, true);
    });

    it('returns error for missing', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { typeHandle: 'warranty_registration', objectHandle: 'missing' },
      });
      assert.ok(result.error);
    });
  });

  describe('create_custom_object', () => {
    const tool = findTool('create_custom_object');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { typeHandle: 'warranty_registration', values: { serial_number: 'SN999' } },
        allowApply: false,
      });
      assert.ok(result.error);
      assert.ok(result.wouldCreate);
    });

    it('creates with values object', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { typeHandle: 'warranty_registration', values: { serial_number: 'SN999' } },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.equal(result.object.valuesJson, '{"serial_number":"SN999"}');
    });

    it('creates with valuesJson string', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { typeHandle: 'warranty_registration', valuesJson: '{"serial_number":"SN888"}' },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.equal(result.object.valuesJson, '{"serial_number":"SN888"}');
    });

    it('defaults to empty JSON for missing values', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { typeHandle: 'warranty_registration' },
        allowApply: true,
      });
      assert.equal(result.success, true);
      assert.equal(result.object.valuesJson, '{}');
    });
  });

  describe('update_custom_object', () => {
    const tool = findTool('update_custom_object');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { id: 'obj-1', values: { serial_number: 'UPDATED' } },
        allowApply: false,
      });
      assert.ok(result.error);
      assert.ok(result.wouldUpdate);
    });

    it('updates with --apply', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { id: 'obj-1', values: { serial_number: 'UPDATED' } },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });

  describe('delete_custom_object', () => {
    const tool = findTool('delete_custom_object');

    it('returns preview without --apply', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { id: 'obj-1' },
        allowApply: false,
      });
      assert.ok(result.error);
      assert.ok(result.wouldDelete);
    });

    it('deletes with --apply', async () => {
      const result = await tool.handler({
        commerce: makeCommerce(),
        params: { id: 'obj-1' },
        allowApply: true,
      });
      assert.equal(result.success, true);
    });
  });
});

// ============================================================================
// Structural checks
// ============================================================================

describe('customObjectTools structure', () => {
  it('exports 12 tools', () => {
    assert.equal(customObjectTools.length, 12);
  });

  it('all tools have required fields', () => {
    for (const tool of customObjectTools) {
      assert.ok(tool.name, 'missing name');
      assert.ok(typeof tool.handler === 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });

  it('write tools require --apply', () => {
    const writeTools = customObjectTools.filter(
      (t) => t.permission === 'write' || t.permission === 'delete',
    );
    assert.ok(writeTools.length >= 5, 'Expected at least 5 write/delete tools');
  });
});
