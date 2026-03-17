/**
 * Stripe Platform Adapter for StateSet Commerce
 *
 * Unlike Shopify, Stripe is primarily a webhook-first adapter — you don't
 * "import products" from Stripe. Instead, payment events flow in via webhooks
 * and sync into StateSet's local database in real time.
 *
 * Supported entities: customers, payments, subscriptions, invoices
 */

import { BasePlatformAdapter } from '../base-adapter.js';
import {
  mapToStateSet,
  mapPaymentIntentToStateSet,
  mapChargeToStateSet,
  mapCustomerToStateSet,
  mapSubscriptionToStateSet,
  mapInvoiceToStateSet,
  mapDisputeToStateSet,
} from './mapper.js';
import { createStripeWebhookHandlers, getSupportedStripeEvents } from './webhooks.js';
import { verifyStripeSignature } from './signature.js';

/**
 * Flatten a nested object for Stripe's URL-encoded body format.
 * { metadata: { foo: 'bar' } } → { 'metadata[foo]': 'bar' }
 */
function _flattenObject(obj, prefix = '') {
  const result = {};
  for (const [key, value] of Object.entries(obj)) {
    const fullKey = prefix ? `${prefix}[${key}]` : key;
    if (
      value !== null &&
      value !== undefined &&
      typeof value === 'object' &&
      !Array.isArray(value)
    ) {
      Object.assign(result, _flattenObject(value, fullKey));
    } else if (value !== null && value !== undefined) {
      result[fullKey] = String(value);
    }
  }
  return result;
}

export class StripeAdapter extends BasePlatformAdapter {
  /**
   * @param {Object} [config]
   * @param {string} [config.webhookSecret] - Stripe webhook signing secret (whsec_...)
   */
  constructor(config = {}) {
    super('stripe');
    this.config = config;
  }

  /**
   * Test connection. If an API key is configured, pings the Stripe API.
   */
  async testConnection() {
    if (this.config.apiKey) {
      try {
        const resp = await fetch('https://api.stripe.com/v1/balance', {
          headers: { Authorization: `Bearer ${this.config.apiKey}` },
        });
        return resp.ok;
      } catch {
        return false;
      }
    }
    return !!this.config.webhookSecret;
  }

  /**
   * Make an authenticated request to the Stripe API.
   * @param {string} method
   * @param {string} path - e.g., '/v1/payment_intents'
   * @param {Object} [body] - URL-encoded body params
   * @returns {Promise<Object>}
   */
  async _stripeRequest(method, path, body) {
    if (!this.config.apiKey) {
      throw new Error('Stripe API key is required for write operations. Set config.apiKey.');
    }

    const url = `https://api.stripe.com${path}`;
    const options = {
      method,
      headers: {
        Authorization: `Bearer ${this.config.apiKey}`,
        'Content-Type': 'application/x-www-form-urlencoded',
      },
      signal: AbortSignal.timeout(15_000),
    };

    if (body) {
      options.body = new URLSearchParams(_flattenObject(body)).toString();
    }

    const response = await fetch(url, options);
    const data = await response.json();

    if (!response.ok) {
      const errMsg = data?.error?.message || `Stripe API error: ${response.status}`;
      throw new Error(errMsg);
    }

    return data;
  }

  /**
   * Create a Stripe PaymentIntent.
   * @param {Object} params - { amount (cents), currency, description, metadata }
   * @returns {Promise<Object>}
   */
  async createPaymentIntent(params) {
    return this._stripeRequest('POST', '/v1/payment_intents', params);
  }

  /**
   * Create a Stripe Refund.
   * @param {Object} params - { payment_intent, amount (cents), reason, metadata }
   * @returns {Promise<Object>}
   */
  async createRefund(params) {
    return this._stripeRequest('POST', '/v1/refunds', params);
  }

  /**
   * Update order fulfillment metadata in Stripe (via PaymentIntent metadata update).
   * @param {string} paymentIntentId
   * @param {Object} metadata
   * @returns {Promise<Object>}
   */
  async updateFulfillment(paymentIntentId, metadata) {
    return this._stripeRequest('POST', `/v1/payment_intents/${paymentIntentId}`, {
      metadata: {
        ...metadata,
        fulfillment_status: metadata.fulfillment_status || 'fulfilled',
        fulfilled_at: new Date().toISOString(),
      },
    });
  }

  /**
   * Create a Stripe Customer.
   * @param {Object} params - { email, name, metadata }
   * @returns {Promise<Object>}
   */
  async createCustomer(params) {
    return this._stripeRequest('POST', '/v1/customers', params);
  }

  /**
   * Map a Stripe entity to StateSet format.
   */
  mapToStateSet(entityType, record, _context = {}) {
    return mapToStateSet(entityType, record);
  }

