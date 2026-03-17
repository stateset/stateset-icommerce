/**
 * Tests for cli/src/a2a/health.js
 *
 * Covers: createHealthService — check(), live(), ready(), subsystem metrics,
 * sequencer degraded status.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

import { createHealthService } from '../../src/a2a/health.js';

// ---------------------------------------------------------------------------
// Mock factories
// ---------------------------------------------------------------------------

/** Store mock whose listPayments resolves or rejects. */
function createOkStore() {
  return {
    listPayments: async () => [],
  };
}

function createFailingStore(errorMessage = 'SQLITE_ERROR: connection refused') {
  return {
    listPayments: async () => {
      throw new Error(errorMessage);
    },
  };
}

/** Store with only listAgentCards (no listPayments). */
function createAgentCardOnlyStore() {
  return {
    listAgentCards: () => [],
  };
}

/** Store with neither listPayments nor listAgentCards. */
function createMinimalStore() {
  return {};
}

/** Sequencer mock that resolves. */
function createOkSequencer() {
  return {
    getPaymentStatus: async () => ({ status: 'not_found' }),
  };
}

/** Sequencer mock that rejects with 404. */
function createNotFoundSequencer() {
  return {
    getPaymentStatus: async () => {
      throw new Error('404 Not Found');
    },
  };
}

/** Sequencer mock that rejects with a non-404 error. */
function createUnreachableSequencer() {
  return {
    getPaymentStatus: async () => {
      throw new Error('ECONNREFUSED');
    },
  };
}

/** Running subsystem mock. */
function createRunningSubsystem() {
  return {
    getMetrics: () => ({
      running: true,
      totalTicks: 42,
      lastTickAt: new Date().toISOString(),
    }),
  };
}

/** Stopped subsystem mock. */
function createStoppedSubsystem() {
  return {
    getMetrics: () => ({
      running: false,
      totalTicks: 0,
      lastTickAt: null,
    }),
  };
}

// ---------------------------------------------------------------------------
// 1. check() — healthy when DB is ok
// ---------------------------------------------------------------------------

describe('HealthService.check() — healthy', () => {
  it('returns status=healthy when database is reachable', async () => {
    const health = createHealthService(createOkStore());
    const result = await health.check();

    assert.equal(result.status, 'healthy');
    assert.ok(result.timestamp);
    assert.ok(result.startedAt);
    assert.ok(typeof result.uptime === 'number');
    assert.equal(result.checks.database.status, 'ok');
  });

  it('includes sequencer as not_configured when no sequencer is provided', async () => {
    const health = createHealthService(createOkStore());
    const result = await health.check();

    assert.equal(result.checks.sequencer.status, 'not_configured');
  });

  it('marks sequencer ok when it resolves', async () => {
    const health = createHealthService(createOkStore(), createOkSequencer());
    const result = await health.check();

    assert.equal(result.status, 'healthy');
    assert.equal(result.checks.sequencer.status, 'ok');
    assert.ok(typeof result.checks.sequencer.latencyMs === 'number');
  });

  it('marks sequencer ok when it returns 404', async () => {
    const health = createHealthService(createOkStore(), createNotFoundSequencer());
    const result = await health.check();

    assert.equal(result.checks.sequencer.status, 'ok');
  });

  it('falls back to listAgentCards when listPayments is unavailable', async () => {
    const health = createHealthService(createAgentCardOnlyStore());
    const result = await health.check();

    assert.equal(result.status, 'healthy');
    assert.equal(result.checks.database.status, 'ok');
  });

  it('succeeds with minimal store (no listPayments or listAgentCards)', async () => {
    const health = createHealthService(createMinimalStore());
    const result = await health.check();

    assert.equal(result.status, 'healthy');
    assert.equal(result.checks.database.status, 'ok');
  });
});

// ---------------------------------------------------------------------------
// 2. check() — unhealthy when DB throws
// ---------------------------------------------------------------------------

describe('HealthService.check() — unhealthy', () => {
  it('returns status=unhealthy when database throws', async () => {
    const health = createHealthService(createFailingStore());
    const result = await health.check();

    assert.equal(result.status, 'unhealthy');
    assert.equal(result.checks.database.status, 'error');
    assert.ok(result.checks.database.error);
    assert.ok(result.checks.database.error.includes('SQLITE_ERROR'));
  });

  it('includes the error message from the database', async () => {
    const health = createHealthService(createFailingStore('disk full'));
    const result = await health.check();

    assert.equal(result.checks.database.error, 'disk full');
  });
});

// ---------------------------------------------------------------------------
// 3. live() — always returns alive
// ---------------------------------------------------------------------------

describe('HealthService.live()', () => {
  it('returns alive status', () => {
    const health = createHealthService(createOkStore());
    const result = health.live();

    assert.equal(result.status, 'alive');
    assert.ok(result.timestamp);
  });

  it('returns alive even when store is failing', () => {
    const health = createHealthService(createFailingStore());
    const result = health.live();

    assert.equal(result.status, 'alive');
  });

  it('is a synchronous call', () => {
    const health = createHealthService(createOkStore());
    const result = health.live();

    // Not a promise
    assert.equal(typeof result.then, 'undefined');
    assert.equal(result.status, 'alive');
  });

  it('includes an ISO timestamp', () => {
    const health = createHealthService(createOkStore());
    const result = health.live();

    // Validate ISO format
    assert.ok(!isNaN(Date.parse(result.timestamp)), 'timestamp should be valid ISO');
  });
});

