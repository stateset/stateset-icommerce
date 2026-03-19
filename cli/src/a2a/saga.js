/**
 * A2A Transaction Saga Framework — Multi-Step Commerce Orchestration
 *
 * Executes multi-step commerce flows (quote->negotiate->escrow->pay->fulfill->rate)
 * with automatic rollback (compensation) on failure. Each saga is a sequence
 * of steps with execute and compensate functions, tracked by a unique saga ID.
 *
 * Features:
 *   - Define sagas as ordered steps with execute + compensate pairs
 *   - Auto-compensate on failure in reverse step order
 *   - Idempotency: re-executing a saga with the same ID skips completed steps
 *   - Per-step timeout enforcement
 *   - Per-step retry with configurable attempts
 *   - Event emission at each lifecycle point
 *   - Pre-built saga templates for common commerce flows
 *
 * @example
 * ```javascript
 * import { createSagaOrchestrator, PURCHASE_SAGA } from './saga.js';
 *
 * const orchestrator = createSagaOrchestrator(store, services);
 * const result = await orchestrator.execute(PURCHASE_SAGA, {
 *   buyerAddress: '0xBuyer',
 *   sellerAddress: '0xSeller',
 *   amount: 100,
 * });
 * // result => { sagaId, status: 'completed', steps: [...] }
 * ```
 */

import { randomUUID } from 'node:crypto';
import { EventEmitter } from 'node:events';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Valid saga statuses */
const SAGA_STATUS = {
  PENDING: 'pending',
  RUNNING: 'running',
  COMPLETED: 'completed',
  FAILED: 'failed',
  COMPENSATING: 'compensating',
  COMPENSATED: 'compensated',
  CANCELLED: 'cancelled',
};

/** Valid step statuses */
const STEP_STATUS = {
  PENDING: 'pending',
  RUNNING: 'running',
  COMPLETED: 'completed',
  FAILED: 'failed',
  COMPENSATING: 'compensating',
  COMPENSATED: 'compensated',
  COMPENSATION_FAILED: 'compensation_failed',
  SKIPPED: 'skipped',
  TIMED_OUT: 'timed_out',
};

/** Default step timeout (30 seconds) */
const DEFAULT_STEP_TIMEOUT_MS = 30_000;

/** Default step retries */
const DEFAULT_STEP_RETRIES = 0;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Run a function with a timeout. Rejects with a timeout error if the
 * function does not resolve within the given duration.
 *
 * @param {Function} fn - Async function to execute
 * @param {number} timeoutMs - Timeout in milliseconds
 * @returns {Promise<*>} Result of fn
 */
function withTimeout(fn, timeoutMs) {
  if (!timeoutMs || timeoutMs <= 0) {
    return fn();
  }

  return new Promise((resolve, reject) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        reject(new Error(`Step timed out after ${timeoutMs}ms`));
      }
    }, timeoutMs);

    // Prevent timer from keeping the process alive
    if (timer.unref) timer.unref();

    fn()
      .then((result) => {
        if (!settled) {
          settled = true;
          clearTimeout(timer);
          resolve(result);
        }
      })
      .catch((err) => {
        if (!settled) {
          settled = true;
          clearTimeout(timer);
          reject(err);
        }
      });
  });
}

/**
 * Retry an async function up to `maxRetries` times with exponential backoff.
 *
 * @param {Function} fn - Async function to execute
 * @param {number} maxRetries - Maximum retry attempts (0 = no retries)
 * @returns {Promise<*>} Result of fn
 */
async function withRetries(fn, maxRetries) {
  let lastError;
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await fn();
    } catch (err) {
      lastError = err;
      if (attempt < maxRetries) {
        const delay = Math.min(1000 * Math.pow(2, attempt), 10_000);
        await new Promise((r) => setTimeout(r, delay));
      }
    }
  }
  throw lastError;
}

/**
 * Format a saga state for external consumers (camelCase).
 *
 * @param {Object} saga - Internal saga state
 * @returns {Object} Formatted saga
 */
