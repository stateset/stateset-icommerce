/**
 * Actionable Errors — Structured Error System for AI Agents
 *
 * Every error tells the agent what went wrong AND what to do next.
 * This enables autonomous recovery without human intervention.
 *
 * @example
 * ```javascript
 * import { budgetExceeded, toAgentResponse } from './errors.js';
 *
 * try {
 *   await processPayment(amount);
 * } catch (err) {
 *   if (amount > budget) {
 *     throw budgetExceeded({ requested: amount, remaining: budget });
 *   }
 *   throw err;
 * }
 *
 * // In error handler:
 * const response = toAgentResponse(error);
 * // { success: false, error: { code, message, recovery, suggestedAction, retryable } }
 * ```
 */

// ---------------------------------------------------------------------------
// Error codes
// ---------------------------------------------------------------------------

/** @enum {string} */
export const A2AErrorCode = {
  BUDGET_EXCEEDED: 'BUDGET_EXCEEDED',
  INSUFFICIENT_BALANCE: 'INSUFFICIENT_BALANCE',
  ESCROW_CONDITION_UNMET: 'ESCROW_CONDITION_UNMET',
  QUOTE_EXPIRED: 'QUOTE_EXPIRED',
  SEQUENCER_UNAVAILABLE: 'SEQUENCER_UNAVAILABLE',
  AGENT_NOT_FOUND: 'AGENT_NOT_FOUND',
  DUPLICATE_PAYMENT: 'DUPLICATE_PAYMENT',
  DISPUTE_DEADLINE_EXPIRED: 'DISPUTE_DEADLINE_EXPIRED',
  RATE_LIMITED: 'RATE_LIMITED',
  SETTLEMENT_PENDING: 'SETTLEMENT_PENDING',
};

// ---------------------------------------------------------------------------
// A2AError class
// ---------------------------------------------------------------------------

/**
 * Structured error that gives AI agents recovery guidance.
 *
 * @extends Error
 */
export class A2AError extends Error {
  /**
   * @param {string} message - Human-readable error message
   * @param {Object} opts
   * @param {string} opts.code - Machine-readable error code (from A2AErrorCode)
   * @param {string} opts.recovery - Plain-language recovery guidance for the agent
   * @param {string} [opts.suggestedAction] - MCP tool name the agent should call next
   * @param {boolean} opts.retryable - Whether the agent should retry
   * @param {number} [opts.retryAfterMs] - Milliseconds to wait before retrying
   * @param {Object} [opts.details] - Additional structured context
   */
  constructor(message, opts = {}) {
    super(message);
    this.name = 'A2AError';
    this.code = opts.code ?? 'UNKNOWN';
    this.recovery = opts.recovery ?? '';
    this.suggestedAction = opts.suggestedAction ?? null;
    this.retryable = opts.retryable ?? false;
    this.retryAfterMs = opts.retryAfterMs ?? null;
    this.details = opts.details ?? null;
  }
}

// ---------------------------------------------------------------------------
// Error factories
// ---------------------------------------------------------------------------

/**
 * Budget exceeded — agent tried to spend more than its daily/total budget.
 *
 * @param {Object} [details]
 * @param {number} [details.requested] - Amount the agent tried to spend
 * @param {number} [details.remaining] - Remaining budget
 * @param {string} [details.resetAt] - When the budget resets
 * @returns {A2AError}
 */
export function budgetExceeded(details = {}) {
  const msg = details.requested
    ? `Budget exceeded: requested ${details.requested}, remaining ${details.remaining ?? 0}`
    : 'Budget exceeded';
  return new A2AError(msg, {
    code: A2AErrorCode.BUDGET_EXCEEDED,
    recovery: 'Pause subscriptions or wait for daily reset',
    retryable: false,
    details,
  });
}

/**
 * Insufficient balance — wallet does not have enough funds.
 *
 * @param {Object} [details]
 * @param {number} [details.required] - Amount needed
 * @param {number} [details.available] - Current balance
 * @param {string} [details.asset] - Asset type (e.g., USDC)
 * @returns {A2AError}
 */
export function insufficientBalance(details = {}) {
  const msg = details.required
    ? `Insufficient balance: need ${details.required} ${details.asset ?? ''}, have ${details.available ?? 0}`
    : 'Insufficient balance';
  return new A2AError(msg, {
    code: A2AErrorCode.INSUFFICIENT_BALANCE,
    recovery: 'Fund wallet or reduce amount',
    retryable: false,
    details,
  });
}

/**
 * Escrow condition not met — one or more release conditions are unsatisfied.
 *
 * @param {Object} [details]
 * @param {string} [details.escrowId] - Escrow ID
 * @param {Array} [details.unmetConditions] - List of { type, reason } for unmet conditions
 * @returns {A2AError}
 */
export function escrowConditionUnmet(details = {}) {
  const unmet = details.unmetConditions || [];
  const conditionList = unmet.map((c) => `${c.type}: ${c.reason || 'not met'}`).join('; ');
  const msg = conditionList
    ? `Escrow conditions not met: ${conditionList}`
    : 'Escrow conditions not met';
  const recovery =
    unmet.length > 0
      ? `Fulfill the following conditions: ${unmet.map((c) => c.type).join(', ')}`
      : 'Check and fulfill all escrow release conditions';
  return new A2AError(msg, {
    code: A2AErrorCode.ESCROW_CONDITION_UNMET,
    recovery,
    retryable: false,
    details,
  });
}

/**
 * Quote expired — the quote TTL has passed.
 *
 * @param {Object} [details]
 * @param {string} [details.quoteId] - Expired quote ID
 * @param {string} [details.expiredAt] - When it expired
 * @returns {A2AError}
 */
