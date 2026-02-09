/**
 * Unit tests for mcp-tool-composer.js — ToolComposer, ORCHESTRATION_TEMPLATES
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ToolComposer, ORCHESTRATION_TEMPLATES } from '../../src/mcp-tool-composer.js';

// ===========================================================================
// orchestrate — success path
// ===========================================================================

describe('ToolComposer.orchestrate — success', () => {
  let composer;
  beforeEach(() => {
    composer = new ToolComposer(null);
  });

  it('returns success=true with results for multi-step', async () => {
    const result = await composer.orchestrate('test-flow', [
      { tool: 'step_a', params: { a: 1 } },
      { tool: 'step_b', params: { b: 2 } },
    ]);
    assert.equal(result.success, true);
    assert.equal(result.name, 'test-flow');
    assert.equal(result.results.length, 2);
    assert.ok(result.orchestrationId.startsWith('orch-'));
    assert.ok(result.completedAt);
  });

  it('passes validation on each step', async () => {
    const result = await composer.orchestrate('validated', [
      {
        tool: 'step_a',
        params: {},
        validate: () => ({ valid: true }),
      },
      {
        tool: 'step_b',
        params: {},
        validate: () => ({ valid: true }),
      },
    ]);
    assert.equal(result.success, true);
    assert.equal(result.results.length, 2);
  });

  it('each result includes step index and tool name', async () => {
    const result = await composer.orchestrate('indexed', [
      { tool: 'alpha', params: {} },
      { tool: 'beta', params: {} },
    ]);
    assert.equal(result.results[0].step, 0);
    assert.equal(result.results[0].tool, 'alpha');
    assert.equal(result.results[1].step, 1);
    assert.equal(result.results[1].tool, 'beta');
  });
});

// ===========================================================================
// orchestrate — validation failure + rollback
// ===========================================================================

describe('ToolComposer.orchestrate — failure', () => {
  let composer;
  beforeEach(() => {
    composer = new ToolComposer(null);
  });

  it('validation failure triggers rollback and returns success=false', async () => {
    let rolledBack = false;
    const result = await composer.orchestrate('fail-flow', [
      {
        tool: 'step_a',
        params: {},
        validate: () => ({ valid: true }),
        rollback: async () => {
          rolledBack = true;
          return 'rolled-back-a';
        },
      },
      {
        tool: 'step_b',
        params: {},
        validate: () => ({ valid: false, error: 'bad data' }),
      },
    ]);
    assert.equal(result.success, false);
    assert.ok(result.error.includes('Validation failed'));
    assert.ok(result.error.includes('bad data'));
    assert.equal(rolledBack, true);
    assert.equal(result.rollbacks.length, 1);
    assert.equal(result.rollbacks[0].status, 'success');
  });

  it('rollback failure is captured and does not throw', async () => {
    const result = await composer.orchestrate('rollback-fail', [
      {
        tool: 'step_a',
        params: {},
        rollback: async () => {
          throw new Error('rollback boom');
        },
      },
      {
        tool: 'step_b',
        params: {},
        validate: () => ({ valid: false, error: 'oops' }),
      },
    ]);
    assert.equal(result.success, false);
    assert.equal(result.rollbacks.length, 1);
    assert.equal(result.rollbacks[0].status, 'failed');
    assert.ok(result.rollbacks[0].error.includes('rollback boom'));
  });

  it('returns progress and totalSteps on failure', async () => {
    const result = await composer.orchestrate('partial', [
      { tool: 'a', params: {} },
      { tool: 'b', params: {}, validate: () => ({ valid: false, error: 'fail' }) },
      { tool: 'c', params: {} },
    ]);
    assert.equal(result.success, false);
    // step a completes fully, step b executes then validation fails
    // both are pushed to results before the error is thrown
    assert.equal(result.progress, 2);
    assert.equal(result.totalSteps, 3);
  });
});

// ===========================================================================
// orchestrate — rollbacks in reverse order
// ===========================================================================

describe('ToolComposer.orchestrate — rollback order', () => {
  it('rollbacks execute in reverse order', async () => {
    const composer = new ToolComposer(null);
    const order = [];

    const result = await composer.orchestrate('reverse', [
      {
        tool: 'step_0',
        params: {},
        rollback: async () => {
          order.push(0);
        },
      },
      {
        tool: 'step_1',
        params: {},
        rollback: async () => {
          order.push(1);
        },
      },
      {
        tool: 'step_2',
        params: {},
        validate: () => ({ valid: false, error: 'fail at 2' }),
      },
    ]);

    assert.equal(result.success, false);
    // Rollbacks should be in reverse: step_1 first, then step_0
    assert.deepEqual(order, [1, 0]);
  });
});

// ===========================================================================
// orchestrate — events
// ===========================================================================

describe('ToolComposer.orchestrate — events', () => {
  it('emits orchestration:started, step:started, step:completed, orchestration:completed on success', async () => {
    const composer = new ToolComposer(null);
    const events = [];

    composer.on('orchestration:started', (e) => events.push({ type: 'orch:start', ...e }));
    composer.on('step:started', (e) => events.push({ type: 'step:start', step: e.step }));
    composer.on('step:completed', (e) => events.push({ type: 'step:done', step: e.step }));
    composer.on('orchestration:completed', (e) => events.push({ type: 'orch:done' }));

    await composer.orchestrate('events-test', [
      { tool: 'a', params: {} },
      { tool: 'b', params: {} },
    ]);

    assert.ok(events.some((e) => e.type === 'orch:start'));
    assert.ok(events.some((e) => e.type === 'step:start' && e.step === 1));
    assert.ok(events.some((e) => e.type === 'step:start' && e.step === 2));
    assert.ok(events.some((e) => e.type === 'step:done' && e.step === 1));
    assert.ok(events.some((e) => e.type === 'step:done' && e.step === 2));
    assert.ok(events.some((e) => e.type === 'orch:done'));
  });

  it('emits orchestration:failed and rollback events on failure', async () => {
    const composer = new ToolComposer(null);
    const events = [];

    composer.on('orchestration:failed', (e) => events.push({ type: 'orch:fail' }));
    composer.on('rollback:success', (e) => events.push({ type: 'rb:ok', step: e.step }));

    await composer.orchestrate('fail-events', [
      {
        tool: 'a',
        params: {},
        rollback: async () => 'ok',
      },
      {
        tool: 'b',
        params: {},
        validate: () => ({ valid: false, error: 'nope' }),
      },
    ]);

    assert.ok(events.some((e) => e.type === 'orch:fail'));
    assert.ok(events.some((e) => e.type === 'rb:ok' && e.step === 0));
  });

  it('emits rollback:failed when rollback throws', async () => {
    const composer = new ToolComposer(null);
    const events = [];

    composer.on('rollback:failed', (e) => events.push({ type: 'rb:fail', step: e.step }));

    await composer.orchestrate('rb-fail-events', [
      {
        tool: 'a',
        params: {},
        rollback: async () => {
          throw new Error('boom');
        },
      },
      {
        tool: 'b',
        params: {},
        validate: () => ({ valid: false, error: 'nope' }),
      },
    ]);

    assert.ok(events.some((e) => e.type === 'rb:fail' && e.step === 0));
  });
});

// ===========================================================================
// executeTool — default implementation
// ===========================================================================

describe('ToolComposer.executeTool', () => {
  it('returns tool, params, and executedAt', async () => {
    const composer = new ToolComposer(null);
    const result = await composer.executeTool('create_order', { id: 1 });
    assert.equal(result.tool, 'create_order');
    assert.deepEqual(result.params, { id: 1 });
    assert.ok(result.executedAt);
  });
});

// ===========================================================================
// getStatus / getActiveOrchestrations
// ===========================================================================

describe('ToolComposer.getStatus / getActiveOrchestrations', () => {
  let composer;
  beforeEach(() => {
    composer = new ToolComposer(null);
  });

  it('getStatus returns undefined for nonexistent ID', () => {
    assert.equal(composer.getStatus('nonexistent'), undefined);
  });

  it('getActiveOrchestrations returns empty array initially', () => {
    const active = composer.getActiveOrchestrations();
    assert.ok(Array.isArray(active));
    assert.equal(active.length, 0);
  });
});

// ===========================================================================
// cancel
// ===========================================================================

describe('ToolComposer.cancel', () => {
  let composer;
  beforeEach(() => {
    composer = new ToolComposer(null);
  });

  it('throws for nonexistent orchestration', async () => {
    await assert.rejects(
      () => composer.cancel('nonexistent'),
      (err) => {
        assert.ok(err.message.includes('not found'));
        return true;
      },
    );
  });

  it('throws for non-running orchestration', async () => {
    // Manually set a completed orchestration
    composer.orchestrations.set('orch-done', { status: 'completed' });
    await assert.rejects(
      () => composer.cancel('orch-done'),
      (err) => {
        assert.ok(err.message.includes('not running'));
        return true;
      },
    );
  });

  it('cancels a running orchestration', async () => {
    composer.orchestrations.set('orch-run', { status: 'running' });
    const events = [];
    composer.on('orchestration:cancelled', (e) => events.push(e));

    const result = await composer.cancel('orch-run');
    assert.equal(result.status, 'cancelled');
    assert.ok(events.some((e) => e.orchestrationId === 'orch-run'));
  });
});

// ===========================================================================
// ORCHESTRATION_TEMPLATES
// ===========================================================================

describe('ORCHESTRATION_TEMPLATES', () => {
  it('has checkout template', () => {
    assert.ok('checkout' in ORCHESTRATION_TEMPLATES);
    assert.ok(ORCHESTRATION_TEMPLATES.checkout.name);
    assert.ok(Array.isArray(ORCHESTRATION_TEMPLATES.checkout.steps));
    assert.ok(ORCHESTRATION_TEMPLATES.checkout.steps.length > 0);
  });

  it('checkout template includes expected tools', () => {
    const tools = ORCHESTRATION_TEMPLATES.checkout.steps.map((s) => s.tool);
    assert.ok(tools.includes('get_cart'));
    assert.ok(tools.includes('create_order'));
    assert.ok(tools.includes('process_payment'));
  });

  it('has return template', () => {
    assert.ok('return' in ORCHESTRATION_TEMPLATES);
    assert.ok(ORCHESTRATION_TEMPLATES.return.name);
    assert.ok(Array.isArray(ORCHESTRATION_TEMPLATES.return.steps));
    const tools = ORCHESTRATION_TEMPLATES.return.steps.map((s) => s.tool);
    assert.ok(tools.includes('approve_return'));
    assert.ok(tools.includes('refund_payment'));
  });

  it('has fulfillment template', () => {
    assert.ok('fulfillment' in ORCHESTRATION_TEMPLATES);
    assert.ok(ORCHESTRATION_TEMPLATES.fulfillment.name);
    assert.ok(Array.isArray(ORCHESTRATION_TEMPLATES.fulfillment.steps));
    const tools = ORCHESTRATION_TEMPLATES.fulfillment.steps.map((s) => s.tool);
    assert.ok(tools.includes('get_order'));
    assert.ok(tools.includes('ship_order'));
  });
});

// ===========================================================================
// createOrderWithReservation — delegates to orchestrate
// ===========================================================================

describe('ToolComposer.createOrderWithReservation', () => {
  it('delegates to orchestrate with correct name', async () => {
    const composer = new ToolComposer(null);
    const result = await composer.createOrderWithReservation({
      items: [{ sku: 'WIDGET-001', quantity: 5 }],
      customerId: 'cust-1',
    });
    assert.ok(result.orchestrationId);
    assert.equal(result.name, 'create-order-with-reservation');
    // Default executeTool returns { tool, params, executedAt } which lacks reservation.id
    // so validation will fail — that's expected with the stub
    // The key test is that it delegates correctly
    assert.ok('success' in result);
  });
});

// ===========================================================================
// processReturnWithRestock — delegates to orchestrate
// ===========================================================================

describe('ToolComposer.processReturnWithRestock', () => {
  it('delegates to orchestrate with correct name', async () => {
    const composer = new ToolComposer(null);
    const result = await composer.processReturnWithRestock({
      returnId: 'ret-1',
      sku: 'WIDGET-001',
      quantity: 3,
      orderId: 'ord-1',
      amount: 29.99,
    });
    assert.ok(result.orchestrationId);
    assert.equal(result.name, 'process-return-with-restock');
    assert.ok('success' in result);
  });
});

// ===========================================================================
// completeCheckout — delegates to orchestrate
// ===========================================================================

describe('ToolComposer.completeCheckout', () => {
  it('delegates to orchestrate with correct name', async () => {
    const composer = new ToolComposer(null);
    const result = await composer.completeCheckout({
      cartId: 'cart-1',
      paymentMethod: 'credit_card',
    });
    assert.ok(result.orchestrationId);
    assert.equal(result.name, 'complete-checkout');
    assert.ok('success' in result);
  });
});