  /**
   * Reverse mapping: convert StateSet records to Stripe API params.
   * @param {string} entityType
   * @param {Object} record
   * @returns {Object}
   */
  mapFromStateSet(entityType, record) {
    switch (entityType) {
      case 'payment_intent':
        return {
          amount: Math.round((record.amount || record.total || 0) * 100),
          currency: (record.currency || 'usd').toLowerCase(),
          description: record.description || record.memo || undefined,
          metadata: { stateset_order_id: record.orderId || record.id },
        };
      case 'refund':
        return {
          payment_intent: record.stripePaymentIntentId || record.externalId,
          amount: record.amount ? Math.round(record.amount * 100) : undefined,
          reason: record.reason || 'requested_by_customer',
          metadata: { stateset_return_id: record.returnId || record.id },
        };
      case 'customer':
        return {
          email: record.email,
          name: record.name || `${record.firstName || ''} ${record.lastName || ''}`.trim(),
          metadata: { stateset_customer_id: record.id },
        };
      default:
        throw new Error(
          `Unsupported reverse mapping entity: ${entityType}; reverse mapping is not supported`,
        );
    }
  }

  /**
   * Stripe is webhook-first — no batch API fetching.
   */
  // eslint-disable-next-line require-yield
  async *fetchBatches(_entityType, _options = {}) {
    throw new Error(
      'Stripe adapter is webhook-first. Use createWebhookHandlers() for real-time sync.',
    );
  }

  /**
   * No file import for Stripe.
   */
  // eslint-disable-next-line require-yield
  async *parseBatchesFromFile(_entityType, _filePath, _format, _batchSize) {
    throw new Error('Stripe adapter does not support file-based import.');
  }

  /**
   * Process a webhook event by mapping the payload.
   * @param {string} eventType - Stripe event type (e.g., 'payment_intent.succeeded')
   * @param {Object} payload - Stripe event payload
   * @returns {Object|null}
   */
  handleWebhook(eventType, payload) {
    const obj = payload.data?.object || payload;

    const mappers = {
      'payment_intent.succeeded': mapPaymentIntentToStateSet,
      'payment_intent.payment_failed': mapPaymentIntentToStateSet,
      'payment_intent.canceled': mapPaymentIntentToStateSet,
      'charge.succeeded': mapChargeToStateSet,
      'charge.refunded': mapChargeToStateSet,
      'charge.dispute.created': mapDisputeToStateSet,
      'customer.created': mapCustomerToStateSet,
      'customer.updated': mapCustomerToStateSet,
      'customer.subscription.created': mapSubscriptionToStateSet,
      'customer.subscription.updated': mapSubscriptionToStateSet,
      'customer.subscription.deleted': mapSubscriptionToStateSet,
      'invoice.paid': mapInvoiceToStateSet,
      'invoice.payment_failed': mapInvoiceToStateSet,
    };

    const mapper = mappers[eventType];
    if (!mapper) return null;
    return mapper(obj);
  }

  /**
   * Get supported webhook event types.
   */
  getSupportedWebhookTopics() {
    return getSupportedStripeEvents();
  }

  /**
   * Create webhook handlers for real-time sync.
   */
  createWebhookHandlers(commerce, idMapStore) {
    return createStripeWebhookHandlers(commerce, idMapStore);
  }

  /**
   * Get the list of entity types this adapter supports.
   */
  getSupportedEntities() {
    return ['customers', 'payments', 'subscriptions', 'invoices'];
  }

  /**
   * Import order (not applicable for Stripe — webhook-first).
   */
  getImportOrder() {
    return ['customers', 'subscriptions'];
  }

  /**
   * Verify a Stripe webhook signature.
   * @param {string} rawBody
   * @param {string} signatureHeader
   * @returns {{ valid: boolean, error?: string }}
   */
  verifyWebhookSignature(rawBody, signatureHeader) {
    if (!this.config.webhookSecret) {
      return { valid: false, error: 'No webhook secret configured' };
    }
    return verifyStripeSignature(rawBody, signatureHeader, this.config.webhookSecret);
  }
}

// Re-export everything
export {
  mapToStateSet,
  mapPaymentIntentToStateSet,
  mapChargeToStateSet,
  mapCustomerToStateSet,
  mapSubscriptionToStateSet,
  mapRefundToStateSet,
  mapInvoiceToStateSet,
  mapDisputeToStateSet,
  PAYMENT_INTENT_STATUS_MAP,
  CHARGE_STATUS_MAP,
  REFUND_STATUS_MAP,
  SUBSCRIPTION_STATUS_MAP,
  INVOICE_STATUS_MAP,
  DISPUTE_STATUS_MAP,
  centsToDecimal,
  timestampToIso,
} from './mapper.js';
export {
  verifyStripeSignature,
  parseStripeSignatureHeader,
  computeSignature,
} from './signature.js';
export { createStripeWebhookHandlers, getSupportedStripeEvents } from './webhooks.js';
