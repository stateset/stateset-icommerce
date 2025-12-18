#!/usr/bin/env node

/**
 * StateSet Commerce CLI - Direct Mode (No AI)
 *
 * Simple command-line interface for common commerce operations
 * without AI interpretation.
 *
 * Usage:
 *   stateset-direct customers list
 *   stateset-direct orders get <id>
 *   stateset-direct inventory stock <sku>
 */

import { Commerce } from '@stateset/embedded';
import { parseArgs } from 'node:util';

const HELP = `
StateSet Commerce CLI - Direct Mode

USAGE:
  stateset-direct [global-options] <resource> <action> [args]

GLOBAL OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --json             Output as JSON
  --help, -h         Show this help message

RESOURCES & ACTIONS:

  customers
    list                          List all customers
    get <id|email>                Get customer by ID or email
    create <email> <first> <last> Create a customer
    count                         Count customers

  orders
    list                          List all orders
    get <id>                      Get order by ID
    ship <id> [tracking]          Ship an order
    cancel <id>                   Cancel an order
    count                         Count orders

  products
    list                          List all products
    get <id>                      Get product by ID
    variant <sku>                 Get variant by SKU
    count                         Count products

  inventory
    stock <sku>                   Get stock level for SKU
    adjust <sku> <qty> <reason>   Adjust stock (positive or negative)
    create <sku> <name> [qty]     Create inventory item

  returns
    list                          List all returns
    get <id>                      Get return by ID
    approve <id>                  Approve a return
    reject <id> <reason>          Reject a return
    count                         Count returns

EXAMPLES:
  stateset-direct customers list
  stateset-direct orders get abc-123-def
  stateset-direct inventory stock WIDGET-001
  stateset-direct inventory adjust WIDGET-001 -5 "Sold 5 units"
  stateset-direct --json products list
`;

