/**
 * Smart Suggestion Engine for StateSet CLI
 *
 * Provides intelligent command suggestions, typo correction,
 * and contextual help based on user input.
 */

/**
 * Common commerce intents and their mappings
 */
const INTENT_PATTERNS = {
  // Customer intents
  list_customers: [
    /show\s*(all\s*)?(my\s*)?customers?/i,
    /get\s*(all\s*)?(my\s*)?customers?/i,
    /customers?\s*list/i,
    /who\s*are\s*my\s*customers?/i,
  ],
  get_customer: [
    /show\s*(me\s*)?customer\s+(.+)/i,
    /get\s*customer\s+(.+)/i,
    /find\s*customer\s+(.+)/i,
    /customer\s+(.+)/i,
  ],
  create_customer: [
    /create\s*(a\s*)?customer/i,
    /add\s*(a\s*)?(new\s*)?customer/i,
    /new\s*customer/i,
    /register\s*(a\s*)?customer/i,
  ],

  // Order intents
  list_orders: [
    /show\s*(all\s*)?(my\s*)?orders?/i,
    /get\s*(all\s*)?(my\s*)?orders?/i,
    /orders?\s*list/i,
    /what\s*orders?\s*(do\s*we\s*have|are\s*there)/i,
  ],
  get_order: [
    /show\s*(me\s*)?order\s+(.+)/i,
    /get\s*order\s+(.+)/i,
    /order\s*#?(\w+)/i,
    /find\s*order\s+(.+)/i,
  ],
  ship_order: [
    /ship\s*(the\s*)?order/i,
    /mark\s*(order\s*)?(as\s*)?shipped/i,
    /send\s*(out\s*)?(the\s*)?order/i,
    /fulfill\s*(the\s*)?order/i,
  ],
  cancel_order: [/cancel\s*(the\s*)?order/i, /void\s*(the\s*)?order/i, /delete\s*(the\s*)?order/i],

  // Inventory intents
  get_stock: [
    /how\s*much\s*(stock|inventory)/i,
    /stock\s*(level|count)?(\s*for)?/i,
    /inventory\s*(level|count)?(\s*for)?/i,
    /check\s*(the\s*)?(stock|inventory)/i,
    /do\s*we\s*have\s*(any\s*)?(.+)\s*in\s*stock/i,
  ],
  adjust_inventory: [
    /adjust\s*(the\s*)?(stock|inventory)/i,
    /add\s*(\d+)\s*(units?)?/i,
    /remove\s*(\d+)\s*(units?)?/i,
    /update\s*(the\s*)?(stock|inventory)/i,
  ],
  low_stock: [
    /low\s*stock/i,
    /out\s*of\s*stock/i,
    /what\s*(items?|products?)\s*(need|are\s*low)/i,
    /reorder\s*(alert|needed)/i,
  ],

  // Return intents
  list_returns: [
    /show\s*(all\s*)?(my\s*)?returns?/i,
    /get\s*(all\s*)?(my\s*)?returns?/i,
    /pending\s*returns?/i,
    /what\s*returns?\s*(do\s*we\s*have|are\s*there)/i,
  ],
  approve_return: [/approve\s*(the\s*)?return/i, /accept\s*(the\s*)?return/i],
  reject_return: [
    /reject\s*(the\s*)?return/i,
    /deny\s*(the\s*)?return/i,
    /decline\s*(the\s*)?return/i,
  ],

  // Analytics intents
  sales_summary: [
    /sales\s*(summary|report|stats)/i,
    /revenue\s*(report|summary)?/i,
    /how\s*much\s*(did\s*we|have\s*we)\s*(make|sell|earn)/i,
    /total\s*sales/i,
  ],
  top_products: [
    /top\s*(selling\s*)?products?/i,
    /best\s*sellers?/i,
    /popular\s*products?/i,
    /what\s*sells\s*(best|most)/i,
  ],
  top_customers: [
    /top\s*customers?/i,
    /best\s*customers?/i,
    /vip\s*customers?/i,
    /who\s*(are\s*my|spends?\s*the\s*most)/i,
  ],

  // Vector search intents
  vector_search_products: [
    /find\s+similar\s+products?/i,
    /search\s+products?\s+like\s+(.+)/i,
    /semantic\s+search\s+products?/i,
    /vector\s+search\s+products?/i,
  ],
  vector_search_customers: [
    /find\s+similar\s+customers?/i,
    /search\s+customers?\s+like\s+(.+)/i,
    /semantic\s+search\s+customers?/i,
    /vector\s+search\s+customers?/i,
  ],
  vector_search_orders: [
    /find\s+similar\s+orders?/i,
    /search\s+orders?\s+like\s+(.+)/i,
    /semantic\s+search\s+orders?/i,
    /vector\s+search\s+orders?/i,
  ],
  vector_search_inventory: [
    /find\s+similar\s+inventory/i,
    /search\s+inventory\s+like\s+(.+)/i,
    /semantic\s+search\s+inventory/i,
    /vector\s+search\s+inventory/i,
  ],

  // Cart intents
  create_cart: [
    /create\s*(a\s*)?(new\s*)?cart/i,
    /start\s*(a\s*)?(new\s*)?(cart|checkout|order)/i,
    /new\s*cart/i,
  ],
  complete_checkout: [
    /complete\s*(the\s*)?(checkout|order|cart)/i,
    /finish\s*(the\s*)?(checkout|order|cart)/i,
    /checkout/i,
    /place\s*(the\s*)?order/i,
  ],
};

