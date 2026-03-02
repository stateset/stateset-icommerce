/**
 * WooCommerce Platform Adapter for StateSet Commerce
 *
 * Full platform integration:
 * - API-based data import (customers, products, inventory, orders)
 * - Bidirectional data mapping (WooCommerce <-> StateSet)
 * - Webhook handling for real-time sync
 * - JSON file import
 */

import { BasePlatformAdapter } from '../base-adapter.js';
import { WooCommerceClient } from './client.js';
import {
  mapToStateSet,
  mapFromStateSet,
  mapCustomerToStateSet,
  mapProductToStateSet,
  mapOrderToStateSet,
} from './mapper.js';
import {
  createWooCommerceWebhookHandlers,
  getSupportedWooCommerceTopics,
  verifyWooCommerceSignature,
} from './webhooks.js';
import fs from 'fs';

export class WooCommerceAdapter extends BasePlatformAdapter {
  /**
   * @param {Object} [config]
   * @param {string} [config.siteUrl] - WooCommerce site URL
   * @param {string} [config.consumerKey] - REST API consumer key
   * @param {string} [config.consumerSecret] - REST API consumer secret
   * @param {string} [config.apiVersion='wc/v3'] - API version
   * @param {string} [config.webhookSecret] - Webhook signing secret
   */
  constructor(config = {}) {
    super('woocommerce');
    this.config = config;

    // Only create client if credentials are provided
    this.client =
      config.siteUrl && config.consumerKey && config.consumerSecret
        ? new WooCommerceClient(config)
        : null;
  }

  /**
   * Test the connection to the WooCommerce site.
   * @returns {Promise<boolean>}
   */
  async testConnection() {
    if (!this.client) return false;
    return this.client.testConnection();
  }

  /**
   * Map a WooCommerce entity to StateSet format.
   */
  mapToStateSet(entityType, record, context = {}) {
    return mapToStateSet(entityType, record, context);
  }

  /**
   * Map a StateSet entity back to WooCommerce format.
   */
  mapFromStateSet(entityType, record) {
    return mapFromStateSet(entityType, record);
  }

  /**
   * Fetch batches from the WooCommerce API.
   */
  async *fetchBatches(entityType, options = {}) {
    if (!this.client) {
      throw new Error(
        'WooCommerce client not configured. Provide siteUrl, consumerKey, and consumerSecret.',
      );
    }

    let page = 0;
    const fetcher = this._getFetcher(entityType);

    for await (const records of fetcher(options)) {
      page++;
      yield {
        entityType,
        records,
        page,
        hasMore: true,
      };
    }
  }

  /**
   * Parse batches from a JSON file.
   * WooCommerce adapter only supports JSON (no CSV parser).
   */
  async *parseBatchesFromFile(entityType, filePath, format, batchSize = 50) {
    if (format !== 'json') {
      throw new Error(`WooCommerce adapter only supports JSON file import. Got: ${format}`);
    }
    yield* this._parseJsonBatches(entityType, filePath, batchSize);
  }

  /**
   * Process an incoming webhook event.
   * @param {string} eventType - e.g., 'order.created', 'product.updated'
   * @param {Object} payload - Webhook body
   * @returns {import('../base-adapter.js').MappedRecord|null}
   */
  handleWebhook(eventType, payload) {
    const mappers = {
      'order.created': (p) => mapOrderToStateSet(p),
      'order.updated': (p) => mapOrderToStateSet(p),
      'product.created': mapProductToStateSet,
      'product.updated': mapProductToStateSet,
      'customer.created': mapCustomerToStateSet,
      'customer.updated': mapCustomerToStateSet,
    };

    const mapper = mappers[eventType];
    if (!mapper) return null;
    return mapper(payload);
  }

  /**
   * Get supported webhook topics.
   * @returns {string[]}
   */
  getSupportedWebhookTopics() {
    return getSupportedWooCommerceTopics();
  }

  /**
   * Create webhook handlers for real-time sync.
   * @param {Object} commerce
   * @param {import('../id-map-store.js').IdMapStore} idMapStore
   * @returns {Object}
   */
  createWebhookHandlers(commerce, idMapStore) {
    return createWooCommerceWebhookHandlers(commerce, idMapStore);
  }

  /**
   * Verify a WooCommerce webhook signature.
   * @param {string} rawBody
   * @param {string} signatureHeader
   * @returns {{ valid: boolean, error?: string }}
   */
  verifyWebhookSignature(rawBody, signatureHeader) {
    if (!this.config.webhookSecret) {
      return { valid: false, error: 'No webhook secret configured' };
    }
    return verifyWooCommerceSignature(rawBody, signatureHeader, this.config.webhookSecret);
  }

  /**
   * Get the list of entity types this adapter supports.
   * @returns {string[]}
   */
  getSupportedEntities() {
    return ['customers', 'products', 'inventory', 'orders'];
  }

  /**
   * Ensure dependencies are imported before order-linked data.
   * @returns {string[]}
   */
  getImportOrder() {
    return ['customers', 'products', 'inventory', 'orders'];
  }

  // ---------------------------------------------------------------------------
  // Private helpers
  // ---------------------------------------------------------------------------

  /** @private */
  _getFetcher(entityType) {
    switch (entityType) {
      case 'customers':
        return (opts) => this.client.getCustomers(opts);
      case 'products':
        return (opts) => this.client.getProducts(opts);
      case 'inventory':
        // WooCommerce inventory is fetched via products endpoint with stock fields
        return (opts) => this.client.getProducts({ ...opts });
      case 'orders':
        return (opts) => this.client.getOrders(opts);
      default:
        throw new Error(`Unsupported entity type for API fetch: ${entityType}`);
    }
  }

  /** @private */
  async *_parseJsonBatches(entityType, filePath, batchSize) {
    const content = fs.readFileSync(filePath, 'utf-8');
    const data = JSON.parse(content);

    // WooCommerce REST API response is an array, or { entityType: [...] }
    const records = Array.isArray(data) ? data : data[entityType] || [];

    let batch = [];
    let page = 0;

    for (const record of records) {
      batch.push(record);
      if (batch.length >= batchSize) {
        page++;
        yield { records: batch, page, hasMore: true };
        batch = [];
      }
    }

    if (batch.length > 0) {
      page++;
      yield { records: batch, page, hasMore: false };
    }
  }
}

// Re-export everything
export { WooCommerceClient, WooCommerceApiError } from './client.js';
export { validateUrl, buildBasicAuth, RateLimiter } from './client.js';
export {
  mapToStateSet,
  mapFromStateSet,
  mapCustomerToStateSet,
  mapProductToStateSet,
  mapOrderToStateSet,
  mapInventoryToStateSet,
  mapCustomerFromStateSet,
  mapProductFromStateSet,
  mapOrderStatus,
  mapProductStatus,
  derivePaymentStatus,
  stripHtml,
} from './mapper.js';
export {
  createWooCommerceWebhookHandlers,
  getSupportedWooCommerceTopics,
  verifyWooCommerceSignature,
} from './webhooks.js';
