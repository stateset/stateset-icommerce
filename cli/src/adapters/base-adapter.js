/**
 * Base Platform Adapter for StateSet Commerce
 *
 * Defines the interface that all platform adapters must implement.
 * Each adapter maps external commerce platform data to StateSet's internal model.
 *
 * @typedef {Object} ImportBatch
 * @property {string} entityType - 'customer', 'product', 'order', 'inventory'
 * @property {Array<Object>} records - Raw records from the platform
 * @property {number} page - Page number (for progress reporting)
 * @property {boolean} hasMore - Whether more pages are available
 *
 * @typedef {Object} MappedRecord
 * @property {string} entityType
 * @property {string} externalId - Platform-specific ID
 * @property {Object} data - StateSet-format data ready for Commerce.create*()
 * @property {Object} raw - Original platform record (for audit)
 *
 * @typedef {Object} PlatformAdapter
 * @property {string} platformName - e.g., 'shopify', 'woocommerce'
 * @property {() => Promise<boolean>} testConnection - Verify API credentials
 * @property {(entityType: string, record: Object) => MappedRecord} mapToStateSet
 * @property {(entityType: string, statesetRecord: Object) => Object} mapFromStateSet
 * @property {(entityType: string, options?: Object) => AsyncGenerator<ImportBatch>} fetchBatches
 * @property {(eventType: string, payload: Object) => MappedRecord|null} handleWebhook
 */

/**
 * Abstract base class for platform adapters.
 * Subclasses must implement all methods marked with @abstract.
 */
export class BasePlatformAdapter {
  /**
   * @param {string} platformName
   */
  constructor(platformName) {
    if (new.target === BasePlatformAdapter) {
      throw new Error('BasePlatformAdapter is abstract and cannot be instantiated directly');
    }
    this.platformName = platformName;
  }

  /**
   * Test the connection to the external platform.
   * @abstract
   * @returns {Promise<boolean>}
   */
  async testConnection() {
    throw new Error('testConnection() must be implemented by subclass');
  }

  /**
   * Map a single external record to StateSet format.
   * @abstract
   * @param {string} entityType - 'customer', 'product', 'order', 'inventory'
   * @param {Object} record - Raw external platform record
   * @param {Object} [context] - Optional context (e.g., idMap for resolving references)
   * @returns {MappedRecord}
   */
  mapToStateSet(_entityType, record, _context = {}) {
    throw new Error('mapToStateSet() must be implemented by subclass');
  }

  /**
   * Map a StateSet record back to the external platform format.
   * @abstract
   * @param {string} entityType
   * @param {Object} statesetRecord
   * @returns {Object}
   */
  mapFromStateSet(_entityType, _statesetRecord) {
    throw new Error('mapFromStateSet() must be implemented by subclass');
  }

  /**
   * Fetch records from the platform in batches (for API-based imports).
   * @abstract
   * @param {string} entityType
   * @param {Object} [options] - Pagination, date filters, etc.
   * @returns {AsyncGenerator<ImportBatch>}
   */
  async fetchBatches(_entityType, _options = {}) {
    throw new Error('fetchBatches() must be implemented by subclass');
  }

  /**
   * Process an incoming webhook event from the platform.
   * @abstract
   * @param {string} eventType - e.g., 'orders/create', 'products/update'
   * @param {Object} payload - Webhook body
   * @returns {MappedRecord|null}
   */
  handleWebhook(_eventType, _payload) {
    throw new Error('handleWebhook() must be implemented by subclass');
  }

  /**
   * Get the list of entity types this adapter supports.
   * @returns {string[]}
   */
  getSupportedEntities() {
    return ['customers', 'products', 'orders', 'inventory'];
  }

  /**
   * Get the canonical import order (respecting FK dependencies).
   * @returns {string[]}
   */
  getImportOrder() {
    return ['customers', 'products', 'inventory', 'orders'];
  }
}

export default BasePlatformAdapter;
