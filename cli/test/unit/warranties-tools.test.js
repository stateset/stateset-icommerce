import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { warrantyTools } from '../../src/tools/warranties.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const byName = Object.fromEntries(warrantyTools.map((t) => [t.name, t]));

// ---------------------------------------------------------------------------
// Module exports
// ---------------------------------------------------------------------------

describe('warrantyTools — module exports', () => {
  it('exports an array of 7 tools', () => {
    assert.ok(Array.isArray(warrantyTools));
    assert.equal(warrantyTools.length, 7);
  });

  it('exports expected tool names', () => {
    const names = warrantyTools.map((t) => t.name);
    assert.deepStrictEqual(names, [
      'list_warranties',
      'get_warranty',
      'create_warranty',
      'create_warranty_claim',
      'approve_warranty_claim',
      'deny_warranty_claim',
      'complete_warranty_claim',
    ]);
  });

  it('all tools have handler functions', () => {
    for (const tool of warrantyTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of warrantyTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of warrantyTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });
});

// ---------------------------------------------------------------------------
// Permissions
// ---------------------------------------------------------------------------

describe('warrantyTools — permissions', () => {
  it('list_warranties has read permission', () => {
    assert.equal(byName['list_warranties'].permission, 'read');
  });

  it('get_warranty has read permission', () => {
    assert.equal(byName['get_warranty'].permission, 'read');
  });

  it('write tools have write permission', () => {
    assert.equal(byName['create_warranty'].permission, 'write');
    assert.equal(byName['create_warranty_claim'].permission, 'write');
    assert.equal(byName['approve_warranty_claim'].permission, 'write');
    assert.equal(byName['deny_warranty_claim'].permission, 'write');
    assert.equal(byName['complete_warranty_claim'].permission, 'write');
  });
});

// ---------------------------------------------------------------------------
// Input schemas
// ---------------------------------------------------------------------------

describe('warrantyTools — input schemas', () => {
  it('list_warranties has empty inputSchema', () => {
    assert.deepStrictEqual(byName['list_warranties'].inputSchema, {});
  });

  it('get_warranty has warrantyId', () => {
    assert.ok(byName['get_warranty'].inputSchema.warrantyId);
  });

  it('create_warranty has customerId', () => {
    assert.ok(byName['create_warranty'].inputSchema.customerId);
  });

  it('create_warranty has warrantyType enum', () => {
    assert.ok(byName['create_warranty'].inputSchema.warrantyType);
  });

  it('create_warranty has durationMonths', () => {
    assert.ok(byName['create_warranty'].inputSchema.durationMonths);
  });

  it('create_warranty has serialNumber', () => {
    assert.ok(byName['create_warranty'].inputSchema.serialNumber);
  });

  it('create_warranty_claim has warrantyId and issueDescription support', () => {
    const schema = byName['create_warranty_claim'].inputSchema;
    assert.ok(schema.warrantyId);
    assert.ok(schema.issueDescription);
    assert.ok(schema.description);
  });

  it('create_warranty_claim supports contactEmail and contactPhone', () => {
    assert.ok(byName['create_warranty_claim'].inputSchema.contactEmail);
    assert.ok(byName['create_warranty_claim'].inputSchema.contactPhone);
  });

  it('approve_warranty_claim has claimId', () => {
    assert.ok(byName['approve_warranty_claim'].inputSchema.claimId);
  });

  it('deny_warranty_claim has claimId and reason', () => {
    const schema = byName['deny_warranty_claim'].inputSchema;
    assert.ok(schema.claimId);
    assert.ok(schema.reason);
  });

  it('complete_warranty_claim has claimId and resolution', () => {
    const schema = byName['complete_warranty_claim'].inputSchema;
    assert.ok(schema.claimId);
    assert.ok(schema.resolution);
  });
});

// ---------------------------------------------------------------------------
// Apply guards
// ---------------------------------------------------------------------------

describe('warrantyTools — apply guards', () => {
  it('create_warranty requires --apply', async () => {
    const result = await byName['create_warranty'].handler({
      commerce: {},
      params: { customerId: 'cust-1' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('create_warranty_claim requires --apply', async () => {
    const result = await byName['create_warranty_claim'].handler({
      commerce: {},
      params: { warrantyId: 'war-1', issueDescription: 'Broken screen' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('approve_warranty_claim requires --apply', async () => {
    const result = await byName['approve_warranty_claim'].handler({
      commerce: {},
      params: { claimId: 'claim-1' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('deny_warranty_claim requires --apply', async () => {
    const result = await byName['deny_warranty_claim'].handler({
      commerce: {},
      params: { claimId: 'claim-1', reason: 'Out of coverage' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('complete_warranty_claim requires --apply', async () => {
    const result = await byName['complete_warranty_claim'].handler({
      commerce: {},
      params: { claimId: 'claim-1', resolution: 'repair' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('create_warranty_claim rejects missing issueDescription and alias', async () => {
    const result = await byName['create_warranty_claim'].handler({
      commerce: {},
      params: { warrantyId: 'war-1' },
      allowApply: true,
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'issueDescription is required');
  });
});
