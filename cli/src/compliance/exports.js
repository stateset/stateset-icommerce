/**
 * Compliance & Regulatory Exports Service
 *
 * Provides audit trail exports, tax reporting (1099-K), GDPR data portability
 * and erasure, compliance summaries, and SOC2 evidence gathering.
 *
 * Uses a factory function pattern — call `createComplianceService(store, options)`
 * where `store` is an object with a `.db` property (better-sqlite3 instance).
 * Queries existing A2A tables and optionally the main commerce database for
 * full GDPR coverage across customers, orders, carts, payments, invoices,
 * shipments, subscriptions, and warranties.
 */

import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Convert an array of flat objects to a CSV string (RFC 4180).
 *
 * @param {Array<Record<string, unknown>>} records
 * @returns {string}
 */
function recordsToCSV(records) {
  if (records.length === 0) return '';
  const headers = Object.keys(records[0]);
  const rows = records.map((r) =>
    headers
      .map((h) => {
        const val = r[h];
        if (val === null || val === undefined) return '';
        const str = String(val);
        // Escape CSV: quote if contains comma, newline, or quote
        if (str.includes(',') || str.includes('\n') || str.includes('"')) {
          return `"${str.replace(/"/g, '""')}"`;
        }
        return str;
      })
      .join(','),
  );
  return [headers.join(','), ...rows].join('\n');
}

/**
 * Translate a human-readable period label into an ISO date range.
 *
 * @param {string} period - day | week | month | quarter | year
 * @returns {{ from: string, to: string }}
 */
function periodToDateRange(period) {
  const now = new Date();
  const to = now.toISOString();
  let from;
  switch (period) {
    case 'day':
      from = new Date(now - 86400000).toISOString();
      break;
    case 'week':
      from = new Date(now - 7 * 86400000).toISOString();
      break;
    case 'month':
      from = new Date(now - 30 * 86400000).toISOString();
      break;
    case 'quarter':
      from = new Date(now - 90 * 86400000).toISOString();
      break;
    case 'year':
      from = new Date(now - 365 * 86400000).toISOString();
      break;
    default:
      from = new Date(now - 30 * 86400000).toISOString();
  }
  return { from, to };
}

/**
 * SHA-256 hash of a string, returned as hex.
 *
 * @param {string} value
 * @returns {string}
 */
