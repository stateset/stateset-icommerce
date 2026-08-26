/**
 * A2A Store — SQLite DDL and idempotent column migrations.
 *
 * Extracted verbatim from `cli/src/a2a/store.js` (Aug 2026 decomposition).
 */

/**
 * Full A2A DDL, executed on every `init()` (all statements are `IF NOT EXISTS`).
 * Table order is load-bearing for migrations — do not reorder.
 * @type {string}
 */
export const A2A_SCHEMA = `
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
CREATE TABLE IF NOT EXISTS a2a_market_quotes (
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

CREATE INDEX IF NOT EXISTS idx_a2a_market_quotes_buyer ON a2a_market_quotes(buyer_address);
CREATE INDEX IF NOT EXISTS idx_a2a_market_quotes_seller ON a2a_market_quotes(seller_address);
CREATE INDEX IF NOT EXISTS idx_a2a_market_quotes_status ON a2a_market_quotes(status);
CREATE INDEX IF NOT EXISTS idx_a2a_market_quotes_expires ON a2a_market_quotes(expires_at);

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
  tenant_id TEXT NOT NULL DEFAULT 'legacy',
  store_id TEXT NOT NULL DEFAULT 'legacy',
  status TEXT NOT NULL DEFAULT 'filed',
  escrow_id TEXT NOT NULL,
  quote_id TEXT,
  claimant_address TEXT NOT NULL,
  respondent_address TEXT NOT NULL,
  reason TEXT NOT NULL,
  category TEXT NOT NULL DEFAULT 'non_delivery',
  amount_decimal TEXT NOT NULL,
  asset TEXT NOT NULL,
  resolution_type TEXT,
  buyer_amount_decimal TEXT,
  seller_amount_decimal TEXT,
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
CREATE INDEX IF NOT EXISTS idx_a2a_disputes_claimant ON a2a_disputes(claimant_address);

-- Dispute Evidence
CREATE TABLE IF NOT EXISTS a2a_dispute_evidence (
  id TEXT PRIMARY KEY,
  tenant_id TEXT NOT NULL DEFAULT 'legacy',
  store_id TEXT NOT NULL DEFAULT 'legacy',
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

-- Webhook Dead Letter Queue — quarantined permanently failed notifications
CREATE TABLE IF NOT EXISTS a2a_webhook_dlq (
  id TEXT PRIMARY KEY,
  original_notification_id TEXT NOT NULL,
  recipient_address TEXT NOT NULL,
  endpoint_url TEXT NOT NULL,
  event_type TEXT NOT NULL,
  payload TEXT NOT NULL,
  signature TEXT,
  attempts INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,
  last_attempt_at TEXT,
  original_created_at TEXT NOT NULL,
  quarantined_at TEXT NOT NULL,
  replayed_at TEXT,
  replay_status TEXT
);

CREATE INDEX IF NOT EXISTS idx_a2a_dlq_recipient ON a2a_webhook_dlq(recipient_address);
CREATE INDEX IF NOT EXISTS idx_a2a_dlq_event ON a2a_webhook_dlq(event_type);
CREATE INDEX IF NOT EXISTS idx_a2a_dlq_quarantined ON a2a_webhook_dlq(quarantined_at);

-- Webhook Configuration
CREATE TABLE IF NOT EXISTS a2a_webhook_config (
  agent_address TEXT PRIMARY KEY,
  endpoint_url TEXT NOT NULL,
  secret TEXT,
  enabled_events TEXT NOT NULL DEFAULT '["*"]',
  active INTEGER NOT NULL DEFAULT 1,
  client_cert TEXT,
  client_key TEXT,
  ca_cert TEXT,
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

-- Agent Cards (A2A agent identity & capability registration)
CREATE TABLE IF NOT EXISTS a2a_runtime_agent_cards (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  wallet_address TEXT UNIQUE NOT NULL,
  public_key TEXT,
  supported_networks TEXT DEFAULT '["set_chain"]',
  supported_assets TEXT DEFAULT '["USDC"]',
  a2a_skills TEXT DEFAULT '["buy","sell","quote"]',
  payment_addresses TEXT,
  endpoint_url TEXT,
  description TEXT,
  trust_level TEXT DEFAULT 'sandbox',
  active INTEGER DEFAULT 1,
  suspended_at TEXT,
  created_at TEXT,
  updated_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_a2a_runtime_cards_wallet ON a2a_runtime_agent_cards(wallet_address);
CREATE INDEX IF NOT EXISTS idx_a2a_runtime_cards_active ON a2a_runtime_agent_cards(active);
CREATE INDEX IF NOT EXISTS idx_a2a_runtime_cards_trust ON a2a_runtime_agent_cards(trust_level);

-- RFQ Broadcasts
CREATE TABLE IF NOT EXISTS a2a_rfqs (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL DEFAULT 'open',
  buyer_address TEXT NOT NULL,
  buyer_agent_id TEXT,
  items TEXT NOT NULL DEFAULT '[]',
  seller_filter TEXT,
  max_responses INTEGER NOT NULL DEFAULT 10,
  deadline TEXT NOT NULL,
  scoring_criteria TEXT NOT NULL DEFAULT 'cheapest',
  winning_quote_id TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  awarded_at TEXT,
  closed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_a2a_rfqs_buyer ON a2a_rfqs(buyer_address);
CREATE INDEX IF NOT EXISTS idx_a2a_rfqs_status ON a2a_rfqs(status);
CREATE INDEX IF NOT EXISTS idx_a2a_rfqs_deadline ON a2a_rfqs(deadline);

-- RFQ Responses
CREATE TABLE IF NOT EXISTS a2a_rfq_responses (
  id TEXT PRIMARY KEY,
  rfq_id TEXT NOT NULL,
  seller_address TEXT NOT NULL,
  quote_id TEXT NOT NULL,
  score REAL,
  rank INTEGER,
  status TEXT NOT NULL DEFAULT 'pending',
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_rfq_responses_rfq ON a2a_rfq_responses(rfq_id);
CREATE INDEX IF NOT EXISTS idx_a2a_rfq_responses_seller ON a2a_rfq_responses(seller_address);

-- SLA Definitions
CREATE TABLE IF NOT EXISTS a2a_sla_definitions (
  id TEXT PRIMARY KEY,
  service_id TEXT NOT NULL,
  response_time_ms INTEGER,
  uptime_percent REAL,
  quality_min_score REAL,
  throughput_rps INTEGER,
  penalty_percent REAL NOT NULL DEFAULT 5.0,
  penalty_type TEXT NOT NULL DEFAULT 'credit',
  active INTEGER NOT NULL DEFAULT 1,
  metadata TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_sla_defs_service ON a2a_sla_definitions(service_id);
CREATE INDEX IF NOT EXISTS idx_a2a_sla_defs_active ON a2a_sla_definitions(active);

-- SLA Violations
CREATE TABLE IF NOT EXISTS a2a_sla_violations (
  id TEXT PRIMARY KEY,
  sla_id TEXT NOT NULL,
  service_id TEXT NOT NULL,
  violation_type TEXT NOT NULL,
  expected_value REAL NOT NULL,
  actual_value REAL NOT NULL,
  severity TEXT NOT NULL DEFAULT 'warning',
  penalty_amount REAL,
  resolved INTEGER NOT NULL DEFAULT 0,
  metadata TEXT,
  created_at TEXT NOT NULL,
  resolved_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_a2a_sla_violations_sla ON a2a_sla_violations(sla_id);
CREATE INDEX IF NOT EXISTS idx_a2a_sla_violations_service ON a2a_sla_violations(service_id);
CREATE INDEX IF NOT EXISTS idx_a2a_sla_violations_resolved ON a2a_sla_violations(resolved);

-- Workflows
CREATE TABLE IF NOT EXISTS a2a_workflows (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  definition TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  total_cost REAL NOT NULL DEFAULT 0,
  current_step TEXT,
  error TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_a2a_workflows_status ON a2a_workflows(status);

-- Workflow Steps
CREATE TABLE IF NOT EXISTS a2a_workflow_steps (
  id TEXT PRIMARY KEY,
  workflow_id TEXT NOT NULL,
  step_name TEXT NOT NULL,
  step_type TEXT NOT NULL DEFAULT 'quote_request',
  agent_address TEXT,
  params TEXT,
  depends_on TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL DEFAULT 'pending',
  result TEXT,
  cost REAL NOT NULL DEFAULT 0,
  error TEXT,
  started_at TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_a2a_wf_steps_workflow ON a2a_workflow_steps(workflow_id);
CREATE INDEX IF NOT EXISTS idx_a2a_wf_steps_status ON a2a_workflow_steps(status);
`;

/**
 * A2A Store — idempotent ALTER TABLE migrations run after the DDL.
 *
 * Methods are copied verbatim onto `A2AStore.prototype` by `applyStoreMixins()`
 * in `../store.js`; this class is never instantiated on its own.
 * Every method runs with `this` bound to the owning A2AStore (`this.db` is the
 * open better-sqlite3 handle).
 *
 * @this {import('../store.js').A2AStore}
 */
export class A2ASchemaMigrations {
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
        this.db.exec(`ALTER TABLE a2a_market_quotes ADD COLUMN ${name} ${type}`);
      } catch {
        // Column already exists — expected during idempotent migration
      }
    }
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
        // Column already exists — expected during idempotent migration
      }
    }
  }

  _migrateAgentCards() {
    const columns = [['payment_addresses', 'TEXT']];
    for (const [name, type] of columns) {
      try {
        this.db.exec(`ALTER TABLE a2a_runtime_agent_cards ADD COLUMN ${name} ${type}`);
      } catch {
        // Column already exists — expected during idempotent migration
      }
    }
  }
}
