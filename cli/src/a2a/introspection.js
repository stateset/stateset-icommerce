/**
 * A2A Agent Introspection Service
 *
 * Gives agents the ability to query their own operational state,
 * decision history, and performance metrics. All data is stored
 * in-memory (Maps) -- no SQLite dependency.
 *
 * @example
 * ```javascript
 * const introspection = createIntrospectionService();
 *
 * introspection.recordDecision({
 *   agentAddress: '0xAgent',
 *   type: 'quote_eval',
 *   action: 'accept',
 *   reason: 'Price below budget threshold',
 *   context: { quoteId: 'q-1', amount: 50 },
 * });
 *
 * introspection.recordTick({
 *   agentAddress: '0xAgent',
 *   durationMs: 120,
 *   quotesEvaluated: 5,
 *   quotesAccepted: 2,
 *   quotesRejected: 3,
 *   paymentsExecuted: 1,
 *   errors: 0,
 * });
 *
 * const dashboard = introspection.getAgentDashboard('0xAgent');
 * ```
 */

/** Valid decision types */
const DECISION_TYPES = ['quote_eval', 'payment', 'strategy_change', 'budget_check'];

/** Valid decision actions */
const DECISION_ACTIONS = ['accept', 'reject', 'skip'];

/**
 * Create an in-memory introspection service instance.
 *
 * @returns {Object} Introspection service API
 */
