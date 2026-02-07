/**
 * Unit tests for dry-run.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import {
  PREVIEWABLE_OPERATIONS,
  DryRunManager,
  createDryRunManager,
  formatDryRunResult,
  parseDryRunFlag,
} from '../../src/dry-run.js';

// ===========================================================================
// PREVIEWABLE_OPERATIONS
// ===========================================================================

describe('PREVIEWABLE_OPERATIONS', () => {
  it('defines known operations', () => {
    const ops = Object.keys(PREVIEWABLE_OPERATIONS);
    assert.ok(ops.includes('create_customer'));
    assert.ok(ops.includes('create_order'));
    assert.ok(ops.includes('ship_order'));
    assert.ok(ops.includes('cancel_order'));
    assert.ok(ops.includes('adjust_inventory'));
    assert.ok(ops.includes('create_return'));
    assert.ok(ops.includes('create_cart'));
    assert.ok(ops.includes('complete_checkout'));
  });

  it('each operation has description and format function', () => {
    for (const [name, op] of Object.entries(PREVIEWABLE_OPERATIONS)) {
      assert.ok(typeof op.description === 'string', `${name} missing description`);
      assert.ok(typeof op.format === 'function', `${name} missing format`);
    }
  });

  it('create_order format shows item count and total', () => {
    const formatted = PREVIEWABLE_OPERATIONS.create_order.format({
      customerId: 'cust-1',
      items: [
        { quantity: 2, unitPrice: 10 },
        { quantity: 1, unitPrice: 25 },
      ],
      currency: 'USD',
    });
    assert.ok(formatted.includes('cust-1'));
    assert.ok(formatted.includes('2 items'));
    assert.ok(formatted.includes('45.00'));
  });

  it('ship_order format includes tracking number when present', () => {
    const withTracking = PREVIEWABLE_OPERATIONS.ship_order.format({
      orderId: 'ORD-1',
      trackingNumber: 'TRACK123',
    });
    assert.ok(withTracking.includes('TRACK123'));

    const withoutTracking = PREVIEWABLE_OPERATIONS.ship_order.format({
      orderId: 'ORD-1',
    });
    assert.ok(!withoutTracking.includes('tracking'));
  });

  it('adjust_inventory format shows sign', () => {
    const positive = PREVIEWABLE_OPERATIONS.adjust_inventory.format({
      sku: 'SKU-1',
      quantity: 10,
      reason: 'restock',
    });
    assert.ok(positive.includes('+10'));

    const negative = PREVIEWABLE_OPERATIONS.adjust_inventory.format({
      sku: 'SKU-1',
      quantity: -5,
      reason: 'damaged',
    });
    assert.ok(negative.includes('-5'));
  });
});

// ===========================================================================
// DryRunManager
// ===========================================================================

describe('DryRunManager', () => {
  it('defaults to disabled', () => {
    const mgr = new DryRunManager();
    assert.strictEqual(mgr.isEnabled(), false);
  });

  it('can be created enabled', () => {
    const mgr = new DryRunManager({ enabled: true });
    assert.strictEqual(mgr.isEnabled(), true);
  });

  it('setEnabled toggles mode', () => {
    const mgr = new DryRunManager();
    mgr.setEnabled(true);
    assert.strictEqual(mgr.isEnabled(), true);
    mgr.setEnabled(false);
    assert.strictEqual(mgr.isEnabled(), false);
  });

  it('preview records operation and returns preview', () => {
    const mgr = new DryRunManager({ enabled: true });
    const preview = mgr.preview('create_customer', {
      email: 'alice@example.com',
      firstName: 'Alice',
      lastName: 'Smith',
    });

    assert.strictEqual(preview.operation, 'create_customer');
    assert.ok(preview.formatted.includes('alice@example.com'));
    assert.ok(preview.timestamp);
  });

  it('preview works for unknown operations with fallback format', () => {
    const mgr = new DryRunManager({ enabled: true });
    const preview = mgr.preview('unknown_op', { foo: 'bar' });
    assert.strictEqual(preview.description, 'Execute unknown_op');
    assert.ok(preview.formatted.includes('unknown_op'));
  });

  it('getOperations returns copy of recorded operations', () => {
    const mgr = new DryRunManager({ enabled: true });
    mgr.preview('create_cart', { customerEmail: 'bob@test.org' });
    mgr.preview('add_cart_item', { cartId: 'CART-1', sku: 'SKU-1', quantity: 2 });

    const ops = mgr.getOperations();
    assert.strictEqual(ops.length, 2);
    // Verify it's a copy
    ops.push({ fake: true });
    assert.strictEqual(mgr.getOperations().length, 2);
  });

  it('clear empties recorded operations', () => {
    const mgr = new DryRunManager({ enabled: true });
    mgr.preview('cancel_order', { orderId: 'ORD-1' });
    assert.strictEqual(mgr.getOperations().length, 1);
    mgr.clear();
    assert.strictEqual(mgr.getOperations().length, 0);
  });

  it('getSummary groups by operation type', () => {
    const mgr = new DryRunManager({ enabled: true });
    mgr.preview('create_cart', {});
    mgr.preview('add_cart_item', { cartId: 'C1', sku: 'A' });
    mgr.preview('add_cart_item', { cartId: 'C1', sku: 'B' });

    const summary = mgr.getSummary();
    assert.strictEqual(summary.total, 3);
    assert.strictEqual(summary.byType.create_cart, 1);
    assert.strictEqual(summary.byType.add_cart_item, 2);
  });

  it('getSummary returns zero for empty manager', () => {
    const mgr = new DryRunManager();
    const summary = mgr.getSummary();
    assert.strictEqual(summary.total, 0);
    assert.deepStrictEqual(summary.byType, {});
  });

  it('formatOperations returns "no operations" when empty', () => {
    const mgr = new DryRunManager();
    assert.ok(mgr.formatOperations().includes('No operations'));
  });

  it('formatOperations lists numbered operations', () => {
    const mgr = new DryRunManager({ enabled: true });
    mgr.preview('cancel_order', { orderId: 'ORD-1' });
    mgr.preview('create_return', { orderId: 'ORD-1', reason: 'defective' });

    const output = mgr.formatOperations();
    assert.ok(output.includes('1.'));
    assert.ok(output.includes('2.'));
    assert.ok(output.includes('Total: 2'));
  });

  it('formatOperations verbose mode shows params', () => {
    const mgr = new DryRunManager({ enabled: true });
    mgr.preview('cancel_order', { orderId: 'ORD-1' });

    const verbose = mgr.formatOperations({ verbose: true });
    assert.ok(verbose.includes('ORD-1'));
  });

  it('onPreview callback is called', () => {
    let called = false;
    const mgr = new DryRunManager({
      enabled: true,
      onPreview: () => {
        called = true;
      },
    });
    mgr.preview('cancel_cart', { cartId: 'C1' });
    assert.strictEqual(called, true);
  });

  it('isWriteOperation returns true for known operations', () => {
    const mgr = new DryRunManager();
    assert.strictEqual(mgr.isWriteOperation('create_order'), true);
    assert.strictEqual(mgr.isWriteOperation('ship_order'), true);
    assert.strictEqual(mgr.isWriteOperation('complete_checkout'), true);
  });

  it('isWriteOperation returns false for unknown operations', () => {
    const mgr = new DryRunManager();
    assert.strictEqual(mgr.isWriteOperation('list_orders'), false);
    assert.strictEqual(mgr.isWriteOperation('get_customer'), false);
  });

  describe('wrapTool', () => {
    it('executes handler when dry-run is disabled', async () => {
      const mgr = new DryRunManager({ enabled: false });
      const handler = async () => ({ content: [{ type: 'text', text: 'real result' }] });
      const wrapped = mgr.wrapTool('create_order', handler);

      const result = await wrapped({ customerId: 'cust-1' });
      assert.ok(result.content[0].text.includes('real result'));
    });

    it('returns dry-run preview for write operations when enabled', async () => {
      const mgr = new DryRunManager({ enabled: true });
      const handler = async () => ({ content: [{ type: 'text', text: 'real result' }] });
      const wrapped = mgr.wrapTool('create_order', handler);

      const result = await wrapped({ customerId: 'cust-1' });
      const parsed = JSON.parse(result.content[0].text);
      assert.strictEqual(parsed.dryRun, true);
      assert.strictEqual(parsed.wouldExecute, 'create_order');
    });

    it('executes handler for read operations even when enabled', async () => {
      const mgr = new DryRunManager({ enabled: true });
      const handler = async () => ({ content: [{ type: 'text', text: 'data' }] });
      const wrapped = mgr.wrapTool('list_orders', handler);

      const result = await wrapped({});
      assert.ok(result.content[0].text.includes('data'));
    });
  });
});

// ===========================================================================
// createDryRunManager
// ===========================================================================

describe('createDryRunManager', () => {
  it('returns a DryRunManager instance', () => {
    const mgr = createDryRunManager();
    assert.ok(mgr instanceof DryRunManager);
  });

  it('passes options through', () => {
    const mgr = createDryRunManager({ enabled: true });
    assert.strictEqual(mgr.isEnabled(), true);
  });
});

// ===========================================================================
// formatDryRunResult
// ===========================================================================

describe('formatDryRunResult', () => {
  it('formats a result with operation name', () => {
    const result = {
      operation: 'create_order',
      formatted: 'Create order for customer cust-1',
      params: { customerId: 'cust-1' },
    };
    const output = formatDryRunResult(result);
    assert.ok(output.includes('[DRY-RUN]'));
    assert.ok(output.includes('create_order'));
    assert.ok(output.includes('Create order'));
  });

  it('includes params when showParams is true', () => {
    const result = {
      operation: 'cancel_order',
      formatted: 'Cancel order ORD-1',
      params: { orderId: 'ORD-1' },
    };
    const output = formatDryRunResult(result, { showParams: true });
    assert.ok(output.includes('ORD-1'));
    assert.ok(output.includes('Parameters'));
  });

  it('omits color codes when color is false', () => {
    const result = {
      operation: 'cancel_order',
      formatted: 'Cancel order ORD-1',
      params: {},
    };
    const output = formatDryRunResult(result, { color: false });
    assert.ok(!output.includes('\x1b['));
  });
});

// ===========================================================================
// parseDryRunFlag
// ===========================================================================

describe('parseDryRunFlag', () => {
  it('detects --dry-run flag', () => {
    const { enabled, args } = parseDryRunFlag(['--apply', '--dry-run', 'list orders']);
    assert.strictEqual(enabled, true);
    assert.ok(!args.includes('--dry-run'));
    assert.ok(args.includes('--apply'));
    assert.ok(args.includes('list orders'));
  });

  it('detects -n shorthand', () => {
    const { enabled, args } = parseDryRunFlag(['-n', 'create order']);
    assert.strictEqual(enabled, true);
    assert.ok(!args.includes('-n'));
    assert.ok(args.includes('create order'));
  });

  it('returns enabled false when flag not present', () => {
    const { enabled, args } = parseDryRunFlag(['--apply', 'list orders']);
    assert.strictEqual(enabled, false);
    assert.strictEqual(args.length, 2);
  });

  it('handles empty args array', () => {
    const { enabled, args } = parseDryRunFlag([]);
    assert.strictEqual(enabled, false);
    assert.strictEqual(args.length, 0);
  });
});
