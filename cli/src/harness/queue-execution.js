/**
 * Lane-based queued/parallel execution wrappers around runAgentLoop.
 *
 * Extracted from claude-harness.js. The loop runner is dependency-injected to
 * avoid a circular import; there is no module-scope state (the command queue
 * singleton lives in command-queue.js, as before).
 */

import { getCommandQueue } from '../command-queue.js';
import { loadAgentSettings } from '../settings.js';

function getConfiguredCommandQueue(settingsOverrides = null) {
  const resolvedSettings = loadAgentSettings(settingsOverrides || {});
  const queueSettings = resolvedSettings.queue || {};
  return getCommandQueue({
    maxLanes: queueSettings.maxLanes,
    laneTimeoutMs: queueSettings.laneTimeoutMs ?? queueSettings.laneTimeout,
    maxQueueSize: queueSettings.maxQueueSize,
    idleCleanupMs: queueSettings.idleCleanupMs,
    parallelConcurrency: queueSettings.parallelConcurrency,
    waitWarningMs: queueSettings.waitWarningMs,
    runningWarningMs: queueSettings.runningWarningMs,
    warningThrottleMs: queueSettings.warningThrottleMs,
    monitorIntervalMs: queueSettings.monitorIntervalMs,
    emitWarnings: queueSettings.emitWarnings,
  });
}

/**
 * Build the queue-wrapped runners bound to the given runAgentLoop.
 */
export function createQueueRunners({ runAgentLoop }) {
  /**
   * Run agent loop with lane-based serialization.
   * Operations for the same session execute serially to prevent race conditions.
   *
   * @param {Object} options - Same options as runAgentLoop plus:
   * @param {boolean} options.useQueue - Enable queue-based serialization (default: true)
   * @param {string} options.laneId - Custom lane ID (default: sessionId or 'default')
   * @returns {Promise<Object>} - Same result as runAgentLoop
   */
  async function runAgentLoopQueued(options) {
    const { useQueue = true, laneId, ...loopOptions } = options;

    // Determine lane ID - use session ID for serialization
    const effectiveLaneId = laneId || options.resumeSessionId || 'default';

    if (!useQueue) {
      return runAgentLoop(loopOptions);
    }

    // Get the command queue singleton
    const queue = getConfiguredCommandQueue(options.settings || null);

    // Enqueue the operation in the appropriate lane
    return queue.enqueue(
      effectiveLaneId,
      async () => {
        return runAgentLoop(loopOptions);
      },
      {
        request: typeof options.request === 'string' ? options.request.slice(0, 50) : '',
        agent: options.agent,
      },
    );
  }

  /**
   * Run multiple agent requests in parallel lanes.
   * Each request gets its own lane for concurrent execution.
   *
   * @param {Object[]} requests - Array of runAgentLoop options
   * @returns {Promise<Object[]>} - Array of results
   */
  async function runAgentLoopParallel(requests) {
    const queue = getConfiguredCommandQueue(
      requests.find((options) => options?.settings)?.settings || null,
    );

    return Promise.all(
      requests.map((options, index) => {
        const laneId =
          options?.laneId || options?.resumeSessionId || options?.sessionId || `parallel:${index}`;
        return queue.enqueueParallel(
          laneId,
          async () => {
            return runAgentLoop(options);
          },
          { index, resumeSessionId: options?.resumeSessionId || null },
        );
      }),
    );
  }

  return { runAgentLoopQueued, runAgentLoopParallel };
}

/**
 * Remove a specific queue lane.
 *
 * @param {string} laneId
 * @param {{ force?: boolean }} [options]
 */
export function removeQueueLane(laneId, options = {}) {
  const queue = getCommandQueue();
  return queue.removeLane(laneId, options);
}

/**
 * Clear queue lanes.
 *
 * @param {{ force?: boolean }} [options]
 */
export function clearQueueLanes(options = {}) {
  const queue = getCommandQueue();
  return queue.clearLanes(options);
}

/**
 * Get queue statistics for monitoring.
 * @returns {Object}
 */
export function getQueueStats(laneId = null) {
  const queue = getCommandQueue();
  if (!laneId) {
    return queue.getStats();
  }

  const laneStats = queue.getLaneStats(laneId);
  if (!laneStats) return null;
  return { laneId, ...laneStats };
}
