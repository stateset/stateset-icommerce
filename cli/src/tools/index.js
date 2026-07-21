/**
 * Tool Registry
 *
 * Central registry for all MCP tools, enabling modular tool loading
 * and lazy initialization for performance.
 */

import { customerTools } from './customers.js';
import { orderTools } from './orders.js';
import { customObjectTools } from './custom-objects.js';
import { vectorTools } from './vector.js';

function namedLoader(modulePath, exportName) {
  return () => import(modulePath).then((mod) => mod[exportName]);
}

/**
 * Tool categories for lazy loading
 */
const TOOL_MODULES = {
  customers: () => customerTools,
  orders: () => orderTools,
  'custom-objects': () => customObjectTools,
  vector: () => vectorTools,
  // Additional categories loaded on demand
  products: () => import('./products.js').then((m) => m.default),
  inventory: () => import('./inventory.js').then((m) => m.default),
  returns: () => import('./returns.js').then((m) => m.default),
  carts: () => import('./carts.js').then((m) => m.default),
  analytics: () => import('./analytics.js').then((m) => m.default),
  currency: () => import('./currency.js').then((m) => m.default),
  tax: () => import('./tax.js').then((m) => m.default),
  promotions: () => import('./promotions.js').then((m) => m.default),
  subscriptions: () => import('./subscriptions.js').then((m) => m.default),
  sync: () => import('./sync.js').then((m) => m.default),
  manufacturing: () => import('./manufacturing.js').then((m) => m.default),
  payments: () => import('./payments.js').then((m) => m.default),
  stablecoin: () => import('./stablecoin.js').then((m) => m.default),
  treasury: () => import('./treasury.js').then((m) => m.default),
  erc8004: () => import('./erc8004.js').then((m) => m.default),
  x402: () => import('./x402.js').then((m) => m.default),
  'agent-cards': () => import('./agent-cards.js').then((m) => m.default),
  a2a: () => import('./a2a.js').then((m) => m.default),
  shipments: () => import('./shipments.js').then((m) => m.default),
  suppliers: () => import('./suppliers.js').then((m) => m.default),
  invoices: () => import('./invoices.js').then((m) => m.default),
  warranties: () => import('./warranties.js').then((m) => m.default),
  connectors: () => import('./connectors.js').then((m) => m.default),
  proofs: () => import('./proofs.js').then((m) => m.default),
  'circuit-breaker': () => import('./circuit-breaker.js').then((m) => m.default),
  checkout: () => import('./checkout.js').then((m) => m.default),
  compliance: () => import('./compliance.js').then((m) => m.default),
  catalog: () => import('./catalog.js').then((m) => m.default),
  'agent-runtime': namedLoader('./agent-runtime.js', 'agentRuntimeTools'),
  import: namedLoader('./import.js', 'importTools'),
  policies: namedLoader('./policies.js', 'policyTools'),
  'gift-cards': namedLoader('./gift-cards.js', 'giftCardTools'),
  'store-credits': namedLoader('./store-credits.js', 'storeCreditTools'),
  segments: namedLoader('./segments.js', 'segmentTools'),
  'shipping-zones': namedLoader('./shipping-zones.js', 'shippingZoneTools'),
  'units-of-measure': namedLoader('./units-of-measure.js', 'unitOfMeasureTools'),
  'stock-snapshots': namedLoader('./stock-snapshots.js', 'stockSnapshotTools'),
  'print-stations': namedLoader('./print-stations.js', 'printStationTools'),
  'integration-mappings': namedLoader('./integration-mappings.js', 'integrationMappingTools'),
  'integration-field-mappings': namedLoader(
    './integration-field-mappings.js',
    'integrationFieldMappingTools',
  ),
  'payment-obligations': namedLoader('./payment-obligations.js', 'paymentObligationTools'),
  purgatory: namedLoader('./purgatory.js', 'purgatoryTools'),
  'topology-snapshots': namedLoader('./topology-snapshots.js', 'topologySnapshotTools'),
  'vendor-returns': namedLoader('./vendor-returns.js', 'vendorReturnTools'),
  reviews: namedLoader('./reviews.js', 'reviewTools'),
  wishlists: namedLoader('./wishlists.js', 'wishlistTools'),
  loyalty: namedLoader('./loyalty.js', 'loyaltyTools'),
  fraud: namedLoader('./fraud.js', 'fraudTools'),
  audit: namedLoader('./audit.js', 'auditTools'),
  quality: namedLoader('./quality.js', 'qualityTools'),
  lots: namedLoader('./lots.js', 'lotTools'),
  'search-config': namedLoader('./search-config.js', 'searchConfigTools'),
  serials: namedLoader('./serials.js', 'serialTools'),
  warehouse: namedLoader('./warehouse.js', 'warehouseTools'),
  receiving: namedLoader('./receiving.js', 'receivingTools'),
  fulfillment: namedLoader('./fulfillment.js', 'fulfillmentTools'),
  'accounts-payable': namedLoader('./accounts-payable.js', 'accountsPayableTools'),
  'accounts-receivable': namedLoader('./accounts-receivable.js', 'accountsReceivableTools'),
  'cost-accounting': namedLoader('./cost-accounting.js', 'costAccountingTools'),
  credit: namedLoader('./credit.js', 'creditTools'),
  backorders: namedLoader('./backorders.js', 'backorderTools'),
  'general-ledger': namedLoader('./general-ledger.js', 'generalLedgerTools'),
  'a2a-automation': namedLoader('./a2a-automation.js', 'a2aAutomationTools'),
  'a2a-observability': namedLoader('./a2a-observability.js', 'a2aObservabilityTools'),
  'a2a-platform': namedLoader('./a2a-platform.js', 'a2aPlatformTools'),
  'a2a-intelligence': namedLoader('./a2a-intelligence.js', 'a2aIntelligenceTools'),
  'agent-receipt': namedLoader('./agent-receipt.js', 'agentReceiptTools'),
  'fixed-assets': namedLoader('./fixed-assets.js', 'fixedAssetTools'),
  maintenance: namedLoader('./maintenance.js', 'maintenanceTools'),
  'revenue-recognition': namedLoader('./revenue-recognition.js', 'revenueRecognitionTools'),
  'cycle-counts': namedLoader('./cycle-counts.js', 'cycleCountTools'),
  'edi-documents': namedLoader('./edi-documents.js', 'ediDocumentTools'),
  prepayments: namedLoader('./prepayments.js', 'prepaymentTools'),
  'activity-logs': namedLoader('./activity-logs.js', 'activityLogTools'),
  channels: namedLoader('./channels.js', 'channelTools'),
  companies: namedLoader('./companies.js', 'companyTools'),
  'vendor-credits': namedLoader('./vendor-credits.js', 'vendorCreditTools'),
  'price-schedules': namedLoader('./price-schedules.js', 'priceScheduleTools'),
  'price-levels': namedLoader('./price-levels.js', 'priceLevelTools'),
  'transfer-orders': namedLoader('./transfer-orders.js', 'transferOrderTools'),
  'production-batches': namedLoader('./production-batches.js', 'productionBatchTools'),
  'supplier-skus': namedLoader('./supplier-skus.js', 'supplierSkuTools'),
  'inbound-shipments': namedLoader('./inbound-shipments.js', 'inboundShipmentTools'),
  'agentic-runtime': () => import('../mcp-server.js').then((m) => m.getStaticAgenticRuntimeTools()),
};

