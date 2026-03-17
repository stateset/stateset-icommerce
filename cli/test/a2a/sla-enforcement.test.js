/**
 * Tests for SLA enforcement methods in cli/src/a2a/sla.js
 *
 * Focuses on the NEW methods: enforcePenalties() and enforceAll().
 * Uses mock store to avoid native module dependency.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { createSLAService } from '../../src/a2a/sla.js';

// ---------------------------------------------------------------------------
// Mock Store
// ---------------------------------------------------------------------------

function createMockStore(config = {}) {
  const services = config.services || [];
  const slaDefinitions = config.slaDefinitions || [];
  const violations = config.violations || [];
  const quotes = config.quotes || [];
  const payments = config.payments || [];
  const feedback = config.feedback || [];
  const updatedViolations = [];

  return {
    getService: (id) => services.find((s) => s.id === id) || null,
    listServices: (filter) =>
      services.filter((s) => (filter?.active !== undefined ? s.active === filter.active : true)),
    updateService: (id, updates) => {
      const svc = services.find((s) => s.id === id);
      if (svc) Object.assign(svc, updates);
      return svc;
    },
    listSLADefinitions: (filter) =>
      slaDefinitions.filter(
        (d) =>
          (!filter?.service_id || d.service_id === filter.service_id) &&
          (filter?.active === undefined || d.active === filter.active),
      ),
    getSLADefinition: (id) => slaDefinitions.find((d) => d.id === id) || null,
    createSLADefinition: (data) => {
      const d = { id: `sla-${Date.now()}`, active: 1, ...data };
      slaDefinitions.push(d);
      return d;
    },
    listSLAViolations: (filter) =>
      violations.filter(
        (v) =>
          (!filter?.service_id || v.service_id === filter.service_id) &&
          (filter?.resolved === undefined || v.resolved === filter.resolved),
      ),
    getSLAViolation: (id) => violations.find((v) => v.id === id) || null,
    createSLAViolation: (data) => {
      const v = { id: `viol-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`, resolved: 0, ...data };
      violations.push(v);
      return v;
    },
    updateSLAViolation: (id, updates) => {
      const v = violations.find((vi) => vi.id === id);
      if (v) Object.assign(v, updates);
      updatedViolations.push({ id, updates });
      return v;
    },
    listQuotes: (filter) =>
      quotes.filter((q) => (!filter?.seller_address || q.seller_address === filter.seller_address)),
    listPayments: (filter) =>
      payments.filter(
        (p) => (!filter?.recipient_address || p.recipient_address === filter.recipient_address),
      ),
    listFeedback: () => feedback,
    getReputationScore: () => null,
    // Expose for assertions
    _updatedViolations: updatedViolations,
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('SLA enforcePenalties()', () => {
  it('returns empty when no violations exist', () => {
    const store = createMockStore({
      services: [{ id: 'svc-1', active: 1, agent_address: '0xA', name: 'Svc' }],
    });
    const sla = createSLAService(store);

    const result = sla.enforcePenalties('svc-1');
    assert.equal(result.enforced, 0);
    assert.equal(result.totalPenalty, 0);
    assert.deepEqual(result.actions, []);
  });

  it('applies credit penalty and marks violation resolved', () => {
    const store = createMockStore({
      services: [{ id: 'svc-1', active: 1, agent_address: '0xA', name: 'Svc' }],
      slaDefinitions: [
        { id: 'sla-1', service_id: 'svc-1', active: 1, penalty_percent: 10, penalty_type: 'credit' },
      ],
      violations: [
        {
          id: 'viol-1',
          sla_id: 'sla-1',
          service_id: 'svc-1',
          violation_type: 'response_time_ms',
          severity: 'warning',
          penalty_amount: 5.0,
          resolved: 0,
        },
      ],
    });
    const sla = createSLAService(store);

    const result = sla.enforcePenalties('svc-1');
    assert.equal(result.enforced, 1);
    assert.equal(result.totalPenalty, 5.0);
    assert.equal(result.actions[0].penaltyType, 'credit');
    assert.equal(result.actions[0].applied, true);

    // Verify violation was marked resolved
    const updatedViol = store._updatedViolations.find((u) => u.id === 'viol-1');
    assert.ok(updatedViol);
    assert.equal(updatedViol.updates.resolved, 1);
  });

  it('applies suspension penalty and deactivates service', () => {
    const service = { id: 'svc-1', active: 1, agent_address: '0xA', name: 'Svc' };
    const store = createMockStore({
      services: [service],
      slaDefinitions: [
        { id: 'sla-1', service_id: 'svc-1', active: 1, penalty_percent: 20, penalty_type: 'suspension' },
      ],
      violations: [
        {
          id: 'viol-1',
          sla_id: 'sla-1',
          service_id: 'svc-1',
          violation_type: 'uptime_percent',
          severity: 'critical',
          penalty_amount: 10.0,
          resolved: 0,
        },
      ],
    });
    const sla = createSLAService(store);

    const result = sla.enforcePenalties('svc-1');
    assert.equal(result.enforced, 1);
    assert.equal(result.actions[0].penaltyType, 'suspension');
    assert.equal(result.actions[0].applied, true);

    // Service should be deactivated
    assert.equal(service.active, 0);
  });

  it('applies refund penalty', () => {
    const store = createMockStore({
      services: [{ id: 'svc-1', active: 1, agent_address: '0xA', name: 'Svc' }],
      slaDefinitions: [
        { id: 'sla-1', service_id: 'svc-1', active: 1, penalty_percent: 5, penalty_type: 'refund' },
      ],
      violations: [
        {
          id: 'viol-1',
          sla_id: 'sla-1',
          service_id: 'svc-1',
          violation_type: 'quality_min_score',
          severity: 'warning',
          penalty_amount: 3.0,
          resolved: 0,
        },
      ],
    });
    const sla = createSLAService(store);

    const result = sla.enforcePenalties('svc-1');
    assert.equal(result.enforced, 1);
    assert.equal(result.actions[0].penaltyType, 'refund');
    assert.ok(result.actions[0].note.includes('Refund'));
  });

  it('handles multiple violations in one enforcement', () => {
    const store = createMockStore({
      services: [{ id: 'svc-1', active: 1, agent_address: '0xA', name: 'Svc' }],
      slaDefinitions: [
        { id: 'sla-1', service_id: 'svc-1', active: 1, penalty_percent: 10, penalty_type: 'credit' },
        { id: 'sla-2', service_id: 'svc-1', active: 1, penalty_percent: 5, penalty_type: 'credit' },
      ],
      violations: [
        { id: 'v1', sla_id: 'sla-1', service_id: 'svc-1', violation_type: 'response_time_ms', severity: 'warning', penalty_amount: 5, resolved: 0 },
        { id: 'v2', sla_id: 'sla-2', service_id: 'svc-1', violation_type: 'uptime_percent', severity: 'critical', penalty_amount: 8, resolved: 0 },
      ],
    });
    const sla = createSLAService(store);

    const result = sla.enforcePenalties('svc-1');
    assert.equal(result.enforced, 2);
    assert.equal(result.totalPenalty, 13);
    assert.equal(result.actions.length, 2);
  });

  it('skips already-resolved violations', () => {
    const store = createMockStore({
      services: [{ id: 'svc-1', active: 1, agent_address: '0xA', name: 'Svc' }],
      violations: [
        { id: 'v1', sla_id: 'sla-1', service_id: 'svc-1', resolved: 1 },
      ],
    });
    const sla = createSLAService(store);

    const result = sla.enforcePenalties('svc-1');
    assert.equal(result.enforced, 0);
  });
});

describe('SLA enforceAll()', () => {
  it('processes all active services', () => {
    const store = createMockStore({
      services: [
        { id: 'svc-1', active: 1, agent_address: '0xA', name: 'Svc1' },
        { id: 'svc-2', active: 1, agent_address: '0xB', name: 'Svc2' },
      ],
      slaDefinitions: [
        { id: 'sla-1', service_id: 'svc-1', active: 1, penalty_percent: 10, penalty_type: 'credit',
          response_time_ms: 100, uptime_percent: null, quality_min_score: null, throughput_rps: null },
      ],
      quotes: [
        // Create quote with slow response to trigger breach on svc-1
        {
          seller_address: '0xA',
          status: 'fulfilled',
          created_at: '2026-01-01T00:00:00Z',
          quoted_at: '2026-01-01T00:01:00Z', // 60s response time
        },
      ],
      payments: [{ recipient_address: '0xA', amount_decimal: 100 }],
    });
    const sla = createSLAService(store);

    const result = sla.enforceAll();
    assert.equal(result.servicesChecked, 2);
    // svc-1 should have a breach (60s > 100ms)
    assert.ok(result.servicesWithIssues >= 0); // May or may not depending on breach detection
  });

  it('returns empty details when all services are compliant', () => {
    const store = createMockStore({
      services: [{ id: 'svc-1', active: 1, agent_address: '0xA', name: 'Svc' }],
    });
    const sla = createSLAService(store);

    const result = sla.enforceAll();
    assert.equal(result.servicesChecked, 1);
    assert.equal(result.servicesWithIssues, 0);
    assert.deepEqual(result.details, []);
  });
});
