/**
 * Customer Identity Resolution for StateSet Channel Gateways
 *
 * Maps channel sender IDs (phone numbers, Discord user IDs, Slack user IDs)
 * to commerce customer records. Enables personalized conversations by
 * injecting customer context into agent prompts.
 *
 * Uses SQLite (better-sqlite3) for persistence alongside the session store.
 */

import Database from 'better-sqlite3';
import path from 'path';
import os from 'os';
import fs from 'fs';

const DEFAULT_DB_PATH = path.join(os.homedir(), '.stateset', 'channel-identity.db');

export class CustomerIdentityStore {
  /**
   * @param {Object} [opts]
   * @param {string} [opts.dbPath]
   */
  constructor({ dbPath = DEFAULT_DB_PATH } = {}) {
    const dir = path.dirname(dbPath);
    fs.mkdirSync(dir, { recursive: true });

    this.db = new Database(dbPath);
    this.db.pragma('journal_mode = WAL');

    this.db.exec(`
      CREATE TABLE IF NOT EXISTS channel_identity (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        channel      TEXT NOT NULL,
        sender_id    TEXT NOT NULL,
        customer_id  TEXT NOT NULL,
        linked_at    INTEGER NOT NULL DEFAULT (unixepoch() * 1000),
        linked_by    TEXT DEFAULT 'auto',
        UNIQUE(channel, sender_id)
      )
    `);

    this._get = this.db.prepare(
      `SELECT customer_id, linked_by FROM channel_identity WHERE channel = ? AND sender_id = ?`,
    );

    this._link = this.db.prepare(
      `INSERT INTO channel_identity (channel, sender_id, customer_id, linked_at, linked_by)
       VALUES (?, ?, ?, ?, ?)
       ON CONFLICT(channel, sender_id)
       DO UPDATE SET customer_id = excluded.customer_id, linked_at = excluded.linked_at, linked_by = excluded.linked_by`,
    );

    this._unlink = this.db.prepare(
      `DELETE FROM channel_identity WHERE channel = ? AND sender_id = ?`,
    );

    this._getByCustomer = this.db.prepare(
      `SELECT channel, sender_id FROM channel_identity WHERE customer_id = ?`,
    );
  }

  /**
   * Look up the commerce customer ID for a channel sender.
   *
   * @param {string} channel
   * @param {string} senderId
   * @returns {{ customerId: string, linkedBy: string }|null}
   */
  getCustomerId(channel, senderId) {
    const row = this._get.get(channel, senderId);
    if (!row) return null;
    return { customerId: row.customer_id, linkedBy: row.linked_by };
  }

  /**
   * Link a channel sender to a commerce customer.
   *
   * @param {string} channel
   * @param {string} senderId
   * @param {string} customerId
   * @param {'auto'|'manual'} [linkedBy='auto']
   */
  link(channel, senderId, customerId, linkedBy = 'auto') {
    this._link.run(channel, senderId, customerId, Date.now(), linkedBy);
  }

  /**
   * Remove the identity link for a channel sender.
   *
   * @param {string} channel
   * @param {string} senderId
   */
  unlink(channel, senderId) {
    this._unlink.run(channel, senderId);
  }

  /**
   * Find all channel identities linked to a customer.
   *
   * @param {string} customerId
   * @returns {{ channel: string, senderId: string }[]}
   */
  getChannelsForCustomer(customerId) {
    return this._getByCustomer.all(customerId).map((row) => ({
      channel: row.channel,
      senderId: row.sender_id,
    }));
  }

  close() {
    this.db.close();
  }
}

// ============================================================================
// Identity resolver — used by the message pipeline
// ============================================================================

/**
 * Resolve a sender's identity against the commerce database.
 *
 * Attempts to match by:
 * 1. Cached identity link in the identity store
 * 2. Phone number match (WhatsApp, Signal)
 * 3. Email pattern match (if sender ID looks like an email)
 *
 * @param {Object} opts
 * @param {string} opts.channel
 * @param {string} opts.senderId
 * @param {CustomerIdentityStore} opts.identityStore
 * @param {Object} opts.commerce - Commerce instance
 * @returns {Promise<{ customer: Object|null, source: string }>}
 */
