/**
 * Payment Tools Test Suite
 *
 * Tests for cli/src/tools/payments.js
 * Covers:
 *   legacy APIs (list_payments, get_payment, create_payment, complete_payment, create_refund)
 *   provider APIs (list_payment_providers, create/get/capture/cancel/refund_payment_intent)
 */

import { beforeEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { paymentTools } from '../../src/tools/payments.js';
import { __resetPaymentProviderState } from '../../src/tools/providers/payments.js';

// ============================================================================
// Helper: find tool by name
// ============================================================================

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock factory
// ============================================================================

const mockPayment = {
  id: 'pay_001',
  orderId: 'ord_001',
  amount: '99.99',
  currency: 'USD',
  method: 'credit_card',
  status: 'pending',
  createdAt: '2026-02-21T00:00:00Z',
};

const mockRefund = {
  id: 'ref_001',
  paymentId: 'pay_001',
  amount: '25.00',
  reason: 'defective item',
  status: 'processed',
  createdAt: '2026-02-21T00:00:00Z',
};

function makePaymentCommerce(overrides = {}) {
  return {
    payments: {
      list: async () => [mockPayment],
      count: async () => 1,
      get: async (_id) => mockPayment,
      create: async (data) => ({ ...mockPayment, ...data }),
      markCompleted: async (_id) => ({ ...mockPayment, status: 'completed' }),
      createRefund: async (data) => ({ ...mockRefund, ...data }),
      ...overrides,
    },
  };
}

beforeEach(() => {
  __resetPaymentProviderState();
});

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Payment Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(paymentTools));
  });

  it('has at least 11 tools', () => {
    assert.ok(paymentTools.length >= 11, `Expected >= 11, got ${paymentTools.length}`);
  });

  it('every tool has name, handler, and permission', () => {
    for (const tool of paymentTools) {
      assert.ok(tool.name, 'tool missing name');
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });
});

// ============================================================================
// list_payments
// ============================================================================

describe('list_payments', () => {
  const tool = findTool(paymentTools, 'list_payments');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns list and count', async () => {
    const result = await tool.handler({
      commerce: makePaymentCommerce(),
      params: {},
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(result.payments.length, 1);
    assert.equal(result.payments[0].id, 'pay_001');
  });

  it('returns error when list throws', async () => {
    const commerce = makePaymentCommerce({
      list: async () => {
        throw new Error('DB error');
      },
    });
    try {
      await tool.handler({ commerce, params: {} });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('DB error'));
    }
  });
});

// ============================================================================
// get_payment
// ============================================================================

describe('get_payment', () => {
  const tool = findTool(paymentTools, 'get_payment');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns payment for valid ID', async () => {
    const result = await tool.handler({
      commerce: makePaymentCommerce(),
      params: { paymentId: 'pay_001' },
    });
    assert.equal(result.success, true);
    assert.ok(result.payment);
    assert.equal(result.payment.id, 'pay_001');
    assert.equal(result.payment.amount, '99.99');
  });

  it('returns error when get throws', async () => {
    const commerce = makePaymentCommerce({
      get: async () => {
        throw new Error('Payment not found');
      },
    });
    try {
      await tool.handler({ commerce, params: { paymentId: 'bad_id' } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Payment not found'));
    }
  });
});

// ============================================================================
// create_payment
// ============================================================================

