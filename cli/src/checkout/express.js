/**
 * Express Checkout & Payment Links
 *
 * Factory that creates an express checkout service backed by SQLite.
 * Generates shareable payment links with short codes, supports one-call
 * checkout for both human buyers and A2A agent-to-agent flows.
 *
 * Usage:
 *   const svc = createExpressCheckout(store);   // store has .db (better-sqlite3)
 *   const link = svc.createPaymentLink({ items: [...], currency: 'USD' });
 *   const order = svc.expressCheckout({ linkId: link.linkId, customerId: 'cust-1' });
 */

import { randomUUID, randomBytes } from 'node:crypto';

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

const CHECKOUT_SCHEMA = `
CREATE TABLE IF NOT EXISTS payment_links (
  id TEXT PRIMARY KEY,
  short_code TEXT UNIQUE NOT NULL,
  items TEXT NOT NULL DEFAULT '[]',
  currency TEXT NOT NULL DEFAULT 'USD',
  total REAL NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  customer_id TEXT,
  metadata TEXT,
  views INTEGER NOT NULL DEFAULT 0,
  conversions INTEGER NOT NULL DEFAULT 0,
  expires_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  converted_at TEXT,
  order_id TEXT,
  payment_id TEXT,
  revoked_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_payment_links_short ON payment_links(short_code);
CREATE INDEX IF NOT EXISTS idx_payment_links_status ON payment_links(status);
CREATE INDEX IF NOT EXISTS idx_payment_links_customer ON payment_links(customer_id);
`;

// ---------------------------------------------------------------------------
// Column whitelist — prevents SQL column injection on dynamic queries
// ---------------------------------------------------------------------------

const _UPDATABLE_COLUMNS = new Set([
  'status',
  'customer_id',
  'metadata',
  'views',
  'conversions',
  'expires_at',
  'updated_at',
  'converted_at',
  'order_id',
  'payment_id',
  'revoked_at',
]);

// Valid currency codes (ISO 4217 subset + crypto)
const VALID_CURRENCIES = new Set([
  'USD',
  'EUR',
  'GBP',
  'JPY',
  'CAD',
  'AUD',
  'CHF',
  'CNY',
  'INR',
  'BRL',
  'USDC',
  'USDT',
  'DAI',
  'ssUSD',
]);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Generate an 8-character URL-safe alphanumeric short code.
 * @returns {string}
 */
function generateShortCode() {
  return randomBytes(6).toString('base64url').slice(0, 8).toUpperCase();
}

/**
 * Validate a line-items array.
 * @param {Array} items
 * @throws {Error} on invalid input
 */
function validateItems(items) {
  if (!Array.isArray(items) || items.length === 0) {
    throw new Error('items must be a non-empty array');
  }
  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    if (!item || typeof item !== 'object') {
      throw new Error(`items[${i}] must be an object`);
    }
    if (typeof item.name !== 'string' || item.name.length === 0) {
      throw new Error(`items[${i}].name must be a non-empty string`);
    }
    if (
      typeof item.quantity !== 'number' ||
      !Number.isInteger(item.quantity) ||
      item.quantity <= 0
    ) {
      throw new Error(`items[${i}].quantity must be a positive integer`);
    }
    if (typeof item.unitPrice !== 'number' || item.unitPrice < 0) {
      throw new Error(`items[${i}].unitPrice must be a non-negative number`);
    }
  }
}

/**
 * Calculate total from line items: sum(quantity * unitPrice), rounded to 2 decimals.
 * @param {Array} items
 * @returns {number}
 */
