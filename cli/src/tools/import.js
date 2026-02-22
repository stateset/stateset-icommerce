/**
 * Import Tools Module
 *
 * MCP tool definitions for data import/export operations.
 * Enables importing data from external commerce platforms (Shopify, etc.)
 * into StateSet Commerce.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/**
 * Import tool definitions
 */
export const importTools = [
  {
    name: 'import_shopify_data',
    description:
      'Import data from a Shopify store. Supports API, CSV file, and JSON file sources. Imports customers, products, orders, and inventory in dependency order.',
    inputSchema: {
      source: z
        .enum(['api', 'csv', 'json'])
        .describe(
          'Data source: api (live Shopify API), csv (Shopify CSV export), json (JSON file)',
        ),
      entities: z
        .array(z.enum(['customers', 'products', 'orders', 'inventory']))
        .optional()
        .default(['customers', 'products', 'orders', 'inventory'])
        .describe('Entity types to import'),
      filePath: z.string().optional().describe('File or directory path for csv/json source'),
      incremental: z
        .boolean()
        .optional()
        .default(true)
        .describe('Skip records that already exist (via ID mapping)'),
      dryRun: z.boolean().optional().default(true).describe('Preview import without writing data'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Import Shopify data', {
          source: params.source,
          entities: params.entities,
          incremental: params.incremental,
          filePath: params.filePath || null,
        });
      }

      try {
        const { getAdapter } = await import('../adapters/index.js');
        const { IdMapStore } = await import('../adapters/id-map-store.js');
        const { DataImporter } = await import('../adapters/base-importer.js');

        // Get adapter
        const adapter = await getAdapter('shopify', commerce._shopifyConfig || {});

        // Create ID map store
        const idMapStore = new IdMapStore(commerce.db || commerce._db);

        // Create importer
        const importer = new DataImporter(adapter, commerce, idMapStore);

        // Run import
        const result = await importer.run({
          source: params.source,
          entities: params.entities,
          incremental: params.incremental,
          dryRun: params.dryRun,
          filePath: params.filePath,
        });

        return {
          success: result.success,
          dryRun: result.dryRun,
          durationMs: result.durationMs,
          totalCreated: result.totalCreated,
          totalSkipped: result.totalSkipped,
          totalFailed: result.totalFailed,
          entities: result.entities,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'import_status',
    description: 'Get the status of the most recent import operation.',
    inputSchema: {},
    permission: 'read',
    handler: async () => {
      // The importer stores its last result in-memory; for now return a placeholder
      return {
        success: true,
        message: 'Use import_shopify_data to run an import. Last result is stored per session.',
      };
    },
  },

  {
    name: 'list_id_mappings',
    description:
      'List external ID to StateSet ID mappings for a platform. Useful for verifying imported data.',
    inputSchema: {
      platform: z.string().min(1).describe('Platform name (e.g., "shopify")'),
      entityType: z
        .string()
        .optional()
        .describe('Filter by entity type (customers, products, orders, inventory)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      try {
        const { IdMapStore } = await import('../adapters/id-map-store.js');
        const idMapStore = new IdMapStore(commerce.db || commerce._db);

        const mappings = idMapStore.listByPlatform(params.platform, params.entityType || null);
        return {
          success: true,
          platform: params.platform,
          entityType: params.entityType || 'all',
          count: mappings.length,
          mappings: mappings.slice(0, 100), // Limit response size
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'import_csv',
    description:
      'Import data from a CSV file. Auto-detects Shopify format or uses generic column mapping.',
    inputSchema: {
      filePath: z.string().min(1).describe('Path to CSV file'),
      entityType: z
        .enum(['customers', 'products', 'orders', 'inventory'])
        .describe('Type of data in the CSV'),
      platform: z
        .string()
        .optional()
        .default('shopify')
        .describe('Platform format (shopify, generic)'),
      incremental: z.boolean().optional().default(true),
      dryRun: z.boolean().optional().default(true),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Import CSV data', {
          filePath: params.filePath,
          entityType: params.entityType,
          platform: params.platform,
        });
      }

      try {
        const { getAdapter } = await import('../adapters/index.js');
        const { IdMapStore } = await import('../adapters/id-map-store.js');
        const { DataImporter } = await import('../adapters/base-importer.js');

        const adapter = await getAdapter(params.platform, {});
        const idMapStore = new IdMapStore(commerce.db || commerce._db);
        const importer = new DataImporter(adapter, commerce, idMapStore);

        const result = await importer.run({
          source: 'csv',
          entities: [params.entityType],
          incremental: params.incremental,
          dryRun: params.dryRun,
          filePath: params.filePath,
        });

        return {
          success: result.success,
          dryRun: result.dryRun,
          totalCreated: result.totalCreated,
          totalSkipped: result.totalSkipped,
          totalFailed: result.totalFailed,
          entities: result.entities,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'import_json',
    description: 'Import data from a JSON file (Shopify REST API response format or array).',
    inputSchema: {
      filePath: z.string().min(1).describe('Path to JSON file'),
      entityType: z
        .enum(['customers', 'products', 'orders', 'inventory'])
        .describe('Type of data in the JSON'),
      platform: z.string().optional().default('shopify').describe('Platform format'),
      incremental: z.boolean().optional().default(true),
      dryRun: z.boolean().optional().default(true),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Import JSON data', {
          filePath: params.filePath,
          entityType: params.entityType,
          platform: params.platform,
        });
      }

      try {
        const { getAdapter } = await import('../adapters/index.js');
        const { IdMapStore } = await import('../adapters/id-map-store.js');
        const { DataImporter } = await import('../adapters/base-importer.js');

        const adapter = await getAdapter(params.platform, {});
        const idMapStore = new IdMapStore(commerce.db || commerce._db);
        const importer = new DataImporter(adapter, commerce, idMapStore);

        const result = await importer.run({
          source: 'json',
          entities: [params.entityType],
          incremental: params.incremental,
          dryRun: params.dryRun,
          filePath: params.filePath,
        });

        return {
          success: result.success,
          dryRun: result.dryRun,
          totalCreated: result.totalCreated,
          totalSkipped: result.totalSkipped,
          totalFailed: result.totalFailed,
          entities: result.entities,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'export_data',
    description: 'Export StateSet data to JSON format. Useful for parity testing after imports.',
    inputSchema: {
      entityType: z
        .enum(['customers', 'products', 'orders', 'inventory'])
        .describe('Type of data to export'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      try {
        const { exportToJson } = await import('../adapters/shopify/exporter.js');
        const data = await exportToJson(commerce, params.entityType);
        return {
          success: true,
          entityType: params.entityType,
          count: data.length,
          data,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },
];

export default importTools;
