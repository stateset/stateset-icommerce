/**
 * Data Import Framework for StateSet Commerce
 *
 * Reusable orchestrator that takes a PlatformAdapter and a commerce instance,
 * then imports data in the correct dependency order with progress tracking,
 * incremental support (via IdMapStore), and dry-run mode.
 *
 * @typedef {Object} ImportOptions
 * @property {string} source - 'api' | 'csv' | 'json'
 * @property {string[]} [entities] - Entities to import (default: all in order)
 * @property {boolean} [incremental=true] - Skip records that already exist
 * @property {boolean} [dryRun=false] - Preview without writing
 * @property {number} [batchSize=50] - Records per batch
 * @property {(progress: ImportProgress) => void} [onProgress] - Progress callback
 * @property {string} [filePath] - File/directory path for csv/json source
 * @property {Object} [apiOptions] - Additional options for API fetching
 *
 * @typedef {Object} ImportProgress
 * @property {string} entity - Entity type being imported
 * @property {number} processed - Records processed so far
 * @property {number} created - Records created
 * @property {number} skipped - Records skipped (already exist)
 * @property {number} failed - Records that failed
 * @property {Array<{externalId: string, error: string}>} errors
 * @property {string} phase - 'fetching' | 'mapping' | 'writing' | 'complete'
 *
 * @typedef {Object} ImportResult
 * @property {boolean} success
 * @property {Object<string, ImportProgress>} entities
 * @property {number} durationMs
 * @property {number} totalCreated
 * @property {number} totalSkipped
 * @property {number} totalFailed
 * @property {boolean} dryRun
 */

import { EventEmitter } from 'events';

/**
 * Generic data importer that works with any PlatformAdapter.
 */
export class DataImporter extends EventEmitter {
  /**
   * @param {import('./base-adapter.js').BasePlatformAdapter} adapter
   * @param {Object} commerce - StateSet Commerce instance
   * @param {import('./id-map-store.js').IdMapStore} idMapStore
   * @param {Object} [options]
   */
  constructor(adapter, commerce, idMapStore, options = {}) {
    super();

    if (!adapter) throw new Error('DataImporter requires a platform adapter');
    if (!commerce) throw new Error('DataImporter requires a commerce instance');
    if (!idMapStore) throw new Error('DataImporter requires an IdMapStore');

    this.adapter = adapter;
    this.commerce = commerce;
    this.idMapStore = idMapStore;
    this.options = options;

    /** @type {ImportResult | null} */
    this.lastResult = null;
  }

  /**
   * Run a full import.
   * @param {ImportOptions} importOptions
   * @returns {Promise<ImportResult>}
   */
  async run(importOptions) {
    const {
      source = 'json',
      entities = this.adapter.getImportOrder(),
      incremental = true,
      dryRun = false,
      batchSize = 50,
      onProgress = null,
      filePath = null,
      apiOptions = {},
    } = importOptions;

    const startTime = Date.now();
    const result = {
      success: true,
      entities: {},
      durationMs: 0,
      totalCreated: 0,
      totalSkipped: 0,
      totalFailed: 0,
      dryRun,
    };

    // Enforce import order (FK dependencies)
    const importOrder = this.adapter.getImportOrder();
    const sortedEntities = entities
      .slice()
      .sort((a, b) => importOrder.indexOf(a) - importOrder.indexOf(b));

    for (const entityType of sortedEntities) {
      const progress = {
        entity: entityType,
        processed: 0,
        created: 0,
        skipped: 0,
        failed: 0,
        errors: [],
        phase: 'fetching',
      };

      result.entities[entityType] = progress;
      this.emit('entity:start', { entityType });

      try {
        const batches = this._getBatches(source, entityType, {
          batchSize,
          filePath,
          apiOptions,
        });

        for await (const batch of batches) {
          progress.phase = 'mapping';

          for (const record of batch.records) {
            progress.processed++;

            try {
              const mapped = this.adapter.mapToStateSet(entityType, record, {
                idMap: this.idMapStore,
                platform: this.adapter.platformName,
              });

              if (!mapped || !mapped.externalId) {
                progress.failed++;
                progress.errors.push({
                  externalId: 'unknown',
                  error: 'Mapper returned null or missing externalId',
                });
                continue;
              }

              // Check if already imported
              if (incremental) {
                const existing = this.idMapStore.lookup(
                  this.adapter.platformName,
                  entityType,
                  mapped.externalId,
                );
                if (existing) {
                  progress.skipped++;
                  continue;
                }
              }

              if (dryRun) {
                progress.created++;
                continue;
              }

              // Write to commerce
              progress.phase = 'writing';
              const statesetId = await this._createEntity(entityType, mapped.data);

              // Record the mapping
              this.idMapStore.store(
                this.adapter.platformName,
                entityType,
                mapped.externalId,
                statesetId,
                mapped.raw,
              );

              progress.created++;
            } catch (err) {
              progress.failed++;
              const externalId = record.id || record.Id || record.ID || String(progress.processed);
              progress.errors.push({
                externalId: String(externalId),
                error: err.message,
              });
            }
          }

          if (onProgress) {
            onProgress({ ...progress });
          }
          this.emit('batch:complete', { entityType, progress: { ...progress } });
        }
      } catch (err) {
        progress.failed++;
        progress.errors.push({ externalId: 'batch', error: err.message });
        result.success = false;
      }

      progress.phase = 'complete';
      result.totalCreated += progress.created;
      result.totalSkipped += progress.skipped;
      result.totalFailed += progress.failed;

      if (onProgress) {
        onProgress({ ...progress });
      }
      this.emit('entity:complete', { entityType, progress: { ...progress } });
    }

    result.durationMs = Date.now() - startTime;
    if (result.totalFailed > 0 && result.totalCreated === 0) {
      result.success = false;
    }

    this.lastResult = result;
    this.emit('import:complete', result);
    return result;
  }

  /**
   * Get the last import result.
   * @returns {ImportResult | null}
   */
  getLastResult() {
    return this.lastResult;
  }

  /**
   * Resolve batch source — API, CSV file, or JSON file.
   * @private
   */
  async *_getBatches(source, entityType, options) {
    if (source === 'api') {
      yield* this.adapter.fetchBatches(entityType, options.apiOptions);
    } else if (source === 'csv' || source === 'json') {
      // For file-based sources, the adapter must provide a file parser method
      if (typeof this.adapter.parseBatchesFromFile === 'function') {
        yield* this.adapter.parseBatchesFromFile(
          entityType,
          options.filePath,
          source,
          options.batchSize,
        );
      } else {
        throw new Error(
          `Adapter "${this.adapter.platformName}" does not support ${source} file imports`,
        );
      }
    } else {
      throw new Error(`Unknown import source: ${source}`);
    }
  }

  /**
   * Create an entity using the Commerce class.
   * @private
   * @param {string} entityType
   * @param {Object} data
   * @returns {Promise<string>} The StateSet ID of the created entity
   */
  async _createEntity(entityType, data) {
    switch (entityType) {
      case 'customers': {
        const customer = await this.commerce.customers.create(data);
        return customer.id || customer.customer_id;
      }
      case 'products': {
        const product = await this.commerce.products.create(data);
        return product.id || product.product_id;
      }
      case 'orders': {
        const order = await this.commerce.orders.create(data);
        return order.id || order.order_id;
      }
      case 'inventory': {
        const item = await this.commerce.inventory.create(data);
        return item.id || item.inventory_id || data.sku;
      }
      default:
        throw new Error(`Unknown entity type: ${entityType}`);
    }
  }
}

export default DataImporter;
