/**
 * Unit tests for webhook dead letter queue (DLQ).
 *
 * Covers: quarantineFailedNotifications, listDLQ, getDLQEntry, countDLQ,
 * replayDLQEntry, purgeDLQ, and edge cases.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { A2AStore } from '../../src/a2a/store.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeStore() {
  const store = new A2AStore({ dbPath: ':memory:' });
  store.init();
  return store;
}

const NOW = new Date().toISOString();
const OLD_DATE = new Date(Date.now() - 2 * 86400000).toISOString(); // 2 days ago

function seedNotification(store, id, opts = {}) {
  store.db
    .prepare(
      `INSERT INTO a2a_notification_log
       (id, recipient_address, endpoint_url, event_type, payload, signature,
        status, attempts, last_attempt_at, last_error, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      id,
      opts.recipient || '0xAgent1',
      opts.endpoint || 'https://example.com/webhook',
      opts.eventType || 'payment.completed',
      opts.payload || '{"amount":100}',
      opts.signature || 'sha256=abc123',
      opts.status || 'failed',
      opts.attempts || 3,
      opts.lastAttemptAt || NOW,
      opts.lastError || 'HTTP 503: Service Unavailable',
      opts.createdAt || OLD_DATE,
      opts.updatedAt || NOW,
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Webhook Dead Letter Queue', () => {
  /** @type {A2AStore} */
  let store;

  beforeEach(() => {
    store = makeStore();
  });

  afterEach(() => {
    store.close();
  });

  // =========================================================================
  // quarantineFailedNotifications
  // =========================================================================

  describe('quarantineFailedNotifications', () => {
    it('moves failed notifications to DLQ', () => {
      seedNotification(store, 'notif-1');
      seedNotification(store, 'notif-2');

      const result = store.quarantineFailedNotifications();
      assert.equal(result.quarantined, 2);

      // Original notifications should be removed
      const remaining = store.db
        .prepare("SELECT * FROM a2a_notification_log WHERE status = 'failed'")
        .all();
      assert.equal(remaining.length, 0);

      // DLQ should have entries
      const dlq = store.listDLQ();
      assert.equal(dlq.length, 2);
    });

    it('preserves notification data in DLQ', () => {
      seedNotification(store, 'notif-data', {
        recipient: '0xSpecial',
        endpoint: 'https://api.example.com/hook',
        eventType: 'order.shipped',
        payload: '{"orderId":"ord-1"}',
        signature: 'sha256=xyz',
        attempts: 3,
        lastError: 'ECONNREFUSED',
        createdAt: '2026-01-15T00:00:00Z',
      });

      store.quarantineFailedNotifications();
      const dlq = store.listDLQ();

      assert.equal(dlq.length, 1);
      const entry = dlq[0];
      assert.equal(entry.original_notification_id, 'notif-data');
      assert.equal(entry.recipient_address, '0xSpecial');
      assert.equal(entry.endpoint_url, 'https://api.example.com/hook');
      assert.equal(entry.event_type, 'order.shipped');
      assert.equal(entry.payload, '{"orderId":"ord-1"}');
      assert.equal(entry.signature, 'sha256=xyz');
      assert.equal(entry.attempts, 3);
      assert.equal(entry.last_error, 'ECONNREFUSED');
      assert.equal(entry.original_created_at, '2026-01-15T00:00:00Z');
      assert.ok(entry.quarantined_at, 'should have quarantined_at');
    });

    it('ignores pending and delivered notifications', () => {
      seedNotification(store, 'notif-pending', { status: 'pending', attempts: 1 });
      seedNotification(store, 'notif-delivered', { status: 'delivered', attempts: 1 });
      seedNotification(store, 'notif-failed', { status: 'failed', attempts: 3 });

      const result = store.quarantineFailedNotifications();
      assert.equal(result.quarantined, 1);

      // Pending and delivered should remain
      const remaining = store.db.prepare('SELECT * FROM a2a_notification_log').all();
      assert.equal(remaining.length, 2);
    });

    it('returns zero when no failed notifications', () => {
      const result = store.quarantineFailedNotifications();
      assert.equal(result.quarantined, 0);
    });

    it('respects limit parameter', () => {
      seedNotification(store, 'n1');
      seedNotification(store, 'n2');
      seedNotification(store, 'n3');

      const result = store.quarantineFailedNotifications({ limit: 2 });
      assert.equal(result.quarantined, 2);

      // One should remain in notification log
      const remaining = store.db
        .prepare("SELECT * FROM a2a_notification_log WHERE status = 'failed'")
        .all();
      assert.equal(remaining.length, 1);
    });

    it('is idempotent — second call finds no more failed', () => {
      seedNotification(store, 'notif-1');
      store.quarantineFailedNotifications();
      const result = store.quarantineFailedNotifications();
      assert.equal(result.quarantined, 0);
    });

    it('is atomic — DLQ insert and log delete happen together', () => {
      seedNotification(store, 'notif-atomic');

      const result = store.quarantineFailedNotifications();
      assert.equal(result.quarantined, 1);

      const log = store.db
        .prepare('SELECT * FROM a2a_notification_log WHERE id = ?')
        .get('notif-atomic');
      assert.equal(log, undefined, 'should be removed from log');

      const dlq = store.getDLQEntry('dlq-notif-atomic');
      assert.ok(dlq, 'should be in DLQ');
    });
  });

  // =========================================================================
  // listDLQ
  // =========================================================================

  describe('listDLQ', () => {
    it('lists all DLQ entries', () => {
      seedNotification(store, 'n1');
      seedNotification(store, 'n2');
      store.quarantineFailedNotifications();

      const entries = store.listDLQ();
      assert.equal(entries.length, 2);
    });

    it('filters by recipient address', () => {
      seedNotification(store, 'n1', { recipient: '0xAgentA' });
      seedNotification(store, 'n2', { recipient: '0xAgentB' });
      store.quarantineFailedNotifications();

      const entries = store.listDLQ({ recipient_address: '0xAgentA' });
      assert.equal(entries.length, 1);
      assert.equal(entries[0].recipient_address, '0xAgentA');
    });

    it('filters by event type', () => {
      seedNotification(store, 'n1', { eventType: 'payment.completed' });
      seedNotification(store, 'n2', { eventType: 'order.created' });
      store.quarantineFailedNotifications();

      const entries = store.listDLQ({ event_type: 'order.created' });
      assert.equal(entries.length, 1);
      assert.equal(entries[0].event_type, 'order.created');
    });

    it('supports pagination', () => {
      for (let i = 0; i < 5; i++) {
        seedNotification(store, `n${i}`);
      }
      store.quarantineFailedNotifications();

      const page1 = store.listDLQ({ limit: 2, offset: 0 });
      const page2 = store.listDLQ({ limit: 2, offset: 2 });
      const page3 = store.listDLQ({ limit: 2, offset: 4 });

      assert.equal(page1.length, 2);
      assert.equal(page2.length, 2);
      assert.equal(page3.length, 1);
    });

    it('returns empty array when DLQ is empty', () => {
      const entries = store.listDLQ();
      assert.deepEqual(entries, []);
    });
  });

  // =========================================================================
  // getDLQEntry
  // =========================================================================

  describe('getDLQEntry', () => {
    it('returns entry by ID', () => {
      seedNotification(store, 'notif-1');
      store.quarantineFailedNotifications();

      const entry = store.getDLQEntry('dlq-notif-1');
      assert.ok(entry);
      assert.equal(entry.id, 'dlq-notif-1');
      assert.equal(entry.original_notification_id, 'notif-1');
    });

    it('returns null for non-existent ID', () => {
      const entry = store.getDLQEntry('dlq-nonexistent');
      assert.equal(entry, null);
    });
  });

  // =========================================================================
  // countDLQ
  // =========================================================================

  describe('countDLQ', () => {
    it('counts all DLQ entries', () => {
      seedNotification(store, 'n1');
      seedNotification(store, 'n2');
      seedNotification(store, 'n3');
      store.quarantineFailedNotifications();

      assert.equal(store.countDLQ(), 3);
    });

    it('counts with filters', () => {
      seedNotification(store, 'n1', { recipient: '0xA' });
      seedNotification(store, 'n2', { recipient: '0xB' });
      store.quarantineFailedNotifications();

      assert.equal(store.countDLQ({ recipient_address: '0xA' }), 1);
      assert.equal(store.countDLQ({ recipient_address: '0xB' }), 1);
    });

    it('returns zero for empty DLQ', () => {
      assert.equal(store.countDLQ(), 0);
    });
  });

  // =========================================================================
  // replayDLQEntry
  // =========================================================================

  describe('replayDLQEntry', () => {
    it('moves DLQ entry back to notification log', () => {
      seedNotification(store, 'notif-replay');
      store.quarantineFailedNotifications();

      const result = store.replayDLQEntry('dlq-notif-replay');
      assert.equal(result.replayed, true);

      // Should be back in notification log
      const log = store.db
        .prepare('SELECT * FROM a2a_notification_log WHERE id = ?')
        .get('notif-replay');
      assert.ok(log, 'should be back in notification log');
      assert.equal(log.status, 'pending');
      assert.equal(log.attempts, 0);
      assert.equal(log.last_error, null);
    });

    it('marks DLQ entry as replayed', () => {
      seedNotification(store, 'notif-replay');
      store.quarantineFailedNotifications();

      store.replayDLQEntry('dlq-notif-replay');

      const dlq = store.getDLQEntry('dlq-notif-replay');
      assert.ok(dlq.replayed_at, 'should have replayed_at');
      assert.equal(dlq.replay_status, 'replayed');
    });

    it('returns false for non-existent DLQ entry', () => {
      const result = store.replayDLQEntry('dlq-nonexistent');
      assert.equal(result.replayed, false);
    });

    it('preserves original notification data on replay', () => {
      seedNotification(store, 'notif-data', {
        recipient: '0xSpecial',
        endpoint: 'https://api.test.com/hook',
        eventType: 'refund.processed',
        payload: '{"refundId":"ref-1"}',
      });
      store.quarantineFailedNotifications();

      store.replayDLQEntry('dlq-notif-data');

      const log = store.db
        .prepare('SELECT * FROM a2a_notification_log WHERE id = ?')
        .get('notif-data');
      assert.equal(log.recipient_address, '0xSpecial');
      assert.equal(log.endpoint_url, 'https://api.test.com/hook');
      assert.equal(log.event_type, 'refund.processed');
      assert.equal(log.payload, '{"refundId":"ref-1"}');
    });
  });

  // =========================================================================
  // purgeDLQ
  // =========================================================================

  describe('purgeDLQ', () => {
    it('purges entries older than specified days', () => {
      seedNotification(store, 'old-1', { createdAt: '2025-01-01T00:00:00Z' });
      seedNotification(store, 'old-2', { createdAt: '2025-06-01T00:00:00Z' });
      store.quarantineFailedNotifications();

      // Manually backdate the quarantined_at to simulate old entries
      store.db
        .prepare("UPDATE a2a_webhook_dlq SET quarantined_at = '2025-01-01T00:00:00Z'")
        .run();

      const result = store.purgeDLQ({ olderThanDays: 30 });
      assert.equal(result.purged, 2);
      assert.equal(store.countDLQ(), 0);
    });

    it('does not purge recent entries', () => {
      seedNotification(store, 'recent-1');
      store.quarantineFailedNotifications();
      // quarantined_at is now (today), so 30-day purge shouldn't touch it

      const result = store.purgeDLQ({ olderThanDays: 30 });
      assert.equal(result.purged, 0);
      assert.equal(store.countDLQ(), 1);
    });

    it('defaults to 30 days', () => {
      seedNotification(store, 'old-1');
      store.quarantineFailedNotifications();

      // Backdate by 31 days
      const past = new Date(Date.now() - 31 * 86400000).toISOString();
      store.db
        .prepare('UPDATE a2a_webhook_dlq SET quarantined_at = ?')
        .run(past);

      const result = store.purgeDLQ();
      assert.equal(result.purged, 1);
    });

    it('returns zero when DLQ is empty', () => {
      const result = store.purgeDLQ();
      assert.equal(result.purged, 0);
    });
  });

  // =========================================================================
  // Integration / Edge Cases
  // =========================================================================

  describe('integration', () => {
    it('full lifecycle: fail → quarantine → replay → re-deliver', () => {
      // 1. Notification fails
      seedNotification(store, 'lifecycle-notif', { status: 'failed', attempts: 3 });

      // 2. Quarantine
      const q = store.quarantineFailedNotifications();
      assert.equal(q.quarantined, 1);

      // 3. Verify in DLQ
      assert.equal(store.countDLQ(), 1);

      // 4. Replay
      const r = store.replayDLQEntry('dlq-lifecycle-notif');
      assert.equal(r.replayed, true);

      // 5. Notification is back in pending state
      const pending = store.getPendingNotifications(3, 10);
      assert.equal(pending.length, 1);
      assert.equal(pending[0].id, 'lifecycle-notif');
      assert.equal(pending[0].status, 'pending');
      assert.equal(pending[0].attempts, 0);
    });

    it('quarantine + purge lifecycle', () => {
      seedNotification(store, 'purge-me');
      store.quarantineFailedNotifications();

      // Backdate
      const oldDate = new Date(Date.now() - 60 * 86400000).toISOString();
      store.db
        .prepare('UPDATE a2a_webhook_dlq SET quarantined_at = ?')
        .run(oldDate);

      const result = store.purgeDLQ({ olderThanDays: 30 });
      assert.equal(result.purged, 1);
      assert.equal(store.countDLQ(), 0);
    });

    it('multiple quarantines do not duplicate DLQ entries', () => {
      seedNotification(store, 'once-only');

      // First quarantine
      store.quarantineFailedNotifications();
      assert.equal(store.countDLQ(), 1);

      // No more failed notifications to quarantine
      const result = store.quarantineFailedNotifications();
      assert.equal(result.quarantined, 0);
      assert.equal(store.countDLQ(), 1);
    });
  });
});
