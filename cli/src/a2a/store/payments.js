/**
 * A2A Store — payments and payment requests.
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

import { randomUUID } from 'node:crypto';

/**
 * A2A Store — payments and payment requests.
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2APaymentsMethods {
  // ===========================================================================
  // Payments
  // ===========================================================================

  createPayment(payment) {
    this.init();
    const stmt = this.db.prepare(`
      INSERT INTO a2a_payments (
        id, status, sender_agent_id, sender_address, recipient_agent_id, recipient_address,
        amount, amount_decimal, asset, network, memo, reference_type, reference_id,
        idempotency_key, intent_id, tx_hash, block_number, metadata, created_at, updated_at, completed_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    stmt.run(
      payment.id || randomUUID(),
      payment.status || 'pending',
      payment.sender_agent_id || null,
      payment.sender_address,
      payment.recipient_agent_id || null,
      payment.recipient_address,
      payment.amount,
      payment.amount_decimal,
      payment.asset || 'USDC',
      payment.network || 'set_chain',
      payment.memo || null,
      payment.reference_type || null,
      payment.reference_id || null,
      payment.idempotency_key || null,
      payment.intent_id || null,
      payment.tx_hash || null,
      payment.block_number || null,
      payment.metadata || null,
      payment.created_at || new Date().toISOString(),
      payment.updated_at || new Date().toISOString(),
      payment.completed_at || null,
    );

    return this.getPayment(payment.id);
  }

  getPayment(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_payments WHERE id = ?').get(id);
  }

  getPaymentByIdempotencyKey(key) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_payments WHERE idempotency_key = ?').get(key);
  }

  updatePayment(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_payments', Object.keys(updates));
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(value);
      }
    }

    if (fields.length === 0) return this.getPayment(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db.prepare(`UPDATE a2a_payments SET ${fields.join(', ')} WHERE id = ?`).run(...values);
    return this.getPayment(id);
  }

  listPayments(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.sender_address) {
      conditions.push('sender_address = ?');
      params.push(filter.sender_address);
    }
    if (filter.recipient_address) {
      conditions.push('recipient_address = ?');
      params.push(filter.recipient_address);
    }
    if (filter.sender_agent_id) {
      conditions.push('sender_agent_id = ?');
      params.push(filter.sender_agent_id);
    }
    if (filter.recipient_agent_id) {
      conditions.push('recipient_agent_id = ?');
      params.push(filter.recipient_agent_id);
    }
    if (filter.status) {
      conditions.push('status = ?');
      params.push(filter.status);
    }
    if (filter.asset) {
      conditions.push('asset = ?');
      params.push(filter.asset);
    }
    if (filter.network) {
      conditions.push('network = ?');
      params.push(filter.network);
    }
    if (filter.reference_type) {
      conditions.push('reference_type = ?');
      params.push(filter.reference_type);
    }
    if (filter.reference_id) {
      conditions.push('reference_id = ?');
      params.push(filter.reference_id);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 50;
    const offset = filter.offset || 0;

    return this.db
      .prepare(`SELECT * FROM a2a_payments ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`)
      .all(...params, limit, offset);
  }

  sumPayments(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.sender_address) {
      conditions.push('sender_address = ?');
      params.push(filter.sender_address);
    }
    if (filter.recipient_address) {
      conditions.push('recipient_address = ?');
      params.push(filter.recipient_address);
    }
    if (filter.status) {
      conditions.push('status = ?');
      params.push(filter.status);
    }
    if (filter.asset) {
      conditions.push('asset = ?');
      params.push(filter.asset);
    }
    if (filter.network) {
      conditions.push('network = ?');
      params.push(filter.network);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const result = this.db
      .prepare(`SELECT COALESCE(SUM(amount_decimal), 0) as total FROM a2a_payments ${where}`)
      .get(...params);

    return result?.total || 0;
  }

  summarizePayments(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.sender_address) {
      conditions.push('sender_address = ?');
      params.push(filter.sender_address);
    }
    if (filter.recipient_address) {
      conditions.push('recipient_address = ?');
      params.push(filter.recipient_address);
    }
    if (filter.sender_agent_id) {
      conditions.push('sender_agent_id = ?');
      params.push(filter.sender_agent_id);
    }
    if (filter.recipient_agent_id) {
      conditions.push('recipient_agent_id = ?');
      params.push(filter.recipient_agent_id);
    }
    if (filter.status) {
      conditions.push('status = ?');
      params.push(filter.status);
    }
    if (filter.asset) {
      conditions.push('asset = ?');
      params.push(filter.asset);
    }
    if (filter.network) {
      conditions.push('network = ?');
      params.push(filter.network);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    return this.db
      .prepare(
        `
        SELECT
          asset,
          network,
          COUNT(*) AS payment_count,
          COALESCE(SUM(amount_decimal), 0) AS total_amount
        FROM a2a_payments
        ${where}
        GROUP BY asset, network
        ORDER BY asset ASC, network ASC
      `,
      )
      .all(...params);
  }

  // ===========================================================================
  // Payment Requests
  // ===========================================================================

  createPaymentRequest(request) {
    this.init();
    const stmt = this.db.prepare(`
      INSERT INTO a2a_payment_requests (
        id, status, requester_agent_id, requester_address, payer_agent_id, payer_address,
        amount, amount_decimal, asset, accepted_networks, description, line_items,
        reference_type, reference_id, expires_at, allow_partial, minimum_amount,
        amount_paid, payment_ids, callback_url, metadata, created_at, updated_at, paid_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    const acceptedNetworks = Array.isArray(request.accepted_networks)
      ? JSON.stringify(request.accepted_networks)
      : request.accepted_networks || '["set_chain"]';

    const paymentIds = Array.isArray(request.payment_ids)
      ? JSON.stringify(request.payment_ids)
      : request.payment_ids || '[]';

    stmt.run(
      request.id || randomUUID(),
      request.status || 'pending',
      request.requester_agent_id || null,
      request.requester_address,
      request.payer_agent_id || null,
      request.payer_address || null,
      request.amount,
      request.amount_decimal,
      request.asset || 'USDC',
      acceptedNetworks,
      request.description,
      request.line_items || null,
      request.reference_type || null,
      request.reference_id || null,
      request.expires_at,
      request.allow_partial ? 1 : 0,
      request.minimum_amount || null,
      request.amount_paid || 0,
      paymentIds,
      request.callback_url || null,
      request.metadata || null,
      request.created_at || new Date().toISOString(),
      request.updated_at || new Date().toISOString(),
      request.paid_at || null,
    );

    return this.getPaymentRequest(request.id);
  }

  getPaymentRequest(id) {
    this.init();
    const row = this.db.prepare('SELECT * FROM a2a_payment_requests WHERE id = ?').get(id);
    return row ? this._mapPaymentRequest(row) : null;
  }

  updatePaymentRequest(id, updates) {
    this.init();
    this._validateUpdateKeys('a2a_payment_requests', Object.keys(updates));
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        if (key === 'payment_ids' && Array.isArray(value)) {
          fields.push(`${key} = ?`);
          values.push(JSON.stringify(value));
        } else if (key === 'allow_partial') {
          fields.push(`${key} = ?`);
          values.push(value ? 1 : 0);
        } else {
          fields.push(`${key} = ?`);
          values.push(value);
        }
      }
    }

    if (fields.length === 0) return this.getPaymentRequest(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db
      .prepare(`UPDATE a2a_payment_requests SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getPaymentRequest(id);
  }

  listPaymentRequests(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.requester_address) {
      conditions.push('requester_address = ?');
      params.push(filter.requester_address);
    }
    if (filter.payer_address) {
      conditions.push('payer_address = ?');
      params.push(filter.payer_address);
    }
    if (filter.status) {
      conditions.push('status = ?');
      params.push(filter.status);
    }
    if (!filter.include_expired) {
      conditions.push("(status = 'paid' OR expires_at > datetime('now'))");
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 50;
    const offset = filter.offset || 0;

    const rows = this.db
      .prepare(
        `SELECT * FROM a2a_payment_requests ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`,
      )
      .all(...params, limit, offset);

    return rows.map(this._mapPaymentRequest);
  }

  _mapPaymentRequest(row) {
    return {
      ...row,
      allow_partial: Boolean(row.allow_partial),
      accepted_networks: JSON.parse(row.accepted_networks || '["set_chain"]'),
      payment_ids: JSON.parse(row.payment_ids || '[]'),
    };
  }
}
