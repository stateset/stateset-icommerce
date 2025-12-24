/**
 * Inventory Commands Module
 *
 * Handles all inventory-related CLI operations for stateset-direct
 */

/**
 * Execute inventory commands
 * @param {string} action - The action to perform
 * @param {Array} args - Command arguments
 * @param {Object} options - Command options
 * @returns {Promise<any>} Command result
 */
export async function execute(action, args, { commerce, output, jsonOutput, resolveSku }) {
  switch (action) {
    case 'list': {
      const products = await commerce.products.list();
      const skuItems = [];

      for (const product of products) {
        if (product.variants) {
          for (const variant of product.variants) {
            if (variant.sku) {
              const stock = await commerce.inventory.getStock(variant.sku);
              skuItems.push({
                sku: variant.sku,
                name: variant.name || product.name,
                onHand: stock?.totalOnHand ?? 0,
                available: stock?.totalAvailable ?? 0,
                allocated: stock?.totalAllocated ?? 0
              });
            }
          }
        }
      }

      return formatInventoryList(skuItems, { output, jsonOutput });
    }

    case 'stock': {
      const skuArg = args[0];
      if (!skuArg) {
        throw new Error('Usage: inventory stock <sku>\n\nProvide a SKU to check stock levels.');
      }

      const sku = await resolveSku(skuArg);
      const stock = await commerce.inventory.getStock(sku);

      if (!stock) {
        throw new Error(`No inventory found for SKU: ${skuArg}\n\nTry 'stateset-direct inventory list' to see all inventory.`);
      }

      return formatStockDetail(stock, { output, jsonOutput });
    }

    case 'adjust': {
      const [skuArg, qtyStr, ...reasonParts] = args;
      const qty = parseInt(qtyStr, 10);
      const reason = reasonParts.join(' ');

      if (!skuArg || isNaN(qty) || !reason) {
        throw new Error(
          'Usage: inventory adjust <sku> <quantity> <reason>\n\n' +
          'Examples:\n' +
          '  stateset-direct inventory adjust WIDGET-001 10 "Received shipment"\n' +
          '  stateset-direct inventory adjust WIDGET-001 -5 "Damaged items"'
        );
      }

      const sku = await resolveSku(skuArg);
      await commerce.inventory.adjust(sku, qty, reason);
      const stock = await commerce.inventory.getStock(sku);

      return formatAdjustment(sku, qty, stock, { output, jsonOutput });
    }

    case 'create': {
      const [sku, name, qtyStr] = args;
      const initialQuantity = qtyStr ? parseInt(qtyStr, 10) : 0;

      if (!sku || !name) {
        throw new Error(
          'Usage: inventory create <sku> <name> [initialQuantity]\n\n' +
          'Example: stateset-direct inventory create WIDGET-002 "Premium Widget" 100'
        );
      }

      const item = await commerce.inventory.createItem({ sku, name, initialQuantity });

      return formatItemCreated(item, { output, jsonOutput });
    }

    case 'low': {
      const threshold = parseInt(args[0], 10) || 10;
      const products = await commerce.products.list();
      const lowStock = [];

      for (const product of products) {
        if (product.variants) {
          for (const variant of product.variants) {
            if (variant.sku) {
              const stock = await commerce.inventory.getStock(variant.sku);
              if (stock && stock.totalAvailable <= threshold) {
                lowStock.push({
                  sku: variant.sku,
                  name: variant.name || product.name,
                  available: stock.totalAvailable,
                  onHand: stock.totalOnHand
                });
              }
            }
          }
        }
      }

      return formatLowStock(lowStock, threshold, { output, jsonOutput });
    }

    case 'reserve': {
      const [skuArg, qtyStr, orderId] = args;
      const qty = parseInt(qtyStr, 10);

      if (!skuArg || isNaN(qty)) {
        throw new Error(
          'Usage: inventory reserve <sku> <quantity> [orderId]\n\n' +
          'Reserve inventory for an order.'
        );
      }

      const sku = await resolveSku(skuArg);
      const reservation = await commerce.inventory.reserve(sku, qty, orderId);

      return {
        reservation,
        formatted: `Reserved ${qty} units of ${sku}${orderId ? ` for order ${orderId}` : ''}`
      };
    }

    case 'release': {
      const [skuArg, qtyStr] = args;
      const qty = parseInt(qtyStr, 10);

      if (!skuArg || isNaN(qty)) {
        throw new Error('Usage: inventory release <sku> <quantity>\n\nRelease reserved inventory.');
      }

      const sku = await resolveSku(skuArg);
      await commerce.inventory.release(sku, qty);
      const stock = await commerce.inventory.getStock(sku);

      return {
        stock,
        formatted: `Released ${qty} units of ${sku}. New available: ${stock.totalAvailable}`
      };
    }

    default:
      throw new Error(
        `Unknown action: inventory ${action}\n\n` +
        'Available actions:\n' +
        '  list              List all inventory with stock levels\n' +
        '  stock <sku>       Get stock level for SKU\n' +
        '  adjust <sku> <qty> <reason>  Adjust stock\n' +
        '  create <sku> <name> [qty]    Create inventory item\n' +
        '  low [threshold]   List low stock items\n' +
        '  reserve <sku> <qty> [orderId]  Reserve inventory\n' +
        '  release <sku> <qty>  Release reserved inventory'
      );
  }
}

