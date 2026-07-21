/**
 * Unit tests for compliance/exports.js — Compliance & Regulatory Exports
 *
 * Covers: exportAuditTrail, generate1099K, generateGDPRExport, deleteGDPRData,
 * generateComplianceSummary, generateSOC2Evidence, recordsToCSV helper, edge cases
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';
import { createComplianceService } from '../../src/compliance/exports.js';

// ---------------------------------------------------------------------------
// Schema — mirrors tables the compliance service queries
// ---------------------------------------------------------------------------

const SCHEMA = `
CREATE TABLE IF NOT EXISTS a2a_payments (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL DEFAULT 'pending',
  sender_agent_id TEXT,
  sender_address TEXT NOT NULL,
  recipient_agent_id TEXT,
  recipient_address TEXT NOT NULL,
  amount INTEGER NOT NULL,
  amount_decimal REAL NOT NULL,
  asset TEXT NOT NULL DEFAULT 'USDC',
  network TEXT NOT NULL DEFAULT 'set_chain',
  memo TEXT,
  reference_type TEXT,
  reference_id TEXT,
  idempotency_key TEXT UNIQUE,
  intent_id TEXT,
  tx_hash TEXT,
  block_number INTEGER,
  metadata TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT
);

CREATE TABLE IF NOT EXISTS a2a_disputes (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL DEFAULT 'filed',
  escrow_id TEXT NOT NULL,
  quote_id TEXT,
  filed_by TEXT NOT NULL,
  filed_against TEXT NOT NULL,
  reason TEXT NOT NULL,
  category TEXT NOT NULL DEFAULT 'non_delivery',
  amount_disputed INTEGER NOT NULL,
  amount_decimal REAL NOT NULL,
  asset TEXT NOT NULL DEFAULT 'USDC',
  resolution_type TEXT,
  resolution_amount INTEGER,
  resolution_note TEXT,
  resolved_by TEXT,
  evidence_deadline TEXT,
  review_deadline TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  resolved_at TEXT
);

CREATE TABLE IF NOT EXISTS a2a_circuit_breaker_events (
  id TEXT PRIMARY KEY,
  agent_name TEXT NOT NULL,
  event_type TEXT NOT NULL,
  reason TEXT,
  amount REAL,
  state_before TEXT,
  state_after TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_cb_events_agent ON a2a_circuit_breaker_events(agent_name);
CREATE INDEX IF NOT EXISTS idx_cb_events_type ON a2a_circuit_breaker_events(event_type);

CREATE TABLE IF NOT EXISTS a2a_spending_ledger (
  id TEXT PRIMARY KEY,
  agent_name TEXT NOT NULL,
  amount REAL NOT NULL,
  success INTEGER NOT NULL DEFAULT 1,
  error TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_spending_agent ON a2a_spending_ledger(agent_name);
CREATE INDEX IF NOT EXISTS idx_spending_created ON a2a_spending_ledger(created_at);

CREATE TABLE IF NOT EXISTS agent_cards (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  wallet_address TEXT UNIQUE NOT NULL,
  public_key TEXT,
  supported_networks TEXT DEFAULT '["set_chain"]',
  supported_assets TEXT DEFAULT '["USDC"]',
  a2a_skills TEXT DEFAULT '["buy","sell","quote"]',
  endpoint_url TEXT,
  description TEXT,
  trust_level TEXT DEFAULT 'sandbox',
  active INTEGER DEFAULT 1,
  suspended_at TEXT,
  created_at TEXT,
  updated_at TEXT
);

CREATE TABLE IF NOT EXISTS a2a_notification_log (
  id TEXT PRIMARY KEY,
  recipient_address TEXT NOT NULL,
  endpoint_url TEXT NOT NULL DEFAULT '',
  event_type TEXT NOT NULL,
  payload TEXT NOT NULL DEFAULT '{}',
  signature TEXT,
  status TEXT NOT NULL DEFAULT 'pending',
  attempts INTEGER NOT NULL DEFAULT 0,
  last_attempt_at TEXT,
  last_error TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS a2a_sla_violations (
  id TEXT PRIMARY KEY,
  sla_id TEXT NOT NULL,
  service_id TEXT NOT NULL,
  violation_type TEXT NOT NULL DEFAULT 'breach',
  expected_value REAL NOT NULL DEFAULT 0,
  actual_value REAL NOT NULL DEFAULT 0,
  metric TEXT NOT NULL DEFAULT 'latency',
  severity TEXT NOT NULL DEFAULT 'warning',
  penalty_amount REAL,
  resolved INTEGER NOT NULL DEFAULT 0,
  metadata TEXT,
  created_at TEXT NOT NULL
);
`;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeStore() {
  const db = new Database(':memory:');
  db.pragma('journal_mode = WAL');
  db.exec(SCHEMA);
  return { db };
}

function seedPayments(store, count, opts = {}) {
  const now = new Date();
  for (let i = 0; i < count; i++) {
    const created = new Date(now - i * 86400000).toISOString();
    store.db
      .prepare(
        `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, network, memo, created_at, updated_at, completed_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        opts.idPrefix ? `${opts.idPrefix}-${i}` : `pay-${i}`,
        opts.status || 'completed',
        opts.sender || '0xSender',
        opts.recipient || '0xRecipient',
        (opts.amount || 100) * 100,
        opts.amount || 100,
        opts.asset || 'USDC',
        opts.network || 'set_chain',
        opts.memo || null,
        created,
        created,
        (opts.status || 'completed') === 'completed' ? created : null,
      );
  }
}

function seedCircuitBreakerEvents(store, count, opts = {}) {
  const now = new Date();
  for (let i = 0; i < count; i++) {
    const created = new Date(now - i * 86400000).toISOString();
    store.db
      .prepare(
        `INSERT INTO a2a_circuit_breaker_events (id, agent_name, event_type, reason, amount, state_before, state_after, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        `cb-${i}`,
        opts.agentName || 'agent-1',
        opts.eventType || 'trip',
        opts.reason || 'spending limit exceeded',
        opts.amount || 500,
        opts.stateBefore || 'closed',
        opts.stateAfter || 'open',
        created,
      );
  }
}

function seedSpendingLedger(store, count, opts = {}) {
  const now = new Date();
  for (let i = 0; i < count; i++) {
    const created = new Date(now - i * 86400000).toISOString();
    store.db
      .prepare(
        `INSERT INTO a2a_spending_ledger (id, agent_name, amount, success, error, created_at)
       VALUES (?, ?, ?, ?, ?, ?)`,
      )
      .run(
        `spend-${i}`,
        opts.agentName || 'agent-1',
        opts.amount || 100,
        opts.success !== undefined ? opts.success : 1,
        opts.error || null,
        created,
      );
  }
}

function seedAgentCard(store, opts = {}) {
  const now = new Date().toISOString();
  store.db
    .prepare(
      `INSERT INTO agent_cards (id, name, wallet_address, description, trust_level, active, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      opts.id || 'card-1',
      opts.name || 'TestAgent',
      opts.walletAddress || '0xTestWallet',
      opts.description || 'A test agent',
      opts.trustLevel || 'sandbox',
      opts.active !== undefined ? opts.active : 1,
      opts.createdAt || now,
      opts.updatedAt || now,
    );
}

function seedNotifications(store, count, opts = {}) {
  const now = new Date();
  for (let i = 0; i < count; i++) {
    const created = new Date(now - i * 86400000).toISOString();
    store.db
      .prepare(
        `INSERT INTO a2a_notification_log (id, recipient_address, endpoint_url, event_type, payload, status, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        `notif-${i}`,
        opts.recipientAddress || '0xTestWallet',
        opts.endpointUrl || 'https://example.com/webhook',
        opts.eventType || 'payment.completed',
        opts.payload || '{"test": true}',
        opts.status || 'delivered',
        created,
      );
  }
}

function seedDisputes(store, count, opts = {}) {
  const now = new Date();
  for (let i = 0; i < count; i++) {
    const created = new Date(now - i * 86400000).toISOString();
    store.db
      .prepare(
        `INSERT INTO a2a_disputes (id, status, escrow_id, filed_by, filed_against, reason, category, amount_disputed, amount_decimal, asset, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        opts.idPrefix ? `${opts.idPrefix}-${i}` : `dispute-${i}`,
        opts.status || 'filed',
        opts.escrowId || `escrow-${i}`,
        opts.filedBy || '0xComplainant',
        opts.filedAgainst || '0xRespondent',
        opts.reason || 'Non-delivery',
        opts.category || 'non_delivery',
        (opts.amount || 50) * 100,
        opts.amount || 50,
        opts.asset || 'USDC',
        created,
        created,
      );
  }
}

function seedSLAViolations(store, count, opts = {}) {
  const now = new Date();
  for (let i = 0; i < count; i++) {
    const created = new Date(now - i * 86400000).toISOString();
    store.db
      .prepare(
        `INSERT INTO a2a_sla_violations (id, sla_id, service_id, metric, severity, created_at)
       VALUES (?, ?, ?, ?, ?, ?)`,
      )
      .run(
        `sla-v-${i}`,
        opts.slaId || `sla-${i}`,
        opts.serviceId || 'svc-1',
        opts.metric || 'latency',
        opts.severity || 'warning',
        created,
      );
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('createComplianceService', () => {
  it('throws if store has no .db', () => {
    assert.throws(() => createComplianceService(null), /requires a store/);
    assert.throws(() => createComplianceService({}), /requires a store/);
  });

  it('returns an object with all expected methods', () => {
    const store = makeStore();
    const svc = createComplianceService(store);
    assert.equal(typeof svc.exportAuditTrail, 'function');
    assert.equal(typeof svc.generate1099K, 'function');
    assert.equal(typeof svc.generateGDPRExport, 'function');
    assert.equal(typeof svc.deleteGDPRData, 'function');
    assert.equal(typeof svc.generateComplianceSummary, 'function');
    assert.equal(typeof svc.generateSOC2Evidence, 'function');
  });
});

// ===========================================================================
// exportAuditTrail
// ===========================================================================

describe('exportAuditTrail', () => {
  let store, svc;

  beforeEach(() => {
    store = makeStore();
    svc = createComplianceService(store);
  });

  it('returns empty records when no data exists', () => {
    const result = svc.exportAuditTrail();
    assert.equal(result.count, 0);
    assert.deepEqual(result.records, []);
    assert.equal(result.format, 'json');
  });

  it('returns payment records', () => {
    seedPayments(store, 3);
    const result = svc.exportAuditTrail();
    const paymentRecords = result.records.filter((r) => r.source === 'payment');
    assert.ok(paymentRecords.length >= 3);
  });

  it('returns circuit breaker events', () => {
    seedCircuitBreakerEvents(store, 2);
    const result = svc.exportAuditTrail();
    const cbRecords = result.records.filter((r) => r.source === 'circuit_breaker');
    assert.equal(cbRecords.length, 2);
  });

  it('returns spending ledger records', () => {
    seedSpendingLedger(store, 4);
    const result = svc.exportAuditTrail();
    const spendRecords = result.records.filter((r) => r.source === 'spending_ledger');
    assert.equal(spendRecords.length, 4);
  });

  it('combines records from all sources', () => {
    seedPayments(store, 2);
    seedCircuitBreakerEvents(store, 2);
    seedSpendingLedger(store, 2);
    const result = svc.exportAuditTrail();
    const sources = new Set(result.records.map((r) => r.source));
    assert.ok(sources.has('payment'));
    assert.ok(sources.has('circuit_breaker'));
    assert.ok(sources.has('spending_ledger'));
    assert.equal(result.count, 6);
  });

  it('sorts records by created_at DESC', () => {
    seedPayments(store, 5);
    const result = svc.exportAuditTrail();
    for (let i = 1; i < result.records.length; i++) {
      assert.ok(result.records[i - 1].created_at >= result.records[i].created_at);
    }
  });

  it('respects date range filtering (from/to)', () => {
    seedPayments(store, 10);
    const now = new Date();
    const from = new Date(now - 3 * 86400000).toISOString();
    const to = now.toISOString();
    const result = svc.exportAuditTrail({ from, to });
    // Only payments within last 3 days
    assert.ok(result.count <= 4); // 0, 1, 2, 3 days ago
    for (const r of result.records) {
      assert.ok(r.created_at >= from);
      assert.ok(r.created_at <= to);
    }
  });

  it('filters by agent name', () => {
    seedPayments(store, 3, { sender: 'agent-A', recipient: 'agent-B' });
    seedPayments(store, 2, { sender: 'agent-C', recipient: 'agent-D', idPrefix: 'pay2' });
    seedCircuitBreakerEvents(store, 2, { agentName: 'agent-A' });
    const result = svc.exportAuditTrail({ agentName: 'agent-A' });
    for (const r of result.records) {
      if (r.source === 'payment') {
        assert.ok(r.sender_address === 'agent-A' || r.recipient_address === 'agent-A');
      } else if (r.source === 'circuit_breaker') {
        assert.equal(r.agent_name, 'agent-A');
      }
    }
  });

  it('filters by event type', () => {
    seedPayments(store, 3);
    seedCircuitBreakerEvents(store, 2, { eventType: 'trip' });
    const result = svc.exportAuditTrail({ eventType: 'trip' });
    const cbRecords = result.records.filter((r) => r.source === 'circuit_breaker');
    assert.equal(cbRecords.length, 2);
    // Should not include payments when filtering for trip events
    const paymentRecords = result.records.filter((r) => r.source === 'payment');
    assert.equal(paymentRecords.length, 0);
  });

  it('respects limit parameter', () => {
    seedPayments(store, 20);
    const result = svc.exportAuditTrail({ limit: 5 });
    assert.equal(result.count, 5);
    assert.equal(result.records.length, 5);
    assert.ok(result.totalAvailable >= 5);
  });

  it('uses json format by default', () => {
    const result = svc.exportAuditTrail();
    assert.equal(result.format, 'json');
    assert.equal(result.csv, undefined);
  });

  it('produces CSV when format=csv', () => {
    seedPayments(store, 3);
    const result = svc.exportAuditTrail({ format: 'csv' });
    assert.equal(result.format, 'csv');
    assert.equal(typeof result.csv, 'string');
    const lines = result.csv.split('\n');
    // header + 3 data rows
    assert.ok(lines.length >= 4);
    assert.ok(lines[0].includes('source'));
  });

  it('includes period in result', () => {
    const result = svc.exportAuditTrail();
    assert.ok(result.period);
    assert.ok(result.period.from);
    assert.ok(result.period.to);
  });
});

// ===========================================================================
// generate1099K
// ===========================================================================

describe('generate1099K', () => {
  let store, svc;

  beforeEach(() => {
    store = makeStore();
    svc = createComplianceService(store);
  });

  it('throws if year is missing', () => {
    assert.throws(() => svc.generate1099K({ agentAddress: '0xAgent' }), /year and agentAddress/);
  });

  it('throws if agentAddress is missing', () => {
    assert.throws(() => svc.generate1099K({ year: 2025 }), /year and agentAddress/);
  });

  it('returns correct gross amount for completed payments', () => {
    const year = new Date().getFullYear();
    // Seed payments within current year
    for (let i = 0; i < 5; i++) {
      const d = new Date(year, 3, i + 1).toISOString();
      store.db
        .prepare(
          `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, created_at, updated_at, completed_at)
         VALUES (?, 'completed', '0xBuyer', '0xSeller', ?, ?, 'USDC', ?, ?, ?)`,
        )
        .run(`1099-pay-${i}`, 5000, 50, d, d, d);
    }
    const result = svc.generate1099K({ year, agentAddress: '0xSeller' });
    assert.equal(result.grossAmount, 250);
    assert.equal(result.transactionCount, 5);
  });

  it('returns correct transaction count', () => {
    const year = new Date().getFullYear();
    for (let i = 0; i < 3; i++) {
      const d = new Date(year, 6, i + 1).toISOString();
      store.db
        .prepare(
          `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, created_at, updated_at, completed_at)
         VALUES (?, 'completed', '0xBuyer', '0xSeller', 10000, 100, 'USDC', ?, ?, ?)`,
        )
        .run(`tc-pay-${i}`, d, d, d);
    }
    const result = svc.generate1099K({ year, agentAddress: '0xSeller' });
    assert.equal(result.transactionCount, 3);
  });

  it('returns 12 months in output', () => {
    const year = new Date().getFullYear();
    const result = svc.generate1099K({ year, agentAddress: '0xSeller' });
    assert.equal(result.months.length, 12);
    assert.equal(result.months[0].month, 1);
    assert.equal(result.months[11].month, 12);
  });

  it('handles no transactions gracefully', () => {
    const result = svc.generate1099K({ year: 2025, agentAddress: '0xNobody' });
    assert.equal(result.grossAmount, 0);
    assert.equal(result.transactionCount, 0);
    assert.equal(result.months.length, 12);
    for (const m of result.months) {
      assert.equal(m.amount, 0);
      assert.equal(m.count, 0);
    }
  });

  it('filters by year', () => {
    // Payment in 2024
    const d2024 = '2024-06-15T12:00:00.000Z';
    store.db
      .prepare(
        `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, created_at, updated_at, completed_at)
       VALUES ('old-pay', 'completed', '0xBuyer', '0xSeller', 10000, 100, 'USDC', ?, ?, ?)`,
      )
      .run(d2024, d2024, d2024);
    // Payment in 2025
    const d2025 = '2025-06-15T12:00:00.000Z';
    store.db
      .prepare(
        `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, created_at, updated_at, completed_at)
       VALUES ('new-pay', 'completed', '0xBuyer', '0xSeller', 20000, 200, 'USDC', ?, ?, ?)`,
      )
      .run(d2025, d2025, d2025);

    const result2024 = svc.generate1099K({ year: 2024, agentAddress: '0xSeller' });
    assert.equal(result2024.grossAmount, 100);
    const result2025 = svc.generate1099K({ year: 2025, agentAddress: '0xSeller' });
    assert.equal(result2025.grossAmount, 200);
  });

  it('filters by agent address', () => {
    const year = new Date().getFullYear();
    const d = new Date(year, 5, 1).toISOString();
    store.db
      .prepare(
        `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, created_at, updated_at, completed_at)
       VALUES ('aa-pay', 'completed', '0xBuyer', '0xAgentA', 5000, 50, 'USDC', ?, ?, ?)`,
      )
      .run(d, d, d);
    store.db
      .prepare(
        `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, created_at, updated_at, completed_at)
       VALUES ('bb-pay', 'completed', '0xBuyer', '0xAgentB', 7500, 75, 'USDC', ?, ?, ?)`,
      )
      .run(d, d, d);

    const resultA = svc.generate1099K({ year, agentAddress: '0xAgentA' });
    assert.equal(resultA.grossAmount, 50);
    const resultB = svc.generate1099K({ year, agentAddress: '0xAgentB' });
    assert.equal(resultB.grossAmount, 75);
  });

  it('handles partial year data with correct monthly breakdown', () => {
    const year = new Date().getFullYear();
    // Only seed January and March
    const jan = new Date(year, 0, 15).toISOString();
    const mar = new Date(year, 2, 15).toISOString();
    store.db
      .prepare(
        `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, created_at, updated_at, completed_at)
       VALUES ('jan-pay', 'completed', '0xBuyer', '0xSeller', 10000, 100, 'USDC', ?, ?, ?)`,
      )
      .run(jan, jan, jan);
    store.db
      .prepare(
        `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, created_at, updated_at, completed_at)
       VALUES ('mar-pay', 'completed', '0xBuyer', '0xSeller', 20000, 200, 'USDC', ?, ?, ?)`,
      )
      .run(mar, mar, mar);

    const result = svc.generate1099K({ year, agentAddress: '0xSeller' });
    assert.equal(result.months[0].amount, 100); // January
    assert.equal(result.months[0].count, 1);
    assert.equal(result.months[1].amount, 0); // February
    assert.equal(result.months[2].amount, 200); // March
    assert.equal(result.months[2].count, 1);
  });

  it('excludes non-completed payments', () => {
    const year = new Date().getFullYear();
    const d = new Date(year, 5, 1).toISOString();
    store.db
      .prepare(
        `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, created_at, updated_at)
       VALUES ('pending-pay', 'pending', '0xBuyer', '0xSeller', 99900, 999, 'USDC', ?, ?)`,
      )
      .run(d, d);
    store.db
      .prepare(
        `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, created_at, updated_at, completed_at)
       VALUES ('done-pay', 'completed', '0xBuyer', '0xSeller', 5000, 50, 'USDC', ?, ?, ?)`,
      )
      .run(d, d, d);

    const result = svc.generate1099K({ year, agentAddress: '0xSeller' });
    assert.equal(result.grossAmount, 50);
    assert.equal(result.transactionCount, 1);
  });

  it('includes payee and generatedAt fields', () => {
    const result = svc.generate1099K({ year: 2025, agentAddress: '0xPayee' });
    assert.equal(result.payee, '0xPayee');
    assert.equal(result.year, 2025);
    assert.ok(result.generatedAt);
  });
});

// ===========================================================================
// generateGDPRExport
// ===========================================================================

describe('generateGDPRExport', () => {
  let store, svc;

  beforeEach(() => {
    store = makeStore();
    svc = createComplianceService(store);
  });

  it('throws if customerId is missing', () => {
    assert.throws(() => svc.generateGDPRExport(), /customerId is required/);
    assert.throws(() => svc.generateGDPRExport(''), /customerId is required/);
  });

  it('includes personal data from agent_cards', () => {
    seedAgentCard(store, { walletAddress: '0xUser1', name: 'User One' });
    const result = svc.generateGDPRExport('0xUser1');
    assert.ok(result.personalData.length >= 1);
    assert.equal(result.personalData[0].wallet_address, '0xUser1');
  });

  it('includes payments as sender and recipient', () => {
    seedPayments(store, 2, { sender: '0xUser1', recipient: '0xOther' });
    seedPayments(store, 3, { sender: '0xOther', recipient: '0xUser1', idPrefix: 'recv' });
    const result = svc.generateGDPRExport('0xUser1');
    assert.equal(result.payments.length, 5);
  });

  it('includes communications from notification log', () => {
    seedNotifications(store, 4, { recipientAddress: '0xUser1' });
    const result = svc.generateGDPRExport('0xUser1');
    assert.equal(result.communications.length, 4);
  });

  it('includes disputes filed by or against the customer', () => {
    seedDisputes(store, 2, { filedBy: '0xUser1', filedAgainst: '0xOther' });
    seedDisputes(store, 1, { filedBy: '0xOther', filedAgainst: '0xUser1', idPrefix: 'disp2' });
    const result = svc.generateGDPRExport('0xUser1');
    assert.equal(result.disputes.length, 3);
  });

  it('includes export timestamp', () => {
    const result = svc.generateGDPRExport('0xUser1');
    assert.ok(result.exportedAt);
    assert.ok(new Date(result.exportedAt).getTime() > 0);
  });

  it('returns customerId in result', () => {
    const result = svc.generateGDPRExport('0xUser1');
    assert.equal(result.customerId, '0xUser1');
  });

  it('handles unknown customer with empty arrays', () => {
    const result = svc.generateGDPRExport('0xNobody');
    assert.deepEqual(result.personalData, []);
    assert.deepEqual(result.payments, []);
    assert.deepEqual(result.communications, []);
    assert.deepEqual(result.disputes, []);
  });

  it('matches by agent card ID as well', () => {
    seedAgentCard(store, { id: 'agent-uuid-1', walletAddress: '0xWallet1', name: 'AgentX' });
    const result = svc.generateGDPRExport('agent-uuid-1');
    assert.ok(result.personalData.length >= 1);
  });
});

// ===========================================================================
// deleteGDPRData
// ===========================================================================

describe('deleteGDPRData', () => {
  let store, svc;

  beforeEach(() => {
    store = makeStore();
    svc = createComplianceService(store);
  });

  it('throws if customerId is missing', () => {
    assert.throws(() => svc.deleteGDPRData(), /customerId is required/);
    assert.throws(() => svc.deleteGDPRData(''), /customerId is required/);
  });

  it('deletes agent card data', () => {
    seedAgentCard(store, { walletAddress: '0xDelete' });
    const result = svc.deleteGDPRData('0xDelete');
    assert.ok(result.deleted.some((d) => d.table === 'agent_cards'));
    const remaining = store.db
      .prepare('SELECT COUNT(*) AS cnt FROM agent_cards WHERE wallet_address = ?')
      .get('0xDelete');
    assert.equal(remaining.cnt, 0);
  });

  it('deletes notification log entries', () => {
    seedNotifications(store, 3, { recipientAddress: '0xDelete' });
    const result = svc.deleteGDPRData('0xDelete');
    assert.ok(result.deleted.some((d) => d.table === 'a2a_notification_log'));
    const remaining = store.db
      .prepare('SELECT COUNT(*) AS cnt FROM a2a_notification_log WHERE recipient_address = ?')
      .get('0xDelete');
    assert.equal(remaining.cnt, 0);
  });

  it('deletes payment records when keepTransactions=false', () => {
    seedPayments(store, 3, { sender: '0xDelete' });
    const result = svc.deleteGDPRData('0xDelete', { keepTransactions: false });
    assert.ok(result.deleted.some((d) => d.table.includes('a2a_payments')));
    const remaining = store.db
      .prepare('SELECT COUNT(*) AS cnt FROM a2a_payments WHERE sender_address = ?')
      .get('0xDelete');
    assert.equal(remaining.cnt, 0);
  });

  it('anonymizes payments when keepTransactions=true', () => {
    seedPayments(store, 3, { sender: '0xDelete' });
    const result = svc.deleteGDPRData('0xDelete', { keepTransactions: true });
    assert.ok(result.retained.some((r) => r.table === 'a2a_payments' && r.action === 'anonymized'));
    // Original address should be gone
    const original = store.db
      .prepare('SELECT COUNT(*) AS cnt FROM a2a_payments WHERE sender_address = ?')
      .get('0xDelete');
    assert.equal(original.cnt, 0);
    // Anonymized records should exist
    const anon = store.db
      .prepare("SELECT COUNT(*) AS cnt FROM a2a_payments WHERE sender_address LIKE 'anon_%'")
      .get();
    assert.ok(anon.cnt >= 3);
  });

  it('anonymizes disputes when keepTransactions=true', () => {
    seedDisputes(store, 2, { filedBy: '0xDelete' });
    const result = svc.deleteGDPRData('0xDelete', { keepTransactions: true });
    assert.ok(result.retained.some((r) => r.table === 'a2a_disputes' && r.action === 'anonymized'));
    const original = store.db
      .prepare('SELECT COUNT(*) AS cnt FROM a2a_disputes WHERE filed_by = ?')
      .get('0xDelete');
    assert.equal(original.cnt, 0);
  });

  it('returns deleted items list', () => {
    seedAgentCard(store, { walletAddress: '0xDelete' });
    seedNotifications(store, 2, { recipientAddress: '0xDelete' });
    const result = svc.deleteGDPRData('0xDelete');
    assert.ok(result.deleted.length >= 2);
    for (const d of result.deleted) {
      assert.ok(d.table);
      assert.ok(d.count > 0);
    }
  });

  it('returns retained items list when keepTransactions=true', () => {
    seedPayments(store, 2, { sender: '0xDelete' });
    const result = svc.deleteGDPRData('0xDelete', { keepTransactions: true });
    assert.ok(result.retained.length >= 1);
    for (const r of result.retained) {
      assert.ok(r.table);
      assert.ok(r.action === 'anonymized');
      assert.ok(r.anonymizedAs);
    }
  });

  it('handles unknown customer without error', () => {
    const result = svc.deleteGDPRData('0xNobody');
    assert.deepEqual(result.deleted, []);
    assert.deepEqual(result.retained, []);
    assert.equal(result.customerId, '0xNobody');
  });

  it('double delete is safe (idempotent)', () => {
    seedAgentCard(store, { walletAddress: '0xDelete' });
    svc.deleteGDPRData('0xDelete');
    const result2 = svc.deleteGDPRData('0xDelete');
    assert.deepEqual(result2.deleted, []);
  });
});

// ===========================================================================
// generateComplianceSummary
// ===========================================================================

describe('generateComplianceSummary', () => {
  let store, svc;

  beforeEach(() => {
    store = makeStore();
    svc = createComplianceService(store);
  });

  it('returns correct total transactions', () => {
    seedPayments(store, 5);
    const result = svc.generateComplianceSummary({ period: 'month' });
    assert.equal(result.totalTransactions, 5);
  });

  it('returns correct total volume', () => {
    seedPayments(store, 3, { amount: 200 });
    const result = svc.generateComplianceSummary({ period: 'month' });
    assert.equal(result.totalVolume, 600);
  });

  it('returns correct average transaction size', () => {
    seedPayments(store, 4, { amount: 100 });
    const result = svc.generateComplianceSummary({ period: 'month' });
    assert.equal(result.avgTransactionSize, 100);
  });

  it('calculates dispute rate correctly', () => {
    seedPayments(store, 10);
    seedDisputes(store, 2);
    const result = svc.generateComplianceSummary({ period: 'month' });
    assert.equal(result.disputeRate, 0.2);
  });

  it('handles zero transactions (no division by zero)', () => {
    const result = svc.generateComplianceSummary({ period: 'month' });
    assert.equal(result.totalTransactions, 0);
    assert.equal(result.totalVolume, 0);
    assert.equal(result.avgTransactionSize, 0);
    assert.equal(result.disputeRate, 0);
  });

  it('handles zero disputes (rate = 0)', () => {
    seedPayments(store, 5);
    const result = svc.generateComplianceSummary({ period: 'month' });
    assert.equal(result.disputeRate, 0);
    assert.equal(result.disputeCount, 0);
  });

  it('filters by day period', () => {
    // Seed one payment today and one 5 days ago
    const now = new Date();
    const today = now.toISOString();
    const fiveDaysAgo = new Date(now - 5 * 86400000).toISOString();
    store.db
      .prepare(
        `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, created_at, updated_at)
       VALUES ('today-pay', 'completed', '0xA', '0xB', 10000, 100, 'USDC', ?, ?)`,
      )
      .run(today, today);
    store.db
      .prepare(
        `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, created_at, updated_at)
       VALUES ('old-pay', 'completed', '0xA', '0xB', 20000, 200, 'USDC', ?, ?)`,
      )
      .run(fiveDaysAgo, fiveDaysAgo);
    const result = svc.generateComplianceSummary({ period: 'day' });
    assert.equal(result.totalTransactions, 1);
    assert.equal(result.totalVolume, 100);
  });

  it('filters by week period', () => {
    seedPayments(store, 3); // 0, 1, 2 days ago
    const result = svc.generateComplianceSummary({ period: 'week' });
    assert.ok(result.totalTransactions >= 3);
  });

  it('filters by quarter period', () => {
    seedPayments(store, 5); // 0-4 days ago, all within quarter
    const result = svc.generateComplianceSummary({ period: 'quarter' });
    assert.ok(result.totalTransactions >= 5);
  });

  it('filters by agent name', () => {
    seedPayments(store, 3, { sender: '0xAgentX', recipient: '0xOther' });
    seedPayments(store, 2, { sender: '0xOther', recipient: '0xAgentY', idPrefix: 'other' });
    const result = svc.generateComplianceSummary({ period: 'month', agentName: '0xAgentX' });
    assert.equal(result.totalTransactions, 3);
  });

  it('returns top agents sorted by volume', () => {
    seedPayments(store, 2, { sender: '0xBigSpender', amount: 500 });
    seedPayments(store, 5, { sender: '0xSmallSpender', amount: 10, idPrefix: 'sm' });
    const result = svc.generateComplianceSummary({ period: 'month' });
    assert.ok(result.topAgents.length > 0);
    // Top agent should have highest volume
    if (result.topAgents.length >= 2) {
      assert.ok(result.topAgents[0].volume >= result.topAgents[1].volume);
    }
  });

  it('includes agentCount', () => {
    seedPayments(store, 2, { sender: '0xA', recipient: '0xB' });
    seedPayments(store, 1, { sender: '0xC', recipient: '0xD', idPrefix: 'cd' });
    const result = svc.generateComplianceSummary({ period: 'month' });
    assert.ok(result.agentCount >= 2);
  });

  it('includes policy violations count', () => {
    seedCircuitBreakerEvents(store, 3, { eventType: 'trip' });
    const result = svc.generateComplianceSummary({ period: 'month' });
    assert.equal(result.policyViolations, 3);
  });
});

// ===========================================================================
// generateSOC2Evidence
// ===========================================================================

describe('generateSOC2Evidence', () => {
  let store, svc;

  beforeEach(() => {
    store = makeStore();
    svc = createComplianceService(store);
  });

  it('generates evidence for access_control', () => {
    seedAgentCard(store);
    const result = svc.generateSOC2Evidence({ controls: ['access_control'] });
    assert.equal(result.controls.length, 1);
    assert.equal(result.controls[0].control, 'access_control');
    assert.equal(result.controls[0].status, 'gathered');
    assert.ok(result.controls[0].records.length >= 1);
  });

  it('generates evidence for change_management', () => {
    seedCircuitBreakerEvents(store, 3);
    const result = svc.generateSOC2Evidence({ controls: ['change_management'] });
    assert.equal(result.controls[0].control, 'change_management');
    assert.equal(result.controls[0].status, 'gathered');
  });

  it('generates evidence for encryption', () => {
    const result = svc.generateSOC2Evidence({ controls: ['encryption'] });
    assert.equal(result.controls[0].control, 'encryption');
    assert.equal(result.controls[0].status, 'gathered');
    assert.ok(result.controls[0].details);
    assert.equal(result.controls[0].details.algorithm, 'Ed25519 + AES-256-GCM');
  });

  it('generates evidence for monitoring', () => {
    seedCircuitBreakerEvents(store, 2);
    seedSLAViolations(store, 3);
    const result = svc.generateSOC2Evidence({ controls: ['monitoring'] });
    assert.equal(result.controls[0].control, 'monitoring');
    assert.equal(result.controls[0].status, 'gathered');
    assert.ok(result.controls[0].circuitBreakerEvents.length >= 2);
    assert.ok(result.controls[0].slaViolations.length >= 3);
  });

  it('generates evidence for incident_response', () => {
    seedDisputes(store, 2);
    seedCircuitBreakerEvents(store, 1, { eventType: 'trip' });
    const result = svc.generateSOC2Evidence({ controls: ['incident_response'] });
    assert.equal(result.controls[0].control, 'incident_response');
    assert.equal(result.controls[0].status, 'gathered');
    assert.ok(result.controls[0].disputes.length >= 2);
    assert.ok(result.controls[0].circuitBreakerTrips.length >= 1);
  });

  it('handles unsupported controls gracefully', () => {
    const result = svc.generateSOC2Evidence({ controls: ['unknown_control'] });
    assert.equal(result.controls.length, 1);
    assert.equal(result.controls[0].status, 'unsupported');
    assert.ok(result.controls[0].message.includes('unknown_control'));
  });

  it('includes generatedAt timestamp', () => {
    const result = svc.generateSOC2Evidence({ controls: ['encryption'] });
    assert.ok(result.generatedAt);
    assert.ok(new Date(result.generatedAt).getTime() > 0);
  });

  it('combines multiple controls', () => {
    seedAgentCard(store);
    seedCircuitBreakerEvents(store, 2);
    const result = svc.generateSOC2Evidence({
      controls: ['access_control', 'encryption', 'monitoring'],
    });
    assert.equal(result.controls.length, 3);
    const controlNames = result.controls.map((c) => c.control);
    assert.deepEqual(controlNames, ['access_control', 'encryption', 'monitoring']);
  });
});

// ===========================================================================
// CSV helper (_recordsToCSV)
// ===========================================================================

describe('recordsToCSV helper', () => {
  let store, svc;

  beforeEach(() => {
    store = makeStore();
    svc = createComplianceService(store);
  });

  it('returns correct headers', () => {
    const csv = svc._recordsToCSV([{ name: 'Alice', age: 30 }]);
    const lines = csv.split('\n');
    assert.equal(lines[0], 'name,age');
  });

  it('handles null values as empty strings', () => {
    const csv = svc._recordsToCSV([{ a: null, b: 'x' }]);
    const lines = csv.split('\n');
    assert.equal(lines[1], ',x');
  });

  it('handles undefined values as empty strings', () => {
    const csv = svc._recordsToCSV([{ a: undefined, b: 'y' }]);
    const lines = csv.split('\n');
    assert.equal(lines[1], ',y');
  });

  it('escapes commas by quoting', () => {
    const csv = svc._recordsToCSV([{ msg: 'hello, world' }]);
    const lines = csv.split('\n');
    assert.equal(lines[1], '"hello, world"');
  });

  it('escapes double quotes by doubling them', () => {
    const csv = svc._recordsToCSV([{ msg: 'say "hello"' }]);
    const lines = csv.split('\n');
    assert.equal(lines[1], '"say ""hello"""');
  });

  it('escapes newlines by quoting', () => {
    const csv = svc._recordsToCSV([{ msg: 'line1\nline2' }]);
    const lines = csv.split('\n');
    // The first line is header, second line starts the quoted field
    assert.ok(csv.includes('"line1\nline2"'));
  });

  it('returns empty string for empty records', () => {
    const csv = svc._recordsToCSV([]);
    assert.equal(csv, '');
  });
});

// ===========================================================================
// periodToDateRange helper
// ===========================================================================

describe('periodToDateRange helper', () => {
  let store, svc;

  beforeEach(() => {
    store = makeStore();
    svc = createComplianceService(store);
  });

  it('returns from/to for day period', () => {
    const range = svc._periodToDateRange('day');
    const diff = new Date(range.to) - new Date(range.from);
    // Should be approximately 1 day (86400000ms)
    assert.ok(diff >= 86300000 && diff <= 86500000);
  });

  it('returns from/to for week period', () => {
    const range = svc._periodToDateRange('week');
    const diff = new Date(range.to) - new Date(range.from);
    assert.ok(diff >= 7 * 86300000 && diff <= 7 * 86500000);
  });

  it('returns from/to for month period', () => {
    const range = svc._periodToDateRange('month');
    const diff = new Date(range.to) - new Date(range.from);
    assert.ok(diff >= 30 * 86300000 && diff <= 30 * 86500000);
  });

  it('returns from/to for quarter period', () => {
    const range = svc._periodToDateRange('quarter');
    const diff = new Date(range.to) - new Date(range.from);
    assert.ok(diff >= 90 * 86300000 && diff <= 90 * 86500000);
  });

  it('returns from/to for year period', () => {
    const range = svc._periodToDateRange('year');
    const diff = new Date(range.to) - new Date(range.from);
    assert.ok(diff >= 365 * 86300000 && diff <= 365 * 86500000);
  });

  it('defaults to month for unknown period', () => {
    const range = svc._periodToDateRange('blah');
    const diff = new Date(range.to) - new Date(range.from);
    assert.ok(diff >= 30 * 86300000 && diff <= 30 * 86500000);
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('edge cases', () => {
  let store, svc;

  beforeEach(() => {
    store = makeStore();
    svc = createComplianceService(store);
  });

  it('handles very large date ranges without error', () => {
    seedPayments(store, 5);
    const result = svc.exportAuditTrail({
      from: '2000-01-01T00:00:00.000Z',
      to: '2099-12-31T23:59:59.999Z',
    });
    assert.ok(result.count >= 5);
  });

  it('handles concurrent queries on same service instance', () => {
    seedPayments(store, 10);
    // Run multiple queries - SQLite in-memory is single-threaded but should not crash
    const r1 = svc.exportAuditTrail({ limit: 3 });
    const r2 = svc.generateComplianceSummary({ period: 'month' });
    const r3 = svc.exportAuditTrail({ limit: 5 });
    assert.equal(r1.count, 3);
    assert.equal(r2.totalTransactions, 10);
    assert.equal(r3.count, 5);
  });

  it('handles special characters in data', () => {
    store.db
      .prepare(
        `INSERT INTO a2a_payments (id, status, sender_address, recipient_address, amount, amount_decimal, asset, memo, created_at, updated_at)
       VALUES ('special-pay', 'completed', '0xSender', '0xRecipient', 10000, 100, 'USDC', ?, ?, ?)`,
      )
      .run(
        'memo with \'quotes\' and "double" and <html>',
        new Date().toISOString(),
        new Date().toISOString(),
      );
    const result = svc.exportAuditTrail({ format: 'csv' });
    assert.ok(result.csv.includes('quotes'));
  });

  it('handles Unicode agent names', () => {
    seedAgentCard(store, { name: 'Agent Unicode', walletAddress: '0xUnicode' });
    store.db
      .prepare(
        `INSERT INTO a2a_circuit_breaker_events (id, agent_name, event_type, reason, created_at)
       VALUES ('uni-cb', 'Agent Unicode', 'trip', 'test', ?)`,
      )
      .run(new Date().toISOString());
    const result = svc.exportAuditTrail({ agentName: 'Agent Unicode' });
    assert.ok(result.records.length >= 1);
  });

  it('GDPR export then delete round-trip', () => {
    seedAgentCard(store, { walletAddress: '0xRoundTrip' });
    seedPayments(store, 2, { sender: '0xRoundTrip' });
    seedNotifications(store, 1, { recipientAddress: '0xRoundTrip' });

    const exported = svc.generateGDPRExport('0xRoundTrip');
    assert.ok(exported.personalData.length >= 1);
    assert.ok(exported.payments.length >= 2);
    assert.ok(exported.communications.length >= 1);

    const deleted = svc.deleteGDPRData('0xRoundTrip');
    assert.ok(deleted.deleted.length >= 2);

    // Verify nothing remains
    const postDelete = svc.generateGDPRExport('0xRoundTrip');
    assert.deepEqual(postDelete.personalData, []);
    assert.deepEqual(postDelete.payments, []);
    assert.deepEqual(postDelete.communications, []);
  });

  it('compliance summary with all periods', () => {
    seedPayments(store, 3);
    for (const period of ['day', 'week', 'month', 'quarter', 'year']) {
      const result = svc.generateComplianceSummary({ period });
      assert.equal(result.period, period);
      assert.ok(result.dateRange);
    }
  });
});
