/**
 * A2A Integration Layer — Smart Wrappers for Automatic Intelligence
 *
 * Wires all standalone A2A modules (memory, rules, idempotency, tracing,
 * cost analytics, introspection, scheduler) into the actual commerce flows.
 * An agent that calls `pay()` automatically gets idempotency, rule evaluation,
 * tracing, cost recording, memory, and introspection without manual wiring.
 *
 * The wrapper is transparent: it passes through ALL methods from coreA2A
 * unchanged except the wrapped ones. If any service is null/undefined, that
 * integration layer is silently skipped (graceful degradation).
 *
 * @example
 * ```javascript
 * import { createA2AService } from './index.js';
 * import { initializeServices, createIntegratedA2AService } from './integration.js';
 *
 * const coreA2A = createA2AService(commerce, agentConfig);
 * const services = initializeServices();
 * const a2a = createIntegratedA2AService(coreA2A, services);
 *
 * // Now every pay() call automatically applies all intelligence layers
 * await a2a.pay({ to: '0xSeller', amount: 100, memo: 'Widget purchase' });
 * ```
 */

import { randomUUID } from 'node:crypto';
import { createAgentMemory } from './agent-memory.js';
import { createRulesEngine } from './rules-engine.js';
import { createIdempotencyGuard } from './idempotency.js';
import { createTracingService } from './tracing.js';
import { createCostAnalytics } from './cost-analytics.js';
import { createIntrospectionService } from './introspection.js';
import { createSchedulerService } from './scheduler.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Safely call a function if the service exists.
 * Returns undefined if the service is null/undefined.
 *
 * @param {*} service
 * @param {string} method
 * @param {...*} args
 * @returns {*}
 */
function safeCall(service, method, ...args) {
  if (service === null || service === undefined || typeof service[method] !== 'function') {
    return undefined;
  }
  try {
    return service[method](...args);
  } catch {
    // Graceful degradation: intelligence layer failures should not
    // break core commerce operations.
    return undefined;
  }
}

/**
 * Safely call an async function if the service exists.
 *
 * @param {*} service
 * @param {string} method
 * @param {...*} args
 * @returns {Promise<*>}
 */
async function safeCallAsync(service, method, ...args) {
  if (service === null || service === undefined || typeof service[method] !== 'function') {
    return undefined;
  }
  try {
    return await service[method](...args);
  } catch {
    return undefined;
  }
}

// ---------------------------------------------------------------------------
// initializeServices
// ---------------------------------------------------------------------------

/**
 * Factory that creates ALL intelligence services at once.
 *
 * @param {Object} [options]
 * @param {Object} [options.idempotency] - Options for createIdempotencyGuard
 * @param {Object} [options.tracing]     - Options for createTracingService
 * @param {Object} [options.scheduler]   - Options for createSchedulerService
 * @returns {{
 *   memory: ReturnType<typeof createAgentMemory>,
 *   rules: ReturnType<typeof createRulesEngine>,
 *   idempotency: ReturnType<typeof createIdempotencyGuard>,
 *   tracing: ReturnType<typeof createTracingService>,
 *   costAnalytics: ReturnType<typeof createCostAnalytics>,
 *   introspection: ReturnType<typeof createIntrospectionService>,
 *   scheduler: ReturnType<typeof createSchedulerService>,
 * }}
 */
export function initializeServices(options = {}) {
  return {
    memory: createAgentMemory(),
    rules: createRulesEngine(),
    idempotency: createIdempotencyGuard(options.idempotency),
    tracing: createTracingService(options.tracing),
    costAnalytics: createCostAnalytics(),
    introspection: createIntrospectionService(),
    scheduler: createSchedulerService(options.scheduler),
  };
}

// ---------------------------------------------------------------------------
// createIntegratedA2AService
// ---------------------------------------------------------------------------