function hashValue(value) {
  return createHash('sha256').update(String(value)).digest('hex');
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create a compliance service that queries existing A2A tables and optionally
 * the main commerce database for full GDPR coverage.
 *
 * @param {Object} store  - Object with a `.db` (better-sqlite3 Database)
 * @param {Object} [options] - Additional options
 * @param {string} [options.commerceDbPath] - Path to the main commerce SQLite database
 * @returns {Object} Compliance service methods
 */
export function createComplianceService(store, options = {}) {
  if (!store || !store.db) {
    throw new Error('Compliance service requires a store with a .db property');
  }

  const { db } = store;

  // Lazy-initialized commerce database handle (better-sqlite3)
  let _commerceDb = options._commerceDbOverride || null;
  const _commerceDbOwned = !options._commerceDbOverride;

  /**
   * Get the commerce database handle, opening it lazily on first access.
   * Returns null if no commerce DB path was provided or the file doesn't exist.
   *
   * @returns {import('better-sqlite3').Database | null}
   */
  function getCommerceDb() {
    if (_commerceDb) return _commerceDb;
    const dbPath = options.commerceDbPath;
    if (!dbPath) return null;
    if (dbPath !== ':memory:' && !existsSync(dbPath)) return null;
    try {
      const Database = store.db.constructor;
      _commerceDb = new Database(dbPath, { readonly: false });
      _commerceDb.pragma('journal_mode = WAL');
      return _commerceDb;
    } catch {
      return null;
    }
  }

  /**
   * Check if a table exists in a given database.
   *
   * @param {import('better-sqlite3').Database} database
   * @param {string} tableName
   * @returns {boolean}
   */
  function tableExists(database, tableName) {
    const row = database
      .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?")
      .get(tableName);
    return !!row;
  }

  // -------------------------------------------------------------------------
  // 1. exportAuditTrail
  // -------------------------------------------------------------------------

  /**
   * Export a combined audit trail from circuit-breaker events, spending ledger,
   * and payments.
   */
  function exportAuditTrail({
    from,
    to,
    format = 'json',
    agentName,
    eventType,
    limit = 1000,
  } = {}) {
    const range = from && to ? { from, to } : periodToDateRange('month');
    const fromDate = range.from;
    const toDate = range.to;
    const records = [];

    // --- Circuit breaker events ---
    {
      let sql = `SELECT id, agent_name, event_type, reason, amount, state_before, state_after, metadata, created_at
                 FROM a2a_circuit_breaker_events WHERE created_at >= ? AND created_at <= ?`;
      const params = [fromDate, toDate];
      if (agentName) {
        sql += ' AND agent_name = ?';
        params.push(agentName);
      }
      if (eventType) {
        sql += ' AND event_type = ?';
        params.push(eventType);
      }
      const rows = db.prepare(sql).all(...params);
      for (const row of rows) {
        records.push({ source: 'circuit_breaker', ...row });
      }
    }

    // --- Spending ledger ---
    {
      let sql = `SELECT id, agent_name, amount, success, error, created_at
                 FROM a2a_spending_ledger WHERE created_at >= ? AND created_at <= ?`;
      const params = [fromDate, toDate];
      if (agentName) {
        sql += ' AND agent_name = ?';
        params.push(agentName);
      }
      if (eventType && eventType !== 'spending') {
        // skip spending ledger if caller explicitly asked for a different event type
      } else {
        const rows = db.prepare(sql).all(...params);
        for (const row of rows) {
          records.push({ source: 'spending_ledger', event_type: 'spending', ...row });
        }
      }
    }

    // --- Payments ---
    {
      let sql = `SELECT id, status, sender_address, recipient_address, amount_decimal AS amount,
                        asset, network, memo, created_at
                 FROM a2a_payments WHERE created_at >= ? AND created_at <= ?`;
      const params = [fromDate, toDate];
      if (agentName) {
        sql += ' AND (sender_address = ? OR recipient_address = ?)';
        params.push(agentName, agentName);
      }
      if (eventType && eventType !== 'payment') {
        // skip payments if caller explicitly asked for a different event type
      } else {
        const rows = db.prepare(sql).all(...params);
        for (const row of rows) {
          records.push({ source: 'payment', event_type: 'payment', ...row });
        }
      }
    }

    // Sort by created_at DESC
    records.sort((a, b) => (b.created_at || '').localeCompare(a.created_at || ''));

    // Apply limit
    const limited = records.slice(0, limit);

    const result = {
      records: limited,
      count: limited.length,
      totalAvailable: records.length,
      format,
      period: { from: fromDate, to: toDate },
    };

    if (format === 'csv') {
      result.csv = recordsToCSV(limited);
    }

    return result;
  }

  // -------------------------------------------------------------------------
  // 2. generate1099K
  // -------------------------------------------------------------------------

  /**
   * Generate a 1099-K tax form summary for an agent address.
   */
  function generate1099K({ year, agentAddress }) {
    if (!year || !agentAddress) {
      throw new Error('year and agentAddress are required for 1099-K generation');
    }

    const yearStart = `${year}-01-01T00:00:00.000Z`;
    const yearEnd = `${year}-12-31T23:59:59.999Z`;

    const rows = db
      .prepare(
        `SELECT amount_decimal, created_at FROM a2a_payments
         WHERE recipient_address = ? AND status = 'completed'
         AND created_at >= ? AND created_at <= ?
         ORDER BY created_at ASC`,
      )
      .all(agentAddress, yearStart, yearEnd);

    const grossAmount = rows.reduce((sum, r) => sum + (r.amount_decimal || 0), 0);
    const transactionCount = rows.length;

    // Build monthly breakdown
    const months = [];
    for (let m = 1; m <= 12; m++) {
      const monthRows = rows.filter((r) => {
        const d = new Date(r.created_at);
        return d.getMonth() + 1 === m;
      });
      months.push({
        month: m,
        amount: monthRows.reduce((sum, r) => sum + (r.amount_decimal || 0), 0),
        count: monthRows.length,
      });
    }

    return {
      payee: agentAddress,
      grossAmount: Math.round(grossAmount * 100) / 100,
      transactionCount,
      months,
      year,
      generatedAt: new Date().toISOString(),
    };
  }

  // -------------------------------------------------------------------------
  // 3. generateGDPRExport
  // -------------------------------------------------------------------------

  /**
   * Full GDPR data portability export for a customer/agent identifier.
   */
  function generateGDPRExport(customerId) {
    if (!customerId) {
      throw new Error('customerId is required for GDPR export');
    }

    // Personal data from agent_cards
    const personalData = db
      .prepare(
        `SELECT id, name, wallet_address, description, trust_level, active, created_at, updated_at
         FROM agent_cards WHERE wallet_address = ? OR id = ? OR name = ?`,
      )
      .all(customerId, customerId, customerId);

    // Payments (as sender or recipient)
    const payments = db
      .prepare(
        `SELECT id, status, sender_address, recipient_address, amount_decimal, asset, network, memo, created_at, completed_at
         FROM a2a_payments WHERE sender_address = ? OR recipient_address = ?`,
      )
      .all(customerId, customerId);

    // Communications from notification log
    const communications = db
      .prepare(
        `SELECT id, recipient_address, event_type, payload, status, created_at
         FROM a2a_notification_log WHERE recipient_address = ?`,
      )
      .all(customerId);

    // Orders/disputes filed by or against
    const disputes = db
      .prepare(
        `SELECT id, status, escrow_id, filed_by, filed_against, reason, category, amount_decimal, created_at
         FROM a2a_disputes WHERE filed_by = ? OR filed_against = ?`,
      )
      .all(customerId, customerId);

    // --- Commerce data (main database) ---
    const commerceData = {};
    const cdb = getCommerceDb();
    if (cdb) {
      // Customer profile
      if (tableExists(cdb, 'customers')) {
        commerceData.customers = cdb
          .prepare(
            `SELECT id, email, first_name, last_name, phone, status, accepts_marketing,
                    email_verified, tags, metadata, created_at, updated_at
             FROM customers WHERE id = ? OR email = ?`,
          )
          .all(customerId, customerId);
      }

      // Addresses
      if (tableExists(cdb, 'customer_addresses')) {
        const customerIds = (commerceData.customers || []).map((c) => c.id);
        const allIds = [customerId, ...customerIds];
        if (allIds.length > 0) {
          const placeholders = allIds.map(() => '?').join(',');
          commerceData.addresses = cdb
            .prepare(`SELECT * FROM customer_addresses WHERE customer_id IN (${placeholders})`)
            .all(...allIds);
        }
      }

      // Orders
      if (tableExists(cdb, 'orders')) {
        const customerIds = (commerceData.customers || []).map((c) => c.id);
        const allIds = [customerId, ...customerIds];
        const placeholders = allIds.map(() => '?').join(',');
        commerceData.orders = cdb
          .prepare(`SELECT * FROM orders WHERE customer_id IN (${placeholders})`)
          .all(...allIds);
      }

      // Carts
      if (tableExists(cdb, 'carts')) {
        const customerIds = (commerceData.customers || []).map((c) => c.id);
        const allIds = [customerId, ...customerIds];
        const placeholders = allIds.map(() => '?').join(',');
        commerceData.carts = cdb
          .prepare(
            `SELECT * FROM carts WHERE customer_id IN (${placeholders})
             OR customer_email = ?`,
          )
          .all(...allIds, customerId);
      }

      // Payments
      if (tableExists(cdb, 'payments')) {
        const customerIds = (commerceData.customers || []).map((c) => c.id);
        const allIds = [customerId, ...customerIds];
        const placeholders = allIds.map(() => '?').join(',');
        commerceData.commercePayments = cdb
          .prepare(`SELECT * FROM payments WHERE customer_id IN (${placeholders})`)
          .all(...allIds);
      }

      // Payment methods
      if (tableExists(cdb, 'payment_methods')) {
        const customerIds = (commerceData.customers || []).map((c) => c.id);
        const allIds = [customerId, ...customerIds];
        const placeholders = allIds.map(() => '?').join(',');
        commerceData.paymentMethods = cdb
          .prepare(`SELECT * FROM payment_methods WHERE customer_id IN (${placeholders})`)
          .all(...allIds);
      }

      // Invoices
      if (tableExists(cdb, 'invoices')) {
        const customerIds = (commerceData.customers || []).map((c) => c.id);
        const allIds = [customerId, ...customerIds];
        const placeholders = allIds.map(() => '?').join(',');
        commerceData.invoices = cdb
          .prepare(`SELECT * FROM invoices WHERE customer_id IN (${placeholders})`)
          .all(...allIds);
      }

      // Subscriptions
      if (tableExists(cdb, 'subscriptions')) {
        const customerIds = (commerceData.customers || []).map((c) => c.id);
        const allIds = [customerId, ...customerIds];
        const placeholders = allIds.map(() => '?').join(',');
        commerceData.subscriptions = cdb
          .prepare(`SELECT * FROM subscriptions WHERE customer_id IN (${placeholders})`)
          .all(...allIds);
      }

      // Warranties
      if (tableExists(cdb, 'warranties')) {
        const customerIds = (commerceData.customers || []).map((c) => c.id);
        const allIds = [customerId, ...customerIds];
        const placeholders = allIds.map(() => '?').join(',');
        commerceData.warranties = cdb
          .prepare(`SELECT * FROM warranties WHERE customer_id IN (${placeholders})`)
          .all(...allIds);
      }

      // Warranty claims
      if (tableExists(cdb, 'warranty_claims')) {
        const customerIds = (commerceData.customers || []).map((c) => c.id);
        const allIds = [customerId, ...customerIds];
        const placeholders = allIds.map(() => '?').join(',');
        commerceData.warrantyClaims = cdb
          .prepare(`SELECT * FROM warranty_claims WHERE customer_id IN (${placeholders})`)
          .all(...allIds);
      }

      // Shipments (via orders)
      if (tableExists(cdb, 'shipments') && (commerceData.orders || []).length > 0) {
        const orderIds = commerceData.orders.map((o) => o.id);
        const placeholders = orderIds.map(() => '?').join(',');
        commerceData.shipments = cdb
          .prepare(`SELECT * FROM shipments WHERE order_id IN (${placeholders})`)
          .all(...orderIds);
      }
    }

    return {
      customerId,
      personalData,
      payments,
      communications,
      disputes,
      commerceData: Object.keys(commerceData).length > 0 ? commerceData : undefined,
      exportedAt: new Date().toISOString(),
    };
  }

  // -------------------------------------------------------------------------
  // 4. deleteGDPRData
  // -------------------------------------------------------------------------

  /**
   * GDPR right to erasure — delete or anonymize personal data.
   */
  function deleteGDPRData(customerId, { keepTransactions = false } = {}) {
    if (!customerId) {
      throw new Error('customerId is required for GDPR deletion');
    }

    const deleted = [];
    const retained = [];
    const anonymized = hashValue(customerId).slice(0, 16);
    const anonAddress = `anon_${anonymized}`;

    // Agent cards — always delete personal data
    const agentCards = db
      .prepare(`SELECT id FROM agent_cards WHERE wallet_address = ? OR id = ? OR name = ?`)
      .all(customerId, customerId, customerId);

    if (agentCards.length > 0) {
      db.prepare(`DELETE FROM agent_cards WHERE wallet_address = ? OR id = ? OR name = ?`).run(
        customerId,
        customerId,
        customerId,
      );
      deleted.push({ table: 'agent_cards', count: agentCards.length });
    }

    // Notification log — delete all
    const notifications = db
      .prepare(`SELECT id FROM a2a_notification_log WHERE recipient_address = ?`)
      .all(customerId);
    if (notifications.length > 0) {
      db.prepare(`DELETE FROM a2a_notification_log WHERE recipient_address = ?`).run(customerId);
      deleted.push({ table: 'a2a_notification_log', count: notifications.length });
    }

    // Payments — keep or anonymize
    if (keepTransactions) {
      // Anonymize sender_address and recipient_address
      const senderUpdated = db
        .prepare(`UPDATE a2a_payments SET sender_address = ?, memo = NULL WHERE sender_address = ?`)
        .run(anonAddress, customerId);
      const recipientUpdated = db
        .prepare(
          `UPDATE a2a_payments SET recipient_address = ?, memo = NULL WHERE recipient_address = ?`,
        )
        .run(anonAddress, customerId);
      const totalAnonymized = (senderUpdated.changes || 0) + (recipientUpdated.changes || 0);
      if (totalAnonymized > 0) {
        retained.push({
          table: 'a2a_payments',
          count: totalAnonymized,
          action: 'anonymized',
          anonymizedAs: anonAddress,
        });
      }
    } else {
      const senderPayments = db
        .prepare(`SELECT id FROM a2a_payments WHERE sender_address = ?`)
        .all(customerId);
      const recipientPayments = db
        .prepare(`SELECT id FROM a2a_payments WHERE recipient_address = ?`)
        .all(customerId);
      if (senderPayments.length > 0) {
        db.prepare(`DELETE FROM a2a_payments WHERE sender_address = ?`).run(customerId);
        deleted.push({ table: 'a2a_payments (sender)', count: senderPayments.length });
      }
      if (recipientPayments.length > 0) {
        db.prepare(`DELETE FROM a2a_payments WHERE recipient_address = ?`).run(customerId);
        deleted.push({ table: 'a2a_payments (recipient)', count: recipientPayments.length });
      }
    }

    // Disputes — anonymize addresses if keeping, delete otherwise
    if (keepTransactions) {
      const filedBy = db
        .prepare(`UPDATE a2a_disputes SET filed_by = ? WHERE filed_by = ?`)
        .run(anonAddress, customerId);
      const filedAgainst = db
        .prepare(`UPDATE a2a_disputes SET filed_against = ? WHERE filed_against = ?`)
        .run(anonAddress, customerId);
      const totalDisputeAnon = (filedBy.changes || 0) + (filedAgainst.changes || 0);
      if (totalDisputeAnon > 0) {
        retained.push({
          table: 'a2a_disputes',
          count: totalDisputeAnon,
          action: 'anonymized',
          anonymizedAs: anonAddress,
        });
      }
    } else {
      const disputes = db
        .prepare(`SELECT id FROM a2a_disputes WHERE filed_by = ? OR filed_against = ?`)
        .all(customerId, customerId);
      if (disputes.length > 0) {
        db.prepare(`DELETE FROM a2a_disputes WHERE filed_by = ? OR filed_against = ?`).run(
          customerId,
          customerId,
        );
        deleted.push({ table: 'a2a_disputes', count: disputes.length });
      }
    }

    // -----------------------------------------------------------------------
    // Commerce tables (main database)
    // -----------------------------------------------------------------------
    const cdb = getCommerceDb();
    if (cdb) {
      // Resolve all customer IDs that match (by ID or email)
      const matchedIds = new Set();
      matchedIds.add(customerId);
      if (tableExists(cdb, 'customers')) {
        const rows = cdb
          .prepare(`SELECT id FROM customers WHERE id = ? OR email = ?`)
          .all(customerId, customerId);
        for (const r of rows) matchedIds.add(r.id);
      }
      const customerIds = [...matchedIds];
      const placeholders = customerIds.map(() => '?').join(',');

      const ANON_EMAIL = `${anonAddress}@redacted.invalid`;
      const REDACTED = '[REDACTED]';
      const ANON_ADDR_JSON = JSON.stringify({
        first_name: REDACTED,
        last_name: REDACTED,
        line1: REDACTED,
        city: REDACTED,
        postal_code: '00000',
        country: 'XX',
      });

      // --- Customer profile ---
      if (tableExists(cdb, 'customers')) {
        if (keepTransactions) {
          const res = cdb
            .prepare(
              `UPDATE customers SET
                 email = ?, first_name = ?, last_name = ?,
                 phone = NULL, metadata = NULL, status = 'deleted',
                 updated_at = datetime('now')
               WHERE id IN (${placeholders})`,
            )
            .run(ANON_EMAIL, REDACTED, REDACTED, ...customerIds);
          if (res.changes > 0) {
            retained.push({
              table: 'customers',
              count: res.changes,
              action: 'anonymized',
            });
          }
        } else {
          // Soft-delete (set status) — hard delete could break FK constraints
          const res = cdb
            .prepare(
              `UPDATE customers SET
                 email = ?, first_name = ?, last_name = ?,
                 phone = NULL, metadata = NULL, status = 'deleted',
                 updated_at = datetime('now')
               WHERE id IN (${placeholders})`,
            )
            .run(ANON_EMAIL, REDACTED, REDACTED, ...customerIds);
          if (res.changes > 0) {
            retained.push({
              table: 'customers',
              count: res.changes,
              action: 'anonymized (soft-deleted)',
            });
          }
        }
      }

      // --- Customer addresses ---
      if (tableExists(cdb, 'customer_addresses')) {
        const addrCount = cdb
          .prepare(
            `SELECT COUNT(*) AS cnt FROM customer_addresses WHERE customer_id IN (${placeholders})`,
          )
          .get(...customerIds);
        if (addrCount.cnt > 0) {
          cdb
            .prepare(`DELETE FROM customer_addresses WHERE customer_id IN (${placeholders})`)
            .run(...customerIds);
          deleted.push({ table: 'customer_addresses', count: addrCount.cnt });
        }
      }

      // --- Payment methods (always delete — contains card data) ---
      if (tableExists(cdb, 'payment_methods')) {
        const pmCount = cdb
          .prepare(
            `SELECT COUNT(*) AS cnt FROM payment_methods WHERE customer_id IN (${placeholders})`,
          )
          .get(...customerIds);
        if (pmCount.cnt > 0) {
          cdb
            .prepare(`DELETE FROM payment_methods WHERE customer_id IN (${placeholders})`)
            .run(...customerIds);
          deleted.push({ table: 'payment_methods', count: pmCount.cnt });
        }
      }

      // --- Orders (anonymize addresses, retain for accounting) ---
      if (tableExists(cdb, 'orders')) {
        const res = cdb
          .prepare(
            `UPDATE orders SET
               shipping_address = ?, billing_address = ?, notes = NULL,
               updated_at = datetime('now')
             WHERE customer_id IN (${placeholders})`,
          )
          .run(ANON_ADDR_JSON, ANON_ADDR_JSON, ...customerIds);
        if (res.changes > 0) {
          retained.push({
            table: 'orders',
            count: res.changes,
            action: 'anonymized (addresses redacted)',
          });
        }
      }

      // --- Carts (anonymize PII) ---
      if (tableExists(cdb, 'carts')) {
        const res = cdb
          .prepare(
            `UPDATE carts SET
               customer_email = NULL, customer_phone = NULL, customer_name = NULL,
               shipping_address = NULL, billing_address = NULL, notes = NULL,
               metadata = NULL, updated_at = datetime('now')
             WHERE customer_id IN (${placeholders}) OR customer_email = ?`,
          )
          .run(...customerIds, customerId);
        if (res.changes > 0) {
          retained.push({
            table: 'carts',
            count: res.changes,
            action: 'anonymized',
          });
        }
      }

      // --- Payments (anonymize billing PII, retain amounts for accounting) ---
      if (tableExists(cdb, 'payments')) {
        const res = cdb
          .prepare(
            `UPDATE payments SET
               billing_email = NULL, billing_name = NULL, billing_address = NULL,
               card_last4 = NULL, card_brand = NULL,
               card_exp_month = NULL, card_exp_year = NULL,
               description = NULL, metadata = NULL,
               updated_at = datetime('now')
             WHERE customer_id IN (${placeholders})`,
          )
          .run(...customerIds);
        if (res.changes > 0) {
          retained.push({
            table: 'payments',
            count: res.changes,
            action: 'anonymized (billing PII redacted)',
          });
        }
      }

      // --- Invoices (anonymize billing PII) ---
      if (tableExists(cdb, 'invoices')) {
        const res = cdb
          .prepare(
            `UPDATE invoices SET
               billing_name = NULL, billing_email = NULL, billing_address = NULL,
               billing_city = NULL, billing_state = NULL,
               billing_postal_code = NULL, billing_country = NULL,
               notes = NULL,
               updated_at = datetime('now')
             WHERE customer_id IN (${placeholders})`,
          )
          .run(...customerIds);
        if (res.changes > 0) {
          retained.push({
            table: 'invoices',
            count: res.changes,
            action: 'anonymized (billing PII redacted)',
          });
        }
      }

      // --- Shipments (anonymize recipient PII, via orders) ---
      if (tableExists(cdb, 'shipments') && tableExists(cdb, 'orders')) {
        const orderRows = cdb
          .prepare(`SELECT id FROM orders WHERE customer_id IN (${placeholders})`)
          .all(...customerIds);
        if (orderRows.length > 0) {
          const orderIds = orderRows.map((r) => r.id);
          const oPh = orderIds.map(() => '?').join(',');
          const res = cdb
            .prepare(
              `UPDATE shipments SET
                 recipient_name = ?, recipient_email = NULL, recipient_phone = NULL,
                 shipping_address = ?, notes = NULL,
                 updated_at = datetime('now')
               WHERE order_id IN (${oPh})`,
            )
            .run(REDACTED, ANON_ADDR_JSON, ...orderIds);
          if (res.changes > 0) {
            retained.push({
              table: 'shipments',
              count: res.changes,
              action: 'anonymized (recipient PII redacted)',
            });
          }
        }
      }

      // --- Subscriptions (anonymize addresses) ---
      if (tableExists(cdb, 'subscriptions')) {
        const res = cdb
          .prepare(
            `UPDATE subscriptions SET
               shipping_address = NULL, billing_address = NULL, metadata = NULL,
               updated_at = datetime('now')
             WHERE customer_id IN (${placeholders})`,
          )
          .run(...customerIds);
        if (res.changes > 0) {
          retained.push({
            table: 'subscriptions',
            count: res.changes,
            action: 'anonymized (addresses redacted)',
          });
        }
      }

      // --- Warranty claims (anonymize contact PII) ---
      if (tableExists(cdb, 'warranty_claims')) {
        const res = cdb
          .prepare(
            `UPDATE warranty_claims SET
               contact_phone = NULL, contact_email = NULL,
               shipping_address = NULL, customer_notes = NULL,
               updated_at = datetime('now')
             WHERE customer_id IN (${placeholders})`,
          )
          .run(...customerIds);
        if (res.changes > 0) {
          retained.push({
            table: 'warranty_claims',
            count: res.changes,
            action: 'anonymized (contact PII redacted)',
          });
        }
      }
    }

    return {
      customerId,
      deleted,
      retained,
      deletedAt: new Date().toISOString(),
    };
  }

  // -------------------------------------------------------------------------
  // 5. generateComplianceSummary
  // -------------------------------------------------------------------------

  /**
   * Generate an aggregate compliance summary for a given period.
   */
  function generateComplianceSummary({ period = 'month', agentName } = {}) {
    const range = periodToDateRange(period);

    // Total transactions and volume
    let paymentSql = `SELECT COUNT(*) AS cnt, COALESCE(SUM(amount_decimal), 0) AS vol
                      FROM a2a_payments WHERE created_at >= ? AND created_at <= ?`;
    const paymentParams = [range.from, range.to];
    if (agentName) {
      paymentSql += ' AND (sender_address = ? OR recipient_address = ?)';
      paymentParams.push(agentName, agentName);
    }
    const paymentStats = db.prepare(paymentSql).get(...paymentParams);

    const totalTransactions = paymentStats.cnt || 0;
    const totalVolume = Math.round((paymentStats.vol || 0) * 100) / 100;
    const avgTransactionSize =
      totalTransactions > 0 ? Math.round((totalVolume / totalTransactions) * 100) / 100 : 0;

    // Disputes
    let disputeSql = `SELECT COUNT(*) AS cnt FROM a2a_disputes WHERE created_at >= ? AND created_at <= ?`;
    const disputeParams = [range.from, range.to];
    if (agentName) {
      disputeSql += ' AND (filed_by = ? OR filed_against = ?)';
      disputeParams.push(agentName, agentName);
    }
    const disputeStats = db.prepare(disputeSql).get(...disputeParams);
    const disputeCount = disputeStats.cnt || 0;
    const disputeRate =
      totalTransactions > 0 ? Math.round((disputeCount / totalTransactions) * 10000) / 10000 : 0;

    // Policy violations (circuit breaker trips)
    let violationSql = `SELECT COUNT(*) AS cnt FROM a2a_circuit_breaker_events
                        WHERE event_type = 'trip' AND created_at >= ? AND created_at <= ?`;
    const violationParams = [range.from, range.to];
    if (agentName) {
      violationSql += ' AND agent_name = ?';
      violationParams.push(agentName);
    }
    const violationStats = db.prepare(violationSql).get(...violationParams);
    const policyViolations = violationStats.cnt || 0;

    // Agent count and top agents
    const topAgentSql = `SELECT
                         CASE WHEN sender_address != '' THEN sender_address ELSE recipient_address END AS agent,
                         SUM(amount_decimal) AS volume,
                         COUNT(*) AS tx_count
                       FROM a2a_payments
                       WHERE created_at >= ? AND created_at <= ?
                       GROUP BY agent ORDER BY volume DESC LIMIT 10`;
    const topAgentParams = [range.from, range.to];
    const topAgents = db.prepare(topAgentSql).all(...topAgentParams);

    // Distinct agent count
    const agentCountSql = `SELECT COUNT(DISTINCT addr) AS cnt FROM (
                           SELECT sender_address AS addr FROM a2a_payments WHERE created_at >= ? AND created_at <= ?
                           UNION
                           SELECT recipient_address AS addr FROM a2a_payments WHERE created_at >= ? AND created_at <= ?
                         )`;
    const agentCountParams = [range.from, range.to, range.from, range.to];
    const agentCountRow = db.prepare(agentCountSql).get(...agentCountParams);
    const agentCount = agentCountRow.cnt || 0;

    return {
      period,
      dateRange: range,
      totalTransactions,
      totalVolume,
      avgTransactionSize,
      disputeRate,
      disputeCount,
      policyViolations,
      agentCount,
      topAgents: topAgents.map((a) => ({
        agent: a.agent,
        volume: Math.round((a.volume || 0) * 100) / 100,
        transactionCount: a.tx_count,
      })),
    };
  }

  // -------------------------------------------------------------------------
  // 6. generateSOC2Evidence
  // -------------------------------------------------------------------------

  /**
   * Generate SOC2 audit evidence package for requested controls.
   */
  function generateSOC2Evidence({ controls = [] } = {}) {
    const SUPPORTED_CONTROLS = new Set([
      'access_control',
      'change_management',
      'encryption',
      'monitoring',
      'incident_response',
    ]);

    const evidence = [];

    for (const control of controls) {
      if (!SUPPORTED_CONTROLS.has(control)) {
        evidence.push({
          control,
          status: 'unsupported',
          message: `Control "${control}" is not a supported SOC2 control. Supported: ${[...SUPPORTED_CONTROLS].join(', ')}`,
        });
        continue;
      }

      switch (control) {
        case 'access_control': {
          // Agent registrations and trust level changes
          const agentCards = db
            .prepare(
              `SELECT id, name, wallet_address, trust_level, active, created_at, updated_at
               FROM agent_cards ORDER BY created_at DESC LIMIT 100`,
            )
            .all();
          evidence.push({
            control,
            status: 'gathered',
            description: 'Agent identity registrations and trust levels',
            recordCount: agentCards.length,
            records: agentCards,
          });
          break;
        }

        case 'change_management': {
          // Circuit breaker state changes serve as change management evidence
          const stateChanges = db
            .prepare(
              `SELECT id, agent_name, event_type, state_before, state_after, reason, created_at
               FROM a2a_circuit_breaker_events
               WHERE state_before IS NOT NULL AND state_after IS NOT NULL
               ORDER BY created_at DESC LIMIT 100`,
            )
            .all();
          evidence.push({
            control,
            status: 'gathered',
            description: 'System state changes tracked via circuit breaker events',
            recordCount: stateChanges.length,
            records: stateChanges,
          });
          break;
        }

        case 'encryption': {
          evidence.push({
            control,
            status: 'gathered',
            description: 'Verifiable Encrypted State (VES) cryptography',
            details: {
              algorithm: 'Ed25519 + AES-256-GCM',
              keyDerivation: 'X25519 ECDH + HKDF-SHA256',
              canonicalization: 'RFC 8785 JCS',
              hashFunction: 'Domain-separated SHA-256',
              signingScheme: 'Ed25519 (RFC 8032)',
              merkleTree: 'Binary Merkle tree with SHA-256',
            },
            recordCount: 0,
            records: [],
          });
          break;
        }

        case 'monitoring': {
          // Circuit breaker events + SLA violations
          const cbEvents = db
            .prepare(
              `SELECT id, agent_name, event_type, reason, created_at
               FROM a2a_circuit_breaker_events ORDER BY created_at DESC LIMIT 50`,
            )
            .all();
          const slaViolations = db
            .prepare(
              `SELECT id, sla_id, service_id, metric, severity, created_at
               FROM a2a_sla_violations ORDER BY created_at DESC LIMIT 50`,
            )
            .all();
          evidence.push({
            control,
            status: 'gathered',
            description: 'Monitoring via circuit breaker events and SLA violation tracking',
            recordCount: cbEvents.length + slaViolations.length,
            circuitBreakerEvents: cbEvents,
            slaViolations,
          });
          break;
        }

        case 'incident_response': {
          // Disputes as incident records + circuit breaker trips
          const disputes = db
            .prepare(
              `SELECT id, status, filed_by, filed_against, reason, category, amount_decimal, created_at, resolved_at
               FROM a2a_disputes ORDER BY created_at DESC LIMIT 50`,
            )
            .all();
          const trips = db
            .prepare(
              `SELECT id, agent_name, reason, amount, created_at
               FROM a2a_circuit_breaker_events WHERE event_type = 'trip'
               ORDER BY created_at DESC LIMIT 50`,
            )
            .all();
          evidence.push({
            control,
            status: 'gathered',
            description:
              'Incident response via dispute resolution and circuit breaker trip records',
            recordCount: disputes.length + trips.length,
            disputes,
            circuitBreakerTrips: trips,
          });
          break;
        }
      }
    }

    return {
      controls: evidence,
      generatedAt: new Date().toISOString(),
      version: '1.0.0',
    };
  }

  // -------------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------------

  /**
   * Close the commerce database handle if open and owned by this service.
   */
  function close() {
    if (_commerceDb && _commerceDbOwned) {
      _commerceDb.close();
      _commerceDb = null;
    }
  }

  return {
    exportAuditTrail,
    generate1099K,
    generateGDPRExport,
    deleteGDPRData,
    generateComplianceSummary,
    generateSOC2Evidence,
    close,
    // Expose helpers for testing
    _recordsToCSV: recordsToCSV,
    _periodToDateRange: periodToDateRange,
  };
}
