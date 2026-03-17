/**
 * A2A Health & Readiness Endpoints
 *
 * Provides /health, /ready, and /live endpoints for production deployments.
 * Checks database connectivity, sequencer reachability, and critical subsystem status.
 *
 * @example
 * ```javascript
 * const health = createHealthService(store, sequencerClient, subsystems);
 * const status = await health.check();
 * // { status: 'healthy', checks: { db: 'ok', sequencer: 'ok', ... } }
 * ```
 */

/**
 * Create a health check service.
 *
 * @param {Object} store - A2A store (must have a db handle)
 * @param {Object} [sequencerClient] - x402 sequencer client
 * @param {Object} [subsystems] - Optional subsystem references
 * @param {Object} [subsystems.billingExecutor]
 * @param {Object} [subsystems.disputeResolver]
 * @param {Object} [subsystems.notificationService]
 * @returns {Object} Health service API
 */
export function createHealthService(store, sequencerClient, subsystems = {}) {
  let _startedAt = new Date().toISOString();

  /**
   * Full health check — tests all dependencies.
   * @returns {Promise<Object>} Health status
   */
  async function check() {
    const checks = {};
    let overallHealthy = true;

    // 1. Database check
    try {
      // Simple query to test DB connectivity
      const testResult = store.listPayments
        ? await store.listPayments({ limit: 1 })
        : store.listAgentCards
          ? store.listAgentCards({ limit: 1 })
          : [];
      checks.database = { status: 'ok', latencyMs: 0 };
    } catch (err) {
      checks.database = { status: 'error', error: err.message };
      overallHealthy = false;
    }

    // 2. Sequencer check (optional)
    if (sequencerClient) {
      const seqStart = Date.now();
      try {
        // Try a lightweight request
        await sequencerClient.getPaymentStatus('health-check-probe');
        checks.sequencer = { status: 'ok', latencyMs: Date.now() - seqStart };
      } catch (err) {
        // 404 is fine — it means the sequencer is reachable
        if (err.message && err.message.includes('404')) {
          checks.sequencer = { status: 'ok', latencyMs: Date.now() - seqStart };
        } else {
          checks.sequencer = {
            status: 'degraded',
            error: err.message,
            latencyMs: Date.now() - seqStart,
          };
          // Sequencer being down is degraded, not unhealthy
        }
      }
    } else {
      checks.sequencer = { status: 'not_configured' };
    }

    // 3. Subsystem checks
    if (subsystems.billingExecutor) {
      const metrics = subsystems.billingExecutor.getMetrics();
      checks.billingExecutor = {
        status: metrics.running ? 'running' : 'stopped',
        totalTicks: metrics.totalTicks,
        lastTickAt: metrics.lastTickAt,
      };
    }

    if (subsystems.disputeResolver) {
      const metrics = subsystems.disputeResolver.getMetrics();
      checks.disputeResolver = {
        status: metrics.running ? 'running' : 'stopped',
        totalTicks: metrics.totalTicks,
        lastTickAt: metrics.lastTickAt,
      };
    }

    return {
      status: overallHealthy ? 'healthy' : 'unhealthy',
      timestamp: new Date().toISOString(),
      uptime: _uptimeMs(),
      startedAt: _startedAt,
      checks,
    };
  }

  /**
   * Liveness probe — returns true if the process is alive.
   * Used by Kubernetes /live endpoint.
   */
  function live() {
    return { status: 'alive', timestamp: new Date().toISOString() };
  }

  /**
   * Readiness probe — returns true if the service can accept traffic.
   * Checks only critical dependencies (database).
   */
  async function ready() {
    try {
      const testResult = store.listPayments ? await store.listPayments({ limit: 1 }) : [];
      return { status: 'ready', timestamp: new Date().toISOString() };
    } catch (err) {
      return { status: 'not_ready', error: err.message, timestamp: new Date().toISOString() };
    }
  }

  /**
   * Handle HTTP health check requests.
   * @param {import('node:http').IncomingMessage} req
   * @param {import('node:http').ServerResponse} res
   */
  async function handleHTTP(req, res) {
    const url = new URL(req.url, `http://${req.headers.host || 'localhost'}`);
    const pathname = url.pathname;

    let result;
    let statusCode = 200;

    if (pathname === '/live' || pathname === '/livez') {
      result = live();
    } else if (pathname === '/ready' || pathname === '/readyz') {
      result = await ready();
      if (result.status !== 'ready') statusCode = 503;
    } else {
      // /health or default
      result = await check();
      if (result.status !== 'healthy') statusCode = 503;
    }

    res.writeHead(statusCode, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(result));
  }

  function _uptimeMs() {
    return Date.now() - new Date(_startedAt).getTime();
  }

  return {
    check,
    live,
    ready,
    handleHTTP,
  };
}

export default { createHealthService };
