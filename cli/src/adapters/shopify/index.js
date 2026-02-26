/**
 * Shopify Platform Adapter for StateSet Commerce
 *
 * Reference adapter that demonstrates the full platform integration pattern:
 * - API-based and file-based data import
 * - Bidirectional data mapping (Shopify ↔ StateSet)
 * - Webhook handling for real-time sync
 * - CSV export parsing
 */

import { BasePlatformAdapter } from '../base-adapter.js';
import { ShopifyClient } from './client.js';
import {
  mapToStateSet,
  mapFromStateSet,
  mapCustomerToStateSet,
  mapFulfillmentToStateSet,
  mapProductToStateSet,
  mapOrderToStateSet,
} from './mapper.js';
import { parseCustomerCsv, parseProductCsv, parseOrderCsv } from './csv-parser.js';
import { createShopifyWebhookHandlers, getSupportedTopics } from './webhooks.js';
import fs from 'fs';

export class ShopifyAdapter extends BasePlatformAdapter {
  /**
   * @param {Object} [config]
   * @param {string} [config.shopDomain]
   * @param {string} [config.accessToken]
   * @param {string} [config.apiVersion='2024-01']
   */
  constructor(config = {}) {
    super('shopify');
    this.config = config;

    // Only create client if credentials are provided
    this.client = config.shopDomain && config.accessToken ? new ShopifyClient(config) : null;
  }

  async testConnection() {
    if (!this.client) return false;
    return this.client.testConnection();
  }

  mapToStateSet(entityType, record, context = {}) {
    return mapToStateSet(entityType, record, context);
  }

  mapFromStateSet(entityType, record) {
    return mapFromStateSet(entityType, record);
  }

  /**
   * Fetch batches from the Shopify API.
   */
  async *fetchBatches(entityType, options = {}) {
    if (!this.client) {
      throw new Error('Shopify client not configured. Provide shopDomain and accessToken.');
    }

    let page = 0;
    const fetcher = this._getFetcher(entityType);

    for await (const records of fetcher(options)) {
      page++;
      yield {
        entityType,
        records,
        page,
        hasMore: true, // Will be corrected on the last yield
      };
    }
  }

  /**
   * Parse batches from a file (CSV or JSON).
   * Called by DataImporter._getBatches() for file-based sources.
   */
  async *parseBatchesFromFile(entityType, filePath, format, batchSize = 50) {
    if (format === 'csv') {
      yield* this._parseCsvBatches(entityType, filePath, batchSize);
    } else if (format === 'json') {
      yield* this._parseJsonBatches(entityType, filePath, batchSize);
    } else {
      throw new Error(`Unsupported format: ${format}`);
    }
  }

  /**
   * Process a webhook event.
   */
  handleWebhook(eventType, payload) {
    const mappers = {
      'customers/create': mapCustomerToStateSet,
      'customers/update': mapCustomerToStateSet,
      'products/create': mapProductToStateSet,
      'products/update': mapProductToStateSet,
      'orders/create': (p) => mapOrderToStateSet(p),
      'orders/updated': (p) => mapOrderToStateSet(p),
      'fulfillments/create': (p) => mapFulfillmentToStateSet(p),
      'fulfillments/update': (p) => mapFulfillmentToStateSet(p),
    };

    const mapper = mappers[eventType];
    if (!mapper) return null;
    return mapper(payload);
  }

  /**
   * Get supported webhook topics.
   */
  getSupportedWebhookTopics() {
    return getSupportedTopics();
  }

  /**
   * Create webhook handlers for real-time sync.
   */
  createWebhookHandlers(commerce, idMapStore) {
    return createShopifyWebhookHandlers(commerce, idMapStore);
  }

  /**
   * Get the list of entity types this adapter supports.
   * @returns {string[]}
   */
  getSupportedEntities() {
    return ['customers', 'products', 'inventory', 'orders', 'fulfillments'];
  }

  /**
   * Ensure dependencies are imported before order-linked data.
   * @returns {string[]}
   */
  getImportOrder() {
    return ['customers', 'products', 'inventory', 'orders', 'fulfillments'];
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
        return (opts) => this._fetchInventoryBatches(opts);
      case 'orders':
        return (opts) => this.client.getOrders(opts);
      case 'fulfillments':
        return (opts) => this._fetchFulfillmentBatches(opts);
      default:
        throw new Error(`Unsupported entity type for API fetch: ${entityType}`);
    }
  }

  /** @private */
  async *_fetchInventoryBatches(options = {}) {
    const explicitLocationId = options.locationId || options.location_id || null;
    const locationIds = [];
    if (explicitLocationId) {
      locationIds.push(explicitLocationId);
    } else if (typeof this.client.getLocations === 'function') {
      const locations = await this.client.getLocations();
      if (Array.isArray(locations)) {
        for (const location of locations) {
          if (location?.id !== null && location?.id !== undefined) {
            locationIds.push(String(location.id));
          }
        }
      }
    }

    if (locationIds.length === 0) {
      throw new Error(
        'Shopify inventory import requires locationId (or available store locations).',
      );
    }

    for (const locationId of locationIds) {
      for await (const records of this.client.getInventoryLevels(locationId, options)) {
        yield records;
      }
    }
  }

  /** @private */
  async *_fetchFulfillmentBatches(options = {}) {
    for await (const records of this.client.getFulfillments(options)) {
      yield records;
    }
  }

  /** @private */
  async *_parseCsvBatches(entityType, filePath, batchSize) {
    const parsers = {
      customers: parseCustomerCsv,
      products: parseProductCsv,
      orders: parseOrderCsv,
    };

    const parser = parsers[entityType];
    if (!parser) {
      throw new Error(`No CSV parser for entity type: ${entityType}`);
    }

    yield* parser(filePath, batchSize);
  }

  /** @private */
  async *_parseJsonBatches(entityType, filePath, batchSize) {
    const content = fs.readFileSync(filePath, 'utf-8');
    const data = JSON.parse(content);

    // Shopify REST API response format: { customers: [...] } or just [...]
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
export { ShopifyClient, ShopifyApiError } from './client.js';
export {
  mapToStateSet,
  mapFromStateSet,
  mapCustomerToStateSet,
  mapProductToStateSet,
  mapOrderToStateSet,
  mapInventoryToStateSet,
  mapFulfillmentToStateSet,
  mapFulfillmentFromStateSet,
  stripHtml,
  mapCustomerStatus,
  mapFinancialStatus,
  mapFulfillmentStatus,
} from './mapper.js';
export {
  parseCsvLine,
  parseCsvFile,
  parseCustomerCsv,
  parseProductCsv,
  parseOrderCsv,
} from './csv-parser.js';
export { createShopifyWebhookHandlers, getSupportedTopics } from './webhooks.js';
export { ShopifyImporter } from './importer.js';
export { exportToJson } from './exporter.js';
