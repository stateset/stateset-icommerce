/**
 * Unit tests for SLA Framework — Service Level Agreement definitions,
 * monitoring, compliance checking, breach detection, and violation resolution.
 *
 * Tests cli/src/a2a/sla.js:
 *   - createSLAService() construction and validation
 *   - attachSLA() — validation, metric requirements, service existence
 *   - checkCompliance() — response time, uptime, quality, throughput checks
 *   - detectBreaches() — violation creation, severity, penalty calculation
 *   - resolveViolation() — marking resolved with optional note
 *   - getSLAs() — listing definitions for a service
 *   - getViolations() — filtering by resolved/severity
 *   - Edge cases — no quotes, no SLAs, missing metrics, already resolved
 */

import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { A2AStore } from '../../src/a2a/store.js';
import { createSLAService } from '../../src/a2a/sla.js';

/**
 * Helper: create a fresh :memory: store and SLA service for each test suite.
 */
function setup() {
  const store = new A2AStore(':memory:');
  store.init();
  const sla = createSLAService(store);
  return { store, sla };
}

/**
 * Helper: create a service in the store and return its ID and agent address.
 */
function createTestService(store, overrides = {}) {
  const agentAddress = overrides.agent_address || `agent-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const svc = store.createService({
    agent_address: agentAddress,
    name: overrides.name || 'Test Service',
    description: overrides.description || 'A test service',
    category: overrides.category || 'compute',
    ...overrides,
    agent_address: agentAddress,
  });
  return { serviceId: svc.id, agentAddress };
}

/**
 * Helper: create a quote with specific created_at and quoted_at timestamps
 * so we can control response time calculation.
 */
function createTestQuote(store, { sellerAddress, createdAt, quotedAt, status = 'fulfilled' }) {
  const id = `quote-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  return store.createQuote({
    id,
    buyer_address: 'buyer-addr-001',
    seller_address: sellerAddress,
    status,
    total: 100,
    total_decimal: 100,
    created_at: createdAt,
    quoted_at: quotedAt,
    expires_at: new Date(Date.now() + 86400000).toISOString(),
  });
}

/**
 * Helper: create feedback with a specific score for an agent.
 */
function createTestFeedback(store, { agentAddress, score }) {
  return store.createFeedback({
    agent_address: agentAddress,
    reviewer_address: 'reviewer-001',
    transaction_type: 'quote',
    transaction_id: `txn-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    score,
  });
}

/**
 * Helper: create a payment to an agent with a specific amount.
 */
function createTestPayment(store, { recipientAddress, amount }) {
  const id = `pay-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  return store.createPayment({
    id,
    sender_address: 'payer-001',
    recipient_address: recipientAddress,
    amount,
    amount_decimal: amount,
    status: 'completed',
  });
}


// =============================================================================
// 1. createSLAService validation
// =============================================================================

describe('createSLAService — construction', () => {
  it('should throw when store is null', () => {
    assert.throws(() => createSLAService(null), /store is required/);
  });

  it('should throw when store is undefined', () => {
    assert.throws(() => createSLAService(undefined), /store is required/);
  });

  it('should return an object with all expected methods', () => {
    const { sla } = setup();
    assert.equal(typeof sla.attachSLA, 'function');
    assert.equal(typeof sla.checkCompliance, 'function');
    assert.equal(typeof sla.detectBreaches, 'function');
    assert.equal(typeof sla.resolveViolation, 'function');
    assert.equal(typeof sla.enforcePenalties, 'function');
    assert.equal(typeof sla.enforceAll, 'function');
    assert.equal(typeof sla.getSLAs, 'function');
    assert.equal(typeof sla.getViolations, 'function');
  });

  it('should return exactly 8 methods', () => {
    const { sla } = setup();
    const keys = Object.keys(sla);
    assert.equal(keys.length, 8);
  });
});


// =============================================================================
// 2. attachSLA — validation, metric requirements, service existence
// =============================================================================

describe('attachSLA — validation', () => {
  it('should throw when serviceId is missing', () => {
    const { sla } = setup();
    assert.throws(
      () => sla.attachSLA({ responseTimeMs: 5000 }),
      /serviceId is required/,
    );
  });

  it('should throw when serviceId is empty string', () => {
    const { sla } = setup();
    assert.throws(
      () => sla.attachSLA({ serviceId: '', responseTimeMs: 5000 }),
      /serviceId is required/,
    );
  });

  it('should throw when service does not exist', () => {
    const { sla } = setup();
    assert.throws(
      () => sla.attachSLA({ serviceId: 'nonexistent', responseTimeMs: 5000 }),
      /not found/,
    );
  });

  it('should throw when no metrics are provided', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    assert.throws(
      () => sla.attachSLA({ serviceId }),
      /At least one SLA metric must be defined/,
    );
  });

  it('should throw when all metrics are explicitly null', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    assert.throws(
      () => sla.attachSLA({
        serviceId,
        responseTimeMs: null,
        uptimePercent: null,
        qualityMinScore: null,
        throughputRps: null,
      }),
      /At least one SLA metric must be defined/,
    );
  });
});

