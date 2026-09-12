/**
 * Outbox Pump
 *
 * Turns committed `kernel_outbox` rows into signed VES envelopes.
 *
 * The engine writes its fact in the same transaction as the mutation, so the
 * commit — not the signature — is the point past which an event cannot be
 * lost. Signing afterwards is safe because the envelope is derived
 * deterministically from the committed row: a crash between commit and sign
 * re-derives a byte-identical envelope on the next cycle.
 *
 * Three properties keep that guarantee honest:
 *
 *  1. Identity is `uuidv5(row.id, VES_OUTBOX_NAMESPACE)` and the envelope
 *     carries the row's own `created_at`, so the signing preimage — and
 *     therefore the signature — is a pure function of the committed row.
 *  2. A failure never marks the row published, so it stays drainable. Only a
 *     row that exhausts `maxAttempts` is dead-lettered, so one permanently
 *     bad row cannot block the queue behind it forever.
 *  3. An `event_type` with no VES mapping fails loudly. A wrong event name in
 *     a replicated log is worse than a missing one.
 */

import { createHash } from 'node:crypto';

import { bufferToHex, computePayloadPlainHash } from './crypto.js';

/** Fixed namespace for uuidv5 derivation of VES event ids from outbox row ids. */
export const VES_OUTBOX_NAMESPACE = '6f9619ff-8b86-d011-b42d-00c04fc964ff';

/**
 * better-sqlite3 surfaces a duplicate `_ves_outbox.event_id` with this code.
 * Matched structurally — never on an English message substring — and only ever
 * as the FIRST half of the duplicate test; see `_classifyAppendFailure`.
 */
const SQLITE_UNIQUE_CONSTRAINT = 'SQLITE_CONSTRAINT_UNIQUE';

/**
 * `kernel_outbox.event_type` → VES `event_type`.
 *
 * This is the replicated log's vocabulary, relocated from
 * `cli/src/sync/capture.js` so retiring interception does not change the
 * names already on the wire. It is an allow-list on purpose: anything absent
 * is refused rather than guessed.
 *
 * @type {Record<string, string>}
 */
const EVENT_TYPE_MAP = {
  // Orders
  'order.created': 'order.created',
  'order.updated': 'order.updated',
  'order.status_changed': 'order.status_changed',
  'order.shipped': 'order.shipped',
  'order.cancelled': 'order.cancelled',

  // Customers
  'customer.created': 'customer.created',
  'customer.updated': 'customer.updated',
  'customer.deleted': 'customer.deleted',

  // Products
  'product.created': 'product.created',
  'product.updated': 'product.updated',
  'product.deleted': 'product.deleted',

  // Inventory
  'inventory.created': 'inventory.created',
  'inventory.adjusted': 'inventory.adjusted',
  'inventory.reserved': 'inventory.reserved',
  'inventory.released': 'inventory.released',
  'inventory.confirmed': 'inventory.confirmed',

  // Carts
  'cart.created': 'cart.created',
  'cart.item_added': 'cart.item_added',
  'cart.item_updated': 'cart.item_updated',
  'cart.item_removed': 'cart.item_removed',
  'cart.shipping_set': 'cart.shipping_set',
  'cart.payment_set': 'cart.payment_set',
  'cart.discount_applied': 'cart.discount_applied',
  'cart.checked_out': 'cart.checked_out',
  'cart.cancelled': 'cart.cancelled',
  'cart.abandoned': 'cart.abandoned',

  // Payments
  'payment.created': 'payment.created',
  'payment.completed': 'payment.completed',
  'payment.failed': 'payment.failed',
  'payment.refunded': 'payment.refunded',

  // Returns
  'return.requested': 'return.requested',
  'return.approved': 'return.approved',
  'return.rejected': 'return.rejected',
  'return.completed': 'return.completed',

  // Finance facts emitted by the engine's recorded tier
  'bill.item_added': 'bill.item_added',
  'billing_cycle.scheduled': 'billing_cycle.scheduled',
  'billing_cycle.status_changed': 'billing_cycle.status_changed',
  'credit_account.created': 'credit_account.created',
  'credit_account.transaction_recorded': 'credit_account.transaction_recorded',
  'credit_reservation.released': 'credit_reservation.released',
  'invoice.status_changed': 'invoice.status_changed',
  // Extend as further modules emit recorded facts. Adding a name here is a
  // protocol change: it becomes part of the replicated log's vocabulary.
};

/**
 * RFC 4122 v5 (SHA-1, namespace + name). Deterministic identity is what makes
 * replay idempotent.
 * @param {string} namespace - UUID
 * @param {string} name
 * @returns {string} UUID
 */
