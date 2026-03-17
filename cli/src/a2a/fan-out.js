/**
 * A2A Fan-Out/Join Coordinator — Multi-Agent Parallel Coordination
 *
 * Broadcasts a task to N agents in parallel, waits for results using a
 * configurable join strategy, and aggregates responses.
 *
 * Join strategies:
 *   - `all`         — Wait for all targets (or timeout)
 *   - `first`       — Return on first response
 *   - `majority`    — Return when >50% have responded
 *   - `quorum(n)`   — Return when N responses received
 *   - `best`        — Wait for all, return highest-scored response
 *
 * @example
 * ```javascript
 * const coordinator = createFanOutCoordinator();
 *
 * const coordId = coordinator.scatter({
 *   agentAddress: '0xRequester',
 *   targets: ['0xAgent1', '0xAgent2', '0xAgent3'],
 *   taskType: 'quote',
 *   payload: { items: [{ sku: 'WIDGET-001', quantity: 10 }] },
 *   timeoutMs: 30000,
 *   joinStrategy: 'majority',
 * });
 *
 * // Agents submit results
 * coordinator.registerResponse(coordId, '0xAgent1', { price: 99 });
 * coordinator.registerResponse(coordId, '0xAgent2', { price: 85 });
 *
 * // Join when strategy condition is met
 * const result = await coordinator.join(coordId);
 * ```
 */

import { randomUUID } from 'node:crypto';

// ── Join strategy helpers ──────────────────────────────────────────────────

/**
 * Parse a join strategy string into a structured descriptor.
 *
 * @param {string|Object} strategy
 * @returns {{ type: string, n?: number }}
 */
function parseStrategy(strategy) {
  if (!strategy) return { type: 'all' };

  if (typeof strategy === 'object' && strategy.type) {
    return strategy;
  }

  if (typeof strategy === 'string') {
    const quorumMatch = /^quorum\((\d+)\)$/.exec(strategy);
    if (quorumMatch) {
      return { type: 'quorum', n: parseInt(quorumMatch[1], 10) };
    }
    return { type: strategy };
  }

  return { type: 'all' };
}

/**
 * Check whether the join condition is satisfied.
 *
 * @param {{ type: string, n?: number }} strategy
 * @param {number} responseCount
 * @param {number} totalTargets
 * @returns {boolean}
 */
function isJoinSatisfied(strategy, responseCount, totalTargets) {
  switch (strategy.type) {
    case 'all':
      return responseCount >= totalTargets;
    case 'first':
      return responseCount >= 1;
    case 'majority':
      return responseCount > totalTargets / 2;
    case 'quorum':
      return responseCount >= (strategy.n || 1);
    case 'best':
      return responseCount >= totalTargets;
    default:
      return responseCount >= totalTargets;
  }
}

// ── Aggregation ────────────────────────────────────────────────────────────

/**
 * Aggregate responses based on task type.
 *
 * @param {string} taskType
 * @param {Object[]} responses
 * @param {{ type: string }} strategy
 * @returns {Object}
 */
function aggregateResponses(taskType, responses, strategy) {
  if (responses.length === 0) {
    return { type: 'empty', data: [] };
  }

  switch (taskType) {
    case 'quote': {
      // Sort by price ascending (cheapest first)
      const ranked = [...responses].sort((a, b) => {
        const priceA = a.response?.price ?? Infinity;
        const priceB = b.response?.price ?? Infinity;
        return priceA - priceB;
      });
      return {
        type: 'ranked_quotes',
        data: ranked,
        bestPrice: ranked[0]?.response?.price ?? null,
        bestResponder: ranked[0]?.responderAddress ?? null,
      };
    }

    case 'status': {
      // Merge into unified status object
      const merged = {};
      for (const r of responses) {
        if (r.response && typeof r.response === 'object') {
          merged[r.responderAddress] = r.response;
        }
      }
      return { type: 'merged_status', data: merged };
    }

    default: {
      // For 'best' strategy, return the highest-scored response
      if (strategy.type === 'best') {
        const sorted = [...responses].sort((a, b) => {
          const scoreA = a.response?.score ?? 0;
          const scoreB = b.response?.score ?? 0;
          return scoreB - scoreA;
        });
        return {
          type: 'best',
          data: sorted,
          winner: sorted[0] ?? null,
        };
      }

      // Raw responses for custom task types
      return { type: 'raw', data: responses };
    }
  }
}

