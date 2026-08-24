/**
 * A2A Store — quotes and negotiation.
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { randomUUID } from 'node:crypto';

/**
 * A2A Store — quotes and negotiation.
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2AQuotesMethods {
  // ===========================================================================
  // Quotes
  // ===========================================================================

  createQuote(quote) {
    this.init();
    const stmt = this.db.prepare(`
      INSERT INTO a2a_quotes (
        id, status, buyer_agent_id, buyer_address, seller_agent_id, seller_address,
        items, subtotal, fees, tax, total, total_decimal, asset, accepted_networks,
        expires_at, terms, estimated_delivery, delivery_method, fulfillment_instructions,
        payment_id, payment_request_id, request_message, response_message, metadata,
        created_at, quoted_at, accepted_at, fulfilled_at, updated_at,
        counter_count, negotiation_history, max_rounds, escrow_id
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    const items = Array.isArray(quote.items) ? JSON.stringify(quote.items) : quote.items || '[]';

    const acceptedNetworks = Array.isArray(quote.accepted_networks)
      ? JSON.stringify(quote.accepted_networks)
      : quote.accepted_networks || '["set_chain"]';

    const negotiationHistory = Array.isArray(quote.negotiation_history)
      ? JSON.stringify(quote.negotiation_history)
      : quote.negotiation_history || '[]';

    stmt.run(
      quote.id || randomUUID(),
      quote.status || 'requested',
      quote.buyer_agent_id || null,
      quote.buyer_address,
      quote.seller_agent_id || null,
      quote.seller_address,
      items,
      quote.subtotal || 0,
      quote.fees || 0,
      quote.tax || 0,
      quote.total || 0,
      quote.total_decimal || 0,
      quote.asset || 'USDC',
      acceptedNetworks,
      quote.expires_at,
      quote.terms || null,
      quote.estimated_delivery || null,
      quote.delivery_method || null,
      quote.fulfillment_instructions || null,
      quote.payment_id || null,
      quote.payment_request_id || null,
      quote.request_message || null,
      quote.response_message || null,
      quote.metadata || null,
      quote.created_at || new Date().toISOString(),
      quote.quoted_at || null,
      quote.accepted_at || null,
      quote.fulfilled_at || null,
      quote.updated_at || new Date().toISOString(),
      quote.counter_count || 0,
      negotiationHistory,
      quote.max_rounds || 5,
      quote.escrow_id || null,
    );

    return this.getQuote(quote.id);
  }

  getQuote(id) {
    this.init();
    const row = this.db.prepare('SELECT * FROM a2a_quotes WHERE id = ?').get(id);
    return row ? this._mapQuote(row) : null;
  }

  updateQuote(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_quotes', Object.keys(updates));
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        if (key === 'items' && Array.isArray(value)) {
          fields.push(`${key} = ?`);
          values.push(JSON.stringify(value));
        } else if (key === 'negotiation_history' && Array.isArray(value)) {
          fields.push(`${key} = ?`);
          values.push(JSON.stringify(value));
        } else {
          fields.push(`${key} = ?`);
          values.push(value);
        }
      }
    }

    if (fields.length === 0) return this.getQuote(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db.prepare(`UPDATE a2a_quotes SET ${fields.join(', ')} WHERE id = ?`).run(...values);
    return this.getQuote(id);
  }

  listQuotes(filter = {}) {
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
    if (filter.buyer_agent_id) {
      conditions.push('buyer_agent_id = ?');
      params.push(filter.buyer_agent_id);
    }
    if (filter.seller_agent_id) {
      conditions.push('seller_agent_id = ?');
      params.push(filter.seller_agent_id);
    }
    if (filter.status) {
      conditions.push('status = ?');
      params.push(filter.status);
    }
    if (!filter.include_expired) {
      conditions.push("(status IN ('accepted', 'fulfilled') OR expires_at > datetime('now'))");
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 50;
    const offset = filter.offset || 0;

    const rows = this.db
      .prepare(`SELECT * FROM a2a_quotes ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`)
      .all(...params, limit, offset);

    return rows.map(this._mapQuote);
  }

  _mapQuote(row) {
    return {
      ...row,
      items: JSON.parse(row.items || '[]'),
      accepted_networks: JSON.parse(row.accepted_networks || '["set_chain"]'),
      negotiation_history: JSON.parse(row.negotiation_history || '[]'),
    };
  }
}
