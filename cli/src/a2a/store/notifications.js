/**
 * A2A Store — notification log, webhook DLQ and webhook configuration.
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { randomUUID } from 'node:crypto';

/**
 * A2A Store — notification log, webhook DLQ and webhook configuration.
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2ANotificationsMethods {
  // ===========================================================================
  // Notification Log
  // ===========================================================================

  createNotificationLog(log) {
    this.init();
    const id = log.id || randomUUID();
    const now = new Date().toISOString();

    this.db
      .prepare(
        `INSERT INTO a2a_notification_log (
        id, recipient_address, endpoint_url, event_type, payload, signature,
        status, attempts, last_attempt_at, last_error, delivered_at,
        created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        log.recipient_address,
        log.endpoint_url,
        log.event_type,
        typeof log.payload === 'object' ? JSON.stringify(log.payload) : log.payload,
        log.signature || null,
        log.status || 'pending',
        log.attempts || 0,
        log.last_attempt_at || null,
        log.last_error || null,
        log.delivered_at || null,
        log.created_at || now,
        log.updated_at || now,
      );

    return this.getNotificationLog(id);
  }

  getNotificationLog(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_notification_log WHERE id = ?').get(id) || null;
  }

  updateNotificationLog(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_notification_log', Object.keys(updates));
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(value);
      }
    }

    if (fields.length === 0) return this.getNotificationLog(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db
      .prepare(`UPDATE a2a_notification_log SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getNotificationLog(id);
  }

  listNotificationLog(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.recipient_address) {
      conditions.push('recipient_address = ?');
      params.push(filter.recipient_address);
    }
    if (filter.event_type) {
      conditions.push('event_type = ?');
      params.push(filter.event_type);
    }
    if (filter.status) {
      conditions.push('status = ?');
      params.push(filter.status);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 50;
    const offset = filter.offset || 0;

    return this.db
      .prepare(
        `SELECT * FROM a2a_notification_log ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`,
      )
      .all(...params, limit, offset);
  }

  getPendingNotifications(maxAttempts = 3, limit = 50) {
    this.init();
    return this.db
      .prepare(
        `SELECT * FROM a2a_notification_log
       WHERE status = 'pending' AND attempts < ?
       ORDER BY created_at ASC LIMIT ?`,
      )
      .all(maxAttempts, limit);
  }

  // ===========================================================================
  // Webhook Dead Letter Queue (DLQ)
  // ===========================================================================

  /**
   * Move permanently failed notifications to the dead letter queue.
   * Notifications with status='failed' are quarantined and removed from the
   * active notification log.
   *
   * @param {Object} [options]
   * @param {number} [options.limit=100] - Max notifications to quarantine per call
   * @returns {{ quarantined: number }}
   */
  quarantineFailedNotifications(options = {}) {
    this.init();
    const limit = options.limit || 100;
    const now = new Date().toISOString();

    const failed = this.db
      .prepare(
        `SELECT * FROM a2a_notification_log WHERE status = 'failed' ORDER BY created_at ASC LIMIT ?`,
      )
      .all(limit);

    if (failed.length === 0) return { quarantined: 0 };

    const insertStmt = this.db.prepare(
      `INSERT OR IGNORE INTO a2a_webhook_dlq
       (id, original_notification_id, recipient_address, endpoint_url, event_type,
        payload, signature, attempts, last_error, last_attempt_at,
        original_created_at, quarantined_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    );
    const deleteStmt = this.db.prepare(`DELETE FROM a2a_notification_log WHERE id = ?`);

    const txn = this.db.transaction(() => {
      let count = 0;
      for (const row of failed) {
        const dlqId = `dlq-${row.id}`;
        insertStmt.run(
          dlqId,
          row.id,
          row.recipient_address,
          row.endpoint_url,
          row.event_type,
          row.payload,
          row.signature,
          row.attempts,
          row.last_error,
          row.last_attempt_at,
          row.created_at,
          now,
        );
        deleteStmt.run(row.id);
        count++;
      }
      return count;
    });

    const quarantined = txn();
    return { quarantined };
  }

  /**
   * List dead letter queue entries with optional filters.
   *
   * @param {Object} [filter]
   * @param {string} [filter.recipient_address]
   * @param {string} [filter.event_type]
   * @param {number} [filter.limit=50]
   * @param {number} [filter.offset=0]
   * @returns {Array<Object>}
   */
  listDLQ(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.recipient_address) {
      conditions.push('recipient_address = ?');
      params.push(filter.recipient_address);
    }
    if (filter.event_type) {
      conditions.push('event_type = ?');
      params.push(filter.event_type);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 50;
    const offset = filter.offset || 0;

    return this.db
      .prepare(
        `SELECT * FROM a2a_webhook_dlq ${where} ORDER BY quarantined_at DESC LIMIT ? OFFSET ?`,
      )
      .all(...params, limit, offset);
  }

  /**
   * Get a single DLQ entry by ID.
   *
   * @param {string} id
   * @returns {Object|null}
   */
  getDLQEntry(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_webhook_dlq WHERE id = ?').get(id) || null;
  }

  /**
   * Count DLQ entries, optionally filtered.
   *
   * @param {Object} [filter]
   * @param {string} [filter.recipient_address]
   * @param {string} [filter.event_type]
   * @returns {number}
   */
  countDLQ(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.recipient_address) {
      conditions.push('recipient_address = ?');
      params.push(filter.recipient_address);
    }
    if (filter.event_type) {
      conditions.push('event_type = ?');
      params.push(filter.event_type);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const row = this.db
      .prepare(`SELECT COUNT(*) AS cnt FROM a2a_webhook_dlq ${where}`)
      .get(...params);
    return row.cnt;
  }

  /**
   * Move a DLQ entry back to the notification log for retry.
   * Resets attempts to 0 and status to 'pending'.
   *
   * @param {string} dlqId - DLQ entry ID
   * @returns {{ replayed: boolean }}
   */
  replayDLQEntry(dlqId) {
    this.init();
    const entry = this.getDLQEntry(dlqId);
    if (!entry) return { replayed: false };

    const now = new Date().toISOString();

    const txn = this.db.transaction(() => {
      // Re-insert into notification log with reset state
      this.db
        .prepare(
          `INSERT OR REPLACE INTO a2a_notification_log
           (id, recipient_address, endpoint_url, event_type, payload, signature,
            status, attempts, last_attempt_at, last_error, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, 'pending', 0, NULL, NULL, ?, ?)`,
        )
        .run(
          entry.original_notification_id,
          entry.recipient_address,
          entry.endpoint_url,
          entry.event_type,
          entry.payload,
          entry.signature,
          entry.original_created_at,
          now,
        );

      // Mark DLQ entry as replayed
      this.db
        .prepare(
          `UPDATE a2a_webhook_dlq SET replayed_at = ?, replay_status = 'replayed' WHERE id = ?`,
        )
        .run(now, dlqId);
    });

    txn();
    return { replayed: true };
  }

  /**
   * Purge old DLQ entries.
   *
   * @param {Object} [options]
   * @param {number} [options.olderThanDays=30] - Remove entries quarantined more than N days ago
   * @returns {{ purged: number }}
   */
  purgeDLQ(options = {}) {
    this.init();
    const days = options.olderThanDays || 30;
    const cutoff = new Date(Date.now() - days * 86400000).toISOString();

    const result = this.db
      .prepare(`DELETE FROM a2a_webhook_dlq WHERE quarantined_at < ?`)
      .run(cutoff);

    return { purged: result.changes };
  }

  // ===========================================================================
  // Webhook Configuration
  // ===========================================================================

  upsertWebhookConfig(config) {
    this.init();
    const now = new Date().toISOString();
    const enabledEvents = Array.isArray(config.enabled_events)
      ? JSON.stringify(config.enabled_events)
      : config.enabled_events || '["*"]';

    this.db
      .prepare(
        `INSERT INTO a2a_webhook_config (
        agent_address, endpoint_url, secret, enabled_events, active, client_cert, client_key, ca_cert, created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(agent_address) DO UPDATE SET
        endpoint_url = excluded.endpoint_url,
        secret = excluded.secret,
        enabled_events = excluded.enabled_events,
        active = excluded.active,
        client_cert = excluded.client_cert,
        client_key = excluded.client_key,
        ca_cert = excluded.ca_cert,
        updated_at = excluded.updated_at`,
      )
      .run(
        config.agent_address,
        config.endpoint_url,
        config.secret || null,
        enabledEvents,
        config.active !== undefined ? (config.active ? 1 : 0) : 1,
        config.client_cert || null,
        config.client_key || null,
        config.ca_cert || null,
        config.created_at || now,
        now,
      );

    return this.getWebhookConfig(config.agent_address);
  }

  getWebhookConfig(agentAddress) {
    this.init();
    const row = this.db
      .prepare('SELECT * FROM a2a_webhook_config WHERE agent_address = ?')
      .get(agentAddress);
    return row ? this._mapWebhookConfig(row) : null;
  }

  listWebhookConfigs(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.active !== undefined) {
      conditions.push('active = ?');
      params.push(filter.active ? 1 : 0);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    return this.db
      .prepare(`SELECT * FROM a2a_webhook_config ${where} ORDER BY created_at DESC`)
      .all(...params)
      .map(this._mapWebhookConfig);
  }

  _mapWebhookConfig(row) {
    return {
      ...row,
      enabled_events: JSON.parse(row.enabled_events || '["*"]'),
      active: Boolean(row.active),
    };
  }
}
