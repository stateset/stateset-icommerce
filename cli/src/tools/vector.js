/**
 * Vector search tools for MCP server
 *
 * Provides hybrid semantic + lexical search across products, customers, orders, and inventory.
 * Uses OpenAI text-embedding-3-small model for generating embeddings.
 */

/**
 * Vector search tool definitions
 */
export const vectorTools = [
    {
        name: 'vector_search_products',
        description: 'Search products using natural language query with hybrid semantic + BM25 ranking. Returns products sorted by relevance score.',
        inputSchema: {
            type: 'object',
            properties: {
                query: {
                    type: 'string',
                    description: 'Natural language search query (e.g., "wireless bluetooth headphones", "eco-friendly water bottle")'
                },
                limit: {
                    type: 'number',
                    description: 'Maximum number of results to return (default: 10)',
                    default: 10
                }
            },
            required: ['query']
        }
    },
    {
        name: 'vector_search_customers',
        description: 'Search customers using natural language query with hybrid semantic + BM25 ranking.',
        inputSchema: {
            type: 'object',
            properties: {
                query: {
                    type: 'string',
                    description: 'Natural language search query (e.g., "enterprise customers in tech")'
                },
                limit: {
                    type: 'number',
                    description: 'Maximum number of results to return (default: 10)',
                    default: 10
                }
            },
            required: ['query']
        }
    },
    {
        name: 'vector_search_orders',
        description: 'Search orders using natural language query with hybrid semantic + BM25 ranking.',
        inputSchema: {
            type: 'object',
            properties: {
                query: {
                    type: 'string',
                    description: 'Natural language search query (e.g., "late shipments", "refund requested")'
                },
                limit: {
                    type: 'number',
                    description: 'Maximum number of results to return (default: 10)',
                    default: 10
                }
            },
            required: ['query']
        }
    },
    {
        name: 'vector_search_inventory',
        description: 'Search inventory items using natural language query with hybrid semantic + BM25 ranking.',
        inputSchema: {
            type: 'object',
            properties: {
                query: {
                    type: 'string',
                    description: 'Natural language search query (e.g., "blue widgets", "outdoor gear")'
                },
                limit: {
                    type: 'number',
                    description: 'Maximum number of results to return (default: 10)',
                    default: 10
                }
            },
            required: ['query']
        }
    },
    {
        name: 'vector_index_product',
        description: 'Index a single product for vector search by its ID.',
        inputSchema: {
            type: 'object',
            properties: {
                product_id: {
                    type: 'string',
                    description: 'Product ID (UUID) to index'
                }
            },
            required: ['product_id']
        }
    },
    {
        name: 'vector_index_customer',
        description: 'Index a single customer for vector search by their ID.',
        inputSchema: {
            type: 'object',
            properties: {
                customer_id: {
                    type: 'string',
                    description: 'Customer ID (UUID) to index'
                }
            },
            required: ['customer_id']
        }
    },
    {
        name: 'vector_index_order',
        description: 'Index a single order for vector search by its ID.',
        inputSchema: {
            type: 'object',
            properties: {
                order_id: {
                    type: 'string',
                    description: 'Order ID (UUID) to index'
                }
            },
            required: ['order_id']
        }
    },
    {
        name: 'vector_index_inventory',
        description: 'Index a single inventory item for vector search by its ID.',
        inputSchema: {
            type: 'object',
            properties: {
                item_id: {
                    type: 'string',
                    description: 'Inventory item ID to index'
                }
            },
            required: ['item_id']
        }
    },
    {
        name: 'vector_index_all_products',
        description: 'Index all products in the database for vector search. This may take a while for large catalogs.',
        inputSchema: {
            type: 'object',
            properties: {},
            required: []
        }
    },
    {
        name: 'vector_index_all_customers',
        description: 'Index all customers in the database for vector search.',
        inputSchema: {
            type: 'object',
            properties: {},
            required: []
        }
    },
    {
        name: 'vector_index_all_orders',
        description: 'Index all orders in the database for vector search.',
        inputSchema: {
            type: 'object',
            properties: {},
            required: []
        }
    },
    {
        name: 'vector_index_all_inventory',
        description: 'Index all inventory items in the database for vector search.',
        inputSchema: {
            type: 'object',
            properties: {},
            required: []
        }
    },
    {
        name: 'vector_stats',
        description: 'Get statistics about vector embeddings including counts by entity type.',
        inputSchema: {
            type: 'object',
            properties: {},
            required: []
        }
    },
    {
        name: 'vector_clear',
        description: 'Clear all vector embeddings for a specific entity type.',
        inputSchema: {
            type: 'object',
            properties: {
                entity_type: {
                    type: 'string',
                    enum: ['products', 'customers', 'orders', 'inventory'],
                    description: 'Entity type to clear embeddings for'
                }
            },
            required: ['entity_type']
        }
    },
    {
        name: 'vector_clear_all',
        description: 'Clear all vector embeddings across all entity types.',
        inputSchema: {
            type: 'object',
            properties: {},
            required: []
        }
    }
];