export function createIntrospectionService() {
  /**
   * Decision log entries keyed by agent address.
   * @type {Map<string, Array<Object>>}
   */
  const _decisions = new Map();

  /**
   * Tick metrics entries keyed by agent address.
   * @type {Map<string, Array<Object>>}
   */
  const _ticks = new Map();

  /**
   * Lifecycle state transitions keyed by agent address.
   * @type {Map<string, Array<Object>>}
   */
  const _lifecycles = new Map();

  // ---------------------------------------------------------------------------
  // Decision tracking
  // ---------------------------------------------------------------------------

  /**
   * Record a strategy/runtime decision with reason.
   *
   * @param {Object} decision
   * @param {string} decision.agentAddress - Agent wallet address (required)
   * @param {string} decision.type - Decision type: quote_eval|payment|strategy_change|budget_check
   * @param {string} decision.action - Action taken: accept|reject|skip
   * @param {string} decision.reason - Human-readable reason for the decision
   * @param {Object} [decision.context] - Additional context (quoteId, amount, etc.)
   * @returns {Object} Stored decision entry
   */
  function recordDecision(decision) {
    if (!decision || !decision.agentAddress) {
      throw new Error('agentAddress is required');
    }
    if (!decision.type) {
      throw new Error('type is required');
    }
    if (!decision.action) {
      throw new Error('action is required');
    }

    const entry = {
      agentAddress: decision.agentAddress,
      timestamp: new Date().toISOString(),
      type: decision.type,
      action: decision.action,
      reason: decision.reason || null,
      context: decision.context || null,
    };

    if (!_decisions.has(decision.agentAddress)) {
      _decisions.set(decision.agentAddress, []);
    }
    _decisions.get(decision.agentAddress).push(entry);

    return entry;
  }

  /**
   * Get recent decision history for an agent.
   *
   * @param {string} agentAddress - Agent wallet address
   * @param {number} [limit=50] - Maximum entries to return (most recent first)
   * @returns {Array<Object>} Decision history (newest first)
   */
  function getDecisionHistory(agentAddress, limit = 50) {
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }

    const entries = _decisions.get(agentAddress) || [];
    // Return most recent first, capped at limit
    return entries.slice(-limit).reverse();
  }

  // ---------------------------------------------------------------------------
  // Tick metrics
  // ---------------------------------------------------------------------------

  /**
   * Record tick cycle metrics for an agent.
   *
   * @param {Object} tickResult
   * @param {string} tickResult.agentAddress - Agent wallet address (required)
   * @param {number} tickResult.durationMs - Tick duration in milliseconds
   * @param {number} [tickResult.quotesEvaluated=0] - Quotes evaluated this tick
   * @param {number} [tickResult.quotesAccepted=0] - Quotes accepted this tick
   * @param {number} [tickResult.quotesRejected=0] - Quotes rejected this tick
   * @param {number} [tickResult.paymentsExecuted=0] - Payments executed this tick
   * @param {number} [tickResult.errors=0] - Errors encountered this tick
   * @returns {Object} Stored tick entry
   */
  function recordTick(tickResult) {
    if (!tickResult || !tickResult.agentAddress) {
      throw new Error('agentAddress is required');
    }
    if (tickResult.durationMs === undefined || tickResult.durationMs === null) {
      throw new Error('durationMs is required');
    }

    const entry = {
      agentAddress: tickResult.agentAddress,
      timestamp: new Date().toISOString(),
      durationMs: tickResult.durationMs,
      quotesEvaluated: tickResult.quotesEvaluated || 0,
      quotesAccepted: tickResult.quotesAccepted || 0,
      quotesRejected: tickResult.quotesRejected || 0,
      paymentsExecuted: tickResult.paymentsExecuted || 0,
      errors: tickResult.errors || 0,
    };

    if (!_ticks.has(tickResult.agentAddress)) {
      _ticks.set(tickResult.agentAddress, []);
    }
    _ticks.get(tickResult.agentAddress).push(entry);

    return entry;
  }

  /**
   * Get aggregated tick metrics for an agent.
   *
   * @param {string} agentAddress - Agent wallet address
   * @returns {Object} Tick metrics summary
   */
  function getTickMetrics(agentAddress) {
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }

    const entries = _ticks.get(agentAddress) || [];

    if (entries.length === 0) {
      return {
        avgTickDurationMs: 0,
        ticksPerMinute: 0,
        totalTicks: 0,
        quotesEvaluated: 0,
        paymentsExecuted: 0,
        errorsCount: 0,
      };
    }

    const totalDuration = entries.reduce((sum, e) => sum + e.durationMs, 0);
    const totalQuotesEvaluated = entries.reduce((sum, e) => sum + e.quotesEvaluated, 0);
    const totalPaymentsExecuted = entries.reduce((sum, e) => sum + e.paymentsExecuted, 0);
    const totalErrors = entries.reduce((sum, e) => sum + e.errors, 0);

    // Compute ticks per minute based on time span
    let ticksPerMinute = 0;
    if (entries.length >= 2) {
      const firstTs = new Date(entries[0].timestamp).getTime();
      const lastTs = new Date(entries[entries.length - 1].timestamp).getTime();
      const spanMinutes = (lastTs - firstTs) / 60_000;
      ticksPerMinute = spanMinutes > 0 ? Math.round((entries.length / spanMinutes) * 100) / 100 : 0;
    }

    return {
      avgTickDurationMs: Math.round(totalDuration / entries.length),
      ticksPerMinute,
      totalTicks: entries.length,
      quotesEvaluated: totalQuotesEvaluated,
      paymentsExecuted: totalPaymentsExecuted,
      errorsCount: totalErrors,
    };
  }

  // ---------------------------------------------------------------------------
  // Lifecycle tracking
  // ---------------------------------------------------------------------------

  /**
   * Record an agent lifecycle state transition.
   *
   * @param {string} agentAddress - Agent wallet address
   * @param {string} fromState - Previous state
   * @param {string} toState - New state
   * @param {string} [reason] - Reason for the transition
   * @returns {Object} Stored transition entry
   */
  function recordStateTransition(agentAddress, fromState, toState, reason) {
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }
    if (!fromState) {
      throw new Error('fromState is required');
    }
    if (!toState) {
      throw new Error('toState is required');
    }

    const entry = {
      agentAddress,
      timestamp: new Date().toISOString(),
      fromState,
      toState,
      reason: reason || null,
    };

    if (!_lifecycles.has(agentAddress)) {
      _lifecycles.set(agentAddress, []);
    }
    _lifecycles.get(agentAddress).push(entry);

    return entry;
  }

  /**
   * Get lifecycle state transition history for an agent.
   *
   * @param {string} agentAddress - Agent wallet address
   * @returns {Array<Object>} State transition history (oldest first)
   */
  function getLifecycleHistory(agentAddress) {
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }

    return _lifecycles.get(agentAddress) || [];
  }

  // ---------------------------------------------------------------------------
  // Dashboards & reports
  // ---------------------------------------------------------------------------

  /**
   * Get a full operational dashboard for an agent.
   *
   * Returns a composite view of runtime status, tick metrics,
   * decision summary, and lifecycle info.
   *
   * @param {string} agentAddress - Agent wallet address
   * @returns {Object} Agent operational dashboard
   */
  function getAgentDashboard(agentAddress) {
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }

    const tickMetrics = getTickMetrics(agentAddress);
    const decisions = _decisions.get(agentAddress) || [];
    const ticks = _ticks.get(agentAddress) || [];
    const lifecycle = _lifecycles.get(agentAddress) || [];

    // Decision summary: count by type and action
    const decisionSummary = {};
    for (const d of decisions) {
      const key = `${d.type}:${d.action}`;
      decisionSummary[key] = (decisionSummary[key] || 0) + 1;
    }

    // Determine current lifecycle state
    const currentState = lifecycle.length > 0 ? lifecycle[lifecycle.length - 1].toState : 'unknown';

    // Last tick timestamp
    const lastTickAt = ticks.length > 0 ? ticks[ticks.length - 1].timestamp : null;

    return {
      agentAddress,
      runtimeStatus: currentState,
      lastTickAt,
      tickMetrics,
      decisionSummary,
      totalDecisions: decisions.length,
      lifecycleTransitions: lifecycle.length,
      currentState,
    };
  }

  /**
   * Get a performance report for an agent.
   *
   * Computes operational rates from recorded decisions and ticks.
   *
   * @param {string} agentAddress - Agent wallet address
   * @returns {Object} Performance report
   */
  function getPerformanceReport(agentAddress) {
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }

    const decisions = _decisions.get(agentAddress) || [];
    const ticks = _ticks.get(agentAddress) || [];

    // Quote accept rate = accepted / (accepted + rejected)
    const quoteDecisions = decisions.filter((d) => d.type === 'quote_eval');
    const accepted = quoteDecisions.filter((d) => d.action === 'accept').length;
    const rejected = quoteDecisions.filter((d) => d.action === 'reject').length;
    const quoteAcceptRate =
      accepted + rejected > 0 ? Math.round((accepted / (accepted + rejected)) * 10000) / 10000 : 0;

    // Average tick duration as a proxy for response time
    const totalDuration = ticks.reduce((sum, e) => sum + e.durationMs, 0);
    const avgResponseTimeMs = ticks.length > 0 ? Math.round(totalDuration / ticks.length) : 0;

    // Payment success rate from payment decisions
    const paymentDecisions = decisions.filter((d) => d.type === 'payment');
    const paymentSuccesses = paymentDecisions.filter((d) => d.action === 'accept').length;
    const paymentFailures = paymentDecisions.filter((d) => d.action === 'reject').length;
    const settlementSuccessRate =
      paymentSuccesses + paymentFailures > 0
        ? Math.round((paymentSuccesses / (paymentSuccesses + paymentFailures)) * 10000) / 10000
        : 1;

    // Dispute rate from total decisions (budget_check rejects as proxy)
    const totalTransactions = quoteDecisions.length + paymentDecisions.length;
    const disputeProxies = decisions.filter(
      (d) => d.type === 'budget_check' && d.action === 'reject',
    ).length;
    const disputeRate =
      totalTransactions > 0 ? Math.round((disputeProxies / totalTransactions) * 10000) / 10000 : 0;

    // Uptime percent based on error-free ticks
    const errorFreeTicks = ticks.filter((t) => t.errors === 0).length;
    const uptimePercent =
      ticks.length > 0 ? Math.round((errorFreeTicks / ticks.length) * 10000) / 100 : 100;

    return {
      agentAddress,
      quoteAcceptRate,
      avgResponseTimeMs,
      settlementSuccessRate,
      disputeRate,
      uptimePercent,
    };
  }

  // ---------------------------------------------------------------------------
  // Data management
  // ---------------------------------------------------------------------------

  /**
   * Clear all introspection data for a specific agent.
   *
   * @param {string} agentAddress - Agent wallet address
   */
  function clear(agentAddress) {
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }

    _decisions.delete(agentAddress);
    _ticks.delete(agentAddress);
    _lifecycles.delete(agentAddress);
  }

  return {
    // Decision tracking
    recordDecision,
    getDecisionHistory,

    // Tick metrics
    recordTick,
    getTickMetrics,

    // Lifecycle tracking
    recordStateTransition,
    getLifecycleHistory,

    // Dashboards & reports
    getAgentDashboard,
    getPerformanceReport,

    // Data management
    clear,
  };
}

export { DECISION_TYPES, DECISION_ACTIONS };

export default { createIntrospectionService };