const ALL_TOOL_CATEGORIES = Object.freeze(Object.keys(TOOL_MODULES));

/**
 * ToolRegistry - Manages tool loading and access
 */
export class ToolRegistry {
  constructor() {
    this.tools = new Map();
    this.loadedCategories = new Set();
  }

  /**
   * Load tools for a specific category
   */
  async loadCategory(category) {
    if (this.loadedCategories.has(category)) {
      return;
    }

    const loader = TOOL_MODULES[category];
    if (!loader) {
      throw new Error(`Unknown tool category: ${category}`);
    }

    const tools = await Promise.resolve(loader());
    for (const tool of tools) {
      this.tools.set(tool.name, { ...tool, category });
    }

    this.loadedCategories.add(category);
  }

  /**
   * Load all tools (for full server)
   */
  async loadAll() {
    await Promise.all(Object.keys(TOOL_MODULES).map((cat) => this.loadCategory(cat)));
  }

  /**
   * Load tools for a specific agent
   */
  async loadForAgent(agentName) {
    const agentCategories = AGENT_TOOL_CATEGORIES[agentName];
    if (agentCategories) {
      await Promise.all(agentCategories.map((cat) => this.loadCategory(cat)));
    }
  }

  /**
   * Get a tool by name
   */
  get(name) {
    return this.tools.get(name);
  }