async function main() {
  const args = process.argv.slice(2);

  // Find global options
  let dbPath = './store.db';
  let jsonOutput = false;
  let showHelp = false;

  // Extract global options
  const filteredArgs = [];
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--db' && args[i + 1]) {
      dbPath = args[++i];
    } else if (args[i] === '--json') {
      jsonOutput = true;
    } else if (args[i] === '--help' || args[i] === '-h') {
      showHelp = true;
    } else {
      filteredArgs.push(args[i]);
    }
  }

  if (showHelp || filteredArgs.length === 0) {
    console.log(HELP);
    process.exit(0);
  }

  const [resource, action, ...actionArgs] = filteredArgs;

  // Initialize commerce
  const commerce = new Commerce(dbPath);

  const output = (data) => {
    if (jsonOutput) {
      console.log(JSON.stringify(data, null, 2));
    } else {
      console.log(data);
    }
  };

  const formatTable = (items, columns) => {
    if (jsonOutput) return items; // Return raw items for JSON output
    if (items.length === 0) return 'No results found.';

    // Calculate column widths
    const widths = {};
    for (const col of columns) {
      widths[col] = col.length;
      for (const item of items) {
        const val = String(item[col] ?? '');
        widths[col] = Math.max(widths[col], val.length);
      }
    }

    // Header
    const header = columns.map(c => c.padEnd(widths[c])).join(' | ');
    const separator = columns.map(c => '-'.repeat(widths[c])).join('-+-');

    // Rows
    const rows = items.map(item =>
      columns.map(c => String(item[c] ?? '').padEnd(widths[c])).join(' | ')
    );

    return [header, separator, ...rows].join('\n');
  };

  try {
    switch (resource) {
      // ============================================================================
      // Customers
      // ============================================================================
      case 'customers':
        switch (action) {
          case 'list': {
            const customers = await commerce.customers.list();
            output(formatTable(customers.map(c => ({
              id: c.id.slice(0, 8) + '...',
              email: c.email,
              name: `${c.firstName} ${c.lastName}`,
              status: c.status
            })), ['id', 'email', 'name', 'status']));
            break;
          }
          case 'get': {
            const id = actionArgs[0];
            if (!id) throw new Error('Usage: customers get <id|email>');
            const customer = id.includes('@')
              ? await commerce.customers.getByEmail(id)
              : await commerce.customers.get(id);
            if (!customer) throw new Error('Customer not found');
            output(jsonOutput ? customer : `
Customer: ${customer.firstName} ${customer.lastName}
ID: ${customer.id}
Email: ${customer.email}
Phone: ${customer.phone || 'N/A'}
Status: ${customer.status}
Marketing: ${customer.acceptsMarketing ? 'Yes' : 'No'}
Created: ${customer.createdAt}
`);
            break;
          }
          case 'create': {
            const [email, firstName, lastName] = actionArgs;
            if (!email || !firstName || !lastName) {
              throw new Error('Usage: customers create <email> <firstName> <lastName>');
            }
            const customer = await commerce.customers.create({ email, firstName, lastName });
            output(jsonOutput ? customer : `Created customer: ${customer.id}`);
            break;
          }
          case 'count': {
            const count = await commerce.customers.count();
            output(jsonOutput ? { count } : `Customer count: ${count}`);
            break;
          }
          default:
            throw new Error(`Unknown action: customers ${action}`);
        }
        break;

      // ============================================================================
      // Orders
      // ============================================================================
      case 'orders':
        switch (action) {
          case 'list': {
            const orders = await commerce.orders.list();
            output(formatTable(orders.map(o => ({
              id: o.id.slice(0, 8) + '...',
              number: o.orderNumber,
              status: o.status,
              total: `${o.currency} ${o.totalAmount.toFixed(2)}`,
              items: o.items?.length || 0
            })), ['id', 'number', 'status', 'total', 'items']));
            break;
          }
          case 'get': {
            const id = actionArgs[0];
            if (!id) throw new Error('Usage: orders get <id>');
            const order = await commerce.orders.get(id);
            if (!order) throw new Error('Order not found');
            output(jsonOutput ? order : `
Order: ${order.orderNumber}
ID: ${order.id}
Status: ${order.status}
Total: ${order.currency} ${order.totalAmount.toFixed(2)}
Payment: ${order.paymentStatus}
Fulfillment: ${order.fulfillmentStatus}
Tracking: ${order.trackingNumber || 'N/A'}
Items:
${order.items?.map(i => `  - ${i.name} (${i.sku}) x${i.quantity} @ ${i.unitPrice}`).join('\n') || '  (no items)'}
Created: ${order.createdAt}
`);
            break;
          }
          case 'ship': {
            const [orderId, trackingNumber] = actionArgs;
            if (!orderId) throw new Error('Usage: orders ship <id> [tracking]');
            const order = await commerce.orders.ship(orderId, trackingNumber);
            output(jsonOutput ? order : `Order ${order.orderNumber} shipped${trackingNumber ? ` (${trackingNumber})` : ''}`);
            break;
          }
          case 'cancel': {
            const orderId = actionArgs[0];
            if (!orderId) throw new Error('Usage: orders cancel <id>');
            const order = await commerce.orders.cancel(orderId);
            output(jsonOutput ? order : `Order ${order.orderNumber} cancelled`);
            break;
          }
          case 'count': {
            const count = await commerce.orders.count();
            output(jsonOutput ? { count } : `Order count: ${count}`);
            break;
          }
          default:
            throw new Error(`Unknown action: orders ${action}`);
        }
        break;

      // ============================================================================
      // Products
      // ============================================================================
      case 'products':
        switch (action) {
          case 'list': {
            const products = await commerce.products.list();
            output(formatTable(products.map(p => ({
              id: p.id.slice(0, 8) + '...',
              name: p.name,
              slug: p.slug,
              status: p.status
            })), ['id', 'name', 'slug', 'status']));
            break;
          }
          case 'get': {
            const id = actionArgs[0];
            if (!id) throw new Error('Usage: products get <id>');
            const product = await commerce.products.get(id);
            if (!product) throw new Error('Product not found');
            output(jsonOutput ? product : `
Product: ${product.name}
ID: ${product.id}
Slug: ${product.slug}
Status: ${product.status}
Description: ${product.description || 'N/A'}
Created: ${product.createdAt}
`);
            break;
          }
          case 'variant': {
            const sku = actionArgs[0];
            if (!sku) throw new Error('Usage: products variant <sku>');
            const variant = await commerce.products.getVariantBySku(sku);
            if (!variant) throw new Error(`Variant ${sku} not found`);
            output(jsonOutput ? variant : `
Variant: ${variant.name}
SKU: ${variant.sku}
Price: ${variant.price}
Compare At: ${variant.compareAtPrice || 'N/A'}
Default: ${variant.isDefault ? 'Yes' : 'No'}
`);
            break;
          }
          case 'count': {
            const count = await commerce.products.count();
            output(jsonOutput ? { count } : `Product count: ${count}`);
            break;
          }
          default:
            throw new Error(`Unknown action: products ${action}`);
        }
        break;

      // ============================================================================
      // Inventory
      // ============================================================================
      case 'inventory':
        switch (action) {
          case 'stock': {
            const sku = actionArgs[0];
            if (!sku) throw new Error('Usage: inventory stock <sku>');
            const stock = await commerce.inventory.getStock(sku);
            if (!stock) throw new Error(`No inventory for SKU ${sku}`);
            output(jsonOutput ? stock : `
Stock for ${stock.sku} (${stock.name}):
  On Hand:   ${stock.totalOnHand}
  Allocated: ${stock.totalAllocated}
  Available: ${stock.totalAvailable}
`);
            break;
          }
          case 'adjust': {
            const [sku, qtyStr, ...reasonParts] = actionArgs;
            const qty = parseInt(qtyStr, 10);
            const reason = reasonParts.join(' ');
            if (!sku || isNaN(qty) || !reason) {
              throw new Error('Usage: inventory adjust <sku> <quantity> <reason>');
            }
            await commerce.inventory.adjust(sku, qty, reason);
            const stock = await commerce.inventory.getStock(sku);
            output(jsonOutput ? stock : `Adjusted ${sku} by ${qty > 0 ? '+' : ''}${qty}. New on-hand: ${stock.totalOnHand}`);
            break;
          }
          case 'create': {
            const [sku, name, qtyStr] = actionArgs;
            const initialQuantity = qtyStr ? parseInt(qtyStr, 10) : 0;
            if (!sku || !name) {
              throw new Error('Usage: inventory create <sku> <name> [initialQuantity]');
            }
            const item = await commerce.inventory.createItem({ sku, name, initialQuantity });
            output(jsonOutput ? item : `Created inventory item: ${item.sku} (${item.name})`);
            break;
          }
          default:
            throw new Error(`Unknown action: inventory ${action}`);
        }
        break;

      // ============================================================================
      // Returns
      // ============================================================================
      case 'returns':
        switch (action) {
          case 'list': {
            const returns = await commerce.returns.list();
            output(formatTable(returns.map(r => ({
              id: r.id.slice(0, 8) + '...',
              order: r.orderId.slice(0, 8) + '...',
              status: r.status,
              reason: r.reason,
              created: r.createdAt.slice(0, 10)
            })), ['id', 'order', 'status', 'reason', 'created']));
            break;
          }
          case 'get': {
            const id = actionArgs[0];
            if (!id) throw new Error('Usage: returns get <id>');
            const ret = await commerce.returns.get(id);
            if (!ret) throw new Error('Return not found');
            output(jsonOutput ? ret : `
Return: ${ret.id}
Order: ${ret.orderId}
Status: ${ret.status}
Reason: ${ret.reason}
Created: ${ret.createdAt}
`);
            break;
          }
          case 'approve': {
            const id = actionArgs[0];
            if (!id) throw new Error('Usage: returns approve <id>');
            const ret = await commerce.returns.approve(id);
            output(jsonOutput ? ret : `Return ${ret.id} approved`);
            break;
          }
          case 'reject': {
            const [id, ...reasonParts] = actionArgs;
            const reason = reasonParts.join(' ');
            if (!id || !reason) {
              throw new Error('Usage: returns reject <id> <reason>');
            }
            const ret = await commerce.returns.reject(id, reason);
            output(jsonOutput ? ret : `Return ${ret.id} rejected`);
            break;
          }
          case 'count': {
            const count = await commerce.returns.count();
            output(jsonOutput ? { count } : `Return count: ${count}`);
            break;
          }
          default:
            throw new Error(`Unknown action: returns ${action}`);
        }
        break;

      default:
        throw new Error(`Unknown resource: ${resource}\nUse --help for available commands.`);
    }

    process.exit(0);
  } catch (error) {
    if (jsonOutput) {
      console.log(JSON.stringify({ error: error.message }));
    } else {
      console.error(`Error: ${error.message}`);
    }
    process.exit(1);
  }
}

main();
