/**
 * Import Commands Module
 */

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

let lastImportStatus = null;

function recordImportStatus(platform, result) {
  lastImportStatus = {
    platform,
    timestamp: new Date().toISOString(),
    result,
  };
}

export async function execute(action, args, { commerce, jsonOutput }) {
  switch (action) {
    case 'shopify': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: import shopify <payloadJson>');
      const { getAdapter } = await import('../adapters/index.js');
      const { IdMapStore } = await import('../adapters/id-map-store.js');
      const { DataImporter } = await import('../adapters/base-importer.js');
      const payload = parseJsonArg(payloadJson, 'payload');
      const adapter = await getAdapter('shopify', commerce._shopifyConfig || {});
      const idMapStore = new IdMapStore(commerce.db || commerce._db);
      const importer = new DataImporter(adapter, commerce, idMapStore);
      const result = await importer.run(payload);
      recordImportStatus('shopify', result);
      return jsonOutput
        ? result
        : { result, formatted: `Shopify import complete: ${result.totalCreated || 0} created` };
    }

    case 'shopify-shadow': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: import shopify-shadow <payloadJson>');
      const { getAdapter } = await import('../adapters/index.js');
      const { IdMapStore } = await import('../adapters/id-map-store.js');
      const { DataImporter } = await import('../adapters/base-importer.js');
      const payload = parseJsonArg(payloadJson, 'payload');
      const adapter = await getAdapter('shopify-shadow', commerce._shopifyConfig || {});
      const idMapStore = new IdMapStore(commerce.db || commerce._db);
      const importer = new DataImporter(adapter, commerce, idMapStore);
      const result = await importer.run(payload);
      recordImportStatus('shopify-shadow', result);
      return jsonOutput ? result : { result, formatted: `Shopify shadow import complete` };
    }

    case 'status': {
      if (!lastImportStatus) {
        return jsonOutput
          ? { hasResult: false }
          : { formatted: 'No import has been run in this session.' };
      }
      return jsonOutput
        ? lastImportStatus
        : {
            lastImportStatus,
            formatted: `Last import: ${lastImportStatus.platform} at ${lastImportStatus.timestamp}`,
          };
    }

    case 'mappings': {
      const [platform, entityType] = args;
      if (!platform) throw new Error('Usage: import mappings <platform> [entityType]');
      const { IdMapStore } = await import('../adapters/id-map-store.js');
      const idMapStore = new IdMapStore(commerce.db || commerce._db);
      const mappings = idMapStore.listByPlatform(platform, entityType || null);
      return jsonOutput ? mappings : { mappings, formatted: `ID mappings: ${mappings.length}` };
    }

    case 'csv': {
      const [filePath, entityType, platform = 'shopify', incrementalRaw, dryRunRaw] = args;
      if (!filePath || !entityType)
        throw new Error(
          'Usage: import csv <filePath> <entityType> [platform] [incremental] [dryRun]',
        );
      const { getAdapter } = await import('../adapters/index.js');
      const { IdMapStore } = await import('../adapters/id-map-store.js');
      const { DataImporter } = await import('../adapters/base-importer.js');
      const adapter = await getAdapter(platform, {});
      const idMapStore = new IdMapStore(commerce.db || commerce._db);
      const importer = new DataImporter(adapter, commerce, idMapStore);
      const result = await importer.run({
        source: 'csv',
        entities: [entityType],
        incremental:
          incrementalRaw === undefined
            ? true
            : ['true', '1', 'yes', 'y'].includes(String(incrementalRaw).toLowerCase()),
        dryRun:
          dryRunRaw === undefined
            ? true
            : ['true', '1', 'yes', 'y'].includes(String(dryRunRaw).toLowerCase()),
        filePath,
      });
      recordImportStatus(platform, result);
      return jsonOutput ? result : { result, formatted: `CSV import complete for ${entityType}` };
    }

    case 'json': {
      const [filePath, entityType, platform = 'shopify', incrementalRaw, dryRunRaw] = args;
      if (!filePath || !entityType)
        throw new Error(
          'Usage: import json <filePath> <entityType> [platform] [incremental] [dryRun]',
        );
      const { getAdapter } = await import('../adapters/index.js');
      const { IdMapStore } = await import('../adapters/id-map-store.js');
      const { DataImporter } = await import('../adapters/base-importer.js');
      const adapter = await getAdapter(platform, {});
      const idMapStore = new IdMapStore(commerce.db || commerce._db);
      const importer = new DataImporter(adapter, commerce, idMapStore);
      const result = await importer.run({
        source: 'json',
        entities: [entityType],
        incremental:
          incrementalRaw === undefined
            ? true
            : ['true', '1', 'yes', 'y'].includes(String(incrementalRaw).toLowerCase()),
        dryRun:
          dryRunRaw === undefined
            ? true
            : ['true', '1', 'yes', 'y'].includes(String(dryRunRaw).toLowerCase()),
        filePath,
      });
      recordImportStatus(platform, result);
      return jsonOutput ? result : { result, formatted: `JSON import complete for ${entityType}` };
    }

    case 'export': {
      const entityType = args[0];
      if (!entityType) throw new Error('Usage: import export <entityType>');
      const { exportToJson } = await import('../adapters/shopify/exporter.js');
      const data = await exportToJson(commerce, entityType);
      return jsonOutput ? data : { data, formatted: `Exported ${data.length} ${entityType}` };
    }

    case 'woocommerce': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: import woocommerce <payloadJson>');
      const { getAdapter } = await import('../adapters/index.js');
      const { IdMapStore } = await import('../adapters/id-map-store.js');
      const { DataImporter } = await import('../adapters/base-importer.js');
      const payload = parseJsonArg(payloadJson, 'payload');
      const adapter = await getAdapter('woocommerce', {
        siteUrl: payload.siteUrl,
        consumerKey: payload.consumerKey,
        consumerSecret: payload.consumerSecret,
      });
      const idMapStore = new IdMapStore(commerce.db || commerce._db);
      const importer = new DataImporter(adapter, commerce, idMapStore);
      const result = await importer.run({
        source: 'api',
        entities: payload.entities,
        incremental: payload.incremental,
        dryRun: payload.dryRun,
      });
      recordImportStatus('woocommerce', result);
      return jsonOutput ? result : { result, formatted: 'WooCommerce import complete' };
    }

    case 'stripe-webhooks': {
      const [webhookSecret, portRaw] = args;
      if (!webhookSecret) throw new Error('Usage: import stripe-webhooks <webhookSecret> [port]');
      const { getStripeSourceTemplate, WebhookSource } = await import('../webhooks/server.js');
      const template = await getStripeSourceTemplate();
      const source = new WebhookSource({ ...template, secret: webhookSecret });
      const result = {
        source: {
          name: source.name,
          path: source.path,
          signatureHeader: source.signatureHeader,
          eventTypeField: source.eventTypeField,
        },
        port: portRaw ? Number.parseInt(portRaw, 10) : 3000,
      };
      return jsonOutput
        ? result
        : { result, formatted: `Configured Stripe webhooks on ${source.path}` };
    }

    case 'woocommerce-webhooks': {
      const [webhookSecret, portRaw] = args;
      if (!webhookSecret)
        throw new Error('Usage: import woocommerce-webhooks <webhookSecret> [port]');
      const { WebhookSourceTemplates, WebhookSource } = await import('../webhooks/server.js');
      const template = WebhookSourceTemplates.woocommerce;
      const source = new WebhookSource({ ...template, secret: webhookSecret });
      const result = {
        source: {
          name: source.name,
          path: source.path,
          signatureHeader: source.signatureHeader,
          eventTypeField: source.eventTypeField,
        },
        port: portRaw ? Number.parseInt(portRaw, 10) : 3000,
      };
      return jsonOutput
        ? result
        : { result, formatted: `Configured WooCommerce webhooks on ${source.path}` };
    }

    default:
      throw new Error(
        `Unknown action: import ${action}\n\n` +
          'Available actions:\n' +
          '  shopify <payloadJson>                 Import Shopify data\n' +
          '  shopify-shadow <payloadJson>          Run Shopify shadow import\n' +
          '  status                               Get import status\n' +
          '  mappings <platform> [entityType]     List ID mappings\n' +
          '  csv <filePath> <entityType> [platform] [incremental] [dryRun]   Import CSV\n' +
          '  json <filePath> <entityType> [platform] [incremental] [dryRun]  Import JSON\n' +
          '  export <entityType>                  Export data\n' +
          '  woocommerce <payloadJson>            Import WooCommerce data\n' +
          '  stripe-webhooks <webhookSecret> [port]       Configure Stripe webhooks\n' +
          '  woocommerce-webhooks <webhookSecret> [port]  Configure WooCommerce webhooks',
      );
  }
}

export const metadata = {
  name: 'import',
  aliases: ['ingest', 'etl'],
  description: 'Import, export, and webhook-setup commands',
  actions: {
    shopify: { description: 'Import Shopify data', args: ['<payloadJson>'] },
    'shopify-shadow': { description: 'Run Shopify shadow import', args: ['<payloadJson>'] },
    status: { description: 'Get import status', args: [] },
    mappings: { description: 'List ID mappings', args: ['<platform>', '[entityType]'] },
    csv: {
      description: 'Import CSV',
      args: ['<filePath>', '<entityType>', '[platform]', '[incremental]', '[dryRun]'],
    },
    json: {
      description: 'Import JSON',
      args: ['<filePath>', '<entityType>', '[platform]', '[incremental]', '[dryRun]'],
    },
    export: { description: 'Export data', args: ['<entityType>'] },
    woocommerce: { description: 'Import WooCommerce data', args: ['<payloadJson>'] },
    'stripe-webhooks': {
      description: 'Configure Stripe webhooks',
      args: ['<webhookSecret>', '[port]'],
    },
    'woocommerce-webhooks': {
      description: 'Configure WooCommerce webhooks',
      args: ['<webhookSecret>', '[port]'],
    },
  },
};

export default { execute, metadata };