/**
 * Command aliases and corrections
 */
const COMMAND_ALIASES = {
  // Typos and variations
  costumers: 'customers',
  cutomers: 'customers',
  customes: 'customers',
  ordres: 'orders',
  oder: 'order',
  oders: 'orders',
  prodcuts: 'products',
  porducts: 'products',
  inventroy: 'inventory',
  invnetory: 'inventory',
  retruns: 'returns',
  retrun: 'return',

  // Common alternatives
  clients: 'customers',
  buyers: 'customers',
  purchases: 'orders',
  items: 'products',
  goods: 'products',
  stock: 'inventory',
  rmas: 'returns',
  refunds: 'returns',
};

/**
 * SuggestionEngine - Intelligent command suggestions
 */
export class SuggestionEngine {
  constructor(options = {}) {
    this.intentPatterns = options.intentPatterns || INTENT_PATTERNS;
    this.commandAliases = options.commandAliases || COMMAND_ALIASES;
    this.minSimilarity = options.minSimilarity || 0.6;
  }

  /**
   * Detect intent from natural language query
   */
  detectIntent(query) {
    const normalized = query.toLowerCase().trim();

    for (const [intent, patterns] of Object.entries(this.intentPatterns)) {
      for (const pattern of patterns) {
        const match = normalized.match(pattern);
        if (match) {
          return {
            intent,
            confidence: 0.9,
            match: match[0],
            captures: match.slice(1).filter(Boolean),
          };
        }
      }
    }

    return null;
  }

  /**
   * Get command suggestion from intent
   */
  getSuggestion(query) {
    const intent = this.detectIntent(query);

    if (!intent) {
      return this.getFuzzySuggestion(query);
    }

    const suggestion = this.buildSuggestion(intent);
    return {
      ...suggestion,
      original: query,
      intent: intent.intent,
      confidence: intent.confidence,
    };
  }

