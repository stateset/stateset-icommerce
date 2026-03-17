/**
 * Tests that verify the MCP server wiring — proving intelligence services
 * are actually initialized and connected to the commerce flows at runtime.
 *
 * These tests import the actual integration module and verify the wiring
 * pattern used in mcp-server.js works correctly.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createIntegratedA2AService, initializeServices } from '../../src/a2a/integration.js';

function createMockCoreA2A() {
  const payments = [];
  const quotes = [];
  return {
    pay: async (params) => {
      const p = { id: `pay-${Date.now()}`, ...params, status: 'completed' };
      payments.push(p);
      return { success: true, payment: p };
    },
    acceptQuote: async (quoteId) => {
      return { success: true, quoteId, payment: { id: 'pay-from-quote' } };
    },
    requestQuote: async (params) => {
      const q = { id: `q-${Date.now()}`, ...params, status: 'requested' };
      quotes.push(q);
      return { success: true, quote: q };
    },
    listPayments: () => payments,
    listQuotes: () => quotes,
    getPayment: (id) => payments.find((p) => p.id === id),
    getQuote: (id) => quotes.find((q) => q.id === id),
    _payments: payments,
    _quotes: quotes,
  };
}

describe('MCP Server Wiring — Integration Layer Active', () => {
  it('initializeServices creates all 7 services', () => {
    const services = initializeServices();
    assert.ok(services.memory);
    assert.ok(services.rules);
    assert.ok(services.idempotency);
    assert.ok(services.tracing);
    assert.ok(services.costAnalytics);
    assert.ok(services.introspection);
    assert.ok(services.scheduler);
  });

  it('integrated a2a wraps pay() with idempotency', async () => {
    const core = createMockCoreA2A();
    const services = initializeServices();
    const integrated = createIntegratedA2AService(core, services);

    // First call
    const r1 = await integrated.pay({ to: '0xSeller', amount: 10, idempotencyKey: 'idem-1' });
    assert.equal(r1.success, true);

    // Same idempotency key → cached result, no duplicate
    const r2 = await integrated.pay({ to: '0xSeller', amount: 10, idempotencyKey: 'idem-1' });
    assert.equal(r2.success, true);

    // Only 1 actual payment created
    assert.equal(core._payments.length, 1);
  });

  it('integrated a2a records to cost analytics on pay()', async () => {
    const core = createMockCoreA2A();
    const services = initializeServices();
    const integrated = createIntegratedA2AService(core, services);

    await integrated.pay({ to: '0xSeller', amount: 50, asset: 'USDC' });

    const summary = services.costAnalytics.getAgentSpendSummary('unknown');
    assert.ok(summary.totalSpent >= 50);
  });

  it('integrated a2a records to memory on pay()', async () => {
    const core = createMockCoreA2A();
    const services = initializeServices();
    const integrated = createIntegratedA2AService(core, services);

    await integrated.pay({ to: '0xSeller', amount: 25 });

    const profile = services.memory.getCounterpartyProfile('unknown', '0xSeller');
    assert.ok(profile);
    assert.ok(profile.totalInteractions >= 1);
  });

  it('integrated a2a creates tracing span on pay()', async () => {
    const core = createMockCoreA2A();
    const services = initializeServices();
    const integrated = createIntegratedA2AService(core, services);

    await integrated.pay({ to: '0xSeller', amount: 75 });

    const metrics = services.tracing.getMetrics();
    assert.ok(metrics.spanCount >= 1);
  });

  it('integrated a2a evaluates rules on pay()', async () => {
    const core = createMockCoreA2A();
    const services = initializeServices();
    const integrated = createIntegratedA2AService(core, services);

    // Add a blocking rule
    services.rules.addRule({
      name: 'block_large',
      agentAddress: 'unknown',
      condition: { field: 'amount', operator: 'gt', value: 500 },
      action: { type: 'block' },
      priority: 100,
      enabled: true,
    });

    // Small payment should pass
    const r1 = await integrated.pay({ to: '0xSeller', amount: 100 });
    assert.equal(r1.success, true);

    // Large payment should be blocked by rules
    await assert.rejects(
      () => integrated.pay({ to: '0xSeller', amount: 1000 }),
      (err) => err.message.includes('blocked') || err.message.includes('rule') || err.message.includes('Rule'),
    );
  });

  it('integrated a2a records introspection decisions', async () => {
    const core = createMockCoreA2A();
    const services = initializeServices();
    const integrated = createIntegratedA2AService(core, services);

    await integrated.pay({ to: '0xSeller', amount: 30 });

    const decisions = services.introspection.getDecisionHistory('unknown', 10);
    assert.ok(decisions.length >= 1);
    assert.equal(decisions[0].type, 'payment');
  });

  it('passthrough methods still work', async () => {
    const core = createMockCoreA2A();
    const services = initializeServices();
    const integrated = createIntegratedA2AService(core, services);

    await integrated.pay({ to: '0xA', amount: 5 });
    const payments = integrated.listPayments();
    assert.ok(Array.isArray(payments));
    assert.ok(payments.length >= 1);
  });

  it('simulates full MCP server wiring pattern', async () => {
    // This test reproduces the exact pattern used in mcp-server.js:
    // 1. Create core A2A
    // 2. Initialize services
    // 3. Wrap with integration layer
    // 4. Replace a2a() accessor

    const coreA2A = createMockCoreA2A();
    const services = initializeServices();
    const integratedA2A = createIntegratedA2AService(coreA2A, services);

    // Simulate the commerce wrapper
    const commerce = {
      a2a: () => integratedA2A,
      _agentMemory: services.memory,
      _rulesEngine: services.rules,
      _tracingService: services.tracing,
      _costAnalytics: services.costAnalytics,
      _introspectionService: services.introspection,
    };

    // Agent calls pay through commerce.a2a()
    const result = await commerce.a2a().pay({ to: '0xRecipient', amount: 100 });
    assert.equal(result.success, true);

    // Verify all intelligence layers were invoked
    assert.ok(services.tracing.getMetrics().spanCount >= 1, 'tracing span created');
    assert.ok(
      services.costAnalytics.getAgentSpendSummary('unknown').totalSpent >= 100,
      'cost recorded',
    );
    assert.ok(
      services.memory.getCounterpartyProfile('unknown', '0xRecipient').totalInteractions >= 1,
      'memory recorded',
    );
    assert.ok(
      services.introspection.getDecisionHistory('unknown', 1).length >= 1,
      'decision logged',
    );
  });
});
