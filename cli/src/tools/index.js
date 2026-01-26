/**
 * Tool Registry
 *
 * Central registry for all MCP tools, enabling modular tool loading
 * and lazy initialization for performance.
 */

import { customerTools } from './customers.js';
import { orderTools } from './orders.js';
import { vectorTools } from './vector.js';

/**
 * Tool categories for lazy loading
 */
const TOOL_MODULES = {
  customers: () => customerTools,
  orders: () => orderTools,
  vector: () => vectorTools,
  // Additional categories loaded on demand
  products: () => import('./products.js').then(m => m.default),
  inventory: () => import('./inventory.js').then(m => m.default),
  returns: () => import('./returns.js').then(m => m.default),
  carts: () => import('./carts.js').then(m => m.default),
  analytics: () => import('./analytics.js').then(m => m.default),
  currency: () => import('./currency.js').then(m => m.default),
  tax: () => import('./tax.js').then(m => m.default),
  promotions: () => import('./promotions.js').then(m => m.default),
  subscriptions: () => import('./subscriptions.js').then(m => m.default)
};

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
    await Promise.all(
      Object.keys(TOOL_MODULES).map(cat => this.loadCategory(cat))
    );
  }

  /**
   * Load tools for a specific agent
   */
  async loadForAgent(agentName) {
    const agentCategories = AGENT_TOOL_CATEGORIES[agentName];
    if (agentCategories) {
      await Promise.all(agentCategories.map(cat => this.loadCategory(cat)));
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
    return this.getAll().filter(t => t.category === category);
  }

  /**
   * Get tools by permission level
   */
  getByPermission(permission) {
    return this.getAll().filter(t => t.permission === permission);
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
    return this.getAll().filter(t =>
      ['write', 'delete', 'admin'].includes(t.permission)
    );
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
    return this.getAll().map(tool => ({
      name: tool.name,
      description: tool.description,
      inputSchema: tool.inputSchema,
      handler: async (params) => {
        try {
          const result = await tool.handler({
            ...context,
            params
          });
          return {
            content: [{
              type: 'text',
              text: JSON.stringify(result, null, 2)
            }]
          };
        } catch (error) {
          return {
            content: [{
              type: 'text',
              text: JSON.stringify({ error: error.message })
            }]
          };
        }
      }
    }));
  }
}

/**
 * Agent to tool category mappings
 */
export const AGENT_TOOL_CATEGORIES = {
  'customer-service': ['customers', 'orders', 'products', 'inventory', 'returns', 'carts', 'analytics', 'currency', 'tax', 'promotions', 'subscriptions', 'vector'],
  'checkout': ['carts', 'products', 'inventory', 'promotions', 'tax', 'currency', 'vector'],
  'orders': ['orders', 'customers', 'inventory'],
  'inventory': ['inventory', 'products', 'vector'],
  'returns': ['returns', 'orders', 'customers', 'inventory'],
  'analytics': ['analytics', 'vector'],
  'promotions': ['promotions', 'products', 'vector'],
  'subscriptions': ['subscriptions', 'customers'],
  'vector': ['vector', 'products', 'customers']
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
  vector: vectorTools
};

export default {
  ToolRegistry,
  createToolRegistry,
  getToolsForAgent,
  AGENT_TOOL_CATEGORIES,
  immediateTools
};
