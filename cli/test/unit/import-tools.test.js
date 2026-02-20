import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { importTools } from '../../src/tools/import.js';

// ---------------------------------------------------------------------------
// Tool definition validation
// ---------------------------------------------------------------------------

describe('importTools — module exports', () => {
  it('exports an array of 6 tools', () => {
    assert.ok(Array.isArray(importTools));
    assert.equal(importTools.length, 6);
  });

  it('exports expected tool names', () => {
    const names = importTools.map((t) => t.name);
    assert.deepStrictEqual(names, [
      'import_shopify_data',
      'import_status',
      'list_id_mappings',
      'import_csv',
      'import_json',
      'export_data',
    ]);
  });
});

// ---------------------------------------------------------------------------
// Tool schema validation
// ---------------------------------------------------------------------------

describe('importTools — schema shapes', () => {
  const byName = Object.fromEntries(importTools.map((t) => [t.name, t]));

  it('import_shopify_data has required source field', () => {
    const tool = byName['import_shopify_data'];
    assert.ok(tool.inputSchema.source);
    assert.equal(tool.permission, 'write');
    assert.ok(tool.description.includes('Shopify'));
  });

  it('import_status has empty inputSchema', () => {
    const tool = byName['import_status'];
    assert.deepStrictEqual(tool.inputSchema, {});
    assert.equal(tool.permission, 'read');
  });

  it('list_id_mappings requires platform', () => {
    const tool = byName['list_id_mappings'];
    assert.ok(tool.inputSchema.platform);
    assert.equal(tool.permission, 'read');
  });

  it('import_csv requires filePath and entityType', () => {
    const tool = byName['import_csv'];
    assert.ok(tool.inputSchema.filePath);
    assert.ok(tool.inputSchema.entityType);
    assert.equal(tool.permission, 'write');
  });

  it('import_json requires filePath and entityType', () => {
    const tool = byName['import_json'];
    assert.ok(tool.inputSchema.filePath);
    assert.ok(tool.inputSchema.entityType);
    assert.equal(tool.permission, 'write');
  });

  it('export_data requires entityType', () => {
    const tool = byName['export_data'];
    assert.ok(tool.inputSchema.entityType);
    assert.equal(tool.permission, 'read');
  });
});

// ---------------------------------------------------------------------------
// Handler behavior — apply guard
// ---------------------------------------------------------------------------

describe('importTools — apply guard', () => {
  const byName = Object.fromEntries(importTools.map((t) => [t.name, t]));

  it('import_shopify_data returns preview when allowApply is false', async () => {
    const result = await byName['import_shopify_data'].handler({
      commerce: {},
      params: { source: 'api', entities: ['customers'] },
      allowApply: false,
    });
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
    assert.ok(result.wouldDo);
  });

  it('import_csv returns preview when allowApply is false', async () => {
    const result = await byName['import_csv'].handler({
      commerce: {},
      params: { filePath: '/tmp/data.csv', entityType: 'customers', platform: 'shopify' },
      allowApply: false,
    });
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
  });

  it('import_json returns preview when allowApply is false', async () => {
    const result = await byName['import_json'].handler({
      commerce: {},
      params: { filePath: '/tmp/data.json', entityType: 'products', platform: 'shopify' },
      allowApply: false,
    });
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
  });
});

// ---------------------------------------------------------------------------
// Handler behavior — import_status
// ---------------------------------------------------------------------------

describe('importTools — import_status handler', () => {
  const byName = Object.fromEntries(importTools.map((t) => [t.name, t]));

  it('returns success with guidance message', async () => {
    const result = await byName['import_status'].handler({
      commerce: {},
      params: {},
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('import_shopify_data'));
  });
});

// ---------------------------------------------------------------------------
// Handler behavior — error paths (modules not available)
// ---------------------------------------------------------------------------

describe('importTools — error handling', () => {
  const byName = Object.fromEntries(importTools.map((t) => [t.name, t]));

  it('import_shopify_data catches module errors gracefully', async () => {
    // The dynamic imports will fail in test because adapter modules
    // expect a real commerce object. The handler wraps in try/catch.
    const result = await byName['import_shopify_data'].handler({
      commerce: {},
      params: { source: 'api', entities: ['customers'] },
      allowApply: true,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('import_csv catches module errors gracefully', async () => {
    const result = await byName['import_csv'].handler({
      commerce: {},
      params: { filePath: '/tmp/nonexistent.csv', entityType: 'customers' },
      allowApply: true,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('import_json catches module errors gracefully', async () => {
    const result = await byName['import_json'].handler({
      commerce: {},
      params: { filePath: '/tmp/nonexistent.json', entityType: 'products' },
      allowApply: true,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('export_data catches module errors gracefully', async () => {
    const result = await byName['export_data'].handler({
      commerce: {},
      params: { entityType: 'customers' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('list_id_mappings catches module errors gracefully', async () => {
    const result = await byName['list_id_mappings'].handler({
      commerce: {},
      params: { platform: 'shopify' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });
});

// ---------------------------------------------------------------------------
// Handler behavior — descriptions are non-empty
// ---------------------------------------------------------------------------

describe('importTools — descriptions', () => {
  it('all tools have non-empty descriptions', () => {
    for (const tool of importTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });

  it('all tools have handler functions', () => {
    for (const tool of importTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have a permission field', () => {
    for (const tool of importTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });
});
