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
   * Test connection by verifying config is present.
   * Stripe doesn't have a simple "ping" endpoint without API keys.
   */
  async testConnection() {
    return !!this.config.webhookSecret;
  }

  /**
   * Map a Stripe entity to StateSet format.
   */
  mapToStateSet(entityType, record, _context = {}) {
    return mapToStateSet(entityType, record);
  }

  /**
   * Reverse mapping is not supported for Stripe (write-back not applicable).
   */
  mapFromStateSet(_entityType, _record) {
    throw new Error('Stripe adapter does not support reverse mapping');
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