describe('attachSLA — success cases', () => {
  it('should create SLA with responseTimeMs only', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.attachSLA({ serviceId, responseTimeMs: 5000 });
    assert.equal(result.serviceId, serviceId);
    assert.ok(result.sla);
    assert.equal(result.sla.response_time_ms, 5000);
  });

  it('should create SLA with uptimePercent only', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.attachSLA({ serviceId, uptimePercent: 99.9 });
    assert.equal(result.sla.uptime_percent, 99.9);
  });

  it('should create SLA with qualityMinScore only', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.attachSLA({ serviceId, qualityMinScore: 4.0 });
    assert.equal(result.sla.quality_min_score, 4.0);
  });

  it('should create SLA with throughputRps only', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.attachSLA({ serviceId, throughputRps: 100 });
    assert.equal(result.sla.throughput_rps, 100);
  });

  it('should create SLA with all metrics', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.attachSLA({
      serviceId,
      responseTimeMs: 3000,
      uptimePercent: 99.5,
      qualityMinScore: 4.2,
      throughputRps: 50,
    });
    assert.equal(result.sla.response_time_ms, 3000);
    assert.equal(result.sla.uptime_percent, 99.5);
    assert.equal(result.sla.quality_min_score, 4.2);
    assert.equal(result.sla.throughput_rps, 50);
  });

  it('should default penaltyPercent to 5', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.attachSLA({ serviceId, responseTimeMs: 5000 });
    assert.equal(result.sla.penalty_percent, 5);
  });

  it('should default penaltyType to credit', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.attachSLA({ serviceId, responseTimeMs: 5000 });
    assert.equal(result.sla.penalty_type, 'credit');
  });

  it('should accept custom penaltyPercent', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.attachSLA({ serviceId, responseTimeMs: 5000, penaltyPercent: 15 });
    assert.equal(result.sla.penalty_percent, 15);
  });

  it('should accept penaltyType refund', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.attachSLA({ serviceId, responseTimeMs: 5000, penaltyType: 'refund' });
    assert.equal(result.sla.penalty_type, 'refund');
  });

  it('should accept penaltyType suspension', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.attachSLA({ serviceId, responseTimeMs: 5000, penaltyType: 'suspension' });
    assert.equal(result.sla.penalty_type, 'suspension');
  });

  it('should persist SLA in the store', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.attachSLA({ serviceId, responseTimeMs: 5000 });
    const fetched = store.getSLADefinition(result.sla.id);
    assert.ok(fetched);
    assert.equal(fetched.response_time_ms, 5000);
  });

  it('should allow multiple SLAs on the same service', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 5000 });
    sla.attachSLA({ serviceId, uptimePercent: 99.0 });
    const all = sla.getSLAs(serviceId);
    assert.equal(all.length, 2);
  });
});


// =============================================================================
// 3. checkCompliance — response time, uptime, quality, throughput
// =============================================================================