export function quoteExpired(details = {}) {
  const msg = details.quoteId ? `Quote ${details.quoteId} has expired` : 'Quote has expired';
  return new A2AError(msg, {
    code: A2AErrorCode.QUOTE_EXPIRED,
    recovery: 'Request a new quote',
    suggestedAction: 'a2a_request_quote',
    retryable: false,
    details,
  });
}

/**
 * Sequencer unavailable — the payment sequencer is down or unreachable.
 *
 * @param {Object} [details]
 * @param {string} [details.reason] - Why the sequencer is unavailable
 * @returns {A2AError}
 */
export function sequencerUnavailable(details = {}) {
  const msg = details.reason ? `Sequencer unavailable: ${details.reason}` : 'Sequencer unavailable';
  return new A2AError(msg, {
    code: A2AErrorCode.SEQUENCER_UNAVAILABLE,
    recovery: 'Payment queued for later submission',
    retryable: true,
    retryAfterMs: 30_000,
    details,
  });
}

/**
 * Agent not found — the referenced agent is not registered.
 *
 * @param {Object} [details]
 * @param {string} [details.agentId] - The agent ID that was not found
 * @param {string} [details.walletAddress] - The wallet address that was not found
 * @returns {A2AError}
 */
export function agentNotFound(details = {}) {
  const identifier = details.agentId || details.walletAddress || 'unknown';
  return new A2AError(`Agent not found: ${identifier}`, {
    code: A2AErrorCode.AGENT_NOT_FOUND,
    recovery: 'Register agent first',
    suggestedAction: 'register_agent_card',
    retryable: false,
    details,
  });
}

/**
 * Duplicate payment — the payment was already processed (idempotency hit).
 *
 * @param {Object} [details]
 * @param {string} [details.paymentId] - The existing payment ID
 * @param {string} [details.idempotencyKey] - The idempotency key that matched
 * @returns {A2AError}
 */
export function duplicatePayment(details = {}) {
  const msg = details.paymentId
    ? `Payment already processed: ${details.paymentId}`
    : 'Payment already processed';
  return new A2AError(msg, {
    code: A2AErrorCode.DUPLICATE_PAYMENT,
    recovery: 'Payment already processed',
    retryable: false,
    details,
  });
}

/**
 * Dispute deadline expired — the window for filing a dispute has passed.
 *
 * @param {Object} [details]
 * @param {string} [details.escrowId] - The escrow in question
 * @param {string} [details.deadline] - When the deadline was
 * @returns {A2AError}
 */
export function disputeDeadlineExpired(details = {}) {
  const msg = details.escrowId
    ? `Dispute deadline expired for escrow ${details.escrowId}`
    : 'Dispute deadline expired';
  return new A2AError(msg, {
    code: A2AErrorCode.DISPUTE_DEADLINE_EXPIRED,
    recovery: 'File a new dispute or escalate',
    suggestedAction: 'a2a_file_dispute',
    retryable: false,
    details,
  });
}

/**
 * Rate limited — too many requests in the current window.
 *
 * @param {Object} [details]
 * @param {number} [details.retryAfterMs] - Milliseconds to wait before retrying
 * @param {number} [details.limit] - The rate limit that was exceeded
 * @param {number} [details.remaining] - Remaining requests (usually 0)
 * @returns {A2AError}
 */
export function rateLimited(details = {}) {
  const retryAfterMs = details.retryAfterMs ?? 60_000;
  return new A2AError('Rate limited', {
    code: A2AErrorCode.RATE_LIMITED,
    recovery: 'Wait and retry',
    retryable: true,
    retryAfterMs,
    details,
  });
}

/**
 * Settlement pending — the on-chain settlement is in progress.
 *
 * @param {Object} [details]
 * @param {string} [details.intentId] - The x402 payment intent ID
 * @param {string} [details.status] - Current settlement status
 * @returns {A2AError}
 */
export function settlementPending(details = {}) {
  const msg = details.intentId
    ? `Settlement pending for intent ${details.intentId}`
    : 'Settlement pending';
  return new A2AError(msg, {
    code: A2AErrorCode.SETTLEMENT_PENDING,
    recovery: 'Check status with x402_get_intent',
    suggestedAction: 'x402_get_intent',
    retryable: true,
    retryAfterMs: 10_000,
    details,
  });
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

/**
 * Convert any error (A2AError or plain Error) into a structured JSON response
 * suitable for an AI agent.
 *
 * @param {Error|A2AError} error
 * @returns {{ success: false, error: { code: string, message: string, recovery: string, suggestedAction: string|null, retryable: boolean, retryAfterMs?: number } }}
 */
export function toAgentResponse(error) {
  if (error instanceof A2AError) {
    const resp = {
      success: false,
      error: {
        code: error.code,
        message: error.message,
        recovery: error.recovery,
        suggestedAction: error.suggestedAction,
        retryable: error.retryable,
      },
    };
    if (error.retryAfterMs !== null && error.retryAfterMs !== undefined) {
      resp.error.retryAfterMs = error.retryAfterMs;
    }
    if (error.details !== null && error.details !== undefined) {
      resp.error.details = error.details;
    }
    return resp;
  }

  // Plain Error — wrap in generic structure
  return {
    success: false,
    error: {
      code: 'INTERNAL_ERROR',
      message: error.message || 'An unexpected error occurred',
      recovery: 'Contact support or retry',
      suggestedAction: null,
      retryable: false,
    },
  };
}

export default {
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
};