function formatSaga(saga) {
  return {
    sagaId: saga.sagaId,
    name: saga.name,
    status: saga.status,
    steps: saga.steps.map((s) => ({
      name: s.name,
      status: s.status,
      result: s.result ?? null,
      error: s.error ?? null,
      startedAt: s.startedAt ?? null,
      completedAt: s.completedAt ?? null,
    })),
    context: saga.context,
    startedAt: saga.startedAt,
    completedAt: saga.completedAt,
    error: saga.error,
  };
}

// ---------------------------------------------------------------------------
// Saga Orchestrator
// ---------------------------------------------------------------------------

/**
 * Create a saga orchestrator instance.
 *
 * @param {Object} [store] - Optional external store (unused -- in-memory Map)
 * @param {Object} [services] - Service dependencies available to saga steps
 * @returns {Object} Saga orchestrator API
 */
export function createSagaOrchestrator(store, services = {}) {
  /** @type {Map<string, Object>} */
  const _sagas = new Map();
  const emitter = new EventEmitter();

  /**
   * Execute a saga definition.
   *
   * If a saga with the given `sagaId` (from context) already exists and has
   * completed steps, those steps are skipped (idempotency). If no sagaId is
   * provided in context, one is generated.
   *
   * @param {Object} sagaDefinition - Saga template
   * @param {string} sagaDefinition.name - Saga name
   * @param {Object[]} sagaDefinition.steps - Step definitions
   * @param {Object} [context={}] - Execution context passed through steps
   * @returns {Promise<Object>} Final saga state
   */
  async function execute(sagaDefinition, context = {}) {
    if (!sagaDefinition || !sagaDefinition.name) {
      throw new Error('sagaDefinition.name is required');
    }
    if (!Array.isArray(sagaDefinition.steps) || sagaDefinition.steps.length === 0) {
      throw new Error('sagaDefinition.steps must be a non-empty array');
    }

    const sagaId = context.sagaId || randomUUID();
    const now = new Date().toISOString();

    // Check for existing saga (idempotency)
    let saga = _sagas.get(sagaId);
    if (!saga) {
      saga = {
        sagaId,
        name: sagaDefinition.name,
        status: SAGA_STATUS.PENDING,
        steps: sagaDefinition.steps.map((step) => ({
          name: step.name,
          status: STEP_STATUS.PENDING,
          result: null,
          error: null,
          startedAt: null,
          completedAt: null,
          // Keep references to execute/compensate functions
          _execute: step.execute,
          _compensate: step.compensate,
          _timeoutMs: step.timeoutMs ?? DEFAULT_STEP_TIMEOUT_MS,
          _retries: step.retries ?? DEFAULT_STEP_RETRIES,
        })),
        context: { ...context, sagaId, services },
        startedAt: now,
        completedAt: null,
        error: null,
      };
      _sagas.set(sagaId, saga);
    } else {
      // Re-executing existing saga -- update context with new values
      saga.context = { ...saga.context, ...context, sagaId, services };
    }

    // Transition to running
    saga.status = SAGA_STATUS.RUNNING;
    emitter.emit('saga_started', { sagaId, name: saga.name });

    // Execute steps sequentially
    let failedStepIndex = -1;

    for (let i = 0; i < saga.steps.length; i++) {
      const step = saga.steps[i];

      // Idempotency: skip already-completed steps
      if (step.status === STEP_STATUS.COMPLETED) {
        emitter.emit('step_skipped', { sagaId, step: step.name, index: i });
        continue;
      }

      step.status = STEP_STATUS.RUNNING;
      step.startedAt = new Date().toISOString();
      emitter.emit('step_started', { sagaId, step: step.name, index: i });

      try {
        const result = await withRetries(
          () => withTimeout(() => step._execute(saga.context), step._timeoutMs),
          step._retries,
        );

        step.status = STEP_STATUS.COMPLETED;
        step.result = result ?? null;
        step.completedAt = new Date().toISOString();

        // Enrich context with step result for downstream steps
        saga.context[step.name] = result ?? null;

        emitter.emit('step_completed', {
          sagaId,
          step: step.name,
          index: i,
          result: step.result,
        });
      } catch (err) {
        const isTimeout = err.message && err.message.includes('timed out');
        step.status = isTimeout ? STEP_STATUS.TIMED_OUT : STEP_STATUS.FAILED;
        step.error = err.message || String(err);
        step.completedAt = new Date().toISOString();

        emitter.emit('step_failed', {
          sagaId,
          step: step.name,
          index: i,
          error: step.error,
          timedOut: isTimeout,
        });

        failedStepIndex = i;
        break;
      }
    }

    // If all steps completed, mark saga as completed
    if (failedStepIndex === -1) {
      saga.status = SAGA_STATUS.COMPLETED;
      saga.completedAt = new Date().toISOString();
      emitter.emit('saga_completed', { sagaId, name: saga.name });
      return formatSaga(saga);
    }

    // Failure path: compensate in reverse order
    saga.status = SAGA_STATUS.COMPENSATING;
    saga.error = saga.steps[failedStepIndex].error;
    emitter.emit('saga_compensating', {
      sagaId,
      name: saga.name,
      failedStep: saga.steps[failedStepIndex].name,
      error: saga.error,
    });

    await _compensateSteps(saga, failedStepIndex);

    return formatSaga(saga);
  }

  /**
   * Compensate completed steps in reverse order, starting from the step
   * before the failed step.
   *
   * @param {Object} saga - Saga state
   * @param {number} failedStepIndex - Index of the step that failed
   */
  async function _compensateSteps(saga, failedStepIndex) {
    let allCompensated = true;

    // Walk backwards from the step before the failed one
    for (let i = failedStepIndex - 1; i >= 0; i--) {
      const step = saga.steps[i];

      // Only compensate completed steps
      if (step.status !== STEP_STATUS.COMPLETED) {
        continue;
      }

      if (!step._compensate) {
        // No compensate function -- mark as compensated anyway
        step.status = STEP_STATUS.COMPENSATED;
        continue;
      }

      step.status = STEP_STATUS.COMPENSATING;
      emitter.emit('step_compensating', {
        sagaId: saga.sagaId,
        step: step.name,
        index: i,
      });

      try {
        await step._compensate(saga.context, step.result);
        step.status = STEP_STATUS.COMPENSATED;
        emitter.emit('step_compensated', {
          sagaId: saga.sagaId,
          step: step.name,
          index: i,
        });
      } catch (err) {
        step.status = STEP_STATUS.COMPENSATION_FAILED;
        step.error = `Compensation failed: ${err.message || String(err)}`;
        allCompensated = false;
        emitter.emit('step_compensation_failed', {
          sagaId: saga.sagaId,
          step: step.name,
          index: i,
          error: step.error,
        });
        // Continue compensating remaining steps even if one fails
      }
    }

    if (allCompensated) {
      saga.status = SAGA_STATUS.COMPENSATED;
      emitter.emit('saga_compensated', { sagaId: saga.sagaId, name: saga.name });
    } else {
      saga.status = SAGA_STATUS.FAILED;
      emitter.emit('saga_failed', {
        sagaId: saga.sagaId,
        name: saga.name,
        error: saga.error,
        compensationErrors: saga.steps
          .filter((s) => s.status === STEP_STATUS.COMPENSATION_FAILED)
          .map((s) => ({ step: s.name, error: s.error })),
      });
    }

    saga.completedAt = new Date().toISOString();
  }

  /**
   * Get the current status of a saga by ID.
   *
   * @param {string} sagaId - Saga ID
   * @returns {Object|null} Formatted saga state or null
   */
  function getStatus(sagaId) {
    const saga = _sagas.get(sagaId);
    if (!saga) return null;
    return formatSaga(saga);
  }

  /**
   * List sagas, optionally filtered by status and/or name.
   *
   * @param {Object} [filter={}] - Filter criteria
   * @param {string} [filter.status] - Filter by saga status
   * @param {string} [filter.name] - Filter by saga name
   * @returns {Object[]} Array of formatted saga states
   */
  function listSagas(filter = {}) {
    const results = [];
    for (const saga of _sagas.values()) {
      if (filter.status && saga.status !== filter.status) continue;
      if (filter.name && saga.name !== filter.name) continue;
      results.push(formatSaga(saga));
    }
    return results;
  }

  /**
   * Cancel a running saga. Triggers compensation for any completed steps.
   *
   * @param {string} sagaId - Saga ID
   * @returns {Promise<Object>} Final saga state after cancellation
   */
  async function cancelSaga(sagaId) {
    const saga = _sagas.get(sagaId);
    if (!saga) {
      throw new Error(`Saga not found: ${sagaId}`);
    }

    if (saga.status === SAGA_STATUS.COMPLETED) {
      throw new Error('Cannot cancel a completed saga');
    }
    if (saga.status === SAGA_STATUS.COMPENSATED) {
      throw new Error('Saga is already compensated');
    }
    if (saga.status === SAGA_STATUS.CANCELLED) {
      throw new Error('Saga is already cancelled');
    }

    saga.status = SAGA_STATUS.COMPENSATING;
    saga.error = 'Cancelled by user';
    emitter.emit('saga_cancelling', { sagaId, name: saga.name });

    // Find the last completed step to use as the "failed" index
    let lastCompletedIndex = -1;
    for (let i = saga.steps.length - 1; i >= 0; i--) {
      if (saga.steps[i].status === STEP_STATUS.COMPLETED) {
        lastCompletedIndex = i;
        break;
      }
    }

    // Mark any pending/running steps as skipped
    for (const step of saga.steps) {
      if (step.status === STEP_STATUS.PENDING || step.status === STEP_STATUS.RUNNING) {
        step.status = STEP_STATUS.SKIPPED;
      }
    }

    if (lastCompletedIndex >= 0) {
      // Compensate from the step after the last completed one
      await _compensateSteps(saga, lastCompletedIndex + 1);
    }

    // If compensation set status, keep it; otherwise mark cancelled
    if (saga.status === SAGA_STATUS.COMPENSATING) {
      saga.status = SAGA_STATUS.CANCELLED;
      saga.completedAt = new Date().toISOString();
    }

    emitter.emit('saga_cancelled', { sagaId, name: saga.name });
    return formatSaga(saga);
  }

  return {
    execute,
    getStatus,
    listSagas,
    cancelSaga,
    on: emitter.on.bind(emitter),
    off: emitter.removeListener.bind(emitter),
  };
}

