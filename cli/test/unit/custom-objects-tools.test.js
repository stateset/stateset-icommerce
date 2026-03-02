import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { customObjectTools } from '../../src/tools/custom-objects.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const byName = Object.fromEntries(customObjectTools.map((t) => [t.name, t]));

// ---------------------------------------------------------------------------
// Module exports
// ---------------------------------------------------------------------------

describe('customObjectTools — module exports', () => {
  it('exports an array of 12 tools', () => {
    assert.ok(Array.isArray(customObjectTools));
    assert.equal(customObjectTools.length, 12);
  });

  it('exports expected tool names', () => {
    const names = customObjectTools.map((t) => t.name);
    assert.deepStrictEqual(names, [
      'list_custom_object_types',
      'get_custom_object_type',
      'get_custom_object_type_by_handle',
      'create_custom_object_type',
      'update_custom_object_type',
      'delete_custom_object_type',
      'list_custom_objects',
      'get_custom_object',
      'get_custom_object_by_handle',
      'create_custom_object',
      'update_custom_object',
      'delete_custom_object',
    ]);
  });

  it('all tools have handler functions', () => {
    for (const tool of customObjectTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of customObjectTools) {
      assert.ok(
        ['read', 'write', 'delete', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of customObjectTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });
});

// ---------------------------------------------------------------------------
// Permissions
// ---------------------------------------------------------------------------

describe('customObjectTools — permissions', () => {
  it('read tools have read permission', () => {
    assert.equal(byName['list_custom_object_types'].permission, 'read');
    assert.equal(byName['get_custom_object_type'].permission, 'read');
    assert.equal(byName['get_custom_object_type_by_handle'].permission, 'read');
    assert.equal(byName['list_custom_objects'].permission, 'read');
    assert.equal(byName['get_custom_object'].permission, 'read');
    assert.equal(byName['get_custom_object_by_handle'].permission, 'read');
  });

  it('write tools have write permission', () => {
    assert.equal(byName['create_custom_object_type'].permission, 'write');
    assert.equal(byName['update_custom_object_type'].permission, 'write');
    assert.equal(byName['create_custom_object'].permission, 'write');
    assert.equal(byName['update_custom_object'].permission, 'write');
  });

  it('delete tools have delete permission', () => {
    assert.equal(byName['delete_custom_object_type'].permission, 'delete');
    assert.equal(byName['delete_custom_object'].permission, 'delete');
  });
});

// ---------------------------------------------------------------------------
// Input schemas
// ---------------------------------------------------------------------------

describe('customObjectTools — input schemas', () => {
  it('list_custom_object_types has optional search, limit, offset', () => {
    const schema = byName['list_custom_object_types'].inputSchema;
    assert.ok(schema.search);
    assert.ok(schema.limit);
    assert.ok(schema.offset);
  });

  it('get_custom_object_type has id', () => {
    assert.ok(byName['get_custom_object_type'].inputSchema.id);
  });

  it('get_custom_object_type_by_handle has handle', () => {
    assert.ok(byName['get_custom_object_type_by_handle'].inputSchema.handle);
  });

  it('create_custom_object_type has handle and displayName', () => {
    const schema = byName['create_custom_object_type'].inputSchema;
    assert.ok(schema.handle);
    assert.ok(schema.displayName);
  });

  it('create_custom_object_type has fields array', () => {
    assert.ok(byName['create_custom_object_type'].inputSchema.fields);
  });

  it('list_custom_objects has optional filters', () => {
    const schema = byName['list_custom_objects'].inputSchema;
    assert.ok(schema.typeHandle);
    assert.ok(schema.ownerType);
    assert.ok(schema.ownerId);
  });

  it('create_custom_object has typeHandle', () => {
    assert.ok(byName['create_custom_object'].inputSchema.typeHandle);
  });

  it('create_custom_object supports values and valuesJson', () => {
    const schema = byName['create_custom_object'].inputSchema;
    assert.ok(schema.values);
    assert.ok(schema.valuesJson);
  });

  it('delete_custom_object has id', () => {
    assert.ok(byName['delete_custom_object'].inputSchema.id);
  });
});

// ---------------------------------------------------------------------------
// Apply guards — type operations
// ---------------------------------------------------------------------------

describe('customObjectTools — apply guards (types)', () => {
  it('create_custom_object_type requires --apply', async () => {
    const result = await byName['create_custom_object_type'].handler({
      commerce: {},
      params: { handle: 'test', displayName: 'Test Type' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldCreate);
  });

  it('update_custom_object_type requires --apply', async () => {
    const result = await byName['update_custom_object_type'].handler({
      commerce: {},
      params: { id: 'type-1', displayName: 'Updated' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldUpdate);
  });

  it('delete_custom_object_type requires --apply', async () => {
    const result = await byName['delete_custom_object_type'].handler({
      commerce: {},
      params: { id: 'type-1' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldDelete);
  });
});

// ---------------------------------------------------------------------------
// Apply guards — record operations
// ---------------------------------------------------------------------------

describe('customObjectTools — apply guards (records)', () => {
  it('create_custom_object requires --apply', async () => {
    const result = await byName['create_custom_object'].handler({
      commerce: {},
      params: { typeHandle: 'test', values: { foo: 'bar' } },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldCreate);
  });

  it('create_custom_object preview normalizes valuesJson', async () => {
    const result = await byName['create_custom_object'].handler({
      commerce: {},
      params: { typeHandle: 'test', valuesJson: '{"a":1}' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.equal(result.wouldCreate.valuesJson, '{"a":1}');
  });

  it('update_custom_object requires --apply', async () => {
    const result = await byName['update_custom_object'].handler({
      commerce: {},
      params: { id: 'obj-1', values: { foo: 'baz' } },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldUpdate);
  });

  it('delete_custom_object requires --apply', async () => {
    const result = await byName['delete_custom_object'].handler({
      commerce: {},
      params: { id: 'obj-1' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.wouldDelete);
  });
});
