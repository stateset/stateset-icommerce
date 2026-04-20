/**
 * Vector Commands Module
 */

function getVectorSearch(commerce) {
  const apiKey = process.env.OPENAI_API_KEY;
  if (!apiKey) {
    throw new Error('OPENAI_API_KEY environment variable is required for vector search');
  }
  return commerce.vector(apiKey);
}

function parseLimit(value, usage) {
  if (value === undefined) return undefined;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0) throw new Error(usage);
  return parsed;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  const vector = getVectorSearch(commerce);

  switch (action) {
    case 'search-products': {
      const [query, limitRaw] = args;
      if (!query) throw new Error('Usage: vector search-products <query> [limit]');
      const results = await vector.searchProducts(
        query,
        parseLimit(limitRaw, 'Usage: vector search-products <query> [limit]') || 10,
      );
      return formatSearchResults(
        results.map((result) => ({
          id: result.product.id,
          name: result.product.name,
          description: result.product.description,
          score: result.score.toFixed(3),
        })),
        { output, jsonOutput },
      );
    }

    case 'search-customers': {
      const [query, limitRaw] = args;
      if (!query) throw new Error('Usage: vector search-customers <query> [limit]');
      const results = await vector.searchCustomers(
        query,
        parseLimit(limitRaw, 'Usage: vector search-customers <query> [limit]') || 10,
      );
      return formatSearchResults(
        results.map((result) => ({
          id: result.customer.id,
          name: `${result.customer.firstName} ${result.customer.lastName}`,
          description: result.customer.email,
          score: result.score.toFixed(3),
        })),
        { output, jsonOutput },
      );
    }

    case 'search-orders': {
      const [query, limitRaw] = args;
      if (!query) throw new Error('Usage: vector search-orders <query> [limit]');
      const results = await vector.searchOrders(
        query,
        parseLimit(limitRaw, 'Usage: vector search-orders <query> [limit]') || 10,
      );
      return formatSearchResults(
        results.map((result) => ({
          id: result.order.id,
          name: result.order.orderNumber,
          description: result.order.status,
          score: result.score.toFixed(3),
        })),
        { output, jsonOutput },
      );
    }

    case 'search-inventory': {
      const [query, limitRaw] = args;
      if (!query) throw new Error('Usage: vector search-inventory <query> [limit]');
      const results = await vector.searchInventory(
        query,
        parseLimit(limitRaw, 'Usage: vector search-inventory <query> [limit]') || 10,
      );
      return formatSearchResults(
        results.map((result) => ({
          id: result.item.id,
          name: result.item.name,
          description: result.item.sku,
          score: result.score.toFixed(3),
        })),
        { output, jsonOutput },
      );
    }

    case 'index-product': {
      const productId = args[0];
      if (!productId) throw new Error('Usage: vector index-product <productId>');
      await vector.indexProduct(productId);
      return { formatted: `Indexed product ${productId} for vector search` };
    }

    case 'index-customer': {
      const customerId = args[0];
      if (!customerId) throw new Error('Usage: vector index-customer <customerId>');
      await vector.indexCustomer(customerId);
      return { formatted: `Indexed customer ${customerId} for vector search` };
    }

    case 'index-order': {
      const orderId = args[0];
      if (!orderId) throw new Error('Usage: vector index-order <orderId>');
      await vector.indexOrder(orderId);
      return { formatted: `Indexed order ${orderId} for vector search` };
    }

    case 'index-inventory': {
      const itemId = args[0];
      if (!itemId) throw new Error('Usage: vector index-inventory <itemId>');
      await vector.indexInventoryItem(itemId);
      return { formatted: `Indexed inventory item ${itemId} for vector search` };
    }

    case 'stats': {
      const stats = await vector.stats();
      const result = {
        model: stats.model,
        dimensions: stats.dimensions,
        counts: {
          products: stats.productCount,
          customers: stats.customerCount,
          orders: stats.orderCount,
          inventory: stats.inventoryCount,
        },
      };
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Vector stats\n` +
              `${'-'.repeat(22)}\n` +
              `Model:       ${result.model}\n` +
              `Dimensions:  ${result.dimensions}\n` +
              `Products:    ${result.counts.products}\n` +
              `Customers:   ${result.counts.customers}\n` +
              `Orders:      ${result.counts.orders}\n` +
              `Inventory:   ${result.counts.inventory}`,
          };
    }

    case 'clear': {
      const entityType = args[0];
      if (!entityType) throw new Error('Usage: vector clear <entityType>');
      const count = await vector.clear(entityType);
      return { formatted: `Cleared ${count} ${entityType} embeddings` };
    }

    case 'clear-all': {
      const count = await vector.clearAll();
      return { formatted: `Cleared ${count} embeddings` };
    }

    case 'reindex-all': {
      await vector.clearAll();
      const products = await vector.indexAllProducts();
      const customers = await vector.indexAllCustomers();
      const orders = await vector.indexAllOrders();
      const inventory = await vector.indexAllInventory();
      const result = {
        products,
        customers,
        orders,
        inventory,
        total: products + customers + orders + inventory,
      };
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Vector reindex complete\n` +
              `${'-'.repeat(32)}\n` +
              `Products:    ${products}\n` +
              `Customers:   ${customers}\n` +
              `Orders:      ${orders}\n` +
              `Inventory:   ${inventory}\n` +
              `Total:       ${result.total}`,
          };
    }

    default:
      throw new Error(
        `Unknown action: vector ${action}\n\n` +
          'Available actions:\n' +
          '  search-products <query> [limit]    Search products\n' +
          '  search-customers <query> [limit]   Search customers\n' +
          '  search-orders <query> [limit]      Search orders\n' +
          '  search-inventory <query> [limit]   Search inventory\n' +
          '  index-product <productId>          Index product\n' +
          '  index-customer <customerId>        Index customer\n' +
          '  index-order <orderId>              Index order\n' +
          '  index-inventory <itemId>           Index inventory item\n' +
          '  stats                              Get vector stats\n' +
          '  clear <entityType>                 Clear embeddings for entity type\n' +
          '  clear-all                          Clear all embeddings\n' +
          '  reindex-all                        Rebuild all embeddings',
      );
  }
}

