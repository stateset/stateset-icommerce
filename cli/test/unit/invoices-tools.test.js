import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { invoiceTools } from '../../src/tools/invoices.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const byName = Object.fromEntries(invoiceTools.map((t) => [t.name, t]));

// ---------------------------------------------------------------------------
// Module exports
// ---------------------------------------------------------------------------

describe('invoiceTools — module exports', () => {
  it('exports an array of 5 tools', () => {
    assert.ok(Array.isArray(invoiceTools));
    assert.equal(invoiceTools.length, 5);
  });

  it('exports expected tool names', () => {
    const names = invoiceTools.map((t) => t.name);
    assert.deepStrictEqual(names, [
      'list_invoices',
      'create_invoice',
      'send_invoice',
      'record_invoice_payment',
      'get_overdue_invoices',
    ]);
  });

  it('all tools have handler functions', () => {
    for (const tool of invoiceTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of invoiceTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of invoiceTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });
});

// ---------------------------------------------------------------------------
// Permissions
// ---------------------------------------------------------------------------

describe('invoiceTools — permissions', () => {
  it('read tools have read permission', () => {
    assert.equal(byName['list_invoices'].permission, 'read');
    assert.equal(byName['get_overdue_invoices'].permission, 'read');
  });

  it('write tools have write permission', () => {
    assert.equal(byName['create_invoice'].permission, 'write');
    assert.equal(byName['send_invoice'].permission, 'write');
    assert.equal(byName['record_invoice_payment'].permission, 'write');
  });
});

// ---------------------------------------------------------------------------
// Input schemas
// ---------------------------------------------------------------------------

describe('invoiceTools — input schemas', () => {
  it('list_invoices has empty inputSchema', () => {
    assert.deepStrictEqual(byName['list_invoices'].inputSchema, {});
  });

  it('create_invoice has customerId and items', () => {
    const schema = byName['create_invoice'].inputSchema;
    assert.ok(schema.customerId);
    assert.ok(schema.items);
  });

  it('send_invoice has invoiceId', () => {
    assert.ok(byName['send_invoice'].inputSchema.invoiceId);
  });

  it('record_invoice_payment has invoiceId and amount', () => {
    const schema = byName['record_invoice_payment'].inputSchema;
    assert.ok(schema.invoiceId);
    assert.ok(schema.amount);
  });

  it('get_overdue_invoices has empty inputSchema', () => {
    assert.deepStrictEqual(byName['get_overdue_invoices'].inputSchema, {});
  });
});

// ---------------------------------------------------------------------------
// Apply guards
// ---------------------------------------------------------------------------

describe('invoiceTools — apply guards', () => {
  it('create_invoice requires --apply', async () => {
    const result = await byName['create_invoice'].handler({
      commerce: {},
      params: { customerId: 'cust-1', items: '[]' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('send_invoice requires --apply', async () => {
    const result = await byName['send_invoice'].handler({
      commerce: {},
      params: { invoiceId: 'inv-1' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('record_invoice_payment requires --apply', async () => {
    const result = await byName['record_invoice_payment'].handler({
      commerce: {},
      params: { invoiceId: 'inv-1', amount: 100 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });
});

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

describe('invoiceTools — error handling', () => {
  it('create_invoice returns error for invalid JSON items', async () => {
    const result = await byName['create_invoice'].handler({
      commerce: {},
      params: { customerId: 'cust-1', items: '{bad-json' },
      allowApply: true,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('Invalid items JSON'));
  });
});
