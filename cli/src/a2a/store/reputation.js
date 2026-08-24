/**
 * A2A Store — feedback and reputation scores.
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { randomUUID } from 'node:crypto';

/**
 * A2A Store — feedback and reputation scores.
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2AReputationMethods {
  // ===========================================================================
  // Feedback
  // ===========================================================================

  /**
   * Create a feedback record.
   * @param {object} feedback
   * @returns {object} The created feedback row.
   */
  createFeedback(feedback) {
    this.init();
    const stmt = this.db.prepare(`
      INSERT INTO a2a_feedback (
        id, agent_address, reviewer_address, transaction_type, transaction_id,
        score, dimensions, comment, response, response_at, is_revoked, created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    const dimensions =
      typeof feedback.dimensions === 'object' && feedback.dimensions !== null
        ? JSON.stringify(feedback.dimensions)
        : feedback.dimensions || '{}';

    const id = feedback.id || randomUUID();
    const now = new Date().toISOString();

    stmt.run(
      id,
      feedback.agent_address,
      feedback.reviewer_address,
      feedback.transaction_type,
      feedback.transaction_id,
      feedback.score,
      dimensions,
      feedback.comment || null,
      feedback.response || null,
      feedback.response_at || null,
      feedback.is_revoked ? 1 : 0,
      feedback.created_at || now,
    );

    return this.getFeedback(id);
  }

  /**
   * Get a single feedback record by ID.
   * @param {string} id
   * @returns {object|null}
   */
  getFeedback(id) {
    this.init();
    const row = this.db.prepare('SELECT * FROM a2a_feedback WHERE id = ?').get(id);
    return row ? this._mapFeedback(row) : null;
  }

  /**
   * Update a feedback record (e.g. to add a response or revoke).
   * @param {string} id
   * @param {object} updates
   * @returns {object|null}
   */
  updateFeedback(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_feedback', Object.keys(updates));
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        if (key === 'dimensions' && typeof value === 'object' && value !== null) {
          fields.push(`${key} = ?`);
          values.push(JSON.stringify(value));
        } else if (key === 'is_revoked') {
          fields.push(`${key} = ?`);
          values.push(value ? 1 : 0);
        } else {
          fields.push(`${key} = ?`);
          values.push(value);
        }
      }
    }

    if (fields.length === 0) return this.getFeedback(id);

    values.push(id);

    this.db.prepare(`UPDATE a2a_feedback SET ${fields.join(', ')} WHERE id = ?`).run(...values);
    return this.getFeedback(id);
  }

  /**
   * List feedback with optional filters.
   * @param {object} filter
   * @returns {object[]}
   */
  listFeedback(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.agent_address) {
      conditions.push('agent_address = ?');
      params.push(filter.agent_address);
    }
    if (filter.reviewer_address) {
      conditions.push('reviewer_address = ?');
      params.push(filter.reviewer_address);
    }
    if (filter.transaction_type) {
      conditions.push('transaction_type = ?');
      params.push(filter.transaction_type);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 50;
    const offset = filter.offset || 0;

    const rows = this.db
      .prepare(`SELECT * FROM a2a_feedback ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`)
      .all(...params, limit, offset);

    return rows.map(this._mapFeedback);
  }

  /**
   * Get a feedback summary (average score + count) for a given agent.
   * @param {string} agentAddress
   * @returns {{ average_score: number, count: number }}
   */
  getFeedbackSummary(agentAddress) {
    this.init();
    const row = this.db
      .prepare(
        `
      SELECT
        COALESCE(AVG(score), 0) as average_score,
        COUNT(*) as count
      FROM a2a_feedback
      WHERE agent_address = ? AND is_revoked = 0
    `,
      )
      .get(agentAddress);

    return {
      average_score: row?.average_score || 0,
      count: row?.count || 0,
    };
  }

  /** @param {object} row */
  _mapFeedback(row) {
    return {
      ...row,
      dimensions: JSON.parse(row.dimensions || '{}'),
      is_revoked: Boolean(row.is_revoked),
    };
  }

  // ===========================================================================
  // Reputation Scores
  // ===========================================================================

  /**
   * Get a reputation score by agent address.
   * @param {string} agentAddress
   * @returns {object|null}
   */
  getReputationScore(agentAddress) {
    this.init();
    const row = this.db
      .prepare('SELECT * FROM a2a_reputation_scores WHERE agent_address = ?')
      .get(agentAddress);
    return row ? this._mapReputationScore(row) : null;
  }

  /**
   * Upsert (insert or update) a reputation score.
   * @param {object} score
   * @returns {object}
   */
  upsertReputationScore(score) {
    this.init();

    const dimensionScores =
      typeof score.dimension_scores === 'object' && score.dimension_scores !== null
        ? JSON.stringify(score.dimension_scores)
        : score.dimension_scores || '{}';

    const now = new Date().toISOString();

    this.db
      .prepare(
        `
      INSERT INTO a2a_reputation_scores (
        agent_address, total_transactions, successful_transactions, disputed_transactions,
        average_score, dimension_scores, trust_tier, last_updated
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(agent_address) DO UPDATE SET
        total_transactions = excluded.total_transactions,
        successful_transactions = excluded.successful_transactions,
        disputed_transactions = excluded.disputed_transactions,
        average_score = excluded.average_score,
        dimension_scores = excluded.dimension_scores,
        trust_tier = excluded.trust_tier,
        last_updated = excluded.last_updated
    `,
      )
      .run(
        score.agent_address,
        score.total_transactions ?? 0,
        score.successful_transactions ?? 0,
        score.disputed_transactions ?? 0,
        score.average_score ?? 0,
        dimensionScores,
        score.trust_tier || 'sandbox',
        score.last_updated || now,
      );

    return this.getReputationScore(score.agent_address);
  }

  /** @param {object} row */
  _mapReputationScore(row) {
    return {
      ...row,
      dimension_scores: JSON.parse(row.dimension_scores || '{}'),
    };
  }
}