/**
 * Takes the existing A2A service and wraps its key methods with automatic
 * intelligence. The `services` parameter contains all initialized services.
 *
 * @param {Object} coreA2A - The A2A service from createA2AService()
 * @param {Object} services - Intelligence services (any can be null)
 * @param {Object} [services.memory]         - createAgentMemory()
 * @param {Object} [services.rules]          - createRulesEngine()
 * @param {Object} [services.idempotency]    - createIdempotencyGuard()
 * @param {Object} [services.tracing]        - createTracingService()
 * @param {Object} [services.costAnalytics]  - createCostAnalytics()
 * @param {Object} [services.introspection]  - createIntrospectionService()
 * @returns {Object} Enhanced A2A service with all original methods + intelligence
 */
export function createIntegratedA2AService(coreA2A, services = {}) {
  const { memory, rules, idempotency, tracing, costAnalytics, introspection } = services;

  // The agent address we use for recording. Pulled from coreA2A config.
  const agentAddress = coreA2A.walletAddress || 'unknown';

  // -------------------------------------------------------------------------
  // Wrapped pay()
  // -------------------------------------------------------------------------

  /**
   * Enhanced pay() with automatic idempotency, rule evaluation, tracing,
   * cost recording, memory, and introspection.
   *
   * @param {Object} params - Same as coreA2A.pay()
   * @returns {Promise<Object>} Payment result
   */
  async function pay(params) {
    const { to, amount, asset, memo, idempotencyKey } = params;

    const key = idempotencyKey || `a2a-integrated-pay-${randomUUID()}`;
    const recipientAddress =
      typeof to === 'string' ? to : to?.walletAddress || to?.wallet_address || 'unknown';

    // -- Rule evaluation --
    const ruleResult = safeCall(rules, 'evaluate', {
      amount,
      counterparty: recipientAddress,
      operationType: 'payment',
      asset,
    });

    if (ruleResult && !ruleResult.allowed) {
      // Record the blocked decision to introspection
      safeCall(introspection, 'recordDecision', {
        agentAddress,
        type: 'payment',
        action: 'reject',
        reason: ruleResult.explanation,
        context: { amount, to: recipientAddress, asset },
      });

      // Record failed memory interaction
      safeCall(memory, 'recordInteraction', {
        agentAddress,
        counterpartyAddress: recipientAddress,
        interactionType: 'payment_sent',
        outcome: 'rejected',
        amount,
      });

      throw new Error(`Payment blocked by rules: ${ruleResult.explanation}`);
    }

    // -- Execution with idempotency + tracing --
    const executeFn = async () => {
      const executePayment = async (span) => {
        if (span) {
          span.setAttribute('amount', amount);
          span.setAttribute('recipient', recipientAddress);
          span.setAttribute('asset', asset || 'USDC');
          if (memo) span.setAttribute('memo', memo);
        }

        const result = await coreA2A.pay(params);
        const paymentAsset = asset || result?.payment?.asset || null;
        const paymentNetwork = params.network || result?.payment?.network || null;

        // -- Cost analytics --
        safeCall(costAnalytics, 'record', {
          agentAddress,
          counterparty: recipientAddress,
          direction: 'spend',
          amount,
          asset: paymentAsset,
          network: paymentNetwork,
          operation: 'quote_payment',
          metadata: { memo, asset: paymentAsset, network: paymentNetwork },
        });

        // -- Memory --
        safeCall(memory, 'recordInteraction', {
          agentAddress,
          counterpartyAddress: recipientAddress,
          interactionType: 'payment_sent',
          outcome: 'success',
          amount,
          metadata: { asset, memo },
        });

        // -- Introspection --
        safeCall(introspection, 'recordDecision', {
          agentAddress,
          type: 'payment',
          action: 'accept',
          reason: ruleResult ? `Allowed: ${ruleResult.explanation}` : 'No rules engine configured',
          context: { amount, to: recipientAddress, asset },
        });

        return result;
      };

      // Wrap in tracing span if available
      if (tracing) {
        return tracing.withSpan('a2a.pay', executePayment, {
          attributes: {
            'a2a.operation': 'pay',
            'a2a.amount': amount,
            'a2a.recipient': recipientAddress,
            'a2a.asset': asset || 'USDC',
          },
        });
      }
      return executePayment(null);
    };

    // Wrap in idempotency guard if available
    if (idempotency) {
      try {
        return await idempotency.execute(key, executeFn);
      } catch (err) {
        // Record failure to memory
        safeCall(memory, 'recordInteraction', {
          agentAddress,
          counterpartyAddress: recipientAddress,
          interactionType: 'payment_sent',
          outcome: 'failure',
          amount,
          metadata: { error: err.message },
        });

        safeCall(introspection, 'recordDecision', {
          agentAddress,
          type: 'payment',
          action: 'reject',
          reason: `Payment failed: ${err.message}`,
          context: { amount, to: recipientAddress },
        });

        throw err;
      }
    }

    try {
      return await executeFn();
    } catch (err) {
      // Record failure to memory
      safeCall(memory, 'recordInteraction', {
        agentAddress,
        counterpartyAddress: recipientAddress,
        interactionType: 'payment_sent',
        outcome: 'failure',
        amount,
        metadata: { error: err.message },
      });

      safeCall(introspection, 'recordDecision', {
        agentAddress,
        type: 'payment',
        action: 'reject',
        reason: `Payment failed: ${err.message}`,
        context: { amount, to: recipientAddress },
      });

      throw err;
    }
  }

  // -------------------------------------------------------------------------
  // Wrapped acceptQuote()
  // -------------------------------------------------------------------------

  /**
   * Enhanced acceptQuote() with rule evaluation, memory intelligence,
   * tracing, cost recording, and introspection.
   *
   * @param {string} quoteId - Quote ID to accept
   * @returns {Promise<Object>} Acceptance result
   */
  async function acceptQuote(quoteId) {
    // We need quote details for intelligence. The core service fetches
    // the quote internally, but we need it for rule evaluation.
    // We'll pass through and capture the result.

    // Try to get counterparty info from memory if available
    const counterparty = 'unknown'; // Will be enriched from result
    let warning = null;

    const executeAccept = async (span) => {
      if (span) {
        span.setAttribute('quoteId', quoteId);
      }

      // -- Rule evaluation (with what we know pre-accept) --
      const ruleResult = safeCall(rules, 'evaluate', {
        operationType: 'accept_quote',
        quoteId,
      });

      if (ruleResult && !ruleResult.allowed) {
        safeCall(introspection, 'recordDecision', {
          agentAddress,
          type: 'quote_eval',
          action: 'reject',
          reason: ruleResult.explanation,
          context: { quoteId },
        });

        safeCall(memory, 'recordInteraction', {
          agentAddress,
          counterpartyAddress: counterparty,
          interactionType: 'quote_received',
          outcome: 'rejected',
          metadata: { quoteId, reason: ruleResult.explanation },
        });

        throw new Error(`Quote acceptance blocked by rules: ${ruleResult.explanation}`);
      }

      // Execute the core accept
      const result = await coreA2A.acceptQuote(quoteId);

      // Extract counterparty from result
      const sellerAddress = result.quote?.seller || result.quote?.sellerAddress || counterparty;
      const quoteAmount = result.quote?.total || 0;
      const quoteAsset = result.quote?.asset || result.payment?.asset || null;
      const quoteNetwork = result.quote?.network || result.payment?.network || null;

      if (span) {
        span.setAttribute('seller', sellerAddress);
        span.setAttribute('amount', quoteAmount);
      }

      // -- Memory: check counterparty profile --
      const profile = safeCall(memory, 'getCounterpartyProfile', agentAddress, sellerAddress);
      if (profile && profile.riskLevel === 'high') {
        warning = `WARNING: Counterparty ${sellerAddress} has high risk level (${profile.totalInteractions} interactions, ${Math.round(profile.successRate * 100)}% success rate)`;
      }

      // -- Memory: get recommendation --
      const recommendation = safeCall(
        memory,
        'getRecommendation',
        agentAddress,
        sellerAddress,
        'accept_quote',
      );

      // -- Cost analytics --
      if (quoteAmount > 0) {
        safeCall(costAnalytics, 'record', {
          agentAddress,
          counterparty: sellerAddress,
          direction: 'spend',
          amount: quoteAmount,
          asset: quoteAsset,
          network: quoteNetwork,
          operation: 'quote_payment',
          metadata: { quoteId, asset: quoteAsset, network: quoteNetwork },
        });
      }

      // -- Memory interaction --
      safeCall(memory, 'recordInteraction', {
        agentAddress,
        counterpartyAddress: sellerAddress,
        interactionType: 'quote_received',
        outcome: 'accepted',
        amount: quoteAmount,
        metadata: { quoteId },
      });

      // -- Introspection --
      safeCall(introspection, 'recordDecision', {
        agentAddress,
        type: 'quote_eval',
        action: 'accept',
        reason: recommendation
          ? `Accepted. Recommendation: ${recommendation.reason}`
          : 'Accepted (no recommendation data)',
        context: {
          quoteId,
          amount: quoteAmount,
          seller: sellerAddress,
          profile: profile
            ? { riskLevel: profile.riskLevel, successRate: profile.successRate }
            : null,
        },
      });

      // Enrich result with intelligence
      const enriched = { ...result };
      if (warning) {
        enriched.warning = warning;
      }
      if (recommendation) {
        enriched.recommendation = recommendation;
      }

      return enriched;
    };

    // Wrap in tracing if available
    if (tracing) {
      return tracing.withSpan('a2a.acceptQuote', executeAccept, {
        attributes: {
          'a2a.operation': 'acceptQuote',
          'a2a.quoteId': quoteId,
        },
      });
    }
    return executeAccept(null);
  }

  // -------------------------------------------------------------------------
  // Wrapped requestQuote()
  // -------------------------------------------------------------------------

  /**
   * Enhanced requestQuote() with tracing, memory, and introspection.
   *
   * @param {Object} params - Same as coreA2A.requestQuote()
   * @returns {Promise<Object>} Quote request result
   */
  async function requestQuote(params) {
    const { seller } = params;
    const sellerAddress = typeof seller === 'string' ? seller : seller?.walletAddress || 'unknown';

    const executeRequest = async (span) => {
      if (span) {
        span.setAttribute('seller', sellerAddress);
        if (params.items) {
          span.setAttribute('itemCount', params.items.length);
        }
      }

      const result = await coreA2A.requestQuote(params);

      // -- Memory --
      safeCall(memory, 'recordInteraction', {
        agentAddress,
        counterpartyAddress: sellerAddress,
        interactionType: 'quote_sent',
        outcome: 'success',
        metadata: { quoteId: result.quote?.id },
      });

      // -- Introspection --
      safeCall(introspection, 'recordDecision', {
        agentAddress,
        type: 'quote_eval',
        action: 'accept',
        reason: `Quote requested from ${sellerAddress}`,
        context: { seller: sellerAddress, quoteId: result.quote?.id },
      });

      return result;
    };

    // Wrap in tracing if available
    if (tracing) {
      return tracing.withSpan('a2a.requestQuote', executeRequest, {
        attributes: {
          'a2a.operation': 'requestQuote',
          'a2a.seller': sellerAddress,
        },
      });
    }
    return executeRequest(null);
  }

  // -------------------------------------------------------------------------
  // evaluateQuoteWithIntelligence
  // -------------------------------------------------------------------------

  /**
   * Enhanced quote evaluation for the agent runtime tick loop.
   * Enriches the strategy's evaluation with counterparty memory, rules, and
   * introspection recording.
   *
   * @param {Object} quote - Quote to evaluate
   * @param {Object} strategy - Agent strategy with evaluateQuote()
   * @param {string} [quoteAgentAddress] - Override agent address
   * @returns {{ action: string, reason: string, profile: Object|null, ruleResult: Object|null, recommendation: Object|null }}
   */
  function evaluateQuoteWithIntelligence(quote, strategy, quoteAgentAddress) {
    const addr = quoteAgentAddress || agentAddress;
    const sellerAddress = quote.seller || quote.seller_address || quote.sellerAddress || 'unknown';
    const quoteAmount = quote.total || quote.total_decimal || quote.amount || 0;

    // Get counterparty profile from memory
    const profile = safeCall(memory, 'getCounterpartyProfile', addr, sellerAddress);

    // Evaluate rules
    const ruleResult = safeCall(rules, 'evaluate', {
      amount: quoteAmount,
      counterparty: sellerAddress,
      operationType: 'accept_quote',
      quoteId: quote.id,
    });

    // If rules block, return decline immediately
    if (ruleResult && !ruleResult.allowed) {
      const decision = {
        action: 'decline',
        reason: ruleResult.explanation,
        profile: profile || null,
        ruleResult,
        recommendation: null,
      };

      safeCall(introspection, 'recordDecision', {
        agentAddress: addr,
        type: 'quote_eval',
        action: 'reject',
        reason: ruleResult.explanation,
        context: { quoteId: quote.id, amount: quoteAmount, seller: sellerAddress },
      });

      return decision;
    }

    // Get recommendation from memory
    const recommendation = safeCall(
      memory,
      'getRecommendation',
      addr,
      sellerAddress,
      'accept_quote',
    );

    // Build enriched context for strategy evaluation
    const enrichedContext = {
      profile: profile || null,
      ruleResult: ruleResult || null,
      recommendation: recommendation || null,
    };

    // Run strategy evaluation if available
    let strategyResult = { action: 'accept', reason: 'No strategy configured' };
    if (strategy && typeof strategy.evaluateQuote === 'function') {
      try {
        strategyResult = strategy.evaluateQuote(quote, enrichedContext);
      } catch {
        strategyResult = { action: 'skip', reason: 'Strategy evaluation failed' };
      }
    }

    const decision = {
      action: strategyResult.action || 'accept',
      reason: strategyResult.reason || 'Strategy approved',
      profile: profile || null,
      ruleResult: ruleResult || null,
      recommendation: recommendation || null,
    };

    // Record to introspection
    safeCall(introspection, 'recordDecision', {
      agentAddress: addr,
      type: 'quote_eval',
      action: decision.action === 'decline' ? 'reject' : decision.action,
      reason: decision.reason,
      context: {
        quoteId: quote.id,
        amount: quoteAmount,
        seller: sellerAddress,
        profileRisk: profile?.riskLevel,
        recommended: recommendation?.recommended,
      },
    });

    return decision;
  }

  // -------------------------------------------------------------------------
  // Build the proxy that passes through all other methods
  // -------------------------------------------------------------------------

  const wrapped = {
    pay,
    acceptQuote,
    requestQuote,
    evaluateQuoteWithIntelligence,
  };

  // Create a Proxy that intercepts property access. Wrapped methods take
  // priority; everything else falls through to coreA2A.
  const handler = {
    get(target, prop) {
      // Wrapped methods
      if (prop in wrapped) {
        return wrapped[prop];
      }
      // Pass through to core
      if (prop in coreA2A) {
        const value = coreA2A[prop];
        // Bind methods so they keep their `this` context
        if (typeof value === 'function') {
          return value.bind(coreA2A);
        }
        return value;
      }
      // Also expose services for direct access
      if (prop === '_services') {
        return services;
      }
      return undefined;
    },

    has(target, prop) {
      return prop in wrapped || prop in coreA2A || prop === '_services';
    },

    ownKeys() {
      return [...new Set([...Object.keys(wrapped), ...Object.keys(coreA2A), '_services'])];
    },

    getOwnPropertyDescriptor(target, prop) {
      if (prop in wrapped || prop in coreA2A || prop === '_services') {
        return { configurable: true, enumerable: true, writable: false };
      }
      return undefined;
    },
  };

  return new Proxy({}, handler);
}

export default { createIntegratedA2AService, initializeServices };