export async function resolveIdentity({ channel, senderId, identityStore, commerce }) {
  // 1. Check existing link
  const existing = identityStore.getCustomerId(channel, senderId);
  if (existing) {
    try {
      const customer = await commerce.customers.get(existing.customerId);
      if (customer) return { customer, source: 'linked' };
    } catch (err) {
      console.debug('[identity] Linked customer lookup failed:', err.message || err);
    }
  }

  // 2. Try phone-based match for WhatsApp/Signal
  if (channel === 'whatsapp' || channel === 'signal') {
    const phone = normalizePhone(senderId);
    if (phone) {
      try {
        const customers = await commerce.customers.list();
        const match = customers.find((c) => normalizePhone(c.phone) === phone);
        if (match) {
          identityStore.link(channel, senderId, match.id, 'auto');
          return { customer: match, source: 'phone' };
        }
      } catch (err) {
        console.debug('[identity] Phone-based customer match failed:', err.message || err);
      }
    }
  }

  return { customer: null, source: 'none' };
}

/**
 * Build a context string describing the customer for agent prompts.
 *
 * @param {Object} customer
 * @param {Object} [commerce] - Commerce instance for extra data
 * @returns {Promise<string>}
 */
export async function buildCustomerContext(customer, commerce) {
  const parts = [];
  const name = [customer.firstName || customer.first_name, customer.lastName || customer.last_name]
    .filter(Boolean)
    .join(' ');
  parts.push(`Customer: ${name || 'Unknown'}`);

  if (customer.email) parts.push(`Email: ${customer.email}`);
  if (customer.phone) parts.push(`Phone: ${customer.phone}`);

  if (commerce) {
    try {
      const orders = await commerce.orders.list();
      const customerOrders = orders.filter(
        (o) => o.customerId === customer.id || o.customer_id === customer.id,
      );

      if (customerOrders.length > 0) {
        const totalSpend = customerOrders.reduce((sum, o) => sum + (o.total || 0), 0);
        parts.push(`Orders: ${customerOrders.length}`);
        parts.push(`Lifetime value: $${totalSpend.toFixed(2)}`);

        const lastOrder = customerOrders.sort(
          (a, b) =>
            new Date(b.createdAt || b.created_at || 0) - new Date(a.createdAt || a.created_at || 0),
        )[0];
        if (lastOrder) {
          parts.push(
            `Last order: ${lastOrder.orderNumber || lastOrder.order_number || lastOrder.id} (${(lastOrder.status || 'unknown').toUpperCase()})`,
          );
        }
      }
    } catch (err) {
      console.debug('[identity] Customer context enrichment failed:', err.message || err);
    }
  }

  return parts.join(' | ');
}

/**
 * Normalize a phone number for comparison (strip non-digits, remove leading +).
 */
function normalizePhone(phone) {
  if (!phone) return null;
  const digits = String(phone).replace(/[^\d]/g, '');
  // Remove leading country code '1' for US numbers if 11 digits
  if (digits.length === 11 && digits.startsWith('1')) return digits.slice(1);
  return digits || null;
}

// ============================================================================
// Identity middleware — injects customer context into pipeline
// ============================================================================

/**
 * Create a middleware that resolves customer identity and injects context.
 *
 * @param {Object} opts
 * @param {CustomerIdentityStore} opts.identityStore
 * @param {Object} opts.commerce
 * @returns {Function}
 */
export function identityMiddleware({ identityStore, commerce }) {
  return async function identityMiddlewareFn(ctx, next) {
    if (!commerce || !identityStore) {
      await next();
      return;
    }

    const { customer, source } = await resolveIdentity({
      channel: ctx.channel,
      senderId: ctx.senderId,
      identityStore,
      commerce,
    });

    if (customer) {
      ctx.metadata.customer = customer;
      ctx.metadata.customerSource = source;
      ctx.metadata.customerContext = await buildCustomerContext(customer, commerce);
    }

    await next();
  };
}