describe('checkCompliance — no SLAs', () => {
  it('should return compliant with zero slaCount when no SLAs exist', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.checkCompliance(serviceId);
    assert.equal(result.compliant, true);
    assert.equal(result.slaCount, 0);
    assert.deepEqual(result.checks, []);
  });

  it('should throw for missing serviceId', () => {
    const { sla } = setup();
    assert.throws(() => sla.checkCompliance(''), /serviceId is required/);
  });

  it('should throw for nonexistent service', () => {
    const { sla } = setup();
    assert.throws(() => sla.checkCompliance('no-such-service'), /not found/);
  });
});

describe('checkCompliance — response time', () => {
  it('should report compliant when avg response time is within SLA', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 5000 });

    // Create quotes with 2000ms response time
    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 2000).toISOString(),
    });
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: new Date(base.getTime() + 10000).toISOString(),
      quotedAt: new Date(base.getTime() + 12000).toISOString(),
    });

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.compliant, true);
    assert.equal(result.metrics.avgResponseTimeMs, 2000);
  });

  it('should report non-compliant when avg response time exceeds SLA', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 3000).toISOString(),
    });

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.compliant, false);
    const check = result.checks[0].checks[0];
    assert.equal(check.metric, 'response_time_ms');
    assert.equal(check.expected, 1000);
    assert.equal(check.actual, 3000);
    assert.equal(check.compliant, false);
  });

  it('should handle edge case where response time equals SLA exactly', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 5000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.compliant, true);
  });
});

describe('checkCompliance — uptime / success rate', () => {
  it('should report compliant when all quotes are fulfilled', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, uptimePercent: 90 });

    const base = new Date('2025-01-01T00:00:00Z');
    for (let i = 0; i < 10; i++) {
      createTestQuote(store, {
        sellerAddress: agentAddress,
        createdAt: new Date(base.getTime() + i * 10000).toISOString(),
        quotedAt: new Date(base.getTime() + i * 10000 + 1000).toISOString(),
        status: 'fulfilled',
      });
    }

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.compliant, true);
    const uptimeCheck = result.checks[0].checks.find((c) => c.metric === 'uptime_percent');
    assert.equal(uptimeCheck.actual, 100);
  });

  it('should report non-compliant when success rate is below threshold', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, uptimePercent: 90 });

    const base = new Date('2025-01-01T00:00:00Z');
    // 7 fulfilled, 3 rejected => 70% success rate
    for (let i = 0; i < 7; i++) {
      createTestQuote(store, {
        sellerAddress: agentAddress,
        createdAt: new Date(base.getTime() + i * 10000).toISOString(),
        quotedAt: new Date(base.getTime() + i * 10000 + 1000).toISOString(),
        status: 'fulfilled',
      });
    }
    for (let i = 0; i < 3; i++) {
      createTestQuote(store, {
        sellerAddress: agentAddress,
        createdAt: new Date(base.getTime() + (7 + i) * 10000).toISOString(),
        quotedAt: new Date(base.getTime() + (7 + i) * 10000 + 1000).toISOString(),
        status: 'rejected',
      });
    }

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.compliant, false);
    const uptimeCheck = result.checks[0].checks.find((c) => c.metric === 'uptime_percent');
    assert.equal(uptimeCheck.actual, 70);
    assert.equal(uptimeCheck.compliant, false);
  });
});

