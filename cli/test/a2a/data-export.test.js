/**
 * Unit tests for a2a/data-export.js — Data Export Service
 *
 * Covers: exportAgentData, exportAllData, generateReport,
 * exportCSV, getDataStats, date-range filtering, privacy redaction.
 */

import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createDataExportService } from '../../src/a2a/data-export.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Sample payment records */
function makePayments() {
  return [
    {
      id: 'pay-1',
      sender_address: '0xAlice',
      recipient_address: '0xBob',
      amount: 10000000,
      amount_decimal: 10.0,
      asset: 'USDC',
      status: 'completed',
      created_at: '2025-06-15T10:00:00Z',
    },
    {
      id: 'pay-2',
      sender_address: '0xAlice',
      recipient_address: '0xCharlie',
      amount: 20000000,
      amount_decimal: 20.0,
      asset: 'USDC',
      status: 'completed',
      created_at: '2025-07-01T10:00:00Z',
    },
    {
      id: 'pay-3',
      sender_address: '0xBob',
      recipient_address: '0xAlice',
      amount: 5000000,
      amount_decimal: 5.0,
      asset: 'USDC',
      status: 'completed',
      created_at: '2025-07-15T10:00:00Z',
    },
  ];
}

/** Sample quote records */
function makeQuotes() {
  return [
    {
      id: 'q-1',
      buyer_address: '0xAlice',
      seller_address: '0xBob',
      total: 15000000,
      total_decimal: 15.0,
      status: 'fulfilled',
      asset: 'USDC',
      created_at: '2025-06-20T10:00:00Z',
    },
    {
      id: 'q-2',
      buyer_address: '0xCharlie',
      seller_address: '0xAlice',
      total: 25000000,
      total_decimal: 25.0,
      status: 'accepted',
      asset: 'USDC',
      created_at: '2025-07-10T10:00:00Z',
    },
  ];
}

/** Sample dispute records */
function makeDisputes() {
  return [
    {
      id: 'd-1',
      filed_by: '0xAlice',
      filed_against: '0xBob',
      amount_decimal: 10.0,
      status: 'resolved',
      category: 'non_delivery',
      created_at: '2025-07-20T10:00:00Z',
    },
  ];
}

/** Sample escrow records */
function makeEscrows() {
  return [
    {
      id: 'e-1',
      buyer_address: '0xAlice',
      seller_address: '0xBob',
      amount: 10000000,
      amount_decimal: 10.0,
      status: 'released',
      created_at: '2025-06-25T10:00:00Z',
    },
  ];
}

/** Sample subscription records */
function makeSubscriptions() {
  return [
    {
      id: 'sub-1',
      subscriber_address: '0xAlice',
      provider_address: '0xProvider',
      amount_decimal: 49.99,
      status: 'active',
      created_at: '2025-01-01T00:00:00Z',
    },
  ];
}

/** Sample service records */
function makeServices() {
  return [
    {
      id: 'svc-1',
      agent_address: '0xAlice',
      name: 'Widget API',
      created_at: '2025-01-01T00:00:00Z',
    },
  ];
}

/** Sample feedback records */
function makeFeedback() {
  return [
    {
      id: 'fb-1',
      reviewer_address: '0xBob',
      agent_address: '0xAlice',
      score: 5,
      created_at: '2025-07-25T10:00:00Z',
    },
  ];
}

/**
 * Build a mock store with all list methods.
 * Each list method filters by provided filter keys matching record values.
 */
function createMockStore(overrides = {}) {
  const payments = makePayments();
  const quotes = makeQuotes();
  const disputes = makeDisputes();
  const escrows = makeEscrows();
  const subscriptions = makeSubscriptions();
  const services = makeServices();
  const feedback = makeFeedback();

  function simpleFilter(records, filter) {
    if (!filter || Object.keys(filter).length === 0) return [...records];
    return records.filter((r) => {
      for (const [key, val] of Object.entries(filter)) {
        if (val !== undefined && r[key] !== val) return false;
      }
      return true;
    });
  }

  return {
    listPayments: mock.fn((filter) => simpleFilter(payments, filter)),
    listQuotes: mock.fn((filter) => simpleFilter(quotes, filter)),
    listDisputes: mock.fn((filter) => simpleFilter(disputes, filter)),
    listEscrows: mock.fn((filter) => simpleFilter(escrows, filter)),
    listSubscriptions: mock.fn((filter) => simpleFilter(subscriptions, filter)),
    listServices: mock.fn((filter) => simpleFilter(services, filter)),
    listFeedback: mock.fn((filter) => simpleFilter(feedback, filter)),
    getReputationScore: mock.fn((addr) => {
      if (addr === '0xAlice') {
        return { agent_address: '0xAlice', average_score: 4.5, total_reviews: 10 };
      }
      return null;
    }),
    ...overrides,
  };
}