/**
 * Format inventory list for output
 */
function formatInventoryList(items, { output, jsonOutput }) {
  if (jsonOutput) {
    return items;
  }

  if (items.length === 0) {
    return { formatted: 'No inventory found.' };
  }

  const formatted = output.table(
    items,
    [
      { key: 'sku', header: 'SKU' },
      { key: 'name', header: 'Name' },
      { key: 'onHand', header: 'On Hand', align: 'right' },
      { key: 'allocated', header: 'Allocated', align: 'right' },
      { key: 'available', header: 'Available', align: 'right' }
    ]
  );

  return { items, formatted };
}

/**
 * Format stock detail
 */
function formatStockDetail(stock, { output, jsonOutput }) {
  if (jsonOutput) {
    return stock;
  }

  const formatted = `
Stock for ${stock.sku} (${stock.name || 'N/A'}):
${'-'.repeat(40)}
  On Hand:   ${stock.totalOnHand}
  Allocated: ${stock.totalAllocated}
  Available: ${stock.totalAvailable}
`;

  return { stock, formatted };
}

/**
 * Format adjustment result
 */
function formatAdjustment(sku, qty, stock, { output, jsonOutput }) {
  if (jsonOutput) {
    return { success: true, sku, adjustment: qty, stock };
  }

  const sign = qty > 0 ? '+' : '';
  return {
    stock,
    formatted: `Adjusted ${sku} by ${sign}${qty}. New on-hand: ${stock.totalOnHand}`
  };
}

/**
 * Format item created
 */
function formatItemCreated(item, { output, jsonOutput }) {
  if (jsonOutput) {
    return { success: true, item };
  }

  return {
    item,
    formatted: `Created inventory item: ${item.sku} (${item.name})`
  };
}

/**
 * Format low stock list
 */
function formatLowStock(items, threshold, { output, jsonOutput }) {
  if (jsonOutput) {
    return { threshold, items };
  }

  if (items.length === 0) {
    return { formatted: `No items below threshold of ${threshold} units.` };
  }

  const formatted = output.table(
    items,
    [
      { key: 'sku', header: 'SKU' },
      { key: 'name', header: 'Name' },
      { key: 'available', header: 'Available', align: 'right' },
      { key: 'onHand', header: 'On Hand', align: 'right' }
    ]
  );

  return {
    items,
    formatted: `Low stock items (threshold: ${threshold}):\n\n${formatted}`
  };
}

/**
 * Command metadata for help/completion
 */
export const metadata = {
  name: 'inventory',
  aliases: ['i', 'inv', 'stock'],
  description: 'Inventory management commands',
  actions: {
    list: { description: 'List all inventory', args: [] },
    stock: { description: 'Get stock for SKU', args: ['<sku>'] },
    adjust: { description: 'Adjust stock level', args: ['<sku>', '<quantity>', '<reason>'] },
    create: { description: 'Create inventory item', args: ['<sku>', '<name>', '[quantity]'] },
    low: { description: 'List low stock items', args: ['[threshold]'] },
    reserve: { description: 'Reserve inventory', args: ['<sku>', '<quantity>', '[orderId]'] },
    release: { description: 'Release reservation', args: ['<sku>', '<quantity>'] }
  }
};

export default { execute, metadata };