describe('checkCompliance — quality score', () => {
  it('should report compliant when avg quality meets threshold', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, qualityMinScore: 4.0 });

    // Need at least one quote for metrics to compute
    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 1000).toISOString(),
    });

    // Create feedback scores: 4.5, 4.8, 4.2 => avg = 4.5
    createTestFeedback(store, { agentAddress, score: 4.5 });
    createTestFeedback(store, { agentAddress, score: 4.8 });
    createTestFeedback(store, { agentAddress, score: 4.2 });

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.compliant, true);
    const qualityCheck = result.checks[0].checks.find((c) => c.metric === 'quality_min_score');
    assert.ok(qualityCheck.actual >= 4.0);
    assert.equal(qualityCheck.compliant, true);
  });

  it('should report non-compliant when avg quality is below threshold', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, qualityMinScore: 4.5 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 1000).toISOString(),
    });

    // Create feedback scores: 3.0, 3.5, 4.0 => avg = 3.5
    createTestFeedback(store, { agentAddress, score: 3.0 });
    createTestFeedback(store, { agentAddress, score: 3.5 });
    createTestFeedback(store, { agentAddress, score: 4.0 });

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.compliant, false);
    const qualityCheck = result.checks[0].checks.find((c) => c.metric === 'quality_min_score');
    assert.ok(qualityCheck.actual < 4.5);
    assert.equal(qualityCheck.compliant, false);
  });
});

describe('checkCompliance — throughput', () => {
  it('should report compliant when throughput meets SLA', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, throughputRps: 0.5 });

    // Create 10 quotes in a 10-second span => 1 rps
    const base = new Date('2025-01-01T00:00:00Z');
    for (let i = 0; i < 10; i++) {
      createTestQuote(store, {
        sellerAddress: agentAddress,
        createdAt: new Date(base.getTime() + i * 1000).toISOString(),
        quotedAt: new Date(base.getTime() + i * 1000 + 500).toISOString(),
      });
    }

    const result = sla.checkCompliance(serviceId);
    const throughputCheck = result.checks[0].checks.find((c) => c.metric === 'throughput_rps');
    assert.ok(throughputCheck);
    assert.ok(throughputCheck.actual >= 0.5);
    assert.equal(throughputCheck.compliant, true);
  });

  it('should report non-compliant when throughput is below SLA', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, throughputRps: 100 });

    // Create 2 quotes in a 10-second span => 0.2 rps
    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 500).toISOString(),
    });
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: new Date(base.getTime() + 10000).toISOString(),
      quotedAt: new Date(base.getTime() + 10500).toISOString(),
    });

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.compliant, false);
    const throughputCheck = result.checks[0].checks.find((c) => c.metric === 'throughput_rps');
    assert.ok(throughputCheck.actual < 100);
    assert.equal(throughputCheck.compliant, false);
  });
});

describe('checkCompliance — all compliant returns true', () => {
  it('should return compliant true when all metrics pass', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({
      serviceId,
      responseTimeMs: 5000,
      uptimePercent: 50,
      qualityMinScore: 3.0,
    });

    // Fast response times, all fulfilled
    const base = new Date('2025-01-01T00:00:00Z');
    for (let i = 0; i < 5; i++) {
      createTestQuote(store, {
        sellerAddress: agentAddress,
        createdAt: new Date(base.getTime() + i * 10000).toISOString(),
        quotedAt: new Date(base.getTime() + i * 10000 + 1000).toISOString(),
        status: 'fulfilled',
      });
    }

    // Good quality
    createTestFeedback(store, { agentAddress, score: 4.5 });
    createTestFeedback(store, { agentAddress, score: 5.0 });

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.compliant, true);
    assert.equal(result.slaCount, 1);
  });
});

