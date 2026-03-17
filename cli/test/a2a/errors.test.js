/**
 * Tests for cli/src/a2a/errors.js
 *
 * Covers: A2AError class, all 10 error factories, toAgentResponse conversion
 * for both A2AError and plain Error instances.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  A2AError,
  A2AErrorCode,
  budgetExceeded,
  insufficientBalance,
  escrowConditionUnmet,
  quoteExpired,
  sequencerUnavailable,
  agentNotFound,
  duplicatePayment,
  disputeDeadlineExpired,
  rateLimited,
  settlementPending,
  toAgentResponse,
} from '../../src/a2a/errors.js';

// ---------------------------------------------------------------------------
// 1. A2AError class — fields and inheritance
// ---------------------------------------------------------------------------

describe('A2AError — class', () => {
  it('is an instance of Error', () => {
    const err = new A2AError('test', { code: 'TEST', retryable: false });
    assert.ok(err instanceof Error);
    assert.ok(err instanceof A2AError);
  });

  it('has name set to A2AError', () => {
    const err = new A2AError('msg', { code: 'X' });
    assert.equal(err.name, 'A2AError');
  });

  it('stores code, recovery, suggestedAction, retryable, retryAfterMs', () => {
    const err = new A2AError('broke', {
      code: 'BROKE',
      recovery: 'fix it',
      suggestedAction: 'do_something',
      retryable: true,
      retryAfterMs: 5000,
    });

    assert.equal(err.code, 'BROKE');
    assert.equal(err.recovery, 'fix it');
    assert.equal(err.suggestedAction, 'do_something');
    assert.equal(err.retryable, true);
    assert.equal(err.retryAfterMs, 5000);
    assert.equal(err.message, 'broke');
  });

  it('defaults optional fields', () => {
    const err = new A2AError('simple');
    assert.equal(err.code, 'UNKNOWN');
    assert.equal(err.recovery, '');
    assert.equal(err.suggestedAction, null);
    assert.equal(err.retryable, false);
    assert.equal(err.retryAfterMs, null);
    assert.equal(err.details, null);
  });

  it('stores details when provided', () => {
    const details = { foo: 'bar', count: 42 };
    const err = new A2AError('with details', { code: 'D', details });
    assert.deepEqual(err.details, details);
  });

  it('has a proper stack trace', () => {
    const err = new A2AError('stack test', { code: 'S' });
    assert.ok(err.stack.includes('stack test'));
    assert.ok(err.stack.includes('errors.test.js'));
  });
});

// ---------------------------------------------------------------------------
// 2. Error factories — each creates the correct error
// ---------------------------------------------------------------------------

describe('Error factories', () => {
  it('budgetExceeded() creates correct error', () => {
    const err = budgetExceeded({ requested: 500, remaining: 100 });
    assert.ok(err instanceof A2AError);
    assert.equal(err.code, A2AErrorCode.BUDGET_EXCEEDED);
    assert.match(err.message, /Budget exceeded/);
    assert.match(err.message, /500/);
    assert.match(err.message, /100/);
    assert.equal(err.recovery, 'Pause subscriptions or wait for daily reset');
    assert.equal(err.retryable, false);
    assert.equal(err.suggestedAction, null);
  });

  it('budgetExceeded() without details has generic message', () => {
    const err = budgetExceeded();
    assert.equal(err.message, 'Budget exceeded');
    assert.equal(err.code, A2AErrorCode.BUDGET_EXCEEDED);
  });

  it('insufficientBalance() creates correct error', () => {
    const err = insufficientBalance({ required: 200, available: 50, asset: 'USDC' });
    assert.ok(err instanceof A2AError);
    assert.equal(err.code, A2AErrorCode.INSUFFICIENT_BALANCE);
    assert.match(err.message, /200/);
    assert.match(err.message, /50/);
    assert.match(err.message, /USDC/);
    assert.equal(err.recovery, 'Fund wallet or reduce amount');
    assert.equal(err.retryable, false);
  });

  it('escrowConditionUnmet() lists unmet conditions in recovery', () => {
    const err = escrowConditionUnmet({
      escrowId: 'esc-1',
      unmetConditions: [
        { type: 'seller_fulfilled', reason: 'seller has not shipped' },
        { type: 'buyer_confirmed', reason: 'buyer has not confirmed' },
      ],
    });
    assert.ok(err instanceof A2AError);
    assert.equal(err.code, A2AErrorCode.ESCROW_CONDITION_UNMET);
    assert.match(err.message, /seller_fulfilled/);
    assert.match(err.message, /buyer_confirmed/);
    assert.match(err.recovery, /seller_fulfilled/);
    assert.match(err.recovery, /buyer_confirmed/);
    assert.equal(err.retryable, false);
  });

  it('escrowConditionUnmet() without conditions has generic recovery', () => {
    const err = escrowConditionUnmet();
    assert.match(err.recovery, /fulfill all escrow release conditions/);
  });

  it('quoteExpired() has suggestedAction', () => {
    const err = quoteExpired({ quoteId: 'q-42' });
    assert.ok(err instanceof A2AError);
    assert.equal(err.code, A2AErrorCode.QUOTE_EXPIRED);
    assert.match(err.message, /q-42/);
    assert.equal(err.recovery, 'Request a new quote');
    assert.equal(err.suggestedAction, 'a2a_request_quote');
    assert.equal(err.retryable, false);
  });

  it('sequencerUnavailable() is retryable with retryAfterMs', () => {
    const err = sequencerUnavailable({ reason: 'connection refused' });
    assert.ok(err instanceof A2AError);
    assert.equal(err.code, A2AErrorCode.SEQUENCER_UNAVAILABLE);
    assert.match(err.message, /connection refused/);
    assert.equal(err.recovery, 'Payment queued for later submission');
    assert.equal(err.retryable, true);
    assert.equal(err.retryAfterMs, 30_000);
  });

  it('agentNotFound() has suggestedAction register_agent_card', () => {
    const err = agentNotFound({ agentId: 'agent-xyz' });
    assert.ok(err instanceof A2AError);
    assert.equal(err.code, A2AErrorCode.AGENT_NOT_FOUND);
    assert.match(err.message, /agent-xyz/);
    assert.equal(err.recovery, 'Register agent first');
    assert.equal(err.suggestedAction, 'register_agent_card');
    assert.equal(err.retryable, false);
  });

  it('agentNotFound() uses walletAddress when agentId is absent', () => {
    const err = agentNotFound({ walletAddress: '0xABC' });
    assert.match(err.message, /0xABC/);
  });

  it('duplicatePayment() is not retryable', () => {
    const err = duplicatePayment({ paymentId: 'pay-99', idempotencyKey: 'idem-1' });
    assert.ok(err instanceof A2AError);
    assert.equal(err.code, A2AErrorCode.DUPLICATE_PAYMENT);
    assert.match(err.message, /pay-99/);
    assert.equal(err.recovery, 'Payment already processed');
    assert.equal(err.retryable, false);
  });

  it('disputeDeadlineExpired() has suggestedAction a2a_file_dispute', () => {
    const err = disputeDeadlineExpired({ escrowId: 'esc-5' });
    assert.ok(err instanceof A2AError);
    assert.equal(err.code, A2AErrorCode.DISPUTE_DEADLINE_EXPIRED);
    assert.match(err.message, /esc-5/);
    assert.equal(err.recovery, 'File a new dispute or escalate');
    assert.equal(err.suggestedAction, 'a2a_file_dispute');
    assert.equal(err.retryable, false);
  });

  it('rateLimited() is retryable with custom retryAfterMs', () => {
    const err = rateLimited({ retryAfterMs: 15_000, limit: 60, remaining: 0 });
    assert.ok(err instanceof A2AError);
    assert.equal(err.code, A2AErrorCode.RATE_LIMITED);
    assert.equal(err.message, 'Rate limited');
    assert.equal(err.recovery, 'Wait and retry');
    assert.equal(err.retryable, true);
    assert.equal(err.retryAfterMs, 15_000);
  });

  it('rateLimited() defaults retryAfterMs to 60000', () => {
    const err = rateLimited();
    assert.equal(err.retryAfterMs, 60_000);
  });

  it('settlementPending() is retryable with suggestedAction', () => {
    const err = settlementPending({ intentId: 'int-7', status: 'confirming' });
    assert.ok(err instanceof A2AError);
    assert.equal(err.code, A2AErrorCode.SETTLEMENT_PENDING);
    assert.match(err.message, /int-7/);
    assert.equal(err.recovery, 'Check status with x402_get_intent');
    assert.equal(err.suggestedAction, 'x402_get_intent');
    assert.equal(err.retryable, true);
    assert.equal(err.retryAfterMs, 10_000);
  });
});

// ---------------------------------------------------------------------------
// 3. toAgentResponse — plain Error
// ---------------------------------------------------------------------------

describe('toAgentResponse — plain Error', () => {
  it('wraps a plain Error in structured response', () => {
    const err = new Error('something broke');
    const resp = toAgentResponse(err);

    assert.equal(resp.success, false);
    assert.equal(resp.error.code, 'INTERNAL_ERROR');
    assert.equal(resp.error.message, 'something broke');
    assert.equal(resp.error.recovery, 'Contact support or retry');
    assert.equal(resp.error.suggestedAction, null);
    assert.equal(resp.error.retryable, false);
  });

  it('handles Error with no message', () => {
    const err = new Error();
    const resp = toAgentResponse(err);
    assert.equal(resp.error.message, 'An unexpected error occurred');
  });
});

// ---------------------------------------------------------------------------
// 4. toAgentResponse — A2AError with all fields
// ---------------------------------------------------------------------------

describe('toAgentResponse — A2AError', () => {
  it('includes all fields from A2AError', () => {
    const err = sequencerUnavailable({ reason: 'timeout' });
    const resp = toAgentResponse(err);

    assert.equal(resp.success, false);
    assert.equal(resp.error.code, 'SEQUENCER_UNAVAILABLE');
    assert.match(resp.error.message, /timeout/);
    assert.equal(resp.error.recovery, 'Payment queued for later submission');
    assert.equal(resp.error.retryable, true);
    assert.equal(resp.error.retryAfterMs, 30_000);
  });

  it('includes suggestedAction when present', () => {
    const err = quoteExpired({ quoteId: 'q-1' });
    const resp = toAgentResponse(err);

    assert.equal(resp.error.suggestedAction, 'a2a_request_quote');
  });

  it('includes details when present', () => {
    const err = budgetExceeded({ requested: 1000, remaining: 50 });
    const resp = toAgentResponse(err);

    assert.deepEqual(resp.error.details, { requested: 1000, remaining: 50 });
  });

  it('omits retryAfterMs when null', () => {
    const err = duplicatePayment({ paymentId: 'p-1' });
    const resp = toAgentResponse(err);

    assert.equal(resp.error.retryable, false);
    assert.equal('retryAfterMs' in resp.error, false);
  });

  it('omits details when null', () => {
    const err = new A2AError('no details', { code: 'X', retryable: false });
    const resp = toAgentResponse(err);

    assert.equal('details' in resp.error, false);
  });
});

// ---------------------------------------------------------------------------
// 5. Retryable errors include retryAfterMs
// ---------------------------------------------------------------------------

describe('Retryable errors', () => {
  const retryableFactories = [
    { name: 'sequencerUnavailable', fn: sequencerUnavailable, expectedMs: 30_000 },
    { name: 'rateLimited', fn: () => rateLimited({ retryAfterMs: 45_000 }), expectedMs: 45_000 },
    { name: 'settlementPending', fn: settlementPending, expectedMs: 10_000 },
  ];

  for (const { name, fn, expectedMs } of retryableFactories) {
    it(`${name}() has retryable=true and retryAfterMs=${expectedMs}`, () => {
      const err = fn();
      assert.equal(err.retryable, true);
      assert.equal(err.retryAfterMs, expectedMs);
    });
  }
});

// ---------------------------------------------------------------------------
// 6. Error is instanceof Error
// ---------------------------------------------------------------------------

describe('A2AError — instanceof chain', () => {
  it('every factory returns an instance of Error', () => {
    const factories = [
      budgetExceeded,
      insufficientBalance,
      escrowConditionUnmet,
      quoteExpired,
      sequencerUnavailable,
      agentNotFound,
      duplicatePayment,
      disputeDeadlineExpired,
      rateLimited,
      settlementPending,
    ];

    for (const factory of factories) {
      const err = factory();
      assert.ok(err instanceof Error, `${factory.name}() should be instanceof Error`);
      assert.ok(err instanceof A2AError, `${factory.name}() should be instanceof A2AError`);
    }
  });

  it('can be caught by catch(Error)', () => {
    try {
      throw budgetExceeded({ requested: 100 });
    } catch (err) {
      assert.ok(err instanceof Error);
      assert.equal(err.code, 'BUDGET_EXCEEDED');
    }
  });
});

// ---------------------------------------------------------------------------
// 7. Non-retryable errors have retryable: false
// ---------------------------------------------------------------------------

describe('Non-retryable errors', () => {
  const nonRetryableFactories = [
    { name: 'budgetExceeded', fn: budgetExceeded },
    { name: 'insufficientBalance', fn: insufficientBalance },
    { name: 'escrowConditionUnmet', fn: escrowConditionUnmet },
    { name: 'quoteExpired', fn: quoteExpired },
    { name: 'agentNotFound', fn: agentNotFound },
    { name: 'duplicatePayment', fn: duplicatePayment },
    { name: 'disputeDeadlineExpired', fn: disputeDeadlineExpired },
  ];

  for (const { name, fn } of nonRetryableFactories) {
    it(`${name}() has retryable=false`, () => {
      const err = fn();
      assert.equal(err.retryable, false, `${name} should not be retryable`);
    });
  }

  it('non-retryable errors do not have retryAfterMs in agent response', () => {
    const err = budgetExceeded();
    const resp = toAgentResponse(err);
    assert.equal(resp.error.retryable, false);
    // retryAfterMs is omitted (null in error, not included in response)
    assert.equal('retryAfterMs' in resp.error, false);
  });
});

// ---------------------------------------------------------------------------
// 8. A2AErrorCode enum completeness
// ---------------------------------------------------------------------------

describe('A2AErrorCode enum', () => {
  it('contains all 10 error codes', () => {
    const expectedCodes = [
      'BUDGET_EXCEEDED',
      'INSUFFICIENT_BALANCE',
      'ESCROW_CONDITION_UNMET',
      'QUOTE_EXPIRED',
      'SEQUENCER_UNAVAILABLE',
      'AGENT_NOT_FOUND',
      'DUPLICATE_PAYMENT',
      'DISPUTE_DEADLINE_EXPIRED',
      'RATE_LIMITED',
      'SETTLEMENT_PENDING',
    ];

    for (const code of expectedCodes) {
      assert.equal(A2AErrorCode[code], code, `A2AErrorCode should include ${code}`);
    }
    assert.equal(Object.keys(A2AErrorCode).length, 10);
  });
});