  /**
   * Build command suggestion from detected intent
   */
  buildSuggestion(intent) {
    const templates = {
      list_customers: {
        command: 'stateset "list all customers"',
        direct: 'stateset-direct customers list',
        description: 'List all customers',
      },
      get_customer: {
        command: `stateset "get customer ${intent.captures?.[0] || '<id>'}"`,
        direct: `stateset-direct customers get ${intent.captures?.[0] || '<id>'}`,
        description: 'Get customer details',
      },
      create_customer: {
        command: 'stateset --apply "create a customer..."',
        direct: 'stateset-direct customers create <email> <firstName> <lastName>',
        description: 'Create a new customer',
      },
      list_orders: {
        command: 'stateset "show all orders"',
        direct: 'stateset-direct orders list',
        description: 'List all orders',
      },
      get_order: {
        command: `stateset "get order ${intent.captures?.[0] || '<id>'}"`,
        direct: `stateset-direct orders get ${intent.captures?.[0] || '<id>'}`,
        description: 'Get order details',
      },
      ship_order: {
        command: 'stateset --apply "ship order <id> with tracking <number>"',
        direct: 'stateset-direct orders ship <id> [tracking]',
        description: 'Ship an order',
      },
      cancel_order: {
        command: 'stateset --apply "cancel order <id>"',
        direct: 'stateset-direct orders cancel <id>',
        description: 'Cancel an order',
      },
      get_stock: {
        command: 'stateset "check stock for <sku>"',
        direct: 'stateset-direct inventory stock <sku>',
        description: 'Check stock levels',
      },
      adjust_inventory: {
        command: 'stateset --apply "add 10 units to <sku>"',
        direct: 'stateset-direct inventory adjust <sku> <qty> <reason>',
        description: 'Adjust inventory',
      },
      low_stock: {
        command: 'stateset "show low stock items"',
        direct: 'stateset-direct inventory low',
        description: 'List low stock items',
      },
      list_returns: {
        command: 'stateset "show all returns"',
        direct: 'stateset-direct returns list',
        description: 'List all returns',
      },
      approve_return: {
        command: 'stateset --apply "approve return <id>"',
        direct: 'stateset-direct returns approve <id>',
        description: 'Approve a return',
      },
      reject_return: {
        command: 'stateset --apply "reject return <id> <reason>"',
        direct: 'stateset-direct returns reject <id> <reason>',
        description: 'Reject a return',
      },
      sales_summary: {
        command: 'stateset "show me sales summary"',
        direct: 'stateset-analytics "sales summary"',
        description: 'Get sales summary',
      },
      top_products: {
        command: 'stateset "what are my top products"',
        direct: 'stateset-analytics "top products"',
        description: 'Get top selling products',
      },
      top_customers: {
        command: 'stateset "who are my top customers"',
        direct: 'stateset-analytics "top customers"',
        description: 'Get top customers',
      },
      vector_search_products: {
        command: 'stateset "find products similar to <query>"',
        direct: 'stateset "find products similar to <query>"',
        description: 'Semantic + BM25 product search',
      },
      vector_search_customers: {
        command: 'stateset "find customers similar to <query>"',
        direct: 'stateset "find customers similar to <query>"',
        description: 'Semantic + BM25 customer search',
      },
      vector_search_orders: {
        command: 'stateset "find orders mentioning <query>"',
        direct: 'stateset "find orders mentioning <query>"',
        description: 'Semantic + BM25 order search',
      },
      vector_search_inventory: {
        command: 'stateset "find inventory items like <query>"',
        direct: 'stateset "find inventory items like <query>"',
        description: 'Semantic + BM25 inventory search',
      },
      create_cart: {
        command: 'stateset --apply "create a cart for <email>"',
        direct: 'stateset-checkout "create cart"',
        description: 'Create a shopping cart',
      },
      complete_checkout: {
        command: 'stateset --apply "complete checkout for cart <id>"',
        direct: 'stateset-checkout "complete <cart-id>"',
        description: 'Complete checkout',
      },
    };

    return (
      templates[intent.intent] || {
        command: `stateset "${intent.match}"`,
        description: 'Execute command',
      }
    );
  }

  /**
   * Get fuzzy suggestion for unrecognized queries
   */
  getFuzzySuggestion(query) {
    const words = query.toLowerCase().split(/\s+/);
    const corrections = [];

    for (const word of words) {
      // Check for direct alias
      if (this.commandAliases[word]) {
        corrections.push({
          original: word,
          suggestion: this.commandAliases[word],
        });
        continue;
      }

      // Check for similar words
      for (const [typo, correct] of Object.entries(this.commandAliases)) {
        if (this.similarity(word, typo) > this.minSimilarity) {
          corrections.push({
            original: word,
            suggestion: correct,
            similarity: this.similarity(word, typo),
          });
          break;
        }
      }
    }

    if (corrections.length > 0) {
      const correctedQuery = words
        .map((w) => {
          const correction = corrections.find((c) => c.original === w);
          return correction ? correction.suggestion : w;
        })
        .join(' ');

      return {
        original: query,
        suggested: correctedQuery,
        corrections,
        hint: `Did you mean: "${correctedQuery}"?`,
      };
    }

    return null;
  }

