/**
 * A2A Store — escrows (create/release/refund).
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { randomUUID } from 'node:crypto';

/**
 * A2A Store — escrows (create/release/refund).
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2AEscrowMethods {
  // ===========================================================================
  // Escrows
  // ===========================================================================

  /**
   * Create an escrow record.
   * @param {object} escrow
   * @returns {object} The created escrow row.
   */
  createEscrow(escrow) {
    this.init();
    const stmt = this.db.prepare(`
      INSERT INTO a2a_escrows (
        id, status, quote_id, payment_id, buyer_address, seller_address,
        amount, amount_decimal, asset, network, release_conditions,
        funded_at, released_at, disputed_at, dispute_id, expires_at,
        auto_release_after, metadata, created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    const releaseConditions = Array.isArray(escrow.release_conditions)
      ? JSON.stringify(escrow.release_conditions)
      : escrow.release_conditions || '[]';

    const id = escrow.id || randomUUID();
    const now = new Date().toISOString();

    stmt.run(
      id,
      escrow.status || 'created',
      escrow.quote_id || null,
      escrow.payment_id || null,
      escrow.buyer_address,
      escrow.seller_address,
      escrow.amount,
      escrow.amount_decimal,
      escrow.asset || 'USDC',
      escrow.network || 'set_chain',
      releaseConditions,
      escrow.funded_at || null,
      escrow.released_at || null,
      escrow.disputed_at || null,
      escrow.dispute_id || null,
      escrow.expires_at,
      escrow.auto_release_after || null,
      escrow.metadata || null,
      escrow.created_at || now,
      escrow.updated_at || now,
    );

    return this.getEscrow(id);
  }

  /**
   * Get an escrow by ID.
   * @param {string} id
   * @returns {object|null}
   */
  getEscrow(id) {
    this.init();
    const row = this.db.prepare('SELECT * FROM a2a_escrows WHERE id = ?').get(id);
    return row ? this._mapEscrow(row) : null;
  }

  /**
   * Update an escrow record.
   * @param {string} id
   * @param {object} updates
   * @returns {object|null}
   */
  updateEscrow(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_escrows', Object.keys(updates));
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        if (key === 'release_conditions' && Array.isArray(value)) {
          fields.push(`${key} = ?`);
          values.push(JSON.stringify(value));
        } else {
          fields.push(`${key} = ?`);
          values.push(value);
        }
      }
    }

    if (fields.length === 0) return this.getEscrow(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db.prepare(`UPDATE a2a_escrows SET ${fields.join(', ')} WHERE id = ?`).run(...values);
    return this.getEscrow(id);
  }

  /**
   * Atomically release an escrow, preventing double-release via conditional UPDATE.
   * Uses better-sqlite3's transaction() which acquires BEGIN IMMEDIATE by default.
   *
   * @param {string} id - Escrow ID
   * @returns {object} Updated escrow record
   * @throws {Error} If escrow not found or not in releasable status
   */
  releaseEscrowAtomic(id) {
    this.init();
    const txn = this.db.transaction(() => {
      const now = new Date().toISOString();
      const result = this.db
        .prepare(
          `UPDATE a2a_escrows
           SET status = 'released', released_at = ?, updated_at = ?
           WHERE id = ? AND status IN ('funded', 'active')`,
        )
        .run(now, now, id);

      if (result.changes === 0) {
        const current = this.getEscrow(id);
        if (!current) throw new Error(`Escrow not found: ${id}`);
        throw new Error(`Cannot release escrow in status: ${current.status}`);
      }
      return this.getEscrow(id);
    });
    return txn();
  }

  /**
   * Atomically refund an escrow, preventing double-refund via conditional UPDATE.
   *
   * @param {string} id - Escrow ID
   * @returns {object} Updated escrow record
   * @throws {Error} If escrow not found or not in refundable status
   */
  refundEscrowAtomic(id) {
    this.init();
    const txn = this.db.transaction(() => {
      const now = new Date().toISOString();
      const result = this.db
        .prepare(
          `UPDATE a2a_escrows
           SET status = 'refunded', updated_at = ?
           WHERE id = ? AND status IN ('funded', 'active', 'disputed')`,
        )
        .run(now, id);

      if (result.changes === 0) {
        const current = this.getEscrow(id);
        if (!current) throw new Error(`Escrow not found: ${id}`);
        throw new Error(`Cannot refund escrow in status: ${current.status}`);
      }
      return this.getEscrow(id);
    });
    return txn();
  }

  /**
   * List escrows with optional filters.
   * @param {object} filter
   * @returns {object[]}
   */
  listEscrows(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.buyer_address) {
      conditions.push('buyer_address = ?');
      params.push(filter.buyer_address);
    }
    if (filter.seller_address) {
      conditions.push('seller_address = ?');
      params.push(filter.seller_address);
    }
    if (filter.status) {
      conditions.push('status = ?');
      params.push(filter.status);
    }
    if (filter.quote_id) {
      conditions.push('quote_id = ?');
      params.push(filter.quote_id);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 50;
    const offset = filter.offset || 0;

    const rows = this.db
      .prepare(`SELECT * FROM a2a_escrows ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`)
      .all(...params, limit, offset);

    return rows.map(this._mapEscrow);
  }

  /** @param {object} row */
  _mapEscrow(row) {
    return {
      ...row,
      release_conditions: JSON.parse(row.release_conditions || '[]'),
    };
  }
}