describe('create_payment', () => {
  const tool = findTool(paymentTools, 'create_payment');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false (uses applyRequired)', async () => {
    const result = await tool.handler({
      commerce: makePaymentCommerce(),
      params: { orderId: 'ord_001', amount: 99.99 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field from applyRequired');
  });

  it('creates payment with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makePaymentCommerce(),
      params: { orderId: 'ord_001', amount: 99.99, currency: 'USD', method: 'credit_card' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('created'));
    assert.ok(result.payment);
  });

  it('defaults currency to USD and method to credit_card', async () => {
    let calledWith = null;
    const commerce = makePaymentCommerce({
      create: async (data) => {
        calledWith = data;
        return { ...mockPayment, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: { orderId: 'ord_001', amount: 50 },
      allowApply: true,
    });
    assert.equal(calledWith.currency, 'USD');
    assert.equal(calledWith.method, 'credit_card');
  });

  it('converts amount to string before passing to commerce', async () => {
    let calledWith = null;
    const commerce = makePaymentCommerce({
      create: async (data) => {
        calledWith = data;
        return { ...mockPayment, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: { orderId: 'ord_001', amount: 42.5 },
      allowApply: true,
    });
    assert.equal(calledWith.amount, '42.5');
  });

  it('returns error when commerce.payments.create throws', async () => {
    const commerce = makePaymentCommerce({
      create: async () => {
        throw new Error('Payment gateway error');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { orderId: 'ord_001', amount: 99.99 },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Payment gateway error'));
    }
  });
});

// ============================================================================
// complete_payment
// ============================================================================

describe('complete_payment', () => {
  const tool = findTool(paymentTools, 'complete_payment');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false (uses applyRequired)', async () => {
    const result = await tool.handler({
      commerce: makePaymentCommerce(),
      params: { paymentId: 'pay_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field from applyRequired');
  });

  it('completes payment with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makePaymentCommerce(),
      params: { paymentId: 'pay_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('completed'));
    assert.ok(result.payment);
    assert.equal(result.payment.status, 'completed');
  });

  it('returns error when markCompleted throws', async () => {
    const commerce = makePaymentCommerce({
      markCompleted: async () => {
        throw new Error('Payment already completed');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { paymentId: 'pay_001' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Payment already completed'));
    }
  });
});

// ============================================================================
// create_refund
// ============================================================================

describe('create_refund', () => {
  const tool = findTool(paymentTools, 'create_refund');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false (uses applyRequired)', async () => {
    const result = await tool.handler({
      commerce: makePaymentCommerce(),
      params: { paymentId: 'pay_001', amount: 25 },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field from applyRequired');
  });

  it('creates refund with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makePaymentCommerce(),
      params: { paymentId: 'pay_001', amount: 25, reason: 'defective item' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('refund'));
    assert.ok(result.refund);
  });

  it('converts amount to string and passes reason', async () => {
    let calledWith = null;
    const commerce = makePaymentCommerce({
      createRefund: async (data) => {
        calledWith = data;
        return { ...mockRefund, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: { paymentId: 'pay_001', amount: 15.5, reason: 'customer request' },
      allowApply: true,
    });
    assert.equal(calledWith.paymentId, 'pay_001');
    assert.equal(calledWith.amount, '15.5');
    assert.equal(calledWith.reason, 'customer request');
  });

  it('returns error when createRefund throws', async () => {
    const commerce = makePaymentCommerce({
      createRefund: async () => {
        throw new Error('Refund exceeds payment amount');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { paymentId: 'pay_001', amount: 999 },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Refund exceeds payment amount'));
    }
  });
});

// ============================================================================
// list_payment_providers
// ============================================================================

describe('list_payment_providers', () => {
  const tool = findTool(paymentTools, 'list_payment_providers');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns available provider list', async () => {
    const result = await tool.handler({ params: {} });
    assert.equal(result.success, true);
    assert.ok(result.count >= 2);
    assert.ok(result.providers.some((provider) => provider.id === 'deterministic-mock'));
    assert.ok(result.providers.some((provider) => provider.id === 'stripe'));
  });
});

// ============================================================================
// create/get/capture/cancel/refund payment intent
// ============================================================================

describe('payment intent provider lifecycle', () => {
  const createIntentTool = findTool(paymentTools, 'create_payment_intent');
  const getIntentTool = findTool(paymentTools, 'get_payment_intent');
  const listIntentTool = findTool(paymentTools, 'list_payment_intents');
  const listSettlementsTool = findTool(paymentTools, 'list_payment_settlements');
  const listSettlementBatchesTool = findTool(paymentTools, 'list_payment_settlement_batches');
  const createSettlementBatchTool = findTool(paymentTools, 'create_payment_settlement_batch');
  const reconcileProviderTool = findTool(paymentTools, 'reconcile_payment_provider');
  const captureIntentTool = findTool(paymentTools, 'capture_payment_intent');
  const cancelIntentTool = findTool(paymentTools, 'cancel_payment_intent');
  const refundIntentTool = findTool(paymentTools, 'refund_payment_intent');
  const ingestWebhookTool = findTool(paymentTools, 'ingest_payment_provider_webhook');

  it('create_payment_intent requires --apply', async () => {
    const result = await createIntentTool.handler({
      params: {
        providerId: 'deterministic-mock',
        amount: 149.99,
        currency: 'USD',
      },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('create_payment_intent creates intent when allowApply is true', async () => {
    const result = await createIntentTool.handler({
      params: {
        providerId: 'deterministic-mock',
        amount: 149.99,
        currency: 'USD',
        captureMethod: 'manual',
        orderId: 'ord_100',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.intent.status, 'requires_capture');
    assert.equal(result.intent.currency, 'USD');
  });

  it('create_payment_intent is idempotent with idempotencyKey', async () => {
    const params = {
      providerId: 'deterministic-mock',
      amount: 39,
      currency: 'USD',
      idempotencyKey: 'idem-1',
    };
    const first = await createIntentTool.handler({ params, allowApply: true });
    const second = await createIntentTool.handler({ params, allowApply: true });
    assert.equal(first.intent.id, second.intent.id);
    assert.equal(second.idempotent, true);
  });

  it('get_payment_intent returns intent by ID', async () => {
    const created = await createIntentTool.handler({
      params: { amount: 22, currency: 'USD', captureMethod: 'manual' },
      allowApply: true,
    });
    const result = await getIntentTool.handler({
      params: { intentId: created.intent.id },
    });
    assert.equal(result.success, true);
    assert.equal(result.intent.id, created.intent.id);
  });

  it('list_payment_intents returns created intents', async () => {
    await createIntentTool.handler({
      params: { amount: 22, currency: 'USD', captureMethod: 'manual', orderId: 'ord_list_1' },
      allowApply: true,
    });
    const result = await listIntentTool.handler({
      params: { orderId: 'ord_list_1' },
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(result.intents[0].orderId, 'ord_list_1');
  });

  it('create_payment_settlement_batch requires --apply', async () => {
    const created = await createIntentTool.handler({
      params: { amount: 20, currency: 'USD', captureMethod: 'automatic' },
      allowApply: true,
    });
    const result = await createSettlementBatchTool.handler({
      params: { providerId: created.intent.providerId, intentIds: [created.intent.id] },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('creates settlement batch and reconciles pending balances', async () => {
    const created = await createIntentTool.handler({
      params: {
        providerId: 'stripe',
        amount: 110,
        currency: 'USD',
        captureMethod: 'manual',
        orderId: 'ord_settle_1',
      },
      allowApply: true,
    });
    await captureIntentTool.handler({
      params: { intentId: created.intent.id, amount: 100 },
      allowApply: true,
    });
    await refundIntentTool.handler({
      params: { intentId: created.intent.id, amount: 25 },
      allowApply: true,
    });

    const before = await reconcileProviderTool.handler({
      params: { providerId: 'stripe', orderId: 'ord_settle_1', includeBalanced: false },
    });
    assert.equal(before.success, true);
    assert.equal(before.count, 1);
    assert.equal(before.reconciliation[0].reconciliationStatus, 'pending_settlement');
    assert.equal(before.reconciliation[0].outstandingAmount, '75.00');

    const settled = await createSettlementBatchTool.handler({
      params: {
        providerId: 'stripe',
        intentIds: [created.intent.id],
        payoutReference: 'po_100',
        idempotencyKey: 'settle-idem-1',
      },
      allowApply: true,
    });
    assert.equal(settled.success, true);
    assert.equal(settled.count, 1);
    assert.equal(settled.batch.payoutReference, 'po_100');
    assert.equal(settled.settlements[0].amount, '75.00');

    const listedSettlements = await listSettlementsTool.handler({
      params: { providerId: 'stripe', orderId: 'ord_settle_1' },
    });
    assert.equal(listedSettlements.success, true);
    assert.equal(listedSettlements.count, 1);

    const listedBatches = await listSettlementBatchesTool.handler({
      params: { providerId: 'stripe', payoutReference: 'po_100' },
    });
    assert.equal(listedBatches.success, true);
    assert.equal(listedBatches.count, 1);
    assert.equal(listedBatches.batches[0].status, 'paid');

    const after = await reconcileProviderTool.handler({
      params: { providerId: 'stripe', orderId: 'ord_settle_1' },
    });
    assert.equal(after.success, true);
    assert.equal(after.reconciliation[0].reconciliationStatus, 'balanced');
    assert.equal(after.reconciliation[0].outstandingAmount, '0.00');
  });

  it('create_payment_settlement_batch is idempotent with idempotencyKey', async () => {
    const created = await createIntentTool.handler({
      params: { providerId: 'stripe', amount: 30, currency: 'USD', captureMethod: 'automatic' },
      allowApply: true,
    });
    const params = {
      providerId: 'stripe',
      intentIds: [created.intent.id],
      payoutReference: 'po_idem',
      idempotencyKey: 'settlement-idem-2',
    };

    const first = await createSettlementBatchTool.handler({ params, allowApply: true });
    const second = await createSettlementBatchTool.handler({ params, allowApply: true });
    assert.equal(first.batch.id, second.batch.id);
    assert.equal(second.idempotent, true);
  });

  it('capture_payment_intent captures intent with allowApply: true', async () => {
    const created = await createIntentTool.handler({
      params: { amount: 75, currency: 'USD', captureMethod: 'manual' },
      allowApply: true,
    });
    const captured = await captureIntentTool.handler({
      params: { intentId: created.intent.id },
      allowApply: true,
    });
    assert.equal(captured.success, true);
    assert.equal(captured.intent.status, 'succeeded');
    assert.ok(captured.capture);
  });

  it('cancel_payment_intent cancels uncaptured intent', async () => {
    const created = await createIntentTool.handler({
      params: { amount: 15, currency: 'USD', captureMethod: 'manual' },
      allowApply: true,
    });
    const canceled = await cancelIntentTool.handler({
      params: { intentId: created.intent.id, reason: 'customer_cancelled' },
      allowApply: true,
    });
    assert.equal(canceled.success, true);
    assert.equal(canceled.intent.status, 'canceled');
  });

  it('refund_payment_intent refunds captured funds', async () => {
    const created = await createIntentTool.handler({
      params: { amount: 40, currency: 'USD', captureMethod: 'manual' },
      allowApply: true,
    });
    await captureIntentTool.handler({
      params: { intentId: created.intent.id },
      allowApply: true,
    });
    const refunded = await refundIntentTool.handler({
      params: { intentId: created.intent.id, amount: 10, reason: 'partial_return' },
      allowApply: true,
    });
    assert.equal(refunded.success, true);
    assert.equal(refunded.refund.amount, '10.00');
    assert.equal(refunded.intent.status, 'partially_refunded');
  });

  it('refund_payment_intent rejects over-refund amount', async () => {
    const created = await createIntentTool.handler({
      params: { amount: 20, currency: 'USD', captureMethod: 'automatic' },
      allowApply: true,
    });
    await assert.rejects(
      () =>
        refundIntentTool.handler({
          params: { intentId: created.intent.id, amount: 999 },
          allowApply: true,
        }),
      /exceeds remaining refundable/,
    );
  });

  it('ingest_payment_provider_webhook captures matching intent', async () => {
    const created = await createIntentTool.handler({
      params: {
        providerId: 'stripe',
        amount: 50,
        currency: 'USD',
        captureMethod: 'manual',
      },
      allowApply: true,
    });
    const result = await ingestWebhookTool.handler({
      params: {
        providerId: 'stripe',
        eventType: 'payment_intent.succeeded',
        eventId: 'evt_1',
        payload: {
          data: { object: { id: created.intent.providerIntentId } },
          amount_received: 5000,
        },
      },
      allowApply: true,
    });

    assert.equal(result.success, true);
    assert.equal(result.webhook.action, 'captured');
    assert.equal(result.webhook.intent.status, 'succeeded');
  });

  it('ingest_payment_provider_webhook can create settlement from payout event', async () => {
    const created = await createIntentTool.handler({
      params: {
        providerId: 'stripe',
        amount: 60,
        currency: 'USD',
        captureMethod: 'automatic',
        orderId: 'ord_webhook_settle',
      },
      allowApply: true,
    });

    const settled = await ingestWebhookTool.handler({
      params: {
        providerId: 'stripe',
        eventType: 'payout.paid',
        eventId: 'evt_payout_1',
        payload: {
          payoutId: 'po_webhook_1',
          intentIds: [created.intent.id],
        },
      },
      allowApply: true,
    });

    assert.equal(settled.success, true);
    assert.equal(settled.webhook.action, 'settled');
    assert.equal(settled.webhook.batch.payoutReference, 'po_webhook_1');
    assert.equal(settled.webhook.settlements.length, 1);

    const reconciliation = await reconcileProviderTool.handler({
      params: { providerId: 'stripe', orderId: 'ord_webhook_settle' },
    });
    assert.equal(reconciliation.reconciliation[0].reconciliationStatus, 'balanced');
  });

  it('ingest_payment_provider_webhook marks settlement batch failed on payout failure', async () => {
    const created = await createIntentTool.handler({
      params: {
        providerId: 'stripe',
        amount: 35,
        currency: 'USD',
        captureMethod: 'automatic',
      },
      allowApply: true,
    });

    await createSettlementBatchTool.handler({
      params: {
        providerId: 'stripe',
        intentIds: [created.intent.id],
        payoutReference: 'po_fail_1',
      },
      allowApply: true,
    });

    const failed = await ingestWebhookTool.handler({
      params: {
        providerId: 'stripe',
        eventType: 'payout.failed',
        eventId: 'evt_payout_fail_1',
        payload: {
          payoutId: 'po_fail_1',
          reason: 'bank_account_closed',
        },
      },
      allowApply: true,
    });
    assert.equal(failed.success, true);
    assert.equal(failed.webhook.action, 'settlement_failed');
    assert.equal(failed.webhook.batch.status, 'failed');

    const listedBatches = await listSettlementBatchesTool.handler({
      params: { providerId: 'stripe', payoutReference: 'po_fail_1' },
    });
    assert.equal(listedBatches.count, 1);
    assert.equal(listedBatches.batches[0].status, 'failed');
  });

  it('ingest_payment_provider_webhook is idempotent for duplicate event IDs', async () => {
    const created = await createIntentTool.handler({
      params: {
        providerId: 'stripe',
        amount: 25,
        currency: 'USD',
        captureMethod: 'manual',
      },
      allowApply: true,
    });
    const params = {
      providerId: 'stripe',
      eventType: 'payment_intent.succeeded',
      eventId: 'evt_dup',
      payload: {
        data: { object: { id: created.intent.providerIntentId } },
        amount_received: 2500,
      },
    };

    const first = await ingestWebhookTool.handler({ params, allowApply: true });
    const second = await ingestWebhookTool.handler({ params, allowApply: true });

    assert.equal(first.webhook.idempotent, false);
    assert.equal(second.webhook.idempotent, true);
  });
});
