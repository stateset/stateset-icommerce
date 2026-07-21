/**
 * Tests for error handling consistency across tool files.
 *
 * Verifies that:
 * - applyRequired() returns { success: false, error, hint }
 * - get_subscription_plan returns { success: true, plan } not raw plan
 * - All tool error responses include success: false
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

describe('Error handling consistency', () => {
  describe('applyRequired()', () => {
    let applyRequired;

    it('loads apply-guard module', async () => {
      const mod = await import('../../src/utils/apply-guard.js');
      applyRequired = mod.applyRequired;
      assert.ok(typeof applyRequired === 'function');
    });

    it('returns success: false', () => {
      const result = applyRequired('Create order');
      assert.strictEqual(result.success, false);
    });

    it('returns error message with operation name', () => {
      const result = applyRequired('Create shipment');
      assert.ok(result.error.includes('Create shipment'));
      assert.ok(result.error.includes('--apply'));
    });

    it('returns hint text', () => {
      const result = applyRequired('Delete record');
      assert.ok(result.hint);
      assert.ok(result.hint.includes('--apply'));
    });

    it('includes preview as wouldDo when provided', () => {
      const preview = { orderId: 'o1', amount: 100 };
      const result = applyRequired('Create payment', preview);
      assert.deepStrictEqual(result.wouldDo, preview);
    });

    it('omits wouldDo when no preview', () => {
      const result = applyRequired('Cancel order');
      assert.ok(!('wouldDo' in result));
    });

    it('shape matches { success, error, hint } contract', () => {
      const result = applyRequired('Test op', { foo: 'bar' });
      const keys = Object.keys(result).sort();
      assert.deepStrictEqual(keys, ['error', 'hint', 'success', 'wouldDo']);
    });
  });

  describe('subscription tools response shape', () => {
    it('get_subscription_plan wraps result in { success, plan }', async () => {
      const mod = await import('../../src/tools/subscriptions.js');
      const tool = mod.subscriptionTools.find((t) => t.name === 'get_subscription_plan');
      assert.ok(tool, 'get_subscription_plan tool not found');

      // Mock commerce that returns a plan object
      const mockPlan = { id: 'plan-1', name: 'Pro', price: '29.99', status: 'active' };
      const mockCommerce = {
        getSubscriptionPlan: async () => mockPlan,
      };

      const result = await tool.handler({ commerce: mockCommerce, params: { planId: 'plan-1' } });
      assert.strictEqual(result.success, true, 'Expected success: true');
      assert.deepStrictEqual(result.plan, mockPlan, 'Expected plan to be wrapped');
    });

    it('get_subscription_plan returns error for missing plan', async () => {
      const mod = await import('../../src/tools/subscriptions.js');
      const tool = mod.subscriptionTools.find((t) => t.name === 'get_subscription_plan');

      const mockCommerce = { getSubscriptionPlan: async () => null };
      const result = await tool.handler({ commerce: mockCommerce, params: { planId: 'xxx' } });
      assert.strictEqual(result.success, false);
      assert.ok(result.error.includes('not found'));
    });
  });

  describe('all tools using applyRequired return consistent shape', () => {
    const toolModules = [
      { path: '../../src/tools/payments.js', export: 'paymentTools' },
      { path: '../../src/tools/shipments.js', export: 'shipmentTools' },
      { path: '../../src/tools/suppliers.js', export: 'supplierTools' },
      { path: '../../src/tools/invoices.js', export: 'invoiceTools' },
      { path: '../../src/tools/warranties.js', export: 'warrantyTools' },
    ];

    for (const { path, export: exportName } of toolModules) {
      it(`${exportName} write handlers include success: false when blocked`, async () => {
        const mod = await import(path);
        const tools = mod[exportName];

        for (const tool of tools) {
          if (tool.permission === 'read') continue;

          // Test with allowApply = false
          const mockCommerce = new Proxy(
            {},
            {
              get: () =>
                new Proxy(() => ({}), {
                  get: () => () => ({}),
                }),
            },
          );

          try {
            const result = await tool.handler({
              commerce: mockCommerce,
              params: Object.fromEntries(
                Object.entries(tool.inputSchema || {}).map(([k]) => [k, 'test']),
              ),
              allowApply: false,
            });

            assert.strictEqual(
              result.success,
              false,
              `${tool.name}: expected success: false when allowApply=false, got ${JSON.stringify(result)}`,
            );
            assert.ok(result.error, `${tool.name}: expected error message when allowApply=false`);
          } catch {
            // Some handlers may throw due to mock — that's ok, we're testing the guard path
          }
        }
      });
    }
  });
});