// ---------------------------------------------------------------------------
// Pre-built Saga Templates
// ---------------------------------------------------------------------------

/**
 * PURCHASE_SAGA -- Full purchase flow
 *
 * Steps: requestQuote -> acceptQuote -> createEscrow -> fundEscrow
 *        -> awaitFulfillment -> releaseEscrow -> rateAgent
 */
export const PURCHASE_SAGA = {
  name: 'purchase',
  steps: [
    {
      name: 'request_quote',
      execute: async (ctx) => {
        const { services, sellerAddress, items, asset, network } = ctx;
        if (!services.a2a) throw new Error('a2a service required');
        return services.a2a.requestQuote({
          seller: sellerAddress,
          items: items || [{ description: 'Purchase item', quantity: 1 }],
          asset,
          network,
        });
      },
      compensate: async (ctx, result) => {
        // Decline the quote if it was created
        if (result?.quote?.id && ctx.services.a2a) {
          await ctx.services.a2a.declineQuote(result.quote.id, 'Saga cancelled');
        }
      },
      timeoutMs: 30_000,
      retries: 1,
    },
    {
      name: 'accept_quote',
      execute: async (ctx) => {
        const quoteResult = ctx.request_quote;
        if (!quoteResult?.quote?.id) throw new Error('No quote ID from previous step');
        return ctx.services.a2a.acceptQuote(quoteResult.quote.id);
      },
      compensate: async () => {
        // Payment was made -- refund handled by escrow in later steps
      },
      timeoutMs: 30_000,
      retries: 1,
    },
    {
      name: 'create_escrow',
      execute: async (ctx) => {
        const { services, sellerAddress, asset, network } = ctx;
        const quoteResult = ctx.request_quote;
        const total = quoteResult?.quote?.total || ctx.amount || 0;
        return services.a2a.createConditionalPayment({
          sellerAddress,
          amount: total,
          asset,
          network,
          quoteId: quoteResult?.quote?.id,
          conditions: [{ type: 'seller_fulfilled', quoteId: quoteResult?.quote?.id }],
        });
      },
      compensate: async (ctx, result) => {
        // Refund the escrow
        if (result?.escrow?.id && ctx.services.escrow) {
          await ctx.services.escrow.refundEscrow(result.escrow.id);
        }
      },
      timeoutMs: 30_000,
      retries: 2,
    },
    {
      name: 'fund_escrow',
      execute: async (ctx) => {
        const escrowResult = ctx.create_escrow;
        if (!escrowResult?.escrow?.id) throw new Error('No escrow ID from previous step');
        // Escrow was already funded during creation in createConditionalPayment
        return { escrowId: escrowResult.escrow.id, status: 'funded' };
      },
      compensate: async () => {
        // Refund handled by create_escrow compensate
      },
      timeoutMs: 15_000,
      retries: 0,
    },
    {
      name: 'await_fulfillment',
      execute: async (ctx) => {
        const escrowResult = ctx.create_escrow;
        if (!escrowResult?.escrow?.id) throw new Error('No escrow ID');
        const conditions = await ctx.services.a2a.checkPaymentConditions(escrowResult.escrow.id);
        return { conditions, fulfilled: conditions.allMet };
      },
      compensate: async () => {
        // Nothing to undo -- fulfillment is a check
      },
      timeoutMs: 60_000,
      retries: 2,
    },
    {
      name: 'release_escrow',
      execute: async (ctx) => {
        const escrowResult = ctx.create_escrow;
        if (!escrowResult?.escrow?.id) throw new Error('No escrow ID');
        return ctx.services.a2a.settleConditionalPayment(escrowResult.escrow.id);
      },
      compensate: async () => {
        // Cannot un-release escrow -- funds are transferred
        // This is the point of no return
      },
      timeoutMs: 30_000,
      retries: 1,
    },
    {
      name: 'rate_agent',
      execute: async (ctx) => {
        const { services, sellerAddress } = ctx;
        if (services.reputation) {
          return services.reputation.rateAgent({
            agentAddress: sellerAddress,
            score: 5,
            comment: 'Transaction completed via saga',
          });
        }
        return { skipped: true, reason: 'No reputation service' };
      },
      compensate: async () => {
        // Rating can remain even if later steps fail
      },
      timeoutMs: 10_000,
      retries: 0,
    },
  ],
};