// ===========================================================================
// Tests
// ===========================================================================

describe('createDataExportService', () => {
  /** @type {ReturnType<typeof createDataExportService>} */
  let exporter;
  let store;

  beforeEach(() => {
    store = createMockStore();
    exporter = createDataExportService(store);
  });

  it('throws if store is not provided', () => {
    assert.throws(
      () => createDataExportService(null),
      { message: 'store is required' },
    );
  });

  // -----------------------------------------------------------------------
  // exportAgentData
  // -----------------------------------------------------------------------

  describe('exportAgentData', () => {
    it('returns all entity types for the agent', async () => {
      const result = await exporter.exportAgentData('0xAlice');

      assert.ok(result.exportedAt);
      assert.equal(result.agentAddress, '0xAlice');

      // Should have all entity types
      assert.ok(Array.isArray(result.data.payments));
      assert.ok(Array.isArray(result.data.quotes));
      assert.ok(Array.isArray(result.data.escrows));
      assert.ok(Array.isArray(result.data.disputes));
      assert.ok(Array.isArray(result.data.subscriptions));
      assert.ok(Array.isArray(result.data.services));
      assert.ok(Array.isArray(result.data.feedback));

      // Payments: 0xAlice is sender of 2, recipient of 1 = 3 total
      assert.equal(result.data.payments.length, 3);

      // Quotes: buyer of q-1, seller of q-2 = 2 total
      assert.equal(result.data.quotes.length, 2);

      // Reputation
      assert.ok(result.data.reputation);
      assert.equal(result.data.reputation.average_score, 4.5);
    });

    it('deduplicates records that match both sides', async () => {
      // 0xAlice is sender of pay-1 and pay-2, recipient of pay-3
      // All 3 are unique IDs, so all 3 should appear
      const result = await exporter.exportAgentData('0xAlice');
      const paymentIds = result.data.payments.map((p) => p.id);
      const uniqueIds = new Set(paymentIds);
      assert.equal(paymentIds.length, uniqueIds.size, 'No duplicate records');
    });

    it('throws if agentAddress is missing', async () => {
      await assert.rejects(
        () => exporter.exportAgentData(''),
        { message: 'agentAddress is required' },
      );
    });

    it('returns null reputation for unknown agent', async () => {
      const result = await exporter.exportAgentData('0xUnknown');
      assert.equal(result.data.reputation, null);
    });
  });

  // -----------------------------------------------------------------------
  // exportAllData
  // -----------------------------------------------------------------------

  describe('exportAllData', () => {
    it('includes all agents data', async () => {
      const result = await exporter.exportAllData();

      assert.ok(result.exportedAt);
      assert.equal(result.data.payments.length, 3);
      assert.equal(result.data.quotes.length, 2);
      assert.equal(result.data.disputes.length, 1);
      assert.equal(result.data.escrows.length, 1);
      assert.equal(result.data.subscriptions.length, 1);
      assert.equal(result.data.services.length, 1);
      assert.equal(result.data.feedback.length, 1);
    });

    it('returns empty arrays when store has no data', async () => {
      const emptyStore = createMockStore({
        listPayments: mock.fn(() => []),
        listQuotes: mock.fn(() => []),
        listDisputes: mock.fn(() => []),
        listEscrows: mock.fn(() => []),
        listSubscriptions: mock.fn(() => []),
        listServices: mock.fn(() => []),
        listFeedback: mock.fn(() => []),
      });
      const emp = createDataExportService(emptyStore);

      const result = await emp.exportAllData();
      assert.equal(result.data.payments.length, 0);
      assert.equal(result.data.quotes.length, 0);
    });
  });

  // -----------------------------------------------------------------------
  // generateReport
  // -----------------------------------------------------------------------

  describe('generateReport', () => {
    it('computes correct metrics for agent', async () => {
      const report = await exporter.generateReport('0xAlice');

      assert.equal(report.agentAddress, '0xAlice');
      assert.ok(report.generatedAt);

      // Summary: sent 10+20=30, received 5
      assert.equal(report.summary.totalSent, 30);
      assert.equal(report.summary.totalReceived, 5);
      assert.equal(report.summary.netFlow, -25);
      assert.equal(report.summary.transactionCount, 3);
      assert.equal(report.summary.sentCount, 2);
      assert.equal(report.summary.receivedCount, 1);
      assert.equal(report.summary.totalVolume, 35);
      assert.equal(report.summary.aggregateTotalsMeaningful, true);
      assert.equal(report.summary.aggregateAsset, 'USDC');
      assert.deepEqual(report.summary.assets, ['USDC']);
      assert.equal(report.summary.marginMeaningful, true);
      assert.equal(report.summary.breakdownByAsset.USDC.totalSent, 30);
      assert.equal(report.summary.breakdownByAsset.USDC.totalReceived, 5);
      assert.equal(report.summary.breakdownByAsset.USDC.networks.unknown.totalSent, 30);
      assert.equal(report.summary.breakdownByAsset.USDC.networks.unknown.totalReceived, 5);
    });

    it('computes dispute rate', async () => {
      const report = await exporter.generateReport('0xAlice');

      // Quotes: buyer 1 + seller 1 = 2 total
      // Disputes: filed 1, against 0 = 1 total
      // Rate: 1/2 = 50%
      assert.equal(report.quotes.total, 2);
      assert.equal(report.disputes.total, 1);
      assert.equal(report.disputes.disputeRate, 50);
    });

    it('computes top counterparties', async () => {
      const report = await exporter.generateReport('0xAlice');

      assert.ok(Array.isArray(report.topCounterparties));
      assert.ok(report.topCounterparties.length > 0);

      // Bob: 10 (sent) + 5 (received) = 15
      // Charlie: 20 (sent) = 20
      const charlie = report.topCounterparties.find((c) => c.address === '0xCharlie');
      const bob = report.topCounterparties.find((c) => c.address === '0xBob');

      assert.ok(charlie);
      assert.equal(charlie.volume, 20);
      assert.ok(bob);
      assert.equal(bob.volume, 15);
      assert.equal(charlie.aggregateVolumeMeaningful, true);
      assert.equal(charlie.aggregateAsset, 'USDC');
      assert.deepEqual(charlie.assets, ['USDC']);
      assert.equal(bob.breakdownByAsset.USDC.networks.unknown.totalVolume, 15);

      // Charlie should be first (higher volume)
      assert.equal(report.topCounterparties[0].address, '0xCharlie');
    });

    it('computes margin analysis', async () => {
      const report = await exporter.generateReport('0xAlice');

      // netFlow = 5 - 30 = -25, received = 5
      // margin = (-25 / 5) * 100 = -500%
      assert.equal(report.summary.margin, -500);
    });

    it('marks aggregate report totals as non-meaningful for mixed native assets', async () => {
      const mixedStore = createMockStore({
        listPayments: mock.fn((filter) => {
          const payments = [
            {
              id: 'btc-1',
              sender_address: '0xAlice',
              recipient_address: 'bc1qbob',
              amount_decimal: 0.01,
              asset: 'BTC',
              network: 'bitcoin',
              status: 'completed',
              created_at: '2025-07-01T10:00:00Z',
            },
            {
              id: 'zec-1',
              sender_address: '0xCarol',
              recipient_address: '0xAlice',
              amount_decimal: 1.25,
              asset: 'ZEC',
              network: 'zcash',
              status: 'completed',
              created_at: '2025-07-02T10:00:00Z',
            },
            {
              id: 'btc-2',
              sender_address: '0xAlice',
              recipient_address: 'bc1qbob',
              amount_decimal: 0.005,
              asset: 'BTC',
              network: 'bitcoin',
              status: 'completed',
              created_at: '2025-07-03T10:00:00Z',
            },
          ];
          return payments.filter((payment) => {
            for (const [key, value] of Object.entries(filter || {})) {
              if (value !== undefined && payment[key] !== value) return false;
            }
            return true;
          });
        }),
        listQuotes: mock.fn(() => []),
        listDisputes: mock.fn(() => []),
      });
      const mixedExporter = createDataExportService(mixedStore);

      const report = await mixedExporter.generateReport('0xAlice');

      assert.equal(report.summary.aggregateTotalsMeaningful, false);
      assert.equal(report.summary.aggregateAsset, null);
      assert.deepEqual(report.summary.assets, ['BTC', 'ZEC']);
      assert.equal(report.summary.marginMeaningful, false);
      assert.equal(report.summary.breakdownByAsset.BTC.totalSent, 0.015);
      assert.equal(report.summary.breakdownByAsset.BTC.totalReceived, 0);
      assert.equal(report.summary.breakdownByAsset.BTC.networks.bitcoin.totalSent, 0.015);
      assert.equal(report.summary.breakdownByAsset.ZEC.totalReceived, 1.25);
      assert.equal(report.summary.breakdownByAsset.ZEC.networks.zcash.totalReceived, 1.25);

      const bob = report.topCounterparties.find((counterparty) => counterparty.address === 'bc1qbob');
      const carol = report.topCounterparties.find((counterparty) => counterparty.address === '0xCarol');
      assert.ok(bob);
      assert.ok(carol);
      assert.equal(bob.aggregateVolumeMeaningful, true);
      assert.equal(bob.aggregateAsset, 'BTC');
      assert.equal(bob.breakdownByAsset.BTC.networks.bitcoin.totalSent, 0.015);
      assert.equal(carol.aggregateVolumeMeaningful, true);
      assert.equal(carol.aggregateAsset, 'ZEC');
      assert.equal(carol.breakdownByAsset.ZEC.networks.zcash.totalReceived, 1.25);
    });

    it('handles zero transactions gracefully', async () => {
      const emptyStore = createMockStore({
        listPayments: mock.fn(() => []),
        listQuotes: mock.fn(() => []),
        listDisputes: mock.fn(() => []),
      });
      const emp = createDataExportService(emptyStore);

      const report = await emp.generateReport('0xEmpty');

      assert.equal(report.summary.totalVolume, 0);
      assert.equal(report.summary.transactionCount, 0);
      assert.equal(report.summary.margin, 0);
      assert.equal(report.disputes.disputeRate, 0);
      assert.equal(report.topCounterparties.length, 0);
    });

    it('throws if agentAddress is missing', async () => {
      await assert.rejects(
        () => exporter.generateReport(''),
        { message: 'agentAddress is required' },
      );
    });
  });

  // -----------------------------------------------------------------------
  // exportCSV
  // -----------------------------------------------------------------------

  describe('exportCSV', () => {
    it('formats payments as CSV with correct headers', async () => {
      const csv = await exporter.exportCSV('0xAlice', 'payments');

      const lines = csv.split('\n');
      assert.ok(lines.length >= 2); // header + at least one data row

      // Header should include payment fields
      const header = lines[0];
      assert.ok(header.includes('id'));
      assert.ok(header.includes('sender_address'));
      assert.ok(header.includes('recipient_address'));
      assert.ok(header.includes('amount'));
    });

    it('includes all agent records in CSV', async () => {
      const csv = await exporter.exportCSV('0xAlice', 'payments');

      const lines = csv.split('\n');
      // Header + 3 payments (2 as sender, 1 as recipient)
      assert.equal(lines.length, 4);
    });

    it('handles empty results', async () => {
      const csv = await exporter.exportCSV('0xNoOne', 'payments');
      assert.equal(csv, '');
    });

    it('throws on unknown entity type', async () => {
      await assert.rejects(
        () => exporter.exportCSV('0xAlice', 'unknown_type'),
        /Unknown entity type/,
      );
    });

    it('throws if agentAddress is missing', async () => {
      await assert.rejects(
        () => exporter.exportCSV('', 'payments'),
        { message: 'agentAddress is required' },
      );
    });

    it('throws if entityType is missing', async () => {
      await assert.rejects(
        () => exporter.exportCSV('0xAlice', ''),
        { message: 'entityType is required' },
      );
    });

    it('escapes commas and quotes in CSV values', async () => {
      // Add a payment with a comma in the memo-like field
      const customStore = createMockStore({
        listPayments: mock.fn((filter) => {
          if (filter && filter.sender_address === '0xSpecial') {
            return [{
              id: 'pay-special',
              sender_address: '0xSpecial',
              recipient_address: '0xOther',
              amount: 1000,
              memo: 'Payment for "service, part 1"',
              created_at: '2025-01-01T00:00:00Z',
            }];
          }
          return [];
        }),
      });
      const exp = createDataExportService(customStore);

      const csv = await exp.exportCSV('0xSpecial', 'payments');
      const lines = csv.split('\n');

      assert.equal(lines.length, 2); // header + 1 row

      // The memo field should be quoted and internal quotes doubled
      assert.ok(lines[1].includes('"Payment for ""service, part 1"""'));
    });
  });

  // -----------------------------------------------------------------------
  // getDataStats
  // -----------------------------------------------------------------------

  describe('getDataStats', () => {
    it('returns row counts for all tables', async () => {
      const stats = await exporter.getDataStats();

      assert.equal(stats.payments, 3);
      assert.equal(stats.quotes, 2);
      assert.equal(stats.disputes, 1);
      assert.equal(stats.escrows, 1);
      assert.equal(stats.subscriptions, 1);
      assert.equal(stats.services, 1);
      assert.equal(stats.feedback, 1);
      assert.equal(stats.total, 10);
      assert.ok(stats.generatedAt);
    });

    it('returns zero counts for empty store', async () => {
      const emptyStore = createMockStore({
        listPayments: mock.fn(() => []),
        listQuotes: mock.fn(() => []),
        listDisputes: mock.fn(() => []),
        listEscrows: mock.fn(() => []),
        listSubscriptions: mock.fn(() => []),
        listServices: mock.fn(() => []),
        listFeedback: mock.fn(() => []),
      });
      const emp = createDataExportService(emptyStore);

      const stats = await emp.getDataStats();
      assert.equal(stats.total, 0);
      assert.equal(stats.payments, 0);
    });
  });

  // -----------------------------------------------------------------------
  // Date range filtering
  // -----------------------------------------------------------------------

  describe('date range filtering', () => {
    it('filters exportAgentData by since/until', async () => {
      const result = await exporter.exportAgentData('0xAlice', {
        dateRange: {
          since: '2025-07-01T00:00:00Z',
          until: '2025-07-31T23:59:59Z',
        },
      });

      // Only payments in July: pay-2 (Jul 1), pay-3 (Jul 15) = 2
      assert.equal(result.data.payments.length, 2);

      // Only quotes in July: q-2 (Jul 10) = 1
      assert.equal(result.data.quotes.length, 1);
    });

    it('filters exportAllData by date range', async () => {
      const result = await exporter.exportAllData({
        dateRange: {
          since: '2025-07-01T00:00:00Z',
          until: '2025-07-31T23:59:59Z',
        },
      });

      // pay-2 (Jul 1), pay-3 (Jul 15) = 2
      assert.equal(result.data.payments.length, 2);
    });

    it('filters with only since', async () => {
      const result = await exporter.exportAllData({
        dateRange: { since: '2025-07-10T00:00:00Z' },
      });

      // pay-3 (Jul 15), q-2 (Jul 10), d-1 (Jul 20), fb-1 (Jul 25)
      assert.equal(result.data.payments.length, 1);
      assert.equal(result.data.quotes.length, 1);
      assert.equal(result.data.disputes.length, 1);
    });

    it('filters with only until', async () => {
      const result = await exporter.exportAllData({
        dateRange: { until: '2025-06-30T23:59:59Z' },
      });

      // pay-1 (Jun 15), q-1 (Jun 20), e-1 (Jun 25), sub-1 (Jan 1), svc-1 (Jan 1)
      assert.equal(result.data.payments.length, 1);
      assert.equal(result.data.quotes.length, 1);
      assert.equal(result.data.escrows.length, 1);
    });

    it('generateReport respects date range', async () => {
      const report = await exporter.generateReport('0xAlice', {
        since: '2025-07-01T00:00:00Z',
        until: '2025-07-31T23:59:59Z',
      });

      // Sent in July: pay-2 (20), received: pay-3 (5)
      assert.equal(report.summary.totalSent, 20);
      assert.equal(report.summary.totalReceived, 5);
      assert.equal(report.dateRange.since, '2025-07-01T00:00:00Z');
    });

    it('exportCSV respects date range', async () => {
      const csv = await exporter.exportCSV('0xAlice', 'payments', {
        dateRange: {
          since: '2025-07-01T00:00:00Z',
          until: '2025-07-31T23:59:59Z',
        },
      });

      const lines = csv.split('\n');
      // Header + 2 July payments
      assert.equal(lines.length, 3);
    });
  });

  // -----------------------------------------------------------------------
  // Privacy redaction
  // -----------------------------------------------------------------------

  describe('privacy redaction', () => {
    it('masks addresses in exportAgentData', async () => {
      const result = await exporter.exportAgentData('0xAlice', { redact: true });

      // Agent address itself should be masked (0xAlice is <= 10 chars, so becomes ****)
      assert.notEqual(result.agentAddress, '0xAlice');
      assert.ok(
        result.agentAddress.includes('...') || result.agentAddress === '****',
        `Expected masked address, got: ${result.agentAddress}`,
      );

      // Payment addresses should be masked
      for (const payment of result.data.payments) {
        if (payment.sender_address) {
          assert.ok(
            payment.sender_address.includes('...') || payment.sender_address === '****',
            `Expected masked address, got: ${payment.sender_address}`,
          );
        }
      }
    });

    it('rounds amounts in exportAgentData when redacted', async () => {
      // Add a payment with precise decimal
      const customStore = createMockStore({
        listPayments: mock.fn((filter) => {
          if (filter && (filter.sender_address === '0xPrecise' || filter.recipient_address === '0xPrecise')) {
            return [{
              id: 'pay-precise',
              sender_address: '0xPrecise',
              recipient_address: '0xOther',
              amount: 12345678,
              amount_decimal: 12.345678,
              created_at: '2025-01-01T00:00:00Z',
            }];
          }
          return [];
        }),
        listQuotes: mock.fn(() => []),
        listDisputes: mock.fn(() => []),
        listEscrows: mock.fn(() => []),
        listSubscriptions: mock.fn(() => []),
        listServices: mock.fn(() => []),
        listFeedback: mock.fn(() => []),
        getReputationScore: mock.fn(() => null),
      });
      const exp = createDataExportService(customStore);

      const result = await exp.exportAgentData('0xPrecise', { redact: true });

      const payment = result.data.payments[0];
      assert.equal(payment.amount_decimal, 12.35); // Rounded to 2 decimal places
    });

    it('masks addresses in exportAllData', async () => {
      const result = await exporter.exportAllData({ redact: true });

      for (const payment of result.data.payments) {
        if (payment.sender_address) {
          assert.ok(
            payment.sender_address.includes('...') || payment.sender_address === '****',
          );
        }
      }
    });

    it('masks addresses in CSV export', async () => {
      const csv = await exporter.exportCSV('0xAlice', 'payments', { redact: true });

      // Original address should not appear in CSV
      assert.ok(!csv.includes('0xAlice'));
      assert.ok(!csv.includes('0xBob'));
      assert.ok(!csv.includes('0xCharlie'));
    });

    it('address masking keeps first 6 and last 4 chars', async () => {
      const result = await exporter.exportAgentData('0xAlice_long_address_here', { redact: true });

      // The agent address should be masked as "0xAlic...here"
      assert.equal(result.agentAddress, '0xAlic...here');
    });

    it('short addresses are fully masked', async () => {
      const result = await exporter.exportAgentData('0xAB', { redact: true });

      // Short addresses become "****"
      assert.equal(result.agentAddress, '****');
    });
  });

  // -----------------------------------------------------------------------
  // Graceful handling of missing store methods
  // -----------------------------------------------------------------------

  describe('missing store methods', () => {
    it('returns empty arrays if store lacks a list method', async () => {
      const minStore = {
        listPayments: mock.fn(() => [{ id: 'p1', sender_address: '0xA', created_at: '2025-01-01' }]),
        // No other methods
      };
      const exp = createDataExportService(minStore);

      const result = await exp.exportAgentData('0xA');

      assert.ok(result.data.payments.length >= 0);
      assert.equal(result.data.quotes.length, 0);
      assert.equal(result.data.escrows.length, 0);
    });
  });
});