// ── Fan-out coordinator factory ────────────────────────────────────────────

/**
 * Create a fan-out/join coordinator.
 *
 * @returns {Object} Coordinator API
 */
export function createFanOutCoordinator() {
  /** @type {Map<string, Object>} */
  const _coordinations = new Map();

  /**
   * Scatter a task to N agents in parallel. Returns a coordination ID.
   *
   * @param {Object} params
   * @param {string} params.agentAddress - Requesting agent address
   * @param {string[]} params.targets - Target agent addresses
   * @param {string} params.taskType - Type of task (quote, status, custom, etc.)
   * @param {*} params.payload - Task payload sent to each target
   * @param {number} [params.timeoutMs=30000] - Timeout in milliseconds
   * @param {string} [params.joinStrategy='all'] - Join strategy
   * @returns {string} Coordination ID
   */
  function scatter(params) {
    const {
      agentAddress,
      targets,
      taskType,
      payload,
      timeoutMs = 30000,
      joinStrategy = 'all',
    } = params;

    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }
    if (!Array.isArray(targets) || targets.length === 0) {
      throw new Error('targets must be a non-empty array');
    }
    if (!taskType) {
      throw new Error('taskType is required');
    }

    const id = randomUUID();
    const now = new Date();
    const strategy = parseStrategy(joinStrategy);

    const coordination = {
      id,
      agentAddress,
      targets: [...targets],
      taskType,
      payload,
      timeoutMs,
      strategy,
      status: 'pending',
      responses: [],
      pending: [...targets],
      timedOut: [],
      createdAt: now.toISOString(),
      completedAt: null,
      _waiters: [], // Pending join() promises
      _timeoutHandle: null,
    };

    // Set up timeout — the timer must keep the event loop alive so that
    // pending join() promises are resolved when the timeout fires.
    coordination._timeoutHandle = setTimeout(() => {
      _handleTimeout(id);
    }, timeoutMs);

    _coordinations.set(id, coordination);

    return id;
  }

  /**
   * Handle timeout for a coordination.
   *
   * @param {string} coordinationId
   */
  function _handleTimeout(coordinationId) {
    const coord = _coordinations.get(coordinationId);
    if (!coord || coord.status === 'completed') return;

    // Mark remaining pending targets as timed out
    coord.timedOut = [...coord.pending];
    coord.pending = [];
    coord.status = 'completed';
    coord.completedAt = new Date().toISOString();

    // Clear timeout handle
    coord._timeoutHandle = null;

    // Resolve all waiting joiners
    _resolveWaiters(coord);
  }

  /**
   * Resolve pending join() promises for a coordination.
   *
   * @param {Object} coord
   */
  function _resolveWaiters(coord) {
    const aggregated = aggregateResponses(coord.taskType, coord.responses, coord.strategy);
    const result = {
      coordinationId: coord.id,
      status: coord.status,
      taskType: coord.taskType,
      totalTargets: coord.targets.length,
      completedCount: coord.responses.length,
      timedOutCount: coord.timedOut.length,
      responses: coord.responses,
      timedOut: coord.timedOut,
      aggregation: aggregated,
    };

    for (const waiter of coord._waiters) {
      waiter.resolve(result);
    }
    coord._waiters = [];
  }

  /**
   * Register a response from a target agent.
   *
   * @param {string} coordinationId
   * @param {string} responderAddress - Responding agent address
   * @param {*} response - Agent's response
   * @returns {{ accepted: boolean, completedCount: number, totalCount: number }}
   */
  function registerResponse(coordinationId, responderAddress, response) {
    const coord = _coordinations.get(coordinationId);
    if (!coord) {
      throw new Error(`Coordination not found: ${coordinationId}`);
    }

    if (coord.status === 'completed') {
      return {
        accepted: false,
        completedCount: coord.responses.length,
        totalCount: coord.targets.length,
      };
    }

    // Check that responder is a valid target
    if (!coord.targets.includes(responderAddress)) {
      throw new Error(`${responderAddress} is not a target of coordination ${coordinationId}`);
    }

    // Prevent duplicate responses
    if (coord.responses.some((r) => r.responderAddress === responderAddress)) {
      return {
        accepted: false,
        completedCount: coord.responses.length,
        totalCount: coord.targets.length,
      };
    }

    // Record the response
    coord.responses.push({
      responderAddress,
      response,
      receivedAt: new Date().toISOString(),
    });

    // Remove from pending
    coord.pending = coord.pending.filter((t) => t !== responderAddress);

    // Check if join condition is satisfied
    const satisfied = isJoinSatisfied(coord.strategy, coord.responses.length, coord.targets.length);

    if (satisfied) {
      coord.status = 'completed';
      coord.completedAt = new Date().toISOString();

      // Clear timeout
      if (coord._timeoutHandle) {
        clearTimeout(coord._timeoutHandle);
        coord._timeoutHandle = null;
      }

      // Mark remaining pending as neither responded nor timed out
      // They simply didn't respond before the join condition was met
      coord.timedOut = [];

      _resolveWaiters(coord);
    }

    return {
      accepted: true,
      completedCount: coord.responses.length,
      totalCount: coord.targets.length,
    };
  }

  /**
   * Get the current status of a coordination.
   *
   * @param {string} coordinationId
   * @returns {Object}
   */
  function getStatus(coordinationId) {
    const coord = _coordinations.get(coordinationId);
    if (!coord) {
      throw new Error(`Coordination not found: ${coordinationId}`);
    }

    return {
      id: coord.id,
      status: coord.status,
      taskType: coord.taskType,
      agentAddress: coord.agentAddress,
      targets: coord.targets,
      responses: coord.responses,
      pending: coord.pending,
      timedOut: coord.timedOut,
      completedCount: coord.responses.length,
      totalCount: coord.targets.length,
      createdAt: coord.createdAt,
      completedAt: coord.completedAt,
    };
  }

  /**
   * Wait for the join condition to be met or timeout. Returns aggregated result.
   *
   * @param {string} coordinationId
   * @returns {Promise<Object>}
   */
  function join(coordinationId) {
    const coord = _coordinations.get(coordinationId);
    if (!coord) {
      return Promise.reject(new Error(`Coordination not found: ${coordinationId}`));
    }

    // Already completed — return immediately
    if (coord.status === 'completed') {
      const aggregated = aggregateResponses(coord.taskType, coord.responses, coord.strategy);
      return Promise.resolve({
        coordinationId: coord.id,
        status: coord.status,
        taskType: coord.taskType,
        totalTargets: coord.targets.length,
        completedCount: coord.responses.length,
        timedOutCount: coord.timedOut.length,
        responses: coord.responses,
        timedOut: coord.timedOut,
        aggregation: aggregated,
      });
    }

    // Not yet completed — return a promise that resolves when the join is met
    return new Promise((resolve) => {
      coord._waiters.push({ resolve });
    });
  }

  /**
   * Clean up a coordination (clear timers and state).
   *
   * @param {string} coordinationId
   * @returns {boolean}
   */
  function cleanup(coordinationId) {
    const coord = _coordinations.get(coordinationId);
    if (!coord) return false;

    if (coord._timeoutHandle) {
      clearTimeout(coord._timeoutHandle);
      coord._timeoutHandle = null;
    }

    // Reject any pending waiters
    for (const waiter of coord._waiters) {
      waiter.resolve({
        coordinationId: coord.id,
        status: 'cleaned_up',
        taskType: coord.taskType,
        totalTargets: coord.targets.length,
        completedCount: coord.responses.length,
        timedOutCount: coord.timedOut.length,
        responses: coord.responses,
        timedOut: coord.timedOut,
        aggregation: { type: 'empty', data: [] },
      });
    }
    coord._waiters = [];

    _coordinations.delete(coordinationId);
    return true;
  }

  /**
   * Destroy all coordinations and clear timers.
   */
  function destroy() {
    for (const [id] of _coordinations) {
      cleanup(id);
    }
  }

  return {
    scatter,
    registerResponse,
    getStatus,
    join,
    cleanup,
    destroy,
  };
}

export default { createFanOutCoordinator };