function formatSearchResults(results, { output, jsonOutput }) {
  if (jsonOutput) return results;
  if (results.length === 0) return { formatted: 'No vector search results found.' };
  const formatted = output.table(results, [
    { key: 'id', header: 'ID' },
    { key: 'name', header: 'Name' },
    { key: 'description', header: 'Description' },
    { key: 'score', header: 'Score', align: 'right' },
  ]);
  return { results, formatted };
}

export const metadata = {
  name: 'vector',
  aliases: ['vec', 'semantic'],
  description: 'Vector search and indexing commands',
  actions: {
    'search-products': { description: 'Search products', args: ['<query>', '[limit]'] },
    'search-customers': { description: 'Search customers', args: ['<query>', '[limit]'] },
    'search-orders': { description: 'Search orders', args: ['<query>', '[limit]'] },
    'search-inventory': { description: 'Search inventory', args: ['<query>', '[limit]'] },
    'index-product': { description: 'Index product', args: ['<productId>'] },
    'index-customer': { description: 'Index customer', args: ['<customerId>'] },
    'index-order': { description: 'Index order', args: ['<orderId>'] },
    'index-inventory': { description: 'Index inventory item', args: ['<itemId>'] },
    stats: { description: 'Get vector stats', args: [] },
    clear: { description: 'Clear embeddings for entity type', args: ['<entityType>'] },
    'clear-all': { description: 'Clear all embeddings', args: [] },
    'reindex-all': { description: 'Rebuild all embeddings', args: [] },
  },
};

export default { execute, metadata };