  /**
   * Calculate string similarity (Levenshtein-based)
   */
  similarity(a, b) {
    if (a === b) return 1;
    if (a.length === 0 || b.length === 0) return 0;

    const matrix = [];
    for (let i = 0; i <= b.length; i++) {
      matrix[i] = [i];
    }
    for (let j = 0; j <= a.length; j++) {
      matrix[0][j] = j;
    }

    for (let i = 1; i <= b.length; i++) {
      for (let j = 1; j <= a.length; j++) {
        if (b[i - 1] === a[j - 1]) {
          matrix[i][j] = matrix[i - 1][j - 1];
        } else {
          matrix[i][j] = Math.min(
            matrix[i - 1][j - 1] + 1,
            matrix[i][j - 1] + 1,
            matrix[i - 1][j] + 1,
          );
        }
      }
    }

    const distance = matrix[b.length][a.length];
    return 1 - distance / Math.max(a.length, b.length);
  }

  /**
   * Get contextual help based on current state
   */
  getContextualHelp(context = {}) {
    const { lastCommand, error } = context;
    const suggestions = [];

    if (error) {
      // Suggest based on error type
      if (error.includes('not found')) {
        suggestions.push('Try listing available items first');
        suggestions.push('Check if the ID/SKU is correct');
      } else if (error.includes('permission') || error.includes('--apply')) {
        suggestions.push('Add --apply flag to enable write operations');
      }
    }

    if (lastCommand) {
      // Suggest follow-up commands
      if (lastCommand.includes('list_customers')) {
        suggestions.push('Get details: stateset "get customer <email>"');
        suggestions.push('Create new: stateset --apply "create customer..."');
      } else if (lastCommand.includes('list_orders')) {
        suggestions.push('Get details: stateset "get order <id>"');
        suggestions.push('Ship order: stateset --apply "ship order <id>"');
      } else if (lastCommand.includes('create_cart')) {
        suggestions.push('Add items: stateset --apply "add <item> to cart"');
        suggestions.push('Complete: stateset --apply "complete checkout"');
      }
    }

    return suggestions;
  }

  /**
   * Get examples for a specific topic
   */
  getExamples(topic) {
    const examples = {
      customers: [
        'stateset "list all customers"',
        'stateset "get customer alice@example.com"',
        'stateset --apply "create a customer named Alice Smith with email alice@example.com"',
      ],
      orders: [
        'stateset "show all pending orders"',
        'stateset "get order ORD-12345"',
        'stateset --apply "ship order ORD-12345 with tracking FEDEX123"',
        'stateset --apply "cancel order ORD-12345"',
      ],
      inventory: [
        'stateset "how much WIDGET-001 do we have?"',
        'stateset "show low stock items"',
        'stateset --apply "add 50 units to WIDGET-001 - received shipment"',
      ],
      returns: [
        'stateset "show pending returns"',
        'stateset --apply "approve return RET-123"',
        'stateset --apply "reject return RET-123 - outside return window"',
      ],
      analytics: [
        'stateset "what are my total sales this month?"',
        'stateset "who are my top customers?"',
        'stateset "what are my best selling products?"',
      ],
      checkout: [
        'stateset --apply "create a cart for alice@example.com"',
        'stateset --apply --resume <id> "add 2 widgets at $29.99"',
        'stateset --apply --resume <id> "complete the checkout"',
      ],
    };

    return examples[topic] || [];
  }
}

/**
 * Create a suggestion engine
 */
export function createSuggestionEngine(options = {}) {
  return new SuggestionEngine(options);
}

/**
 * Format suggestion for display
 */
export function formatSuggestion(suggestion, options = {}) {
  const { color = true } = options;
  const cyan = color ? '\x1b[36m' : '';
  const yellow = color ? '\x1b[33m' : '';
  const gray = color ? '\x1b[90m' : '';
  const reset = color ? '\x1b[0m' : '';

  let output = '';

  if (suggestion.hint) {
    output += `${yellow}${suggestion.hint}${reset}\n`;
  }

  if (suggestion.command) {
    output += `\n${cyan}AI Mode:${reset} ${suggestion.command}`;
  }

  if (suggestion.direct) {
    output += `\n${cyan}Direct:${reset}  ${suggestion.direct}`;
  }

  if (suggestion.description) {
    output += `\n${gray}${suggestion.description}${reset}`;
  }

  return output;
}

export default {
  SuggestionEngine,
  createSuggestionEngine,
  formatSuggestion,
  INTENT_PATTERNS,
  COMMAND_ALIASES,
};
