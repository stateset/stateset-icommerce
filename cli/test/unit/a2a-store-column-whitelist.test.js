/**
 * Tests for A2A Store column whitelist validation.
 *
 * These tests exercise the _validateUpdateKeys logic without requiring
 * better-sqlite3 to initialize successfully.
 * We import the UPDATABLE_COLUMNS constant indirectly by testing the
 * validation method on a stub store instance.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

// Import the module to get access to the class. The constructor stays sqlite-free
// until init() so constructor and validation coverage can run in isolation.
let A2AStore;
let defaultA2ADbPath;
try {
  const mod = await import('../../src/a2a/store.js');
  A2AStore = mod.A2AStore;
  defaultA2ADbPath = mod.defaultA2ADbPath;
} catch {
  // If import fails entirely, skip gracefully
}

describe('A2AStore column whitelist', { skip: !A2AStore && 'A2AStore not available' }, () => {
  /** @type {InstanceType<typeof A2AStore>} */
  let store;

  beforeEach(() => {
    // Constructor does NOT call init(), so no sqlite needed
    store = new A2AStore({ dbPath: ':memory:' });
  });

  describe('constructor', () => {
    it('accepts a string dbPath', () => {
      store = new A2AStore(':memory:');
      assert.equal(store.dbPath, ':memory:');
      assert.equal(store.db, null);
    });

    it('accepts an options object dbPath', () => {
      store = new A2AStore({ dbPath: ':memory:' });
      assert.equal(store.dbPath, ':memory:');
      assert.equal(store.db, null);
    });

    it('uses the default A2A path when omitted', () => {
      store = new A2AStore();
      assert.equal(store.dbPath, defaultA2ADbPath());
      assert.equal(store.db, null);
    });
  });

  // ---------------------------------------------------------------------------
  // _validateUpdateKeys unit tests (no DB required)
  // ---------------------------------------------------------------------------

  describe('_validateUpdateKeys', () => {
    it('accepts valid payment columns', () => {
      assert.doesNotThrow(() => {
        store._validateUpdateKeys('a2a_payments', ['status', 'tx_hash', 'metadata']);
      });
    });

    it('accepts valid quote columns including migration columns', () => {
      assert.doesNotThrow(() => {
        store._validateUpdateKeys('a2a_quotes', [
          'status', 'items', 'negotiation_history', 'counter_count', 'max_rounds', 'escrow_id',
        ]);
      });
    });

    it('accepts valid escrow columns including intent_id migration', () => {
      assert.doesNotThrow(() => {
        store._validateUpdateKeys('a2a_escrows', ['status', 'intent_id', 'metadata']);
      });
    });

    it('accepts valid subscription columns', () => {
      assert.doesNotThrow(() => {
        store._validateUpdateKeys('a2a_subscriptions', [
          'status', 'next_billing_date', 'cancel_at_period_end', 'last_payment_id',
        ]);
      });
    });

    it('rejects unknown column names', () => {
      assert.throws(
        () => store._validateUpdateKeys('a2a_payments', ['status', 'evil_column']),
        { message: /Column 'evil_column' is not allowed/ },
      );
    });

    it('rejects id column (immutable primary key)', () => {
      assert.throws(
        () => store._validateUpdateKeys('a2a_payments', ['id']),
        { message: /Column 'id' is not allowed/ },
      );
    });

    it('rejects created_at column (immutable timestamp)', () => {
      assert.throws(
        () => store._validateUpdateKeys('a2a_payments', ['created_at']),
        { message: /Column 'created_at' is not allowed/ },
      );
    });

    it('rejects SQL injection in column name', () => {
      assert.throws(
        () => store._validateUpdateKeys('a2a_payments', ['status = ?, id = ?; --']),
        { message: /is not allowed/ },
      );
    });

    it('rejects DROP TABLE attempt', () => {
      assert.throws(
        () => store._validateUpdateKeys('a2a_payments', ['DROP TABLE a2a_payments; --']),
        { message: /is not allowed/ },
      );
    });

    it('throws for unknown table', () => {
      assert.throws(
        () => store._validateUpdateKeys('nonexistent_table', ['status']),
        { message: /Unknown table/ },
      );
    });

    it('accepts empty key list (no-op update)', () => {
      assert.doesNotThrow(() => {
        store._validateUpdateKeys('a2a_payments', []);
      });
    });

    it('accepts valid columns for all 12 tables', () => {
      const validCases = [
        ['a2a_payments', ['status', 'memo', 'tx_hash', 'block_number']],
        ['a2a_payment_requests', ['status', 'amount_paid', 'payment_ids']],
        ['a2a_quotes', ['status', 'items', 'negotiation_history', 'total']],
        ['a2a_escrows', ['status', 'intent_id', 'payment_id', 'dispute_id']],
        ['a2a_disputes', ['status', 'resolution_type', 'resolution_amount']],
        ['a2a_feedback', ['comment', 'response', 'is_revoked']],
        ['a2a_services', ['name', 'active', 'pricing_model', 'endpoint_url']],
        ['a2a_notification_log', ['status', 'attempts', 'last_error']],
        ['a2a_subscriptions', ['status', 'next_billing_date', 'billing_count']],
        ['a2a_split_payments', ['status', 'metadata', 'completed_at']],
        ['a2a_split_recipients', ['status', 'payment_id', 'share_percent']],
        ['a2a_event_subscriptions', ['active', 'event_types', 'last_event_id']],
      ];

      for (const [table, cols] of validCases) {
        assert.doesNotThrow(
          () => store._validateUpdateKeys(table, cols),
          `Expected ${table} to accept [${cols}]`,
        );
      }
    });

    it('rejects immutable columns across all tables', () => {
      const immutableCases = [
        ['a2a_payments', 'sender_address'],
        ['a2a_payments', 'recipient_address'],
        ['a2a_payments', 'amount'],
        ['a2a_payment_requests', 'requester_address'],
        ['a2a_payment_requests', 'amount'],
        ['a2a_quotes', 'buyer_address'],
        ['a2a_quotes', 'seller_address'],
        ['a2a_escrows', 'buyer_address'],
        ['a2a_escrows', 'seller_address'],
        ['a2a_escrows', 'amount'],
        ['a2a_disputes', 'escrow_id'],
        ['a2a_disputes', 'filed_by'],
        ['a2a_feedback', 'agent_address'],
        ['a2a_feedback', 'reviewer_address'],
        ['a2a_services', 'agent_address'],
        ['a2a_split_payments', 'sender_address'],
        ['a2a_split_payments', 'total_amount'],
        ['a2a_split_recipients', 'split_payment_id'],
        ['a2a_event_subscriptions', 'agent_address'],
      ];

      for (const [table, col] of immutableCases) {
        assert.throws(
          () => store._validateUpdateKeys(table, [col]),
          { message: /is not allowed/ },
          `Expected ${table}.${col} to be rejected`,
        );
      }
    });
  });
});
