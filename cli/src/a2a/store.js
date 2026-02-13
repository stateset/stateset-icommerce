/**
 * A2A Commerce Store (SQLite-backed)
 *
 * Persistent storage for A2A payments, payment requests, and quotes.
 */

import Database from 'better-sqlite3';
import { randomUUID } from 'node:crypto';
import path from 'node:path';
import os from 'node:os';

const A2A_SCHEMA = `
-- A2A Payments (direct agent-to-agent transfers)
CREATE TABLE IF NOT EXISTS a2a_payments (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL DEFAULT 'pending',
  sender_agent_id TEXT,
  sender_address TEXT NOT NULL,
  recipient_agent_id TEXT,
  recipient_address TEXT NOT NULL,
  amount INTEGER NOT NULL,
  amount_decimal REAL NOT NULL,
  asset TEXT NOT NULL DEFAULT 'USDC',
  network TEXT NOT NULL DEFAULT 'set_chain',
  memo TEXT,
  reference_type TEXT,
  reference_id TEXT,
  idempotency_key TEXT UNIQUE,
  intent_id TEXT,
  tx_hash TEXT,
  block_number INTEGER,
  metadata TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_a2a_payments_sender ON a2a_payments(sender_address);
CREATE INDEX IF NOT EXISTS idx_a2a_payments_recipient ON a2a_payments(recipient_address);
CREATE INDEX IF NOT EXISTS idx_a2a_payments_status ON a2a_payments(status);
CREATE INDEX IF NOT EXISTS idx_a2a_payments_idempotency ON a2a_payments(idempotency_key);
CREATE INDEX IF NOT EXISTS idx_a2a_payments_reference ON a2a_payments(reference_type, reference_id);

-- Payment Requests
CREATE TABLE IF NOT EXISTS a2a_payment_requests (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL DEFAULT 'pending',
  requester_agent_id TEXT,
  requester_address TEXT NOT NULL,
  payer_agent_id TEXT,
  payer_address TEXT,
  amount INTEGER NOT NULL,
  amount_decimal REAL NOT NULL,
  asset TEXT NOT NULL DEFAULT 'USDC',
  accepted_networks TEXT NOT NULL DEFAULT '["set_chain"]',
  description TEXT NOT NULL,
  line_items TEXT,
  reference_type TEXT,
  reference_id TEXT,
  expires_at TEXT NOT NULL,
  allow_partial INTEGER NOT NULL DEFAULT 0,
  minimum_amount INTEGER,
  amount_paid INTEGER NOT NULL DEFAULT 0,
  payment_ids TEXT NOT NULL DEFAULT '[]',
  callback_url TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  paid_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_a2a_requests_requester ON a2a_payment_requests(requester_address);
CREATE INDEX IF NOT EXISTS idx_a2a_requests_payer ON a2a_payment_requests(payer_address);
CREATE INDEX IF NOT EXISTS idx_a2a_requests_status ON a2a_payment_requests(status);
CREATE INDEX IF NOT EXISTS idx_a2a_requests_expires ON a2a_payment_requests(expires_at);

-- Quotes
CREATE TABLE IF NOT EXISTS a2a_quotes (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL DEFAULT 'requested',
  buyer_agent_id TEXT,
  buyer_address TEXT NOT NULL,
  seller_agent_id TEXT,
  seller_address TEXT NOT NULL,
  items TEXT NOT NULL DEFAULT '[]',
  subtotal INTEGER NOT NULL DEFAULT 0,
  fees INTEGER NOT NULL DEFAULT 0,
  tax INTEGER NOT NULL DEFAULT 0,
  total INTEGER NOT NULL DEFAULT 0,
  total_decimal REAL NOT NULL DEFAULT 0,
  asset TEXT NOT NULL DEFAULT 'USDC',
  accepted_networks TEXT NOT NULL DEFAULT '["set_chain"]',
  expires_at TEXT NOT NULL,
  terms TEXT,
  estimated_delivery TEXT,
  delivery_method TEXT,
  fulfillment_instructions TEXT,
  payment_id TEXT,
  payment_request_id TEXT,
  request_message TEXT,
  response_message TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL,
  quoted_at TEXT,
  accepted_at TEXT,
  fulfilled_at TEXT,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_quotes_buyer ON a2a_quotes(buyer_address);
CREATE INDEX IF NOT EXISTS idx_a2a_quotes_seller ON a2a_quotes(seller_address);
CREATE INDEX IF NOT EXISTS idx_a2a_quotes_status ON a2a_quotes(status);
CREATE INDEX IF NOT EXISTS idx_a2a_quotes_expires ON a2a_quotes(expires_at);
`;

export function defaultA2ADbPath() {
  return path.join(os.homedir(), '.stateset', 'a2a.db');
}

/**
 * A2A Store - SQLite storage for A2A commerce
 */
export class A2AStore {
  constructor(options = {}) {
    this.dbPath = options.dbPath || defaultA2ADbPath();
    this.db = null;
  }

  init() {
    if (this.db) return;
    this.db = new Database(this.dbPath);
    this.db.pragma('journal_mode = WAL');
    this.db.exec(A2A_SCHEMA);
  }

  close() {
    if (this.db) {
      this.db.close();
      this.db = null;
    }
  }

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
      payment.completed_at || null
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

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const result = this.db
      .prepare(`SELECT COALESCE(SUM(amount_decimal), 0) as total FROM a2a_payments ${where}`)
      .get(...params);

    return result?.total || 0;
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
      request.paid_at || null
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

    this.db.prepare(`UPDATE a2a_payment_requests SET ${fields.join(', ')} WHERE id = ?`).run(...values);
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
      .prepare(`SELECT * FROM a2a_payment_requests ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`)
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
        created_at, quoted_at, accepted_at, fulfilled_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    const items = Array.isArray(quote.items)
      ? JSON.stringify(quote.items)
      : quote.items || '[]';

    const acceptedNetworks = Array.isArray(quote.accepted_networks)
      ? JSON.stringify(quote.accepted_networks)
      : quote.accepted_networks || '["set_chain"]';

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
      quote.updated_at || new Date().toISOString()
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
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        if (key === 'items' && Array.isArray(value)) {
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
    };
  }
}

export default A2AStore;
