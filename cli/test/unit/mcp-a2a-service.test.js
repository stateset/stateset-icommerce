// Unit tests for cli/src/mcp/a2a-service.js
//
// Covers:
//  - `createA2AServiceBinding`: the accessor proxies every call to the
//    store, and `setFactory` late-binds a replacement service.
//  - `initializeIntelligenceServices`: loads the real a2a intelligence
//    modules, attaches them to the commerce wrapper, and swaps the A2A
//    factory for the integrated service.

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  createA2AServiceBinding,
  initializeIntelligenceServices,
} from '../../src/mcp/a2a-service.js';

function makeStore() {
  const calls = [];
  const handler = {
    get(_target, prop) {
      if (prop === 'then') return undefined;
      return (...args) => {
        calls.push({ method: prop, args });
        return { method: prop, args };
      };
    },
  };
  return { store: new Proxy({}, handler), calls };
}

describe('createA2AServiceBinding', () => {
  it('proxies A2A calls straight to the store', () => {
    const { store, calls } = makeStore();
    const binding = createA2AServiceBinding(store);
    const service = binding.a2a();

    service.createPayment({ amount: 5 });
    service.getEscrow('esc_1');
    service.updateWorkflowStep('ws_1', { status: 'done' });
    service.getPendingNotifications(3, 10);

    assert.deepEqual(calls, [
      { method: 'createPayment', args: [{ amount: 5 }] },
      { method: 'getEscrow', args: ['esc_1'] },
      { method: 'updateWorkflowStep', args: ['ws_1', { status: 'done' }] },
      { method: 'getPendingNotifications', args: [3, 10] },
    ]);
  });

  it('exposes the full A2A surface (payments, escrow, disputes, RFQ, SLA, workflows)', () => {
    const { store } = makeStore();
    const service = createA2AServiceBinding(store).a2a();
    for (const method of [
      'createPayment',
      'listPaymentRequests',
      'createQuote',
      'listEscrows',
      'createDispute',
      'listEvidenceByDispute',
      'getReputationScore',
      'listServices',
      'upsertWebhookConfig',
      'getDueSubscriptions',
      'listSplitRecipients',
      'createEventSubscription',
      'listEventLog',
      'createRFQ',
      'listRFQResponses',
      'createSLADefinition',
      'listSLAViolations',
      'createWorkflow',
      'listWorkflowSteps',
    ]) {
      assert.equal(typeof service[method], 'function', `${method} should be exposed`);
    }
  });

  it('setFactory late-binds a replacement service without re-registering the accessor', () => {
    const { store } = makeStore();
    const binding = createA2AServiceBinding(store);
    const accessor = binding.a2a;
    const original = binding.getFactory();

    const integrated = { integrated: true };
    binding.setFactory(() => integrated);

    assert.equal(accessor(), integrated);
    assert.notEqual(binding.getFactory(), original);
  });
});

describe('initializeIntelligenceServices', () => {
  it('attaches intelligence services to the commerce wrapper and swaps the factory', async () => {
    const { store } = makeStore();
    const binding = createA2AServiceBinding(store);
    const commerceWithA2A = { a2a: binding.a2a };
    let swapped = null;

    await initializeIntelligenceServices({
      commerceWithA2A,
      a2aStore: store,
      setA2AServiceFactory: (factory) => {
        swapped = factory;
        binding.setFactory(factory);
      },
    });

    assert.equal(commerceWithA2A._store, store);
    for (const key of [
      '_agentMemory',
      '_rulesEngine',
      '_idempotencyGuard',
      '_tracingService',
      '_costAnalytics',
      '_introspectionService',
      '_schedulerService',
      '_messagingService',
      '_rateLimiter',
    ]) {
      assert.ok(commerceWithA2A[key], `${key} should be attached`);
    }
    assert.equal(typeof swapped, 'function');
    // The accessor now returns the integrated service, which still exposes
    // the core methods.
    const integrated = binding.a2a();
    assert.equal(integrated, swapped());
    assert.equal(typeof integrated.createPayment, 'function');
  });

  it('returns a promise (so callers may await readiness)', () => {
    const { store } = makeStore();
    const result = initializeIntelligenceServices({
      commerceWithA2A: { a2a: () => ({}) },
      a2aStore: store,
      setA2AServiceFactory: () => {},
    });
    assert.equal(typeof result.then, 'function');
    return result;
  });
});