// ---------------------------------------------------------------------------
// 4. ready() — ready when DB is ok
// ---------------------------------------------------------------------------

describe('HealthService.ready() — ready', () => {
  it('returns status=ready when database is reachable', async () => {
    const health = createHealthService(createOkStore());
    const result = await health.ready();

    assert.equal(result.status, 'ready');
    assert.ok(result.timestamp);
  });

  it('does not include error field when ready', async () => {
    const health = createHealthService(createOkStore());
    const result = await health.ready();

    assert.equal(result.error, undefined);
  });
});

// ---------------------------------------------------------------------------
// 5. ready() — not_ready when DB throws
// ---------------------------------------------------------------------------

describe('HealthService.ready() — not_ready', () => {
  it('returns status=not_ready when database throws', async () => {
    const health = createHealthService(createFailingStore());
    const result = await health.ready();

    assert.equal(result.status, 'not_ready');
    assert.ok(result.error);
  });

  it('includes the database error message', async () => {
    const health = createHealthService(createFailingStore('timeout'));
    const result = await health.ready();

    assert.equal(result.error, 'timeout');
  });

  it('includes timestamp even when not ready', async () => {
    const health = createHealthService(createFailingStore());
    const result = await health.ready();

    assert.ok(result.timestamp);
  });
});

// ---------------------------------------------------------------------------
// 6. Includes subsystem metrics when provided
// ---------------------------------------------------------------------------

describe('HealthService.check() — subsystem metrics', () => {
  it('includes billingExecutor metrics when provided', async () => {
    const health = createHealthService(createOkStore(), null, {
      billingExecutor: createRunningSubsystem(),
    });
    const result = await health.check();

    assert.equal(result.status, 'healthy');
    assert.ok(result.checks.billingExecutor);
    assert.equal(result.checks.billingExecutor.status, 'running');
    assert.equal(result.checks.billingExecutor.totalTicks, 42);
    assert.ok(result.checks.billingExecutor.lastTickAt);
  });

  it('includes disputeResolver metrics when provided', async () => {
    const health = createHealthService(createOkStore(), null, {
      disputeResolver: createRunningSubsystem(),
    });
    const result = await health.check();

    assert.ok(result.checks.disputeResolver);
    assert.equal(result.checks.disputeResolver.status, 'running');
  });

  it('includes both subsystem metrics when both are provided', async () => {
    const health = createHealthService(createOkStore(), null, {
      billingExecutor: createRunningSubsystem(),
      disputeResolver: createStoppedSubsystem(),
    });
    const result = await health.check();

    assert.equal(result.checks.billingExecutor.status, 'running');
    assert.equal(result.checks.disputeResolver.status, 'stopped');
    assert.equal(result.checks.disputeResolver.totalTicks, 0);
  });

  it('omits subsystem checks when no subsystems are provided', async () => {
    const health = createHealthService(createOkStore());
    const result = await health.check();

    assert.equal(result.checks.billingExecutor, undefined);
    assert.equal(result.checks.disputeResolver, undefined);
  });

  it('reports stopped subsystem as stopped (not unhealthy)', async () => {
    const health = createHealthService(createOkStore(), null, {
      billingExecutor: createStoppedSubsystem(),
    });
    const result = await health.check();

    // Overall status should still be healthy (subsystem stopped is informational)
    assert.equal(result.status, 'healthy');
    assert.equal(result.checks.billingExecutor.status, 'stopped');
  });
});

// ---------------------------------------------------------------------------
// 7. Reports sequencer as degraded when unreachable
// ---------------------------------------------------------------------------

describe('HealthService.check() — sequencer degraded', () => {
  it('marks sequencer as degraded when unreachable (non-404 error)', async () => {
    const health = createHealthService(createOkStore(), createUnreachableSequencer());
    const result = await health.check();

    // Overall is still healthy (sequencer is non-critical)
    assert.equal(result.status, 'healthy');
    assert.equal(result.checks.sequencer.status, 'degraded');
    assert.ok(result.checks.sequencer.error);
    assert.ok(result.checks.sequencer.error.includes('ECONNREFUSED'));
    assert.ok(typeof result.checks.sequencer.latencyMs === 'number');
  });

  it('does not mark overall as unhealthy due to sequencer degradation', async () => {
    const health = createHealthService(createOkStore(), createUnreachableSequencer());
    const result = await health.check();

    // DB is ok, so overall should be healthy despite sequencer being degraded
    assert.equal(result.status, 'healthy');
    assert.equal(result.checks.database.status, 'ok');
    assert.equal(result.checks.sequencer.status, 'degraded');
  });

  it('is unhealthy when both DB fails and sequencer is degraded', async () => {
    const health = createHealthService(createFailingStore(), createUnreachableSequencer());
    const result = await health.check();

    assert.equal(result.status, 'unhealthy');
    assert.equal(result.checks.database.status, 'error');
    assert.equal(result.checks.sequencer.status, 'degraded');
  });
});