  /**
   * Get all loaded tools
   */
  getAll() {
    return Array.from(this.tools.values());
  }

  /**
   * Get tools by category
   */
  getByCategory(category) {
    return this.getAll().filter((t) => t.category === category);
  }

  /**
   * Get tools by permission level
   */
  getByPermission(permission) {
    return this.getAll().filter((t) => t.permission === permission);
  }

  /**
   * Get read-only tools
   */
  getReadOnly() {
    return this.getByPermission('read');
  }

  /**
   * Get write tools (require --apply)
   */
  getWriteTools() {
    return this.getAll().filter((t) => ['write', 'delete', 'admin'].includes(t.permission));
  }

  /**
   * Check if a tool is loaded
   */
  has(name) {
    return this.tools.has(name);
  }

  /**
   * Get tool count
   */
  get size() {
    return this.tools.size;
  }

  /**
   * Convert to MCP server format
   */
  toMcpFormat(context) {
    return this.getAll().map((tool) => ({
      name: tool.name,
      description: tool.description,
      inputSchema: tool.inputSchema,
      handler: async (params) => {
        try {
          const result = await tool.handler({
            ...context,
            params,
          });
          return {
            content: [
              {
                type: 'text',
                text: JSON.stringify(result, null, 2),
              },
            ],
          };
        } catch (error) {
          return {
            content: [
              {
                type: 'text',
                text: JSON.stringify({ error: error.message }),
              },
            ],
          };
        }
      },
    }));
  }
}

/**
 * Agent to tool category mappings
 */
export const AGENT_TOOL_CATEGORIES = {
  'customer-service': [...ALL_TOOL_CATEGORIES],
  checkout: [
    'carts',
    'customers',
    'products',
    'inventory',
    'promotions',
    'tax',
    'currency',
    'payments',
    'vector',
  ],
  orders: ['orders', 'customers', 'inventory', 'shipments', 'payments', 'vector'],
  inventory: ['inventory', 'products', 'manufacturing', 'suppliers', 'vector'],
  returns: ['returns', 'orders', 'customers', 'inventory', 'warranties'],
  analytics: ['analytics', 'vector'],
  promotions: ['promotions', 'products', 'carts', 'vector'],
  subscriptions: ['subscriptions', 'customers', 'payments'],
  manufacturing: ['manufacturing', 'inventory', 'products'],
  payments: ['payments', 'orders', 'customers'],
  shipments: ['shipments', 'orders', 'inventory'],
  suppliers: ['suppliers', 'inventory', 'products', 'analytics'],
  invoices: ['invoices', 'customers', 'orders'],
  warranties: ['warranties', 'products', 'orders'],
  currency: ['currency'],
  tax: ['tax', 'currency'],
  sync: ['sync', 'proofs'],
  stablecoin: ['stablecoin'],
  agents: ['agent-runtime', 'agent-cards', 'a2a', 'x402'],
  storefront: [],
  x402: ['x402', 'agent-cards', 'a2a'],
  'agent-cards': ['agent-cards', 'a2a'],
  a2a: ['a2a', 'agent-cards', 'x402'],
  vector: ['vector', 'products', 'customers'],
};

/**
 * Create a tool registry
 */
export function createToolRegistry() {
  return new ToolRegistry();
}

/**
 * Get tools for an agent (convenience function)
 */
export async function getToolsForAgent(agentName) {
  const registry = createToolRegistry();
  await registry.loadForAgent(agentName);
  return registry.getAll();
}

/**
 * Pre-built tool arrays for immediate use
 * (for backwards compatibility)
 */
export const immediateTools = {
  customers: customerTools,
  orders: orderTools,
  'custom-objects': customObjectTools,
  vector: vectorTools,
};

export default {
  ToolRegistry,
  createToolRegistry,
  getToolsForAgent,
  AGENT_TOOL_CATEGORIES,
  immediateTools,
};