const VECTOR_TOOL_PERMISSIONS = {
    vector_search_products: 'read',
    vector_search_customers: 'read',
    vector_search_orders: 'read',
    vector_search_inventory: 'read',
    vector_stats: 'read',
    vector_index_product: 'write',
    vector_index_customer: 'write',
    vector_index_order: 'write',
    vector_index_inventory: 'write',
    vector_index_all_products: 'admin',
    vector_index_all_customers: 'admin',
    vector_index_all_orders: 'admin',
    vector_index_all_inventory: 'admin',
    vector_clear: 'admin',
    vector_clear_all: 'admin'
};

/**
 * Get VectorSearch instance from Commerce
 * @param {Object} commerce - Commerce instance
 * @returns {Object} VectorSearch instance
 */
function getVectorSearch(commerce) {
    const apiKey = process.env.OPENAI_API_KEY;
    if (!apiKey) {
        throw new Error('OPENAI_API_KEY environment variable is required for vector search');
    }
    return commerce.vector(apiKey);
}

/**
 * Execute vector tool logic and return normalized results
 * @param {string} name - Tool name
 * @param {Object} args - Tool arguments
 * @param {Object} commerce - Commerce instance
 * @returns {Promise<{kind: 'json', data: Object} | {kind: 'text', text: string}>}
 */
