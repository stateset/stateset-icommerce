/**
 * A2A Store — disputes and dispute evidence.
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { randomUUID } from 'node:crypto';

/**
 * A2A Store — disputes and dispute evidence.
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2ADisputesMethods {
  // ===========================================================================
  // Disputes
  // ===========================================================================

  /**
   * Create a dispute record.
   * @param {object} dispute
   * @returns {object} The created dispute row.
   */
  createDispute(dispute) {
    this.init();
    const stmt = this.db.prepare(`
      INSERT INTO a2a_disputes (
        id, status, escrow_id, quote_id, filed_by, filed_against,
        reason, category, amount_disputed, amount_decimal, asset,
        resolution_type, resolution_amount, resolution_note, resolved_by,
        evidence_deadline, review_deadline, metadata,
        created_at, updated_at, resolved_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    const id = dispute.id || randomUUID();
    const now = new Date().toISOString();

    stmt.run(
      id,
      dispute.status || 'filed',
      dispute.escrow_id,
      dispute.quote_id || null,
      dispute.filed_by,
      dispute.filed_against,
      dispute.reason,
      dispute.category || 'non_delivery',
      dispute.amount_disputed,
      dispute.amount_decimal,
      dispute.asset,
      dispute.resolution_type || null,
      dispute.resolution_amount || null,
      dispute.resolution_note || null,
      dispute.resolved_by || null,
      dispute.evidence_deadline || null,
      dispute.review_deadline || null,
      dispute.metadata || null,
      dispute.created_at || now,
      dispute.updated_at || now,
      dispute.resolved_at || null,
    );

    return this.getDispute(id);
  }

  /**
   * Get a dispute by ID.
   * @param {string} id
   * @returns {object|null}
   */
  getDispute(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_disputes WHERE id = ?').get(id) || null;
  }

  /**
   * Update a dispute record.
   * @param {string} id
   * @param {object} updates
   * @returns {object|null}
   */
  updateDispute(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_disputes', Object.keys(updates));
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(value);
      }
    }

    if (fields.length === 0) return this.getDispute(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db.prepare(`UPDATE a2a_disputes SET ${fields.join(', ')} WHERE id = ?`).run(...values);
    return this.getDispute(id);
  }

  /**
   * List disputes with optional filters.
   * @param {object} filter
   * @returns {object[]}
   */
  listDisputes(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.escrow_id) {
      conditions.push('escrow_id = ?');
      params.push(filter.escrow_id);
    }
    if (filter.status) {
      conditions.push('status = ?');
      params.push(filter.status);
    }
    if (filter.filed_by) {
      conditions.push('filed_by = ?');
      params.push(filter.filed_by);
    }
    if (filter.filed_against) {
      conditions.push('filed_against = ?');
      params.push(filter.filed_against);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 50;
    const offset = filter.offset || 0;

    return this.db
      .prepare(`SELECT * FROM a2a_disputes ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`)
      .all(...params, limit, offset);
  }

  // ===========================================================================
  // Dispute Evidence
  // ===========================================================================

  /**
   * Create a dispute evidence record.
   * @param {object} evidence
   * @returns {object} The created evidence row.
   */
  createEvidence(evidence) {
    this.init();
    const stmt = this.db.prepare(`
      INSERT INTO a2a_dispute_evidence (
        id, dispute_id, submitted_by, evidence_type, title,
        description, content, content_hash, created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    const id = evidence.id || randomUUID();
    const now = new Date().toISOString();

    stmt.run(
      id,
      evidence.dispute_id,
      evidence.submitted_by,
      evidence.evidence_type,
      evidence.title,
      evidence.description || null,
      evidence.content || null,
      evidence.content_hash || null,
      evidence.created_at || now,
    );

    return this.getEvidence(id);
  }

  /**
   * Get a single evidence record by ID.
   * @param {string} id
   * @returns {object|null}
   */
  getEvidence(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_dispute_evidence WHERE id = ?').get(id) || null;
  }

  /**
   * List all evidence for a given dispute.
   * @param {string} disputeId
   * @returns {object[]}
   */
  listEvidenceByDispute(disputeId) {
    this.init();
    return this.db
      .prepare('SELECT * FROM a2a_dispute_evidence WHERE dispute_id = ? ORDER BY created_at ASC')
      .all(disputeId);
  }
}
