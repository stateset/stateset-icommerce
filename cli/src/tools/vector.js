/**
 * Vector search tools for MCP server
 *
 * Provides semantic similarity search across products, customers, orders, and inventory.
 * Uses OpenAI text-embedding-3-small model for generating embeddings.
 */

/**
 * Vector search tool definitions
 */
export const vectorTools = [
    {
        name: 'vector_search_products',
        description: 'Search products using natural language query with semantic similarity. Returns products sorted by relevance score.',
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
        description: 'Search customers using natural language query with semantic similarity.',
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
    }
];

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
 * Handle vector search tool calls
 * @param {string} name - Tool name
 * @param {Object} args - Tool arguments
 * @param {Object} commerce - Commerce instance
 * @returns {Promise<Object>} Tool result
 */
export async function handleVectorTool(name, args, commerce) {
    switch (name) {
        case 'vector_search_products': {
            const vector = getVectorSearch(commerce);
            const results = await vector.searchProducts(args.query, args.limit || 10);
            return {
                content: [{
                    type: 'text',
                    text: JSON.stringify({
                        query: args.query,
                        count: results.length,
                        results: results.map(r => ({
                            id: r.product.id,
                            name: r.product.name,
                            description: r.product.description,
                            score: r.score.toFixed(3),
                            distance: r.distance.toFixed(4)
                        }))
                    }, null, 2)
                }]
            };
        }

        case 'vector_search_customers': {
            const vector = getVectorSearch(commerce);
            const results = await vector.searchCustomers(args.query, args.limit || 10);
            return {
                content: [{
                    type: 'text',
                    text: JSON.stringify({
                        query: args.query,
                        count: results.length,
                        results: results.map(r => ({
                            id: r.customer.id,
                            name: `${r.customer.firstName} ${r.customer.lastName}`,
                            email: r.customer.email,
                            score: r.score.toFixed(3),
                            distance: r.distance.toFixed(4)
                        }))
                    }, null, 2)
                }]
            };
        }

        case 'vector_index_product': {
            const vector = getVectorSearch(commerce);
            await vector.indexProduct(args.product_id);
            return {
                content: [{
                    type: 'text',
                    text: `Product ${args.product_id} indexed for vector search`
                }]
            };
        }

        case 'vector_index_customer': {
            const vector = getVectorSearch(commerce);
            await vector.indexCustomer(args.customer_id);
            return {
                content: [{
                    type: 'text',
                    text: `Customer ${args.customer_id} indexed for vector search`
                }]
            };
        }

        case 'vector_index_all_products': {
            const vector = getVectorSearch(commerce);
            const count = await vector.indexAllProducts();
            return {
                content: [{
                    type: 'text',
                    text: `Indexed ${count} products for vector search`
                }]
            };
        }

        case 'vector_index_all_customers': {
            const vector = getVectorSearch(commerce);
            const count = await vector.indexAllCustomers();
            return {
                content: [{
                    type: 'text',
                    text: `Indexed ${count} customers for vector search`
                }]
            };
        }

        case 'vector_stats': {
            const vector = getVectorSearch(commerce);
            const stats = await vector.stats();
            return {
                content: [{
                    type: 'text',
                    text: JSON.stringify({
                        model: stats.model,
                        dimensions: stats.dimensions,
                        counts: {
                            products: stats.productCount,
                            customers: stats.customerCount,
                            orders: stats.orderCount,
                            inventory: stats.inventoryCount
                        },
                        total: stats.productCount + stats.customerCount + stats.orderCount + stats.inventoryCount
                    }, null, 2)
                }]
            };
        }

        case 'vector_clear': {
            const vector = getVectorSearch(commerce);
            const count = await vector.clear(args.entity_type);
            return {
                content: [{
                    type: 'text',
                    text: `Cleared ${count} ${args.entity_type} embeddings`
                }]
            };
        }

        default:
            throw new Error(`Unknown vector tool: ${name}`);
    }
}