function calculateTotal(items) {
  const raw = items.reduce((sum, item) => sum + item.quantity * item.unitPrice, 0);
  return Math.round(raw * 100) / 100;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create an express checkout service.
 *
 * @param {{ db: import('better-sqlite3').Database }} store
 * @returns {object} Express checkout API
 */
export function createExpressCheckout(store) {
  const db = store.db;

  // Ensure tables exist
  db.exec(CHECKOUT_SCHEMA);

  // -------------------------------------------------------------------
  // Prepared statements (lazily cached)
  // -------------------------------------------------------------------

  const stmtInsert = db.prepare(`
    INSERT INTO payment_links
      (id, short_code, items, currency, total, status, customer_id, metadata, views, conversions, expires_at, created_at, updated_at)
    VALUES
      (@id, @short_code, @items, @currency, @total, @status, @customer_id, @metadata, 0, 0, @expires_at, @created_at, @updated_at)
  `);

  const stmtById = db.prepare('SELECT * FROM payment_links WHERE id = ?');

  const stmtByShortCode = db.prepare(
    'SELECT * FROM payment_links WHERE short_code = ? COLLATE NOCASE',
  );

  const stmtIncrViews = db.prepare(
    'UPDATE payment_links SET views = views + 1, updated_at = ? WHERE id = ?',
  );

  const stmtConvert = db.prepare(`
    UPDATE payment_links
    SET status = 'converted',
        conversions = conversions + 1,
        converted_at = @converted_at,
        order_id = @order_id,
        payment_id = @payment_id,
        customer_id = COALESCE(@customer_id, customer_id),
        updated_at = @updated_at
    WHERE id = @id
  `);

  const stmtRevoke = db.prepare(`
    UPDATE payment_links
    SET status = 'revoked',
        revoked_at = @revoked_at,
        updated_at = @updated_at
    WHERE id = @id
  `);

  // -------------------------------------------------------------------
  // Internal helpers
  // -------------------------------------------------------------------

  /**
   * Look up a payment link by ID or short code.
   * @param {string} idOrShortCode
   * @returns {object|undefined}
   */
  function _findLink(idOrShortCode) {
    return stmtById.get(idOrShortCode) || stmtByShortCode.get(idOrShortCode);
  }

  /**
   * Parse a raw DB row into a nicer object.
   * @param {object} row
   * @returns {object}
   */
  function _parseRow(row) {
    if (!row) return undefined;
    return {
      ...row,
      items: JSON.parse(row.items || '[]'),
      metadata: row.metadata ? JSON.parse(row.metadata) : null,
    };
  }

  // -------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------

  /**
   * Create a shareable payment link.
   *
   * @param {object} params
   * @param {Array<{name:string, sku?:string, quantity:number, unitPrice:number}>} params.items
   * @param {string}  [params.currency='USD']
   * @param {number|null} [params.expiresIn=86400] Seconds until expiry, null for no expiry
   * @param {object}  [params.metadata]
   * @param {string}  [params.customerId]
   * @returns {{ linkId:string, shortCode:string, url:string, total:number, expiresAt:string|null, status:string }}
   */
  function createPaymentLink({
    items,
    currency = 'USD',
    expiresIn = 86400,
    metadata,
    customerId,
  } = {}) {
    validateItems(items);

    const upperCurrency = currency.toUpperCase();
    if (!VALID_CURRENCIES.has(upperCurrency)) {
      throw new Error(`Unsupported currency: ${currency}`);
    }

    const total = calculateTotal(items);
    const linkId = randomUUID();
    const shortCode = generateShortCode();
    const now = new Date().toISOString();
    const expiresAt =
      expiresIn !== null && expiresIn !== undefined
        ? new Date(Date.now() + expiresIn * 1000).toISOString()
        : null;

    stmtInsert.run({
      id: linkId,
      short_code: shortCode,
      items: JSON.stringify(items),
      currency: upperCurrency,
      total,
      status: 'active',
      customer_id: customerId || null,
      metadata: metadata ? JSON.stringify(metadata) : null,
      expires_at: expiresAt,
      created_at: now,
      updated_at: now,
    });

    return {
      linkId,
      shortCode,
      url: `https://pay.stateset.com/l/${shortCode}`,
      total,
      expiresAt,
      status: 'active',
    };
  }

  /**
   * Resolve a payment link by ID or short code.
   * Increments view count and checks expiry.
   *
   * @param {string} linkIdOrShortCode
   * @returns {{ link:object, items:Array, total:number, expired:boolean, status:string }|null}
   */
  function resolvePaymentLink(linkIdOrShortCode) {
    const row = _findLink(linkIdOrShortCode);
    if (!row) return null;

    const now = new Date().toISOString();
    stmtIncrViews.run(now, row.id);

    const link = _parseRow({ ...row, views: row.views + 1 });
    const expired = link.expires_at ? new Date(link.expires_at) < new Date() : false;

    return {
      link,
      items: link.items,
      total: link.total,
      expired,
      status: expired && link.status === 'active' ? 'expired' : link.status,
    };
  }

  /**
   * One-call express checkout from a payment link.
   *
   * @param {object} params
   * @param {string} params.linkId  Payment link ID or short code
   * @param {string} [params.customerId]
   * @param {string} [params.paymentMethod]
   * @param {string} [params.walletAddress]
   * @returns {{ orderId:string, paymentId:string, shortCode:string }}
   */
  function expressCheckout({
    linkId,
    customerId,
    paymentMethod: _paymentMethod,
    walletAddress: _walletAddress,
  } = {}) {
    if (!linkId) throw new Error('linkId is required');

    const row = _findLink(linkId);
    if (!row) throw new Error(`Payment link not found: ${linkId}`);

    const link = _parseRow(row);
    if (row.status === 'revoked') throw new Error('Payment link has been revoked');
    if (row.status === 'converted') throw new Error('Payment link has already been converted');

    if (link.expires_at && new Date(link.expires_at) < new Date()) {
      throw new Error('Payment link has expired');
    }

    const orderId = randomUUID();
    const paymentId = randomUUID();
    const now = new Date().toISOString();

    stmtConvert.run({
      id: row.id,
      converted_at: now,
      order_id: orderId,
      payment_id: paymentId,
      customer_id: customerId || null,
      updated_at: now,
    });

    return {
      orderId,
      paymentId,
      shortCode: row.short_code,
    };
  }

  /**
   * Agent-to-agent instant checkout.
   * Creates a payment link and immediately converts it.
   *
   * @param {object} params
   * @param {string} params.buyerAgent
   * @param {string} params.sellerAgent
   * @param {Array}  params.items
   * @param {string} [params.paymentMethod]
   * @param {string} [params.currency='USD']
   * @returns {{ orderId:string, escrowId:string, linkId:string }}
   */
  function agentCheckout({ buyerAgent, sellerAgent, items, paymentMethod, currency = 'USD' } = {}) {
    if (!buyerAgent) throw new Error('buyerAgent is required');
    if (!sellerAgent) throw new Error('sellerAgent is required');
    validateItems(items);

    const link = createPaymentLink({
      items,
      currency,
      expiresIn: null, // Agent links don't expire
      metadata: {
        buyerAgent,
        sellerAgent,
        paymentMethod: paymentMethod || 'a2a',
        type: 'agent_checkout',
      },
    });

    const result = expressCheckout({
      linkId: link.linkId,
      customerId: buyerAgent,
    });

    const escrowId = randomUUID();

    return {
      orderId: result.orderId,
      escrowId,
      linkId: link.linkId,
    };
  }

  /**
   * Get payment link status and metrics.
   *
   * @param {string} linkId
   * @returns {{ link:object, views:number, conversions:number, status:string }|null}
   */
  function getPaymentLinkStatus(linkId) {
    const row = _findLink(linkId);
    if (!row) return null;

    const link = _parseRow(row);

    return {
      link,
      views: row.views,
      conversions: row.conversions,
      status: row.status,
    };
  }

  /**
   * List payment links with optional filters.
   *
   * @param {object} [filters={}]
   * @param {string} [filters.status]
   * @param {string} [filters.customerId]
   * @param {number} [filters.limit=50]
   * @param {number} [filters.offset=0]
   * @returns {Array}
   */
  function listPaymentLinks(filters = {}) {
    const { status, customerId, limit = 50, offset = 0 } = filters;

    const conditions = [];
    const params = [];

    if (status) {
      conditions.push('status = ?');
      params.push(status);
    }
    if (customerId) {
      conditions.push('customer_id = ?');
      params.push(customerId);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const sql = `SELECT * FROM payment_links ${where} ORDER BY created_at DESC, rowid DESC LIMIT ? OFFSET ?`;
    params.push(limit, offset);

    const rows = db.prepare(sql).all(...params);
    return rows.map(_parseRow);
  }

  /**
   * Revoke (cancel) a payment link.
   *
   * @param {string} linkId
   * @returns {{ success:boolean, link:object }}
   */
  function revokePaymentLink(linkId) {
    const row = _findLink(linkId);
    if (!row) throw new Error(`Payment link not found: ${linkId}`);
    if (row.status === 'revoked') throw new Error('Payment link is already revoked');
    if (row.status === 'converted') throw new Error('Cannot revoke a converted payment link');

    const now = new Date().toISOString();
    stmtRevoke.run({ id: row.id, revoked_at: now, updated_at: now });

    const updated = stmtById.get(row.id);
    return {
      success: true,
      link: _parseRow(updated),
    };
  }

  // -------------------------------------------------------------------
  // Return public API
  // -------------------------------------------------------------------

  return {
    createPaymentLink,
    resolvePaymentLink,
    expressCheckout,
    agentCheckout,
    getPaymentLinkStatus,
    listPaymentLinks,
    revokePaymentLink,
  };
}