async function runVectorTool(name, args, commerce) {
    switch (name) {
        case 'vector_search_products': {
            const vector = getVectorSearch(commerce);
            const results = await vector.searchProducts(args.query, args.limit ?? 10);
            return {
                kind: 'json',
                data: {
                    query: args.query,
                    count: results.length,
                    results: results.map(r => ({
                        id: r.product.id,
                        name: r.product.name,
                        description: r.product.description,
                        score: r.score.toFixed(3),
                        distance: r.distance.toFixed(4)
                    }))
                }
            };
        }

        case 'vector_search_customers': {
            const vector = getVectorSearch(commerce);
            const results = await vector.searchCustomers(args.query, args.limit ?? 10);
            return {
                kind: 'json',
                data: {
                    query: args.query,
                    count: results.length,
                    results: results.map(r => ({
                        id: r.customer.id,
                        name: `${r.customer.firstName} ${r.customer.lastName}`,
                        email: r.customer.email,
                        score: r.score.toFixed(3),
                        distance: r.distance.toFixed(4)
                    }))
                }
            };
        }

        case 'vector_search_orders': {
            const vector = getVectorSearch(commerce);
            const results = await vector.searchOrders(args.query, args.limit ?? 10);
            return {
                kind: 'json',
                data: {
                    query: args.query,
                    count: results.length,
                    results: results.map(r => ({
                        id: r.order.id,
                        order_number: r.order.orderNumber,
                        status: r.order.status,
                        total_amount: r.order.totalAmount,
                        score: r.score.toFixed(3),
                        distance: r.distance.toFixed(4)
                    }))
                }
            };
        }

        case 'vector_search_inventory': {
            const vector = getVectorSearch(commerce);
            const results = await vector.searchInventory(args.query, args.limit ?? 10);
            return {
                kind: 'json',
                data: {
                    query: args.query,
                    count: results.length,
                    results: results.map(r => ({
                        id: r.item.id,
                        sku: r.item.sku,
                        name: r.item.name,
                        score: r.score.toFixed(3),
                        distance: r.distance.toFixed(4)
                    }))
                }
            };
        }

        case 'vector_index_product': {
            const vector = getVectorSearch(commerce);
            await vector.indexProduct(args.product_id);
            return { kind: 'text', text: `Product ${args.product_id} indexed for vector search` };
        }

        case 'vector_index_customer': {
            const vector = getVectorSearch(commerce);
            await vector.indexCustomer(args.customer_id);
            return { kind: 'text', text: `Customer ${args.customer_id} indexed for vector search` };
        }

        case 'vector_index_order': {
            const vector = getVectorSearch(commerce);
            await vector.indexOrder(args.order_id);
            return { kind: 'text', text: `Order ${args.order_id} indexed for vector search` };
        }

        case 'vector_index_inventory': {
            const vector = getVectorSearch(commerce);
            await vector.indexInventoryItem(args.item_id);
            return { kind: 'text', text: `Inventory item ${args.item_id} indexed for vector search` };
        }

        case 'vector_index_all_products': {
            const vector = getVectorSearch(commerce);
            const count = await vector.indexAllProducts();
            return { kind: 'text', text: `Indexed ${count} products for vector search` };
        }

        case 'vector_index_all_customers': {
            const vector = getVectorSearch(commerce);
            const count = await vector.indexAllCustomers();
            return { kind: 'text', text: `Indexed ${count} customers for vector search` };
        }

        case 'vector_index_all_orders': {
            const vector = getVectorSearch(commerce);
            const count = await vector.indexAllOrders();
            return { kind: 'text', text: `Indexed ${count} orders for vector search` };
        }

        case 'vector_index_all_inventory': {
            const vector = getVectorSearch(commerce);
            const count = await vector.indexAllInventory();
            return { kind: 'text', text: `Indexed ${count} inventory items for vector search` };
        }

        case 'vector_stats': {
            const vector = getVectorSearch(commerce);
            const stats = await vector.stats();
            return {
                kind: 'json',
                data: {
                    model: stats.model,
                    dimensions: stats.dimensions,
                    counts: {
                        products: stats.productCount,
                        customers: stats.customerCount,
                        orders: stats.orderCount,
                        inventory: stats.inventoryCount
                    },
                    total: stats.productCount + stats.customerCount + stats.orderCount + stats.inventoryCount
                }
            };
        }

        case 'vector_clear': {
            const vector = getVectorSearch(commerce);
            const count = await vector.clear(args.entity_type);
            return { kind: 'text', text: `Cleared ${count} ${args.entity_type} embeddings` };
        }

        case 'vector_clear_all': {
            const vector = getVectorSearch(commerce);
            const count = await vector.clearAll();
            return { kind: 'text', text: `Cleared ${count} embeddings` };
        }

        default:
            throw new Error(`Unknown vector tool: ${name}`);
    }
}

function normalizeVectorResult(result) {
    if (result.kind === 'text') {
        return { message: result.text };
    }
    return result.data;
}

for (const tool of vectorTools) {
    tool.permission = VECTOR_TOOL_PERMISSIONS[tool.name] || 'read';
    tool.handler = async ({ commerce, params, allowApply }) => {
        if (!allowApply && ['write', 'delete', 'admin'].includes(tool.permission)) {
            return {
                error: 'Vector operation not allowed. The --apply flag must be set for write operations.',
                hint: 'Run with --apply to enable vector indexing or clearing.'
            };
        }
        const result = await runVectorTool(tool.name, params, commerce);
        return normalizeVectorResult(result);
    };
}

/**
 * Handle vector search tool calls (MCP format)
 * @param {string} name - Tool name
 * @param {Object} args - Tool arguments
 * @param {Object} commerce - Commerce instance
 * @returns {Promise<Object>} Tool result
 */
export async function handleVectorTool(name, args, commerce) {
    const result = await runVectorTool(name, args, commerce);
    if (result.kind === 'text') {
        return { content: [{ type: 'text', text: result.text }] };
    }
    return { content: [{ type: 'text', text: JSON.stringify(result.data, null, 2) }] };
}