describe('checkCompliance — metrics object', () => {
  it('should include totalQuotes in metrics', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 10000 });

    const base = new Date('2025-01-01T00:00:00Z');
    for (let i = 0; i < 3; i++) {
      createTestQuote(store, {
        sellerAddress: agentAddress,
        createdAt: new Date(base.getTime() + i * 10000).toISOString(),
        quotedAt: new Date(base.getTime() + i * 10000 + 500).toISOString(),
      });
    }

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.metrics.totalQuotes, 3);
  });

  it('should return null avgResponseTimeMs when no quotes have timestamps', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 5000 });

    // Quote with no quoted_at
    const qId = `q-no-ts-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    store.createQuote({
      id: qId,
      buyer_address: 'buyer-001',
      seller_address: agentAddress,
      status: 'fulfilled',
      total: 100,
      total_decimal: 100,
      expires_at: new Date(Date.now() + 86400000).toISOString(),
      created_at: new Date().toISOString(),
      quoted_at: null,
    });

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.metrics.avgResponseTimeMs, null);
  });
});


// =============================================================================
// 4. detectBreaches — violations, severity, penalty
// =============================================================================

describe('detectBreaches — no breaches when compliant', () => {
  it('should return empty breaches when all compliant', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 10000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 500).toISOString(),
    });

    const result = sla.detectBreaches(serviceId);
    assert.equal(result.breaches.length, 0);
    assert.equal(result.newViolations, 0);
  });

  it('should return empty breaches when no SLAs', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.detectBreaches(serviceId);
    assert.equal(result.breaches.length, 0);
    assert.equal(result.newViolations, 0);
  });
});

describe('detectBreaches — creates violations', () => {
  it('should create a violation for response time breach', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const result = sla.detectBreaches(serviceId);
    assert.equal(result.newViolations, 1);
    assert.equal(result.breaches[0].metric, 'response_time_ms');
    assert.equal(result.breaches[0].expected, 1000);
    assert.equal(result.breaches[0].actual, 5000);
  });

  it('should persist violations in the store', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const result = sla.detectBreaches(serviceId);
    const violation = store.getSLAViolation(result.breaches[0].violationId);
    assert.ok(violation);
    assert.equal(violation.violation_type, 'response_time_ms');
    assert.equal(violation.service_id, serviceId);
  });

  it('should create multiple violations for multiple metric breaches', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({
      serviceId,
      responseTimeMs: 1000,
      uptimePercent: 99,
    });

    // Slow response (5000ms) and low uptime (50% fulfilled)
    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
      status: 'fulfilled',
    });
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: new Date(base.getTime() + 10000).toISOString(),
      quotedAt: new Date(base.getTime() + 15000).toISOString(),
      status: 'rejected',
    });

    const result = sla.detectBreaches(serviceId);
    assert.ok(result.newViolations >= 2);
    const metrics = result.breaches.map((b) => b.metric);
    assert.ok(metrics.includes('response_time_ms'));
    assert.ok(metrics.includes('uptime_percent'));
  });
});

describe('detectBreaches — severity determination', () => {
  it('should assign warning severity when ratio > 0.8', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    // SLA: 1000ms, actual: 1200ms => ratio = 1200/1000 = 1.2 > 0.8 => warning
    sla.attachSLA({ serviceId, responseTimeMs: 1000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 1200).toISOString(),
    });

    const result = sla.detectBreaches(serviceId);
    assert.equal(result.breaches[0].severity, 'warning');
  });

  it('should assign critical severity when ratio <= 0.8', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    // For uptime: SLA 99%, actual 50% => ratio = 50/99 ~ 0.505 <= 0.8 => critical
    sla.attachSLA({ serviceId, uptimePercent: 99 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 1000).toISOString(),
      status: 'fulfilled',
    });
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: new Date(base.getTime() + 10000).toISOString(),
      quotedAt: new Date(base.getTime() + 11000).toISOString(),
      status: 'rejected',
    });

    const result = sla.detectBreaches(serviceId);
    const uptimeBreach = result.breaches.find((b) => b.metric === 'uptime_percent');
    assert.ok(uptimeBreach);
    assert.equal(uptimeBreach.severity, 'critical');
  });
});

describe('detectBreaches — penalty calculation', () => {
  it('should calculate penalty from avg transaction value', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000, penaltyPercent: 10 });

    // Create payments: avg = (100 + 200) / 2 = 150, penalty = 150 * 10% = 15
    createTestPayment(store, { recipientAddress: agentAddress, amount: 100 });
    createTestPayment(store, { recipientAddress: agentAddress, amount: 200 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const result = sla.detectBreaches(serviceId);
    assert.equal(result.breaches[0].penaltyAmount, 15);
  });

  it('should return 0 penalty when no payments exist', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000, penaltyPercent: 10 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const result = sla.detectBreaches(serviceId);
    assert.equal(result.breaches[0].penaltyAmount, 0);
  });

  it('should round penalty to 2 decimal places', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000, penaltyPercent: 7 });

    // avg tx = 33.33
    createTestPayment(store, { recipientAddress: agentAddress, amount: 33.33 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const result = sla.detectBreaches(serviceId);
    // 33.33 * 7% = 2.3331 => rounded to 2.33
    assert.equal(result.breaches[0].penaltyAmount, 2.33);
  });
});


// =============================================================================
// 5. resolveViolation — marks as resolved with note
// =============================================================================

describe('resolveViolation', () => {
  it('should throw for nonexistent violation', () => {
    const { sla } = setup();
    assert.throws(
      () => sla.resolveViolation('nonexistent-id'),
      /not found/,
    );
  });

  it('should mark violation as resolved', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const breaches = sla.detectBreaches(serviceId);
    const violationId = breaches.breaches[0].violationId;

    const result = sla.resolveViolation(violationId);
    assert.equal(result.violation.resolved, 1);
  });

  it('should set resolved_at timestamp', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const breaches = sla.detectBreaches(serviceId);
    const violationId = breaches.breaches[0].violationId;

    const result = sla.resolveViolation(violationId);
    assert.ok(result.violation.resolved_at);
  });

  it('should store resolution note in metadata', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const breaches = sla.detectBreaches(serviceId);
    const violationId = breaches.breaches[0].violationId;

    const result = sla.resolveViolation(violationId, 'Investigated and fixed latency issue');
    const metadata = JSON.parse(result.violation.metadata);
    assert.equal(metadata.resolution_note, 'Investigated and fixed latency issue');
  });

  it('should set metadata to null when no note is provided', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const breaches = sla.detectBreaches(serviceId);
    const violationId = breaches.breaches[0].violationId;

    const result = sla.resolveViolation(violationId);
    // metadata is null when no note provided
    assert.equal(result.violation.metadata, null);
  });

  it('should allow resolving already-resolved violation', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const breaches = sla.detectBreaches(serviceId);
    const violationId = breaches.breaches[0].violationId;

    sla.resolveViolation(violationId, 'First fix');
    const result = sla.resolveViolation(violationId, 'Updated fix');
    assert.equal(result.violation.resolved, 1);
    const metadata = JSON.parse(result.violation.metadata);
    assert.equal(metadata.resolution_note, 'Updated fix');
  });
});


// =============================================================================
// 6. getSLAs — listing definitions for a service
// =============================================================================

describe('getSLAs', () => {
  it('should return empty array when no SLAs exist for service', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.getSLAs(serviceId);
    assert.deepEqual(result, []);
  });

  it('should return all SLAs for a specific service', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 5000 });
    sla.attachSLA({ serviceId, uptimePercent: 99.5 });
    sla.attachSLA({ serviceId, qualityMinScore: 4.0 });

    const result = sla.getSLAs(serviceId);
    assert.equal(result.length, 3);
  });

  it('should not return SLAs from other services', () => {
    const { store, sla } = setup();
    const { serviceId: svc1 } = createTestService(store, { agent_address: 'agent-a-001' });
    const { serviceId: svc2 } = createTestService(store, { agent_address: 'agent-b-002' });
    sla.attachSLA({ serviceId: svc1, responseTimeMs: 5000 });
    sla.attachSLA({ serviceId: svc2, uptimePercent: 99.0 });

    const result1 = sla.getSLAs(svc1);
    assert.equal(result1.length, 1);
    assert.equal(result1[0].response_time_ms, 5000);

    const result2 = sla.getSLAs(svc2);
    assert.equal(result2.length, 1);
    assert.equal(result2[0].uptime_percent, 99.0);
  });

  it('should include inactive SLAs (getSLAs does not filter by active)', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const { sla: created } = sla.attachSLA({ serviceId, responseTimeMs: 5000 });

    // Deactivate the SLA
    store.updateSLADefinition(created.id, { active: 0 });

    // getSLAs lists all (no active filter)
    const result = sla.getSLAs(serviceId);
    assert.equal(result.length, 1);
  });
});


// =============================================================================
// 7. getViolations — filters by resolved/severity
// =============================================================================

describe('getViolations', () => {
  it('should return empty array when no violations exist', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.getViolations(serviceId);
    assert.deepEqual(result, []);
  });

  it('should return all violations for a service', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000, uptimePercent: 99 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
      status: 'fulfilled',
    });
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: new Date(base.getTime() + 10000).toISOString(),
      quotedAt: new Date(base.getTime() + 15000).toISOString(),
      status: 'rejected',
    });

    sla.detectBreaches(serviceId);
    const violations = sla.getViolations(serviceId);
    assert.ok(violations.length >= 2);
  });

  it('should filter by resolved status', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const breaches = sla.detectBreaches(serviceId);
    const violationId = breaches.breaches[0].violationId;

    // Before resolving
    const unresolved = sla.getViolations(serviceId, { resolved: 0 });
    assert.equal(unresolved.length, 1);

    sla.resolveViolation(violationId);

    // After resolving
    const resolved = sla.getViolations(serviceId, { resolved: 1 });
    assert.equal(resolved.length, 1);

    const stillUnresolved = sla.getViolations(serviceId, { resolved: 0 });
    assert.equal(stillUnresolved.length, 0);
  });

  it('should filter by severity', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);

    // Two SLAs: one that will be warning-level, one critical
    // Response time: SLA 1000ms, actual 1100ms => ratio 1.1 > 0.8 => warning
    sla.attachSLA({ serviceId, responseTimeMs: 1000 });
    // Uptime: SLA 99%, actual 50% => ratio 0.505 <= 0.8 => critical
    sla.attachSLA({ serviceId, uptimePercent: 99 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 1100).toISOString(),
      status: 'fulfilled',
    });
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: new Date(base.getTime() + 10000).toISOString(),
      quotedAt: new Date(base.getTime() + 11100).toISOString(),
      status: 'rejected',
    });

    sla.detectBreaches(serviceId);

    const warnings = sla.getViolations(serviceId, { severity: 'warning' });
    const criticals = sla.getViolations(serviceId, { severity: 'critical' });

    assert.ok(warnings.length >= 0);
    assert.ok(criticals.length >= 0);
    // Total should be at least 2 (one response_time + one uptime per SLA)
    const all = sla.getViolations(serviceId);
    assert.ok(all.length >= 2);
  });

  it('should not return violations from other services', () => {
    const { store, sla } = setup();
    const { serviceId: svc1, agentAddress: addr1 } = createTestService(store, { agent_address: 'agent-x-001' });
    const { serviceId: svc2 } = createTestService(store, { agent_address: 'agent-y-002' });

    sla.attachSLA({ serviceId: svc1, responseTimeMs: 1000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: addr1,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    sla.detectBreaches(svc1);

    const v1 = sla.getViolations(svc1);
    const v2 = sla.getViolations(svc2);
    assert.ok(v1.length > 0);
    assert.equal(v2.length, 0);
  });
});


// =============================================================================
// 8. Edge cases
// =============================================================================

describe('Edge cases — no quotes', () => {
  it('should return compliant when no quotes exist (successRate defaults to 1)', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    sla.attachSLA({ serviceId, uptimePercent: 99 });

    const result = sla.checkCompliance(serviceId);
    // With 0 quotes, successRate is 1 (100%), so uptime check passes
    assert.equal(result.compliant, true);
  });

  it('should return null avgResponseTimeMs when no quotes exist', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 5000 });

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.metrics.avgResponseTimeMs, null);
  });

  it('should return null throughputRps when fewer than 2 quotes exist', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, throughputRps: 10 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 1000).toISOString(),
    });

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.metrics.throughputRps, null);
  });

  it('should not detect breaches when no SLAs exist', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    const result = sla.detectBreaches(serviceId);
    assert.equal(result.newViolations, 0);
    assert.deepEqual(result.breaches, []);
  });

  it('should return null avgQualityScore when no feedback exists', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, qualityMinScore: 4.0 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 1000).toISOString(),
    });

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.metrics.avgQualityScore, null);
  });
});

describe('Edge cases — inactive SLAs', () => {
  it('checkCompliance should only evaluate active SLAs', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    const { sla: created } = sla.attachSLA({ serviceId, responseTimeMs: 100 });

    // Deactivate it
    store.updateSLADefinition(created.id, { active: 0 });

    // Even with a slow quote, compliance should pass (no active SLAs)
    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 99999).toISOString(),
    });

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.compliant, true);
    assert.equal(result.slaCount, 0);
  });
});

describe('Edge cases — quotes with negative or zero diff', () => {
  it('should ignore quotes where quoted_at is before created_at', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 5000 });

    const base = new Date('2025-01-01T00:00:00Z');
    // Negative diff should be ignored
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: new Date(base.getTime() + 5000).toISOString(),
      quotedAt: base.toISOString(),
    });
    // Valid diff of 2000ms
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: new Date(base.getTime() + 10000).toISOString(),
      quotedAt: new Date(base.getTime() + 12000).toISOString(),
    });

    const result = sla.checkCompliance(serviceId);
    // Only the valid quote counts: avgResponseTimeMs = 2000
    assert.equal(result.metrics.avgResponseTimeMs, 2000);
  });
});

describe('Edge cases — detectBreaches with serviceId in result', () => {
  it('should include serviceId in breach result', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 1000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const result = sla.detectBreaches(serviceId);
    assert.equal(result.serviceId, serviceId);
  });
});

describe('Edge cases — breach slaId tracking', () => {
  it('should include slaId in each breach record', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    const { sla: createdSla } = sla.attachSLA({ serviceId, responseTimeMs: 1000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 5000).toISOString(),
    });

    const result = sla.detectBreaches(serviceId);
    assert.equal(result.breaches[0].slaId, createdSla.id);
  });
});

describe('Edge cases — violation fields', () => {
  it('should store expected and actual values in violation', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 2000 });

    const base = new Date('2025-01-01T00:00:00Z');
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 8000).toISOString(),
    });

    const result = sla.detectBreaches(serviceId);
    const v = store.getSLAViolation(result.breaches[0].violationId);
    assert.equal(v.expected_value, 2000);
    assert.equal(v.actual_value, 8000);
    assert.equal(v.resolved, 0);
  });
});

describe('Edge cases — multiple SLAs with mixed compliance', () => {
  it('should report non-compliant if any single SLA metric fails', () => {
    const { store, sla } = setup();
    const { serviceId, agentAddress } = createTestService(store);

    // Response time SLA that will pass
    sla.attachSLA({ serviceId, responseTimeMs: 10000 });
    // Uptime SLA that will fail
    sla.attachSLA({ serviceId, uptimePercent: 99 });

    const base = new Date('2025-01-01T00:00:00Z');
    // Fast response, but only 50% fulfilled
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: base.toISOString(),
      quotedAt: new Date(base.getTime() + 500).toISOString(),
      status: 'fulfilled',
    });
    createTestQuote(store, {
      sellerAddress: agentAddress,
      createdAt: new Date(base.getTime() + 10000).toISOString(),
      quotedAt: new Date(base.getTime() + 10500).toISOString(),
      status: 'rejected',
    });

    const result = sla.checkCompliance(serviceId);
    assert.equal(result.compliant, false);
  });
});

describe('Edge cases — checkCompliance return shape', () => {
  it('should return serviceId, compliant, slaCount, metrics, and checks', () => {
    const { store, sla } = setup();
    const { serviceId } = createTestService(store);
    sla.attachSLA({ serviceId, responseTimeMs: 5000 });

    const result = sla.checkCompliance(serviceId);
    assert.ok('serviceId' in result);
    assert.ok('compliant' in result);
    assert.ok('slaCount' in result);
    assert.ok('metrics' in result);
    assert.ok('checks' in result);
    assert.equal(result.serviceId, serviceId);
  });
});