export function uuidv5(namespace, name) {
  const nsBytes = Buffer.from(namespace.replace(/-/g, ''), 'hex');
  const hash = createHash('sha1')
    .update(Buffer.concat([nsBytes, Buffer.from(name, 'utf8')]))
    .digest();
  const bytes = Buffer.from(hash.subarray(0, 16));
  bytes[6] = (bytes[6] & 0x0f) | 0x50;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = bytes.toString('hex');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

/**
 * @typedef {Object} DrainResult
 * @property {number} leased - Rows claimed this cycle
 * @property {number} appended - Rows turned into a new signed envelope
 * @property {number} failed - Rows whose publication failed (still drainable)
 * @property {number} duplicates - Rows whose envelope already existed (replay)
 */

export class OutboxPump {
  /**
   * @param {import('better-sqlite3').Database} db
   * @param {import('./outbox.js').Outbox} outbox
   * @param {Object} config
   * @param {{tenantId: string, storeId: string, agentId: string}} config.identity
   * @param {string} [config.leaseOwner]
   * @param {number} [config.leaseSeconds=300]
   * @param {number} [config.maxAttempts=5]
   */
  constructor(db, outbox, config) {
    if (!config || !config.identity) {
      throw new Error('OutboxPump requires config.identity {tenantId, storeId, agentId}');
    }
    this.db = db;
    this.outbox = outbox;
    this.identity = config.identity;
    this.leaseOwner = config.leaseOwner || `pump-${process.pid}`;
    this.leaseSeconds = config.leaseSeconds ?? 300;
    this.maxAttempts = config.maxAttempts ?? 5;
  }

  /**
   * Drain one batch: lease deliverable rows, publish each, settle it.
   *
   * @param {number} [limit=100]
   * @returns {Promise<DrainResult>}
   */
  async drain(limit = 100) {
    const rows = this._lease(limit);

    let appended = 0;
    let failed = 0;
    let duplicates = 0;

    for (const row of rows) {
      const eventType = EVENT_TYPE_MAP[row.event_type];
      if (!eventType) {
        this._fail(row.id, `unmapped kernel_outbox event_type '${row.event_type}'`);
        failed += 1;
        continue;
      }

      const eventId = uuidv5(VES_OUTBOX_NAMESPACE, row.id);
      let payload;
      try {
        payload = JSON.parse(row.payload);
      } catch (error) {
        this._fail(row.id, `unreadable kernel_outbox payload: ${error.message}`);
        failed += 1;
        continue;
      }

      const event = {
        tenantId: this.identity.tenantId,
        storeId: this.identity.storeId,
        entityType: row.aggregate_type,
        entityId: row.aggregate_id,
        eventType,
        payload,
        sourceAgent: this.identity.agentId,
        commandId: row.command_id || undefined,
        eventId,
        createdAt: row.created_at,
      };

      try {
        await this.outbox.append(event);
        this._publish(row.id);
        appended += 1;
      } catch (error) {
        const verdict = this._classifyAppendFailure(error, event);
        if (verdict.duplicate) {
          // The envelope this row derives already exists: a crash between
          // append() and the published_at update. Settling the row is the
          // completion of that interrupted cycle, not a new publication.
          this._publish(row.id);
          duplicates += 1;
          continue;
        }
        this._fail(row.id, verdict.message);
        failed += 1;
      }
    }

    return { leased: rows.length, appended, failed, duplicates };
  }

  /**
   * Claim deliverable rows for this worker.
   *
   * Leasing is what lets a second pump run without doing the same work twice;
   * an expired lease is reclaimable so a crashed worker does not strand rows.
   *
   * @private
   * @param {number} limit
   * @returns {Array<Object>}
   */
  _lease(limit) {
    const now = new Date();
    const nowIso = now.toISOString();
    const expiresIso = new Date(now.getTime() + this.leaseSeconds * 1000).toISOString();

    const select = this.db.prepare(
      `SELECT id, event_type, aggregate_type, aggregate_id, payload,
              command_id, created_at
       FROM kernel_outbox
       WHERE published_at IS NULL
         AND dead_lettered_at IS NULL
         AND (next_attempt_at IS NULL OR next_attempt_at <= ?)
         AND (lease_expires_at IS NULL OR lease_expires_at <= ?)
       ORDER BY created_at ASC, id ASC
       LIMIT ?`,
    );
    const claim = this.db.prepare(
      'UPDATE kernel_outbox SET lease_owner = ?, lease_expires_at = ? WHERE id = ?',
    );

    const leaseBatch = this.db.transaction((max) => {
      const candidates = select.all(nowIso, nowIso, max);
      for (const candidate of candidates) {
        claim.run(this.leaseOwner, expiresIso, candidate.id);
      }
      return candidates;
    });

    return leaseBatch(limit);
  }

  /**
   * Decide whether an append failure is this row's own already-written
   * envelope (a replay) or a genuine error.
   *
   * Two gates, both required, because swallowing a real error as "already
   * published" would lose the event this whole design exists to preserve:
   *
   *  1. the error must structurally be a SQLite UNIQUE-constraint failure
   *     (matched on `code`, never on message text); and
   *  2. the outbox must actually hold a row under OUR derived `event_id`
   *     whose content matches what we were about to write.
   *
   * A UNIQUE failure from anywhere else fails gate 2 and is reported as a
   * failure. A stored row that disagrees with the committed row is a genuine
   * identity collision and is reported as a failure too — publishing it would
   * leave the wrong event in the log.
   *
   * @private
   * @param {Error & {code?: string}} error
   * @param {Object} event - the envelope we tried to append
   * @returns {{duplicate: boolean, message: string}}
   */
  _classifyAppendFailure(error, event) {
    const message = error?.message ?? String(error);

    if (error?.code !== SQLITE_UNIQUE_CONSTRAINT) {
      return { duplicate: false, message };
    }
    if (typeof this.outbox.getByEventId !== 'function') {
      return { duplicate: false, message };
    }

    let existing = null;
    try {
      existing = this.outbox.getByEventId(event.eventId);
    } catch {
      return { duplicate: false, message };
    }
    if (!existing) {
      // Some other UNIQUE constraint tripped. Not our replay — fail loudly.
      return { duplicate: false, message };
    }

    const mismatch = this._describeMismatch(existing, event);
    if (mismatch) {
      return {
        duplicate: false,
        message: `event_id collision for ${event.eventId}: ${mismatch}`,
      };
    }

    return { duplicate: true, message };
  }

  /**
   * Compare a stored envelope with the one derived from the committed row.
   * @private
   * @param {Object} existing
   * @param {Object} event
   * @returns {string|null} a description of the first difference, or null
   */
  _describeMismatch(existing, event) {
    const fields = [
      ['tenantId', existing.tenantId, event.tenantId],
      ['storeId', existing.storeId, event.storeId],
      ['entityType', existing.entityType, event.entityType],
      ['entityId', existing.entityId, event.entityId],
      ['eventType', existing.eventType, event.eventType],
      ['sourceAgent', existing.sourceAgent, event.sourceAgent],
    ];
    for (const [name, stored, derived] of fields) {
      if (stored !== derived) {
        return `${name} is '${stored}', expected '${derived}'`;
      }
    }

    const derivedHash = bufferToHex(computePayloadPlainHash(event.payload));
    if (existing.payloadPlainHash !== derivedHash) {
      return `payload hash is '${existing.payloadPlainHash}', expected '${derivedHash}'`;
    }

    const storedAt = existing.createdAt instanceof Date ? existing.createdAt.getTime() : NaN;
    const derivedAt = new Date(event.createdAt).getTime();
    if (Number.isFinite(storedAt) && Number.isFinite(derivedAt) && storedAt !== derivedAt) {
      return `createdAt is '${existing.createdAt.toISOString()}', expected '${event.createdAt}'`;
    }

    return null;
  }

  /**
   * Settle a row as published and release its lease.
   * @private
   * @param {string} id
   */
  _publish(id) {
    this.db
      .prepare(
        `UPDATE kernel_outbox
         SET published_at = ?, last_error = NULL, lease_owner = NULL, lease_expires_at = NULL
         WHERE id = ?`,
      )
      .run(new Date().toISOString(), id);
  }

  /**
   * Record a failure and release the lease so the row stays drainable. A row
   * that has exhausted its attempts is dead-lettered so one permanently bad
   * row cannot block the queue behind it forever — `sync doctor` surfaces it
   * for an operator.
   *
   * @private
   * @param {string} id
   * @param {string} message
   */
  _fail(id, message) {
    const row = this.db
      .prepare(
        `UPDATE kernel_outbox
         SET attempts = attempts + 1, last_error = ?, lease_owner = NULL, lease_expires_at = NULL
         WHERE id = ?
         RETURNING attempts`,
      )
      .get(message, id);

    if (row && row.attempts >= this.maxAttempts) {
      this.db
        .prepare('UPDATE kernel_outbox SET dead_lettered_at = ? WHERE id = ?')
        .run(new Date().toISOString(), id);
    }
  }
}

/**
 * @param {import('better-sqlite3').Database} db
 * @param {import('./outbox.js').Outbox} outbox
 * @param {Object} config
 * @param {{tenantId: string, storeId: string, agentId: string}} config.identity
 * @returns {OutboxPump}
 */
export function createOutboxPump(db, outbox, config) {
  return new OutboxPump(db, outbox, config);
}
