/**
 * Shopify-specific Data Importer
 *
 * Extends the base DataImporter with Shopify-specific file parsing
 * and API integration.
 */

import { DataImporter } from '../base-importer.js';
import { ShopifyAdapter } from './index.js';
import fs from 'fs';
import path from 'path';

/**
 * Shopify importer — convenience wrapper around DataImporter + ShopifyAdapter.
 */
export class ShopifyImporter extends DataImporter {
  /**
   * @param {Object} commerce - StateSet Commerce instance
   * @param {import('../id-map-store.js').IdMapStore} idMapStore
   * @param {Object} [shopifyConfig] - { shopDomain, accessToken }
   */
  constructor(commerce, idMapStore, shopifyConfig = {}) {
    const adapter = new ShopifyAdapter(shopifyConfig);
    super(adapter, commerce, idMapStore);
  }

  /**
   * Import from a directory of Shopify CSV export files.
   * Looks for customers_export.csv, products_export.csv, orders_export.csv.
   *
   * @param {string} directory
   * @param {Object} [options]
   * @returns {Promise<import('../base-importer.js').ImportResult>}
   */
  async importFromCsvExport(directory, options = {}) {
    const csvFiles = {
      customers: this._findFile(directory, ['customers_export.csv', 'customers.csv']),
      products: this._findFile(directory, ['products_export.csv', 'products.csv']),
      orders: this._findFile(directory, ['orders_export.csv', 'orders.csv']),
    };

    const entities = Object.entries(csvFiles)
      .filter(([, filePath]) => filePath !== null)
      .map(([entityType]) => entityType);

    if (entities.length === 0) {
      return {
        success: false,
        entities: {},
        durationMs: 0,
        totalCreated: 0,
        totalSkipped: 0,
        totalFailed: 0,
        dryRun: options.dryRun || false,
      };
    }

    // Store file paths for the adapter to use
    this._csvFilePaths = csvFiles;

    return this.run({
      source: 'csv',
      entities,
      ...options,
    });
  }

  /**
   * Import from a directory of Shopify JSON API response files.
   *
   * @param {string} directory
   * @param {Object} [options]
   * @returns {Promise<import('../base-importer.js').ImportResult>}
   */
  async importFromJsonDump(directory, options = {}) {
    const jsonFiles = {
      customers: this._findFile(directory, ['customers.json']),
      products: this._findFile(directory, ['products.json']),
      orders: this._findFile(directory, ['orders.json']),
    };

    const entities = Object.entries(jsonFiles)
      .filter(([, filePath]) => filePath !== null)
      .map(([entityType]) => entityType);

    if (entities.length === 0) {
      return {
        success: false,
        entities: {},
        durationMs: 0,
        totalCreated: 0,
        totalSkipped: 0,
        totalFailed: 0,
        dryRun: options.dryRun || false,
      };
    }

    this._jsonFilePaths = jsonFiles;

    return this.run({
      source: 'json',
      entities,
      ...options,
    });
  }

  /**
   * Find a file in a directory by trying multiple names.
   * @private
   */
  _findFile(directory, fileNames) {
    for (const name of fileNames) {
      const filePath = path.join(directory, name);
      if (fs.existsSync(filePath)) return filePath;
    }
    return null;
  }
}

export default ShopifyImporter;
