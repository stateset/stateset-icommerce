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
 * Four properties keep that guarantee honest:
 *
 *  1. Identity is `uuidv5(row.id, VES_OUTBOX_NAMESPACE)` and the envelope
 *     carries the row's own `created_at`, so the signing preimage — and
 *     therefore the signature — is a pure function of the committed row.
 *  2. A failure never marks the row published, so it stays drainable, and a
 *     retry is paced by `next_attempt_at` so a momentary outage cannot burn
 *     five attempts in five milliseconds. Only a row that exhausts
 *     `maxAttempts` is dead-lettered, so one permanently bad row cannot block
 *     the queue behind it forever.
 *  3. An `event_type` with no VES mapping is PARKED, not failed: it records
 *     the error and keeps its attempts, because the fix is a mapping in this
 *     file, not a retry. Burning attempts on a classification gap would
 *     dead-letter the row and delete the event from the log forever — the
 *     exact outcome this phase exists to prevent.
 *  4. Every settle statement is scoped to the lease this worker holds, so two
 *     drainers cannot trample each other's rows.
 */

import { createHash } from 'node:crypto';

import { bufferToHex, computePayloadPlainHash } from './crypto.js';

/** Fixed namespace for uuidv5 derivation of VES event ids from outbox row ids. */
export const VES_OUTBOX_NAMESPACE = '6f9619ff-8b86-d011-b42d-00c04fc964ff';

/**
 * The pump publishes the RECORDED tier only.
 *
 * `record_outbox_fact` stamps `tier = 'recorded'`: these are the engine's
 * transactional facts, and turning them into signed VES envelopes is this
 * component's whole job. Governed rows (`tier = 'governed'`) are a different
 * animal — a versioned `*.v1` vocabulary, already covered by the kernel's
 * sealed receipt chain, and with their own claim/ack protocol in
 * `crates/stateset-db/src/sqlite/kernel_outbox.rs`
 * (`claim_pending` / `mark_published_by` / `record_failure_by`). Draining them
 * here would mean inventing VES names for a vocabulary this component does not
 * own, and racing that Rust consumer for the same rows under a different lease
 * protocol. Left unleased, they stay exactly as they are for whoever publishes
 * them.
 */
const RECORDED_TIER = 'recorded';

/**
 * The one place the pump reaches past the `Outbox` API into its table.
 *
 * `Outbox.getByEventId` maps `created_at` through `new Date(...)`, which is
 * lossy for this purpose: two encodings of the same instant compare equal as
 * Dates but produce DIFFERENT signing preimages, hence different signatures.
 * The collision gate has to compare the stored timestamp byte-for-byte, so it
 * reads the raw column.
 */
const VES_OUTBOX_TABLE = '_ves_outbox';

/**
 * better-sqlite3 surfaces a duplicate `_ves_outbox.event_id` with this code.
 * Matched structurally — never on an English message substring — and only ever
 * as the FIRST half of the duplicate test; see `_classifyAppendFailure`.
 */
const SQLITE_UNIQUE_CONSTRAINT = 'SQLITE_CONSTRAINT_UNIQUE';

/**
 * Every `subscription.*` name the recorded tier can write.
 *
 * `crates/stateset-db/src/sqlite/subscriptions.rs` builds this name
 * dynamically — `&format!("subscription.{event_type}")` over
 * `SubscriptionEventType` — so it is invisible to a grep for string literals.
 * The suffix is the strum `Display` form: strum 0.26 picks the LONGEST
 * `serialize` attribute (`strum_macros/src/helpers/variant_props.rs:41`,
 * `max_by_key(len)`), which for every annotated variant is the snake_case
 * spelling, matching `serialize_all = "snake_case"` on the rest.
 *
 * `SubscriptionEventType` is `#[non_exhaustive]`: a new variant upstream will
 * arrive here unmapped. That is survivable only because an unmapped row is
 * parked rather than dead-lettered — see `_park`.
 */
