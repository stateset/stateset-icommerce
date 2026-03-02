/**
 * Agent Catalog MCP Tools
 *
 * Tool definitions for the machine-readable agent catalog.
 * Enables AI agents to publish, discover, query, and match
 * products and services through structured MCP tool calls.
 */

import { z } from 'zod';

let _catalogSvc = null;

/**
 * Lazy-initialize the catalog service (singleton).
 * @returns {Promise<object>}
 */
async function getCatalogSvc() {
  if (_catalogSvc) return _catalogSvc;
  const { A2AStore } = await import('../a2a/store.js');
  const { createAgentCatalog } = await import('../catalog/agent-catalog.js');
  const store = new A2AStore();
  store.init();
  _catalogSvc = createAgentCatalog(store);
  return _catalogSvc;
}

export const catalogTools = [
  // ==========================================================================
  // Publish Product
  // ==========================================================================
  {
    name: 'publish_product_catalog',
    description:
      'Publish a product to the machine-readable agent catalog. Makes products discoverable by AI agents with capability-based matching, trust levels, and machine-readable specs.',
    inputSchema: {
      productId: z.string().min(1).describe('Product ID'),
      name: z.string().min(1).describe('Product name'),
      description: z.string().optional().describe('Product description'),
      capabilities: z
        .array(z.string())
        .min(1)
        .describe('Required agent capabilities (e.g., ["buy", "fulfill", "ship"])'),
      agentRequirements: z
        .record(z.unknown())
        .optional()
        .describe('Agent requirements as JSON Schema'),
      fulfillmentAgents: z
        .array(z.string())
        .optional()
        .describe('Agent addresses that can fulfill this product'),
      fulfillmentChains: z
        .array(z.string())
        .optional()
        .describe('Blockchain networks for settlement (e.g., ["set_chain", "base"])'),
      minTrustLevel: z
        .enum(['sandbox', 'verified', 'enterprise', 'admin'])
        .default('sandbox')
        .describe('Minimum agent trust level required'),
      maxPrice: z.number().nonnegative().optional().describe('Maximum price'),
      currency: z.string().default('USD').describe('Currency code'),
      machineSpec: z
        .record(z.unknown())
        .optional()
        .describe('Machine-readable product spec (JSON Schema)'),
      tags: z.array(z.string()).optional().describe('Searchable tags'),
      category: z.string().optional().describe('Product category'),
    },
    permission: 'write',
    handler: async ({ params }) => {
      try {
        const svc = await getCatalogSvc();
        const result = svc.publishProduct(params);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // Query Catalog
  // ==========================================================================
  {
    name: 'query_agent_catalog',
    description:
      'Query the agent catalog for products matching filters. Supports capability, trust level, price, fulfillment chain, and category filtering.',
    inputSchema: {
      capability: z
        .string()
        .optional()
        .describe('Filter by required capability (e.g., "buy", "fulfill")'),
      agentTrustLevel: z
        .enum(['sandbox', 'verified', 'enterprise', 'admin'])
        .optional()
        .describe('Agent trust level — returns products the agent can access'),
      maxPrice: z.number().nonnegative().optional().describe('Maximum price filter'),
      fulfillmentChain: z
        .string()
        .optional()
        .describe('Filter by fulfillment chain (e.g., "set_chain")'),
      category: z.string().optional().describe('Filter by category'),
      status: z
        .enum(['active', 'delisted'])
        .optional()
        .describe('Filter by status (default: active)'),
      limit: z.number().int().min(1).max(1000).optional().default(100).describe('Max results'),
      offset: z.number().int().min(0).optional().default(0).describe('Pagination offset'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const svc = await getCatalogSvc();
        const result = svc.queryProducts(params);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // Get Product Spec
  // ==========================================================================
  {
    name: 'get_product_spec',
    description:
      'Get the full machine-readable spec for a catalog product. Returns capabilities, requirements, pricing, trust level, and a JSON Schema fragment.',
    inputSchema: {
      identifier: z.string().min(1).describe('Product ID or catalog entry ID'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const svc = await getCatalogSvc();
        const result = svc.getProductSpec(params.identifier);
        if (!result) {
          return { success: false, error: 'Product not found' };
        }
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // Match Agent to Products
  // ==========================================================================
  {
    name: 'match_agent_to_products',
    description:
      'Find catalog products compatible with an agent based on its capabilities and trust level. Returns products sorted by relevance (capability overlap).',
    inputSchema: {
      agentCapabilities: z
        .array(z.string())
        .min(1)
        .describe('Agent capabilities (e.g., ["buy", "ship", "invoice"])'),
      agentTrustLevel: z
        .enum(['sandbox', 'verified', 'enterprise', 'admin'])
        .default('sandbox')
        .describe('Agent trust level'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const svc = await getCatalogSvc();
        const result = svc.matchAgentToProducts(params.agentCapabilities, params.agentTrustLevel);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // Match Product to Agents
  // ==========================================================================
  {
    name: 'match_product_to_agents',
    description:
      "Find agents compatible with a specific product. Filters available agents by the product's required trust level and capabilities.",
    inputSchema: {
      productId: z.string().min(1).describe('Product ID to match agents for'),
      availableAgents: z
        .array(
          z.object({
            id: z.string().describe('Agent ID'),
            capabilities: z.array(z.string()).describe('Agent capabilities'),
            trustLevel: z
              .enum(['sandbox', 'verified', 'enterprise', 'admin'])
              .describe('Agent trust level'),
          }),
        )
        .min(1)
        .describe('List of available agents to filter'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const svc = await getCatalogSvc();
        const result = svc.matchProductToAgents(params.productId, params.availableAgents);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // Export Catalog
  // ==========================================================================
  {
    name: 'export_agent_catalog',
    description:
      'Export the agent catalog in JSON or OpenAPI format. Useful for sharing the catalog with other systems or generating API documentation.',
    inputSchema: {
      format: z
        .enum(['json', 'openapi'])
        .optional()
        .default('json')
        .describe('Export format: json (default) or openapi'),
      category: z.string().optional().describe('Filter by category'),
      status: z.enum(['active', 'delisted']).optional().describe('Filter by status'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const svc = await getCatalogSvc();
        const result = svc.exportCatalog(params);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },
];

export default catalogTools;
