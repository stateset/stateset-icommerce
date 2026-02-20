/**
 * Adapter Registry for StateSet Commerce
 *
 * Central registry for platform adapters. Enables discovery and instantiation
 * of adapters by platform name.
 */

export { BasePlatformAdapter } from './base-adapter.js';
export { DataImporter } from './base-importer.js';
export { IdMapStore } from './id-map-store.js';

/**
 * Registry of available platform adapters.
 * @type {Map<string, () => Promise<import('./base-adapter.js').BasePlatformAdapter>>}
 */
const adapterFactories = new Map();

/**
 * Register a platform adapter factory.
 * @param {string} platformName
 * @param {() => Promise<BasePlatformAdapter>} factory
 */
export function registerAdapter(platformName, factory) {
  adapterFactories.set(platformName.toLowerCase(), factory);
}

/**
 * Get a registered adapter by platform name.
 * @param {string} platformName
 * @param {Object} config - Platform-specific configuration
 * @returns {Promise<BasePlatformAdapter>}
 */
export async function getAdapter(platformName, config = {}) {
  const factory = adapterFactories.get(platformName.toLowerCase());
  if (!factory) {
    throw new Error(
      `No adapter registered for platform "${platformName}". Available: ${listAdapters().join(', ')}`,
    );
  }
  return factory(config);
}

/**
 * List all registered adapter platform names.
 * @returns {string[]}
 */
export function listAdapters() {
  return Array.from(adapterFactories.keys());
}

// Register built-in adapters
registerAdapter('shopify', async (config) => {
  const { ShopifyAdapter } = await import('./shopify/index.js');
  return new ShopifyAdapter(config);
});