const SUBSCRIPTION_EVENT_SUFFIXES = [
  'created',
  'activated',
  'trial_started',
  'trial_ended',
  'renewed',
  'payment_failed',
  'payment_retry_succeeded',
  'paused',
  'resumed',
  'skipped',
  'cancelled',
  'expired',
  'plan_changed',
  'items_modified',
  'quantity_changed',
  'address_updated',
  'payment_method_updated',
  'discount_applied',
  'discount_removed',
  'refunded',
];

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

  // Recorded facts the engine writes today, one entry per `RecordedFact`
  // site in crates/stateset-db/src/{sqlite,postgres}/. `cart.checked_out`
  // and `customer.created` are already covered above.
  'bill.item_added': 'bill.item_added',
  'billing_cycle.scheduled': 'billing_cycle.scheduled',
  'billing_cycle.status_changed': 'billing_cycle.status_changed',
  'credit_account.created': 'credit_account.created',
  'credit_account.transaction_recorded': 'credit_account.transaction_recorded',
  'credit_reservation.released': 'credit_reservation.released',
  'invoice.status_changed': 'invoice.status_changed',

  // The dynamic subscription family, expanded below.
  ...Object.fromEntries(
    SUBSCRIPTION_EVENT_SUFFIXES.map((suffix) => [
      `subscription.${suffix}`,
      `subscription.${suffix}`,
    ]),
  ),
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
 * @property {number} failed - Rows not published this cycle: a failed append
 *   (retried, then dead-lettered) or an unmapped event type (parked
 *   indefinitely). `last_error` distinguishes them.
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
   * @param {number} [config.retryDelaySeconds=60] - Backoff before a failed
   *   row is retried. Without it, `maxAttempts` drains issued milliseconds
   *   apart would dead-letter a row whose only problem was a momentarily
   *   unavailable key manager.
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
    this.retryDelaySeconds = config.retryDelaySeconds ?? 60;
    if (this.maxAttempts < 1) {
      throw new Error('OutboxPump requires config.maxAttempts >= 1');
    }
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
        // Parked, not failed: retrying cannot fix a missing mapping, and
        // burning attempts on it would dead-letter the row and delete the
        // event from the log. It waits — visible in last_error — until the
        // mapping exists, then drains normally.
        this._park(row.id, `unmapped kernel_outbox event_type '${row.event_type}'`);
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
       WHERE tier = ?
         AND published_at IS NULL
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
      const candidates = select.all(RECORDED_TIER, nowIso, nowIso, max);
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

    // Compared as STRINGS, against the raw column. Two encodings of one
    // instant ('…T00:00:00Z' vs '…T00:00:00+00:00') are equal as Dates but
    // hash to different signing preimages, so an instant comparison would
    // wave through an envelope whose signature is not the one we would have
    // produced. An unreadable or absent row fails closed.
    const storedRow = this.db
      .prepare(`SELECT created_at FROM ${VES_OUTBOX_TABLE} WHERE event_id = ?`)
      .get(event.eventId);
    if (!storedRow) {
      return 'stored envelope disappeared between lookup and timestamp read';
    }
    if (storedRow.created_at !== event.createdAt) {
      return `createdAt is '${storedRow.created_at}', expected '${event.createdAt}'`;
    }

    return null;
  }

  /**
   * Settle a row as published and release its lease.
   *
   * Scoped to the lease this worker holds, mirroring `mark_published_by` in
   * `crates/stateset-db/src/sqlite/kernel_outbox.rs`. Losing the race is safe:
   * the envelope exists, and whoever holds the lease re-derives the identical
   * one and absorbs it as a duplicate.
   *
   * @private
   * @param {string} id
   * @returns {boolean} whether this worker still owned the row
   */
  _publish(id) {
    const result = this.db
      .prepare(
        `UPDATE kernel_outbox
         SET published_at = ?, last_error = NULL, lease_owner = NULL, lease_expires_at = NULL
         WHERE id = ? AND lease_owner = ? AND published_at IS NULL`,
      )
      .run(new Date().toISOString(), id, this.leaseOwner);
    return result.changes === 1;
  }

  /**
   * Record a failure, release the lease, and schedule the retry.
   *
   * The row stays drainable (`published_at` untouched) but is paced by
   * `next_attempt_at`, so a transient outage cannot burn every attempt in one
   * tight loop. A row that exhausts `maxAttempts` is dead-lettered instead, so
   * one permanently bad row cannot block the queue behind it forever.
   *
   * Single statement, lease-scoped, matching `record_failure_by` in
   * `crates/stateset-db/src/sqlite/kernel_outbox.rs` — including its
   * `attempts + 1 >= max` CASE arms, which SQLite evaluates against the
   * pre-update row.
   *
   * Dead-lettered rows are not yet surfaced by any operator tool: `sync
   * doctor` today knows only about quarantined VES events and peer key pins,
   * not `kernel_outbox`. Reporting them is follow-up work.
   *
   * @private
   * @param {string} id
   * @param {string} message
   * @returns {boolean} whether this worker still owned the row
   */
  _fail(id, message) {
    const now = new Date();
    const nextAttemptAt = new Date(now.getTime() + this.retryDelaySeconds * 1000).toISOString();
    const result = this.db
      .prepare(
        `UPDATE kernel_outbox
         SET attempts = attempts + 1,
             last_error = ?,
             lease_owner = NULL,
             lease_expires_at = NULL,
             next_attempt_at = CASE WHEN attempts + 1 >= ? THEN NULL ELSE ? END,
             dead_lettered_at = CASE WHEN attempts + 1 >= ? THEN ? ELSE NULL END
         WHERE id = ? AND lease_owner = ? AND published_at IS NULL`,
      )
      .run(
        message,
        this.maxAttempts,
        nextAttemptAt,
        this.maxAttempts,
        now.toISOString(),
        id,
        this.leaseOwner,
      );
    return result.changes === 1;
  }

  /**
   * Park a row the pump cannot classify.
   *
   * Distinct from `_fail` on purpose: `attempts` is NOT incremented, so the
   * row can never dead-letter on this path. A missing `EVENT_TYPE_MAP` entry
   * is a gap in this file, not a property of the row — retrying cannot fix it,
   * and dead-lettering it would erase a committed event from the replicated
   * log forever. The row waits, its reason in `last_error`, and drains
   * normally once the mapping lands. Pacing it with the retry delay keeps it
   * from being re-examined on every cycle; it never blocks the rows behind it.
   *
   * @private
   * @param {string} id
   * @param {string} message
   * @returns {boolean} whether this worker still owned the row
   */
  _park(id, message) {
    const nextAttemptAt = new Date(Date.now() + this.retryDelaySeconds * 1000).toISOString();
    const result = this.db
      .prepare(
        `UPDATE kernel_outbox
         SET last_error = ?, lease_owner = NULL, lease_expires_at = NULL, next_attempt_at = ?
         WHERE id = ? AND lease_owner = ? AND published_at IS NULL`,
      )
      .run(message, nextAttemptAt, id, this.leaseOwner);
    return result.changes === 1;
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
