/**
 * A2A Store — service listings, RFQs and RFQ responses.
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { randomUUID } from 'node:crypto';

/**
 * A2A Store — service listings, RFQs and RFQ responses.
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2AMarketplaceMethods {
  // ===========================================================================
  // Services
  // ===========================================================================

  /**
   * Create a service record.
   * @param {object} service
   * @returns {object} The created service row.
   */
  createService(service) {
    this.init();
    const stmt = this.db.prepare(`
      INSERT INTO a2a_services (
        id, agent_address, name, description, category, pricing_model,
        pricing_details, active, input_schema, output_schema, endpoint_url,
        avg_response_time, success_rate, transaction_count, metadata,
        created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    const id = service.id || randomUUID();
    const now = new Date().toISOString();

    stmt.run(
      id,
      service.agent_address,
      service.name,
      service.description,
      service.category || 'other',
      service.pricing_model || 'quote',
      service.pricing_details || null,
      service.active !== undefined ? (service.active ? 1 : 0) : 1,
      service.input_schema || null,
      service.output_schema || null,
      service.endpoint_url || null,
      service.avg_response_time || null,
      service.success_rate || null,
      service.transaction_count || 0,
      service.metadata || null,
      service.created_at || now,
      service.updated_at || now,
    );

    return this.getService(id);
  }

  /**
   * Get a single service by ID.
   * @param {string} id
   * @returns {object|null}
   */
  getService(id) {
    this.init();
    const row = this.db.prepare('SELECT * FROM a2a_services WHERE id = ?').get(id);
    return row ? this._mapService(row) : null;
  }

  /**
   * Update a service record.
   * @param {string} id
   * @param {object} updates
   * @returns {object|null}
   */
  updateService(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_services', Object.keys(updates));
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        if (key === 'active') {
          fields.push(`${key} = ?`);
          values.push(value ? 1 : 0);
        } else {
          fields.push(`${key} = ?`);
          values.push(value);
        }
      }
    }

    if (fields.length === 0) return this.getService(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db.prepare(`UPDATE a2a_services SET ${fields.join(', ')} WHERE id = ?`).run(...values);
    return this.getService(id);
  }

  /**
   * List services with optional filters and search.
   * @param {object} filter
   * @returns {object[]}
   */
  listServices(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.agent_address) {
      conditions.push('agent_address = ?');
      params.push(filter.agent_address);
    }
    if (filter.category) {
      conditions.push('category = ?');
      params.push(filter.category);
    }
    if (filter.active !== undefined) {
      conditions.push('active = ?');
      params.push(filter.active ? 1 : 0);
    }
    if (filter.search) {
      conditions.push('(name LIKE ? OR description LIKE ?)');
      const term = `%${filter.search}%`;
      params.push(term, term);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 50;
    const offset = filter.offset || 0;

    const rows = this.db
      .prepare(`SELECT * FROM a2a_services ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`)
      .all(...params, limit, offset);

    return rows.map(this._mapService);
  }

  /** @param {object} row */
  _mapService(row) {
    return {
      ...row,
      active: Boolean(row.active),
    };
  }

  // ===========================================================================
  // RFQs
  // ===========================================================================

  createRFQ(rfq) {
    this.init();
    const id = rfq.id || randomUUID();
    const now = new Date().toISOString();
    this.db
      .prepare(
        `INSERT INTO a2a_rfqs (id, status, buyer_address, buyer_agent_id, items, seller_filter, max_responses, deadline, scoring_criteria, winning_quote_id, metadata, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        rfq.status || 'open',
        rfq.buyer_address,
        rfq.buyer_agent_id || null,
        typeof rfq.items === 'string' ? rfq.items : JSON.stringify(rfq.items || []),
        rfq.seller_filter || null,
        rfq.max_responses ?? 10,
        rfq.deadline,
        rfq.scoring_criteria || 'cheapest',
        rfq.winning_quote_id || null,
        typeof rfq.metadata === 'string'
          ? rfq.metadata
          : rfq.metadata
            ? JSON.stringify(rfq.metadata)
            : null,
        now,
        now,
      );
    return this.getRFQ(id);
  }

  getRFQ(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_rfqs WHERE id = ?').get(id) || null;
  }

  updateRFQ(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_rfqs', Object.keys(updates));
    const fields = [];
    const values = [];
    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(typeof value === 'object' && value !== null ? JSON.stringify(value) : value);
      }
    }
    if (fields.length === 0) return this.getRFQ(id);
    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);
    this.db.prepare(`UPDATE a2a_rfqs SET ${fields.join(', ')} WHERE id = ?`).run(...values);
    return this.getRFQ(id);
  }

  listRFQs(filter = {}) {
    this.init();
    const clauses = [];
    const params = [];
    if (filter.buyer_address) {
      clauses.push('buyer_address = ?');
      params.push(filter.buyer_address);
    }
    if (filter.status) {
      clauses.push('status = ?');
      params.push(filter.status);
    }
    const where = clauses.length > 0 ? `WHERE ${clauses.join(' AND ')}` : '';
    const limit = filter.limit ? `LIMIT ${Math.min(Number(filter.limit), 1000)}` : 'LIMIT 100';
    return this.db
      .prepare(`SELECT * FROM a2a_rfqs ${where} ORDER BY created_at DESC ${limit}`)
      .all(...params);
  }

  // ===========================================================================
  // RFQ Responses
  // ===========================================================================

  createRFQResponse(resp) {
    this.init();
    const id = resp.id || randomUUID();
    const now = new Date().toISOString();
    this.db
      .prepare(
        `INSERT INTO a2a_rfq_responses (id, rfq_id, seller_address, quote_id, score, rank, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        resp.rfq_id,
        resp.seller_address,
        resp.quote_id,
        resp.score ?? null,
        resp.rank ?? null,
        resp.status || 'pending',
        now,
      );
    return this.getRFQResponse(id);
  }

  getRFQResponse(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_rfq_responses WHERE id = ?').get(id) || null;
  }

  updateRFQResponse(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_rfq_responses', Object.keys(updates));
    const fields = [];
    const values = [];
    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(typeof value === 'object' && value !== null ? JSON.stringify(value) : value);
      }
    }
    if (fields.length === 0) return this.getRFQResponse(id);
    values.push(id);
    this.db
      .prepare(`UPDATE a2a_rfq_responses SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getRFQResponse(id);
  }

  listRFQResponses(filter = {}) {
    this.init();
    const clauses = [];
    const params = [];
    if (filter.rfq_id) {
      clauses.push('rfq_id = ?');
      params.push(filter.rfq_id);
    }
    if (filter.seller_address) {
      clauses.push('seller_address = ?');
      params.push(filter.seller_address);
    }
    if (filter.status) {
      clauses.push('status = ?');
      params.push(filter.status);
    }
    const where = clauses.length > 0 ? `WHERE ${clauses.join(' AND ')}` : '';
    return this.db
      .prepare(`SELECT * FROM a2a_rfq_responses ${where} ORDER BY score DESC NULLS LAST`)
      .all(...params);
  }
}
