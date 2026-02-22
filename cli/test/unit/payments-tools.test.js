/**
 * Payment Tools Test Suite
 *
 * Tests for cli/src/tools/payments.js
 * Covers: list_payments, get_payment, create_payment, complete_payment, create_refund
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { paymentTools } from '../../src/tools/payments.js';

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

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Payment Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(paymentTools));
  });

  it('has at least 5 tools', () => {
    assert.ok(paymentTools.length >= 5, `Expected >= 5, got ${paymentTools.length}`);
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