/**
 * SUBSCRIPTION_SAGA -- Subscription creation flow
 *
 * Steps: createSubscription -> processFirstBilling -> activateService
 */
export const SUBSCRIPTION_SAGA = {
  name: 'subscription',
  steps: [
    {
      name: 'create_subscription',
      execute: async (ctx) => {
        const { services, planId, subscriberAddress } = ctx;
        if (!services.subscriptions) throw new Error('subscriptions service required');
        return services.subscriptions.createSubscription({
          planId,
          subscriberAddress,
        });
      },
      compensate: async (ctx, result) => {
        if (result?.subscription?.id && ctx.services.subscriptions) {
          await ctx.services.subscriptions.cancelSubscription(result.subscription.id);
        }
      },
      timeoutMs: 30_000,
      retries: 1,
    },
    {
      name: 'process_first_billing',
      execute: async (ctx) => {
        const subResult = ctx.create_subscription;
        if (!subResult?.subscription?.id) throw new Error('No subscription ID');
        const { services } = ctx;
        if (!services.billing) throw new Error('billing service required');
        return services.billing.processPayment({
          subscriptionId: subResult.subscription.id,
          amount: subResult.subscription.amount || ctx.amount,
          asset: subResult.subscription.asset || ctx.asset,
        });
      },
      compensate: async (ctx, result) => {
        if (result?.paymentId && ctx.services.billing) {
          await ctx.services.billing.refundPayment(result.paymentId);
        }
      },
      timeoutMs: 30_000,
      retries: 2,
    },
    {
      name: 'activate_service',
      execute: async (ctx) => {
        const subResult = ctx.create_subscription;
        if (!subResult?.subscription?.id) throw new Error('No subscription ID');
        const { services } = ctx;
        if (!services.subscriptions) throw new Error('subscriptions service required');
        return services.subscriptions.activateSubscription(subResult.subscription.id);
      },
      compensate: async (ctx, result) => {
        if (result?.subscriptionId && ctx.services.subscriptions) {
          await ctx.services.subscriptions.deactivateSubscription(result.subscriptionId);
        }
      },
      timeoutMs: 15_000,
      retries: 1,
    },
  ],
};

