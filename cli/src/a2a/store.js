/**
 * A2A Commerce Store (SQLite-backed)
 *
 * Persistent storage for A2A payments, payment requests, quotes,
 * escrows, disputes, feedback, reputation, and services.
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

-- Escrows
CREATE TABLE IF NOT EXISTS a2a_escrows (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL DEFAULT 'created',
  quote_id TEXT,
  payment_id TEXT,
  buyer_address TEXT NOT NULL,
  seller_address TEXT NOT NULL,
  amount INTEGER NOT NULL,
  amount_decimal REAL NOT NULL,
  asset TEXT NOT NULL DEFAULT 'USDC',
  network TEXT NOT NULL DEFAULT 'set_chain',
  release_conditions TEXT NOT NULL DEFAULT '[]',
  funded_at TEXT,
  released_at TEXT,
  disputed_at TEXT,
  dispute_id TEXT,
  expires_at TEXT NOT NULL,
  auto_release_after TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_escrows_buyer ON a2a_escrows(buyer_address);
CREATE INDEX IF NOT EXISTS idx_a2a_escrows_seller ON a2a_escrows(seller_address);
CREATE INDEX IF NOT EXISTS idx_a2a_escrows_status ON a2a_escrows(status);
CREATE INDEX IF NOT EXISTS idx_a2a_escrows_quote ON a2a_escrows(quote_id);

-- Disputes
CREATE TABLE IF NOT EXISTS a2a_disputes (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL DEFAULT 'filed',
  escrow_id TEXT NOT NULL,
  quote_id TEXT,
  filed_by TEXT NOT NULL,
  filed_against TEXT NOT NULL,
  reason TEXT NOT NULL,
  category TEXT NOT NULL DEFAULT 'non_delivery',
  amount_disputed INTEGER NOT NULL,
  amount_decimal REAL NOT NULL,
  asset TEXT NOT NULL,
  resolution_type TEXT,
  resolution_amount INTEGER,
  resolution_note TEXT,
  resolved_by TEXT,
  evidence_deadline TEXT,
  review_deadline TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  resolved_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_a2a_disputes_escrow ON a2a_disputes(escrow_id);
CREATE INDEX IF NOT EXISTS idx_a2a_disputes_status ON a2a_disputes(status);
CREATE INDEX IF NOT EXISTS idx_a2a_disputes_filed_by ON a2a_disputes(filed_by);

-- Dispute Evidence
CREATE TABLE IF NOT EXISTS a2a_dispute_evidence (
  id TEXT PRIMARY KEY,
  dispute_id TEXT NOT NULL,
  submitted_by TEXT NOT NULL,
  evidence_type TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT,
  content TEXT,
  content_hash TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (dispute_id) REFERENCES a2a_disputes(id)
);

CREATE INDEX IF NOT EXISTS idx_a2a_evidence_dispute ON a2a_dispute_evidence(dispute_id);

-- Feedback
CREATE TABLE IF NOT EXISTS a2a_feedback (
  id TEXT PRIMARY KEY,
  agent_address TEXT NOT NULL,
  reviewer_address TEXT NOT NULL,
  transaction_type TEXT NOT NULL,
  transaction_id TEXT NOT NULL,
  score INTEGER NOT NULL CHECK(score BETWEEN 1 AND 5),
  dimensions TEXT NOT NULL DEFAULT '{}',
  comment TEXT,
  response TEXT,
  response_at TEXT,
  is_revoked INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  UNIQUE(reviewer_address, transaction_type, transaction_id)
);

CREATE INDEX IF NOT EXISTS idx_a2a_feedback_agent ON a2a_feedback(agent_address);
CREATE INDEX IF NOT EXISTS idx_a2a_feedback_reviewer ON a2a_feedback(reviewer_address);

-- Reputation Scores
CREATE TABLE IF NOT EXISTS a2a_reputation_scores (
  agent_address TEXT PRIMARY KEY,
  total_transactions INTEGER NOT NULL DEFAULT 0,
  successful_transactions INTEGER NOT NULL DEFAULT 0,
  disputed_transactions INTEGER NOT NULL DEFAULT 0,
  average_score REAL NOT NULL DEFAULT 0,
  dimension_scores TEXT NOT NULL DEFAULT '{}',
  trust_tier TEXT NOT NULL DEFAULT 'sandbox',
  last_updated TEXT NOT NULL
);

-- Services
CREATE TABLE IF NOT EXISTS a2a_services (
  id TEXT PRIMARY KEY,
  agent_address TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT NOT NULL,
  category TEXT NOT NULL DEFAULT 'other',
  pricing_model TEXT NOT NULL DEFAULT 'quote',
  pricing_details TEXT,
  active INTEGER NOT NULL DEFAULT 1,
  input_schema TEXT,
  output_schema TEXT,
  endpoint_url TEXT,
  avg_response_time INTEGER,
  success_rate REAL,
  transaction_count INTEGER NOT NULL DEFAULT 0,
  metadata TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_services_agent ON a2a_services(agent_address);
CREATE INDEX IF NOT EXISTS idx_a2a_services_category ON a2a_services(category);
CREATE INDEX IF NOT EXISTS idx_a2a_services_active ON a2a_services(active);

-- Notification Log
CREATE TABLE IF NOT EXISTS a2a_notification_log (
  id TEXT PRIMARY KEY,
  recipient_address TEXT NOT NULL,
  endpoint_url TEXT NOT NULL,
  event_type TEXT NOT NULL,
  payload TEXT NOT NULL,
  signature TEXT,
  status TEXT NOT NULL DEFAULT 'pending',
  attempts INTEGER NOT NULL DEFAULT 0,
  last_attempt_at TEXT,
  last_error TEXT,
  delivered_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_notif_recipient ON a2a_notification_log(recipient_address);
CREATE INDEX IF NOT EXISTS idx_a2a_notif_status ON a2a_notification_log(status);
CREATE INDEX IF NOT EXISTS idx_a2a_notif_event ON a2a_notification_log(event_type);

-- Webhook Configuration
CREATE TABLE IF NOT EXISTS a2a_webhook_config (
  agent_address TEXT PRIMARY KEY,
  endpoint_url TEXT NOT NULL,
  secret TEXT,
  enabled_events TEXT NOT NULL DEFAULT '["*"]',
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- A2A Subscriptions (recurring agent-to-agent payments)
CREATE TABLE IF NOT EXISTS a2a_subscriptions (
  id TEXT PRIMARY KEY,
  subscriber_address TEXT NOT NULL,
  provider_address TEXT NOT NULL,
  service_id TEXT,
  plan_name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  amount INTEGER NOT NULL,
  amount_decimal REAL NOT NULL,
  asset TEXT NOT NULL DEFAULT 'USDC',
  network TEXT NOT NULL DEFAULT 'set_chain',
  billing_interval TEXT NOT NULL DEFAULT 'monthly',
  trial_end_date TEXT,
  current_period_start TEXT NOT NULL,
  current_period_end TEXT NOT NULL,
  next_billing_date TEXT NOT NULL,
  cancel_at_period_end INTEGER NOT NULL DEFAULT 0,
  cancelled_at TEXT,
  past_due_since TEXT,
  max_past_due_cycles INTEGER NOT NULL DEFAULT 3,
  total_billed INTEGER NOT NULL DEFAULT 0,
  total_billed_decimal REAL NOT NULL DEFAULT 0,
  billing_count INTEGER NOT NULL DEFAULT 0,
  last_payment_id TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_subs_subscriber ON a2a_subscriptions(subscriber_address);
CREATE INDEX IF NOT EXISTS idx_a2a_subs_provider ON a2a_subscriptions(provider_address);
CREATE INDEX IF NOT EXISTS idx_a2a_subs_status ON a2a_subscriptions(status);
CREATE INDEX IF NOT EXISTS idx_a2a_subs_next_billing ON a2a_subscriptions(next_billing_date);
CREATE INDEX IF NOT EXISTS idx_a2a_subs_service ON a2a_subscriptions(service_id);

-- Split Payments (multi-party payment splitting)
CREATE TABLE IF NOT EXISTS a2a_split_payments (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL DEFAULT 'pending',
  sender_address TEXT NOT NULL,
  total_amount INTEGER NOT NULL,
  total_amount_decimal REAL NOT NULL,
  asset TEXT NOT NULL DEFAULT 'USDC',
  network TEXT NOT NULL DEFAULT 'set_chain',
  split_type TEXT NOT NULL DEFAULT 'percentage',
  platform_fee_percent REAL,
  platform_fee_amount INTEGER,
  platform_fee_address TEXT,
  memo TEXT,
  reference_type TEXT,
  reference_id TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_a2a_splits_sender ON a2a_split_payments(sender_address);
CREATE INDEX IF NOT EXISTS idx_a2a_splits_status ON a2a_split_payments(status);

-- Split Recipients (individual shares in a split payment)
CREATE TABLE IF NOT EXISTS a2a_split_recipients (
  id TEXT PRIMARY KEY,
  split_payment_id TEXT NOT NULL,
  recipient_address TEXT NOT NULL,
  share_percent REAL,
  share_amount INTEGER,
  share_amount_decimal REAL,
  payment_id TEXT,
  status TEXT NOT NULL DEFAULT 'pending',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (split_payment_id) REFERENCES a2a_split_payments(id)
);

CREATE INDEX IF NOT EXISTS idx_a2a_split_recip_parent ON a2a_split_recipients(split_payment_id);
CREATE INDEX IF NOT EXISTS idx_a2a_split_recip_addr ON a2a_split_recipients(recipient_address);

-- Event Subscriptions (agents subscribing to real-time events)
CREATE TABLE IF NOT EXISTS a2a_event_subscriptions (
  id TEXT PRIMARY KEY,
  agent_address TEXT NOT NULL,
  event_types TEXT NOT NULL DEFAULT '["*"]',
  active INTEGER NOT NULL DEFAULT 1,
  last_event_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_evt_sub_agent ON a2a_event_subscriptions(agent_address);

-- Event Log (persistent event history)
CREATE TABLE IF NOT EXISTS a2a_event_log (
  id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL,
  agent_address TEXT NOT NULL,
  payload TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_evt_log_agent ON a2a_event_log(agent_address);
CREATE INDEX IF NOT EXISTS idx_a2a_evt_log_type ON a2a_event_log(event_type);
CREATE INDEX IF NOT EXISTS idx_a2a_evt_log_created ON a2a_event_log(created_at);
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
    this._migrateQuotes();
    this._migrateEscrows();
  }

  close() {
    if (this.db) {
      this.db.close();
      this.db = null;
    }
  }

  /**
   * Add negotiation columns to the quotes table.
   * Uses ALTER TABLE wrapped in try/catch since ALTER IF NOT EXISTS
   * is not supported — columns may already exist on subsequent runs.
   */
  _migrateQuotes() {
    const columns = [
      ['counter_count', 'INTEGER DEFAULT 0'],
      ['negotiation_history', "TEXT DEFAULT '[]'"],
      ['max_rounds', 'INTEGER DEFAULT 5'],
      ['escrow_id', 'TEXT'],
    ];

    for (const [name, type] of columns) {
      try {
        this.db.exec(`ALTER TABLE a2a_quotes ADD COLUMN ${name} ${type}`);
      } catch {
        // Column already exists — ignore
      }
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
  // Escrow Migration (Phase B — add intent_id column)
  // ===========================================================================

  _migrateEscrows() {
    const columns = [['intent_id', 'TEXT']];
    for (const [name, type] of columns) {
      try {
        this.db.exec(`ALTER TABLE a2a_escrows ADD COLUMN ${name} ${type}`);
      } catch {
        // Column already exists — ignore
      }
    }
  }

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
        agent_address, endpoint_url, secret, enabled_events, active, created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(agent_address) DO UPDATE SET
        endpoint_url = excluded.endpoint_url,
        secret = excluded.secret,
        enabled_events = excluded.enabled_events,
        active = excluded.active,
        updated_at = excluded.updated_at`,
      )
      .run(
        config.agent_address,
        config.endpoint_url,
        config.secret || null,
        enabledEvents,
        config.active !== undefined ? (config.active ? 1 : 0) : 1,
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

  // ===========================================================================
  // A2A Subscriptions
  // ===========================================================================

  createSubscription(sub) {
    this.init();
    const id = sub.id || randomUUID();
    const now = new Date().toISOString();

    this.db
      .prepare(
        `INSERT INTO a2a_subscriptions (
        id, subscriber_address, provider_address, service_id, plan_name,
        status, amount, amount_decimal, asset, network, billing_interval,
        trial_end_date, current_period_start, current_period_end,
        next_billing_date, cancel_at_period_end, cancelled_at, past_due_since,
        max_past_due_cycles, total_billed, total_billed_decimal, billing_count,
        last_payment_id, metadata, created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        sub.subscriber_address,
        sub.provider_address,
        sub.service_id || null,
        sub.plan_name,
        sub.status || 'active',
        sub.amount,
        sub.amount_decimal,
        sub.asset || 'USDC',
        sub.network || 'set_chain',
        sub.billing_interval || 'monthly',
        sub.trial_end_date || null,
        sub.current_period_start || now,
        sub.current_period_end,
        sub.next_billing_date,
        sub.cancel_at_period_end ? 1 : 0,
        sub.cancelled_at || null,
        sub.past_due_since || null,
        sub.max_past_due_cycles ?? 3,
        sub.total_billed || 0,
        sub.total_billed_decimal || 0,
        sub.billing_count || 0,
        sub.last_payment_id || null,
        sub.metadata || null,
        sub.created_at || now,
        sub.updated_at || now,
      );

    return this.getSubscription(id);
  }

  getSubscription(id) {
    this.init();
    const row = this.db.prepare('SELECT * FROM a2a_subscriptions WHERE id = ?').get(id);
    return row ? this._mapSubscription(row) : null;
  }

  updateSubscription(id, updates) {
    this.init();
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        if (key === 'cancel_at_period_end') {
          fields.push(`${key} = ?`);
          values.push(value ? 1 : 0);
        } else {
          fields.push(`${key} = ?`);
          values.push(value);
        }
      }
    }

    if (fields.length === 0) return this.getSubscription(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db
      .prepare(`UPDATE a2a_subscriptions SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getSubscription(id);
  }

  listSubscriptions(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.subscriber_address) {
      conditions.push('subscriber_address = ?');
      params.push(filter.subscriber_address);
    }
    if (filter.provider_address) {
      conditions.push('provider_address = ?');
      params.push(filter.provider_address);
    }
    if (filter.status) {
      conditions.push('status = ?');
      params.push(filter.status);
    }
    if (filter.service_id) {
      conditions.push('service_id = ?');
      params.push(filter.service_id);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 50;
    const offset = filter.offset || 0;

    const rows = this.db
      .prepare(`SELECT * FROM a2a_subscriptions ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`)
      .all(...params, limit, offset);

    return rows.map(this._mapSubscription);
  }

  getDueSubscriptions(now, limit = 50) {
    this.init();
    return this.db
      .prepare(
        `SELECT * FROM a2a_subscriptions
       WHERE status = 'active' AND next_billing_date <= ?
       ORDER BY next_billing_date ASC LIMIT ?`,
      )
      .all(now, limit)
      .map(this._mapSubscription);
  }

  getExpiredTrials(now) {
    this.init();
    return this.db
      .prepare(
        `SELECT * FROM a2a_subscriptions
       WHERE status = 'trial' AND trial_end_date IS NOT NULL AND trial_end_date <= ?`,
      )
      .all(now)
      .map(this._mapSubscription);
  }

  _mapSubscription(row) {
    return {
      ...row,
      cancel_at_period_end: Boolean(row.cancel_at_period_end),
    };
  }

  // ===========================================================================
  // Split Payments
  // ===========================================================================

  createSplitPayment(split) {
    this.init();
    const id = split.id || randomUUID();
    const now = new Date().toISOString();

    this.db
      .prepare(
        `INSERT INTO a2a_split_payments (
        id, status, sender_address, total_amount, total_amount_decimal,
        asset, network, split_type, platform_fee_percent, platform_fee_amount,
        platform_fee_address, memo, reference_type, reference_id,
        metadata, created_at, updated_at, completed_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        split.status || 'pending',
        split.sender_address,
        split.total_amount,
        split.total_amount_decimal,
        split.asset || 'USDC',
        split.network || 'set_chain',
        split.split_type || 'percentage',
        split.platform_fee_percent ?? null,
        split.platform_fee_amount ?? null,
        split.platform_fee_address || null,
        split.memo || null,
        split.reference_type || null,
        split.reference_id || null,
        split.metadata || null,
        split.created_at || now,
        split.updated_at || now,
        split.completed_at || null,
      );

    return this.getSplitPayment(id);
  }

  getSplitPayment(id) {
    this.init();
    const row = this.db.prepare('SELECT * FROM a2a_split_payments WHERE id = ?').get(id);
    if (!row) return null;
    const recipients = this.listSplitRecipients(id);
    return { ...row, recipients };
  }

  updateSplitPayment(id, updates) {
    this.init();
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(value);
      }
    }

    if (fields.length === 0) return this.getSplitPayment(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db
      .prepare(`UPDATE a2a_split_payments SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getSplitPayment(id);
  }

  listSplitPayments(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.sender_address) {
      conditions.push('sender_address = ?');
      params.push(filter.sender_address);
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
        `SELECT * FROM a2a_split_payments ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`,
      )
      .all(...params, limit, offset);
  }

  // ===========================================================================
  // Split Recipients
  // ===========================================================================

  createSplitRecipient(recipient) {
    this.init();
    const id = recipient.id || randomUUID();
    const now = new Date().toISOString();

    this.db
      .prepare(
        `INSERT INTO a2a_split_recipients (
        id, split_payment_id, recipient_address, share_percent,
        share_amount, share_amount_decimal, payment_id, status,
        created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        recipient.split_payment_id,
        recipient.recipient_address,
        recipient.share_percent ?? null,
        recipient.share_amount ?? null,
        recipient.share_amount_decimal ?? null,
        recipient.payment_id || null,
        recipient.status || 'pending',
        recipient.created_at || now,
        recipient.updated_at || now,
      );

    return this.getSplitRecipient(id);
  }

  getSplitRecipient(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_split_recipients WHERE id = ?').get(id) || null;
  }

  updateSplitRecipient(id, updates) {
    this.init();
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(value);
      }
    }

    if (fields.length === 0) return this.getSplitRecipient(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db
      .prepare(`UPDATE a2a_split_recipients SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getSplitRecipient(id);
  }

  listSplitRecipients(splitPaymentId) {
    this.init();
    return this.db
      .prepare(
        'SELECT * FROM a2a_split_recipients WHERE split_payment_id = ? ORDER BY created_at ASC',
      )
      .all(splitPaymentId);
  }

  // ===========================================================================
  // Event Subscriptions
  // ===========================================================================

  createEventSubscription(sub) {
    this.init();
    const id = sub.id || randomUUID();
    const now = new Date().toISOString();
    const eventTypes = Array.isArray(sub.event_types)
      ? JSON.stringify(sub.event_types)
      : sub.event_types || '["*"]';

    this.db
      .prepare(
        `INSERT INTO a2a_event_subscriptions (
        id, agent_address, event_types, active, last_event_id, created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        sub.agent_address,
        eventTypes,
        sub.active !== undefined ? (sub.active ? 1 : 0) : 1,
        sub.last_event_id || null,
        sub.created_at || now,
        sub.updated_at || now,
      );

    return this.getEventSubscription(id);
  }

  getEventSubscription(id) {
    this.init();
    const row = this.db.prepare('SELECT * FROM a2a_event_subscriptions WHERE id = ?').get(id);
    return row ? this._mapEventSubscription(row) : null;
  }

  updateEventSubscription(id, updates) {
    this.init();
    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (value !== undefined) {
        if (key === 'event_types' && Array.isArray(value)) {
          fields.push(`${key} = ?`);
          values.push(JSON.stringify(value));
        } else if (key === 'active') {
          fields.push(`${key} = ?`);
          values.push(value ? 1 : 0);
        } else {
          fields.push(`${key} = ?`);
          values.push(value);
        }
      }
    }

    if (fields.length === 0) return this.getEventSubscription(id);

    fields.push('updated_at = ?');
    values.push(new Date().toISOString());
    values.push(id);

    this.db
      .prepare(`UPDATE a2a_event_subscriptions SET ${fields.join(', ')} WHERE id = ?`)
      .run(...values);
    return this.getEventSubscription(id);
  }

  listEventSubscriptions(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.agent_address) {
      conditions.push('agent_address = ?');
      params.push(filter.agent_address);
    }
    if (filter.active !== undefined) {
      conditions.push('active = ?');
      params.push(filter.active ? 1 : 0);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    return this.db
      .prepare(`SELECT * FROM a2a_event_subscriptions ${where} ORDER BY created_at DESC`)
      .all(...params)
      .map(this._mapEventSubscription);
  }

  _mapEventSubscription(row) {
    return {
      ...row,
      event_types: JSON.parse(row.event_types || '["*"]'),
      active: Boolean(row.active),
    };
  }

  // ===========================================================================
  // Event Log
  // ===========================================================================

  createEventLog(event) {
    this.init();
    const id = event.id || randomUUID();
    const now = new Date().toISOString();

    this.db
      .prepare(
        `INSERT INTO a2a_event_log (id, event_type, agent_address, payload, created_at)
       VALUES (?, ?, ?, ?, ?)`,
      )
      .run(
        id,
        event.event_type,
        event.agent_address,
        typeof event.payload === 'object' ? JSON.stringify(event.payload) : event.payload,
        event.created_at || now,
      );

    return this.getEventLog(id);
  }

  getEventLog(id) {
    this.init();
    return this.db.prepare('SELECT * FROM a2a_event_log WHERE id = ?').get(id) || null;
  }

  listEventLog(filter = {}) {
    this.init();
    const conditions = [];
    const params = [];

    if (filter.agent_address) {
      conditions.push('agent_address = ?');
      params.push(filter.agent_address);
    }
    if (filter.event_type) {
      conditions.push('event_type = ?');
      params.push(filter.event_type);
    }
    if (filter.since) {
      conditions.push('created_at > ?');
      params.push(filter.since);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const limit = filter.limit || 100;
    const offset = filter.offset || 0;

    return this.db
      .prepare(`SELECT * FROM a2a_event_log ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`)
      .all(...params, limit, offset);
  }
}

export default A2AStore;
