# Database Schema

iCommerce uses SQLite as the default storage backend. The schema covers core commerce entities and A2A protocol state.

## Core Commerce Tables

### Entity Relationship Overview

```
customers ─────────────┬──── orders ──────── order_items
                       │       │
                       │       ├──── payments
                       │       ├──── shipments
                       │       └──── returns ──── return_items
                       │
                       ├──── carts ──────── cart_items
                       ├──── subscriptions
                       └──── invoices

products ──── product_variants ──── inventory_items ──── inventory_reservations

suppliers ──── purchase_orders ──── purchase_order_items

bom ──── bom_items ──── work_orders
```

### Key Tables

| Table | Primary Key | Key Columns |
|-------|-------------|-------------|
| `customers` | `id` (UUID) | email, first_name, last_name, phone, created_at |
| `products` | `id` (UUID) | name, sku, price, description, category |
| `orders` | `id` (UUID) | customer_id, status, total, currency, created_at |
| `order_items` | `id` (UUID) | order_id, sku, name, quantity, unit_price |
| `inventory_items` | `id` (UUID) | sku, name, quantity, reserved, reorder_point |
| `inventory_reservations` | `id` (UUID) | sku, quantity, order_id, expires_at |
| `payments` | `id` (UUID) | order_id, amount, currency, status, method |
| `returns` | `id` (UUID) | order_id, status, reason, amount |
| `carts` | `id` (UUID) | customer_id, status, currency |
| `cart_items` | `id` (UUID) | cart_id, sku, name, quantity, unit_price |
| `subscriptions` | `id` (UUID) | customer_id, plan_id, status, interval |
| `subscription_plans` | `id` (UUID) | code, name, price, interval, trial_days |
| `invoices` | `id` (UUID) | customer_id, status, amount, due_date, terms |
| `shipments` | `id` (UUID) | order_id, carrier, tracking_number, status |
| `suppliers` | `id` (UUID) | name, email, lead_time_days |
| `purchase_orders` | `id` (UUID) | supplier_id, status, total, expected_delivery |
| `promotions` | `id` (UUID) | code, name, discount_type, discount_value, active |

### Indexes

Performance indexes are created automatically (migration 025):

```sql
CREATE INDEX idx_orders_customer ON orders(customer_id);
CREATE INDEX idx_orders_status ON orders(status);
CREATE INDEX idx_orders_created ON orders(created_at);
CREATE INDEX idx_inventory_sku ON inventory_items(sku);
CREATE INDEX idx_reservations_sku ON inventory_reservations(sku);
CREATE INDEX idx_reservations_expires ON inventory_reservations(expires_at);
CREATE INDEX idx_payments_order ON payments(order_id);
CREATE INDEX idx_payments_status ON payments(status);
```

## A2A Protocol Tables

The A2A protocol adds 19+ tables for agent-to-agent commerce:

### Core Payment Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| `a2a_payments` | Direct transfers | payer, payee, amount, currency, status, tx_hash |
| `a2a_payment_requests` | Payment requests | from_agent, to_agent, amount, status |
| `a2a_escrows` | Conditional holds | payer, payee, amount, conditions, status, expires_at |
| `a2a_split_payments` | Multi-party splits | source_payment_id, split_type, total_amount |
| `a2a_split_recipients` | Split targets | split_id, agent_id, share/amount, label |
| `a2a_subscriptions` | Recurring A2A | subscriber, provider, amount, interval, status, next_billing |
| `a2a_subscription_charges` | Charge history | subscription_id, amount, status, payment_id |

### Negotiation & Marketplace

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| `a2a_quotes` | Quote negotiation | buyer_agent, seller_agent, status, round, expires_at |
| `a2a_quote_line_items` | Quote items | quote_id, description, quantity, unit_price |
| `a2a_rfqs` | Request for Quotes | buyer_agent, requirements, status, deadline |
| `a2a_rfq_responses` | RFQ responses | rfq_id, seller_agent, price, delivery_time |
| `a2a_services` | Marketplace listings | agent_id, name, category, pricing_model, capabilities |
| `agent_cards` | ERC-8004 Agent Cards | name, description, status, capabilities, endpoints |

### Trust & Compliance

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| `a2a_disputes` | Conflict tracking | transaction_id, filed_by, filed_against, status, reason |
| `a2a_dispute_evidence` | Proof documents | dispute_id, submitted_by, type, description, hash |
| `a2a_reputation_feedback` | Trust scoring | from_agent, to_agent, quality, speed, communication, value, reliability |
| `a2a_sla_definitions` | SLA terms | agent_id, metric, threshold, penalty_rate |
| `a2a_sla_violations` | SLA breaches | sla_id, violation_date, actual_value, penalty_amount |

### Events & Notifications

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| `a2a_notification_log` | Webhook history | recipient, event_type, status, attempts, last_attempt |
| `a2a_webhook_config` | Endpoint config | agent_id, url, secret, events |
| `a2a_event_subscriptions` | Event filters | subscriber_id, event_pattern, webhook_url |
| `a2a_event_log` | Persistent history | event_id, type, payload, timestamp |

### Orchestration

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| `a2a_workflows` | Workflow definitions | name, steps, status, created_at |
| `a2a_workflow_steps` | Workflow step state | workflow_id, step_name, status, result |

### Column Update Safety

The A2A store uses a column whitelist to prevent SQL injection. Only columns in the `UPDATABLE_COLUMNS` map can be modified via update operations. All queries use parameterized prepared statements.

## PostgreSQL

When using PostgreSQL (Tier 2+), the same schema is used with these differences:

- `UUID` type instead of `TEXT` for IDs
- `TIMESTAMPTZ` instead of `TEXT` for timestamps
- `NUMERIC` instead of `REAL` for monetary amounts
- Connection pooling via `sqlx` (configurable `max_connections`)
- True concurrent writers (SQLite allows only one)

## Migrations

Database migrations are managed by the `stateset-migrations` crate:

- Migrations are checksummed for integrity verification
- Rollback support for failed migrations
- Applied automatically on first connection
- Schema version tracked in a `_migrations` table

```bash
# Re-initialize with latest schema
stateset-init --quickstart

# View current schema version
sqlite3 store.db "SELECT * FROM _migrations ORDER BY id DESC LIMIT 5;"
```

## Querying Raw Data

For debugging, you can query the SQLite database directly:

```bash
sqlite3 store.db "SELECT id, status, total, created_at FROM orders LIMIT 10;"
sqlite3 store.db "SELECT sku, quantity, reserved FROM inventory_items WHERE quantity < 10;"
sqlite3 store.db "SELECT status, count(*) FROM a2a_payments GROUP BY status;"
```