/**
 * RFQ_SAGA -- Request for Quotation flow
 *
 * Steps: broadcastRFQ -> collectResponses -> awardWinner -> createEscrow
 *        -> executePayment
 */
export const RFQ_SAGA = {
  name: 'rfq',
  steps: [
    {
      name: 'broadcast_rfq',
      execute: async (ctx) => {
        const { services, items, deadlineMinutes, scoringCriteria } = ctx;
        if (!services.marketplace) throw new Error('marketplace service required');
        return services.marketplace.broadcastRFQ({
          items: items || [],
          deadlineMinutes: deadlineMinutes || 30,
          scoringCriteria: scoringCriteria || 'best_value',
        });
      },
      compensate: async (ctx, result) => {
        if (result?.rfqId && ctx.services.marketplace) {
          await ctx.services.marketplace.cancelRFQ(result.rfqId);
        }
      },
      timeoutMs: 30_000,
      retries: 1,
    },
    {
      name: 'collect_responses',
      execute: async (ctx) => {
        const rfqResult = ctx.broadcast_rfq;
        if (!rfqResult?.rfqId) throw new Error('No RFQ ID from previous step');
        const { services } = ctx;
        return services.marketplace.collectResponses(rfqResult.rfqId);
      },
      compensate: async () => {
        // Responses are informational -- no undo needed
      },
      timeoutMs: 60_000,
      retries: 2,
    },
    {
      name: 'award_winner',
      execute: async (ctx) => {
        const rfqResult = ctx.broadcast_rfq;
        if (!rfqResult?.rfqId) throw new Error('No RFQ ID');
        const { services } = ctx;
        return services.marketplace.awardWinner(rfqResult.rfqId);
      },
      compensate: async (ctx, result) => {
        if (result?.rfqId && ctx.services.marketplace) {
          await ctx.services.marketplace.revokeAward(result.rfqId);
        }
      },
      timeoutMs: 30_000,
      retries: 1,
    },
    {
      name: 'create_escrow',
      execute: async (ctx) => {
        const awardResult = ctx.award_winner;
        if (!awardResult?.winnerAddress) throw new Error('No winner address');
        const { services, asset, network } = ctx;
        if (!services.a2a) throw new Error('a2a service required');
        return services.a2a.createConditionalPayment({
          sellerAddress: awardResult.winnerAddress,
          amount: awardResult.amount || ctx.amount || 0,
          asset,
          network,
          conditions: [{ type: 'seller_fulfilled' }],
        });
      },
      compensate: async (ctx, result) => {
        if (result?.escrow?.id && ctx.services.escrow) {
          await ctx.services.escrow.refundEscrow(result.escrow.id);
        }
      },
      timeoutMs: 30_000,
      retries: 2,
    },
    {
      name: 'execute_payment',
      execute: async (ctx) => {
        const escrowResult = ctx.create_escrow;
        if (!escrowResult?.escrow?.id) throw new Error('No escrow ID');
        const { services } = ctx;
        return services.a2a.settleConditionalPayment(escrowResult.escrow.id);
      },
      compensate: async () => {
        // Payment settled -- cannot reverse on-chain
      },
      timeoutMs: 30_000,
      retries: 1,
    },
  ],
};

export default {
  createSagaOrchestrator,
  PURCHASE_SAGA,
  SUBSCRIPTION_SAGA,
  RFQ_SAGA,
};
