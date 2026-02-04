#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - Direct Mode (No AI)
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
import * as path from 'node:path';
import * as fs from 'node:fs';
import { createConfirmHandler } from '../src/utils/confirm.js';

// ============================================================================
// Resource & Action Aliases
// ============================================================================

const RESOURCE_ALIASES = {
  // Single letter shortcuts
  'c': 'customers',
  'o': 'orders',
  'p': 'products',
  'i': 'inventory',
  'r': 'returns',
  'v': 'vector',
  // Common abbreviations
  'cust': 'customers',
  'ord': 'orders',
  'prod': 'products',
  'inv': 'inventory',
  'ret': 'returns',
  'vec': 'vector',
  'stock': 'inventory'  // natural alias
};

const ACTION_ALIASES = {
  'l': 'list',
  'ls': 'list',
  'g': 'get',
  's': 'ship',
  'x': 'cancel',
  'a': 'adjust',
  'n': 'count',
  '#': 'count'
};

/**
 * Expand resource alias to full name
 */
function expandResource(input) {
  if (!input) return input;
  const lower = input.toLowerCase();
  return RESOURCE_ALIASES[lower] || lower;
}

/**
 * Expand action alias to full name
 */
function expandAction(input) {
  if (!input) return input;
  const lower = input.toLowerCase();
  return ACTION_ALIASES[lower] || lower;
}

const HELP = `
StateSet iCommerce CLI - Direct Mode

USAGE:
  stateset-direct [global-options] <resource> <action> [args]

GLOBAL OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --apply            Enable write operations
  --json             Output as JSON
  --format <fmt>     Output format: table, json (default: table)
  --output <file>    Write output to file
  --yes, -y          Skip confirmation prompts
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
    list                          List all inventory with stock levels
    stock <sku>                   Get stock level for SKU
    adjust <sku> <qty> <reason>   Adjust stock (positive or negative)
    create <sku> <name> [qty]     Create inventory item

  returns
    list                          List all returns
    get <id>                      Get return by ID
    approve <id>                  Approve a return
    reject <id> <reason>          Reject a return
    count                         Count returns

  vector
    search <scope> <query> [limit]  Vector search (products|customers|orders|inventory)
    index <scope> <id>              Index a single entity
    index-all <scope>               Index all entities for a scope
    stats                           Show embedding stats
    clear <scope>                   Clear embeddings for a scope
    clear-all                       Clear all embeddings
    (requires OPENAI_API_KEY)

SHORTCUTS:
  Resources: c=customers, o=orders, p=products, i/inv=inventory, r=returns, v=vector
  Actions:   l/ls=list, g=get, s=ship, x=cancel, a=adjust, n/#=count

EXAMPLES:
  stateset-direct customers list
  stateset-direct c l                         # Same as: customers list
  stateset-direct o g 8aeb                    # orders get (short ID)
  stateset-direct --apply o x 8aeb            # orders cancel
  stateset-direct inv stock WIDGET-001
  stateset-direct i stock WIDGET              # Fuzzy SKU match
  stateset-direct --apply inv a WIDGET -5 "Sold"  # inventory adjust
  stateset-direct o #                         # orders count
  stateset-direct v search products "wireless earbuds" 5
  stateset-direct --json p l                  # products list as JSON

SMART MATCHING:
  - IDs: Use any unique prefix (like git). "8aeb" matches "8aeb3f12-..."
  - SKUs: Partial match supported. "WIDGET" matches "WIDGET-001"

SAFETY:
  - Write operations require --apply
  - Confirmation prompts appear unless --yes is provided
`;

async function main() {
  const args = process.argv.slice(2);

  // Find global options
  let dbPath = './store.db';
  let apply = false;
  let jsonOutput = false;
  let format = 'table';
  let formatProvided = false;
  let outputPath = null;
  let skipConfirm = false;
  let showHelp = false;

  // Extract global options
  const filteredArgs = [];
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--db' && args[i + 1]) {
      dbPath = args[++i];
    } else if (args[i] === '--apply') {
      apply = true;
    } else if (args[i] === '--json') {
      jsonOutput = true;
    } else if (args[i] === '--format' && args[i + 1]) {
      format = args[++i];
      formatProvided = true;
    } else if (args[i].startsWith('--format=')) {
      format = args[i].split('=')[1] || 'table';
      formatProvided = true;
    } else if (args[i] === '--output' && args[i + 1]) {
      outputPath = args[++i];
    } else if (args[i] === '--yes' || args[i] === '-y') {
      skipConfirm = true;
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

  const [rawResource, rawAction, ...actionArgs] = filteredArgs;

  // Expand aliases
  const resource = expandResource(rawResource);
  const action = expandAction(rawAction);

  const outputFormat = formatProvided ? format : (jsonOutput ? 'json' : format);
  if (!['table', 'json'].includes(outputFormat)) {
    throw new Error(`Unsupported format: ${outputFormat}. Use table or json.`);
  }
  const isJsonOutput = outputFormat === 'json';
  jsonOutput = isJsonOutput;

  // Validate database path exists (for file-based databases)
  if (dbPath !== ':memory:') {
    const dir = path.dirname(path.resolve(dbPath));
    if (!fs.existsSync(dir)) {
      throw new Error(`Database directory does not exist: ${dir}`);
    }
  }

  // Initialize commerce
  const commerce = new Commerce(dbPath);

  const outputLines = [];
  const output = (data) => {
    const line = jsonOutput ? JSON.stringify(data, null, 2) : String(data);
    outputLines.push(line);
    if (!outputPath) {
      console.log(line);
    }
  };

  const isWriteAction = (res, act) => {
    const writeActions = {
      customers: new Set(['create']),
      orders: new Set(['ship', 'cancel']),
      inventory: new Set(['adjust', 'create']),
      returns: new Set(['approve', 'reject']),
      vector: new Set(['index', 'index-all', 'clear', 'clear-all'])
    };
    return Boolean(writeActions[res]?.has(act));
  };

  const ensureWriteAllowed = async (res, act, details) => {
    if (!isWriteAction(res, act)) return;
    if (!apply) {
      throw new Error(`Preview mode: would execute '${res} ${act}'. Re-run with --apply to proceed.`);
    }
    const nonInteractive = !process.stdin.isTTY || jsonOutput;
    const confirm = createConfirmHandler({
      output: null,
      assumeYes: skipConfirm,
      nonInteractive
    });
    const confirmed = await confirm({
      operation: `${res}.${act}`,
      details
    });
    if (!confirmed) {
      throw new Error('Confirmation declined');
    }
  };

  // Resolve short ID prefix to full UUID
  // Similar to git short hashes - must be unambiguous
  const resolveId = async (prefix, resource) => {
    // If it looks like a full UUID (contains dashes and is long enough), return as-is
    if (prefix.includes('-') && prefix.length > 20) {
      return prefix;
    }

    // Query the resource to find matching IDs
    let items;
    switch (resource) {
      case 'orders':
        items = await commerce.orders.list();
        break;
      case 'customers':
        items = await commerce.customers.list();
        break;
      case 'products':
        items = await commerce.products.list();
        break;
      case 'returns':
        items = await commerce.returns.list();
        break;
      default:
        throw new Error(`Unknown resource for ID resolution: ${resource}`);
    }

    // Find items whose ID starts with the prefix (case-insensitive)
    const lowerPrefix = prefix.toLowerCase();
    const matches = items.filter(item =>
      item.id.toLowerCase().startsWith(lowerPrefix)
    );

    if (matches.length === 0) {
      throw new Error(`No ${resource.slice(0, -1)} found matching '${prefix}'`);
    }
    if (matches.length > 1) {
      const matchList = matches.slice(0, 5).map(m => m.id.slice(0, 12) + '...').join(', ');
      throw new Error(`Ambiguous ID '${prefix}' - matches ${matches.length} ${resource}: ${matchList}`);
    }

    return matches[0].id;
  };

  // Resolve partial SKU to full SKU via fuzzy matching
  const resolveSku = async (partial) => {
    // Try exact match first
    const exactStock = await commerce.inventory.getStock(partial);
    if (exactStock) return partial;

    // Get all inventory items and find matches
    // Note: This requires listing inventory - we'll search through products variants
    const products = await commerce.products.list();
    const allSkus = [];

    for (const product of products) {
      if (product.variants) {
        for (const variant of product.variants) {
          if (variant.sku) {
            allSkus.push(variant.sku);
          }
        }
      }
    }

    // Also check inventory items directly if available
    // Try to get stock for common patterns
    const upperPartial = partial.toUpperCase();
    const lowerPartial = partial.toLowerCase();

    // Find SKUs that contain the partial (case-insensitive)
    const matches = allSkus.filter(sku => {
      const upperSku = sku.toUpperCase();
      return upperSku === upperPartial ||
             upperSku.startsWith(upperPartial) ||
             upperSku.includes(upperPartial);
    });

    if (matches.length === 0) {
      // No matches in products, return original and let it fail with proper error
      return partial;
    }
    if (matches.length === 1) {
      return matches[0];
    }

    // Multiple matches - prefer exact start match
    const startMatches = matches.filter(sku =>
      sku.toUpperCase().startsWith(upperPartial)
    );
    if (startMatches.length === 1) {
      return startMatches[0];
    }

    // Still ambiguous
    const matchList = matches.slice(0, 5).join(', ');
    throw new Error(`Ambiguous SKU '${partial}' - matches: ${matchList}`);
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

  const normalizeVectorScope = (scope) => {
    if (!scope) return null;
    switch (scope.toLowerCase()) {
      case 'product':
      case 'products':
        return 'products';
      case 'customer':
      case 'customers':
        return 'customers';
      case 'order':
      case 'orders':
        return 'orders';
      case 'inventory':
      case 'item':
      case 'items':
        return 'inventory';
      default:
        return null;
    }
  };

  const parseSearchArgs = (args) => {
    if (args.length === 0) {
      return { query: '', limit: undefined };
    }
    let limit;
    const last = args[args.length - 1];
    if (/^\d+$/.test(last)) {
      limit = parseInt(last, 10);
      args = args.slice(0, -1);
    }
    return { query: args.join(' ').trim(), limit };
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
            const idArg = actionArgs[0];
            if (!idArg) throw new Error('Usage: customers get <id|email>');
            const customer = idArg.includes('@')
              ? await commerce.customers.getByEmail(idArg)
              : await commerce.customers.get(await resolveId(idArg, 'customers'));
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
            await ensureWriteAllowed('customers', 'create', `${email} ${firstName} ${lastName}`);
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
            const idArg = actionArgs[0];
            if (!idArg) throw new Error('Usage: orders get <id>');
            const id = await resolveId(idArg, 'orders');
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
            const [orderIdArg, trackingNumber] = actionArgs;
            if (!orderIdArg) throw new Error('Usage: orders ship <id> [tracking]');
            await ensureWriteAllowed('orders', 'ship', `order ${orderIdArg}${trackingNumber ? ` tracking ${trackingNumber}` : ''}`);
            const orderId = await resolveId(orderIdArg, 'orders');
            const order = await commerce.orders.ship(orderId, trackingNumber);
            output(jsonOutput ? order : `Order ${order.orderNumber} shipped${trackingNumber ? ` (${trackingNumber})` : ''}`);
            break;
          }
          case 'cancel': {
            const orderIdArg = actionArgs[0];
            if (!orderIdArg) throw new Error('Usage: orders cancel <id>');
            await ensureWriteAllowed('orders', 'cancel', `order ${orderIdArg}`);
            const orderId = await resolveId(orderIdArg, 'orders');
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
            const idArg = actionArgs[0];
            if (!idArg) throw new Error('Usage: products get <id>');
            const id = await resolveId(idArg, 'products');
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
            const skuArg = actionArgs[0];
            if (!skuArg) throw new Error('Usage: products variant <sku>');
            const sku = await resolveSku(skuArg);
            const variant = await commerce.products.getVariantBySku(sku);
            if (!variant) throw new Error(`Variant ${skuArg} not found`);
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
                      available: stock?.totalAvailable ?? 0
                    });
                  }
                }
              }
            }
            output(formatTable(skuItems, ['sku', 'name', 'onHand', 'available']));
            break;
          }
          case 'stock': {
            const skuArg = actionArgs[0];
            if (!skuArg) throw new Error('Usage: inventory stock <sku>');
            const sku = await resolveSku(skuArg);
            const stock = await commerce.inventory.getStock(sku);
            if (!stock) throw new Error(`No inventory for SKU ${skuArg}`);
            output(jsonOutput ? stock : `
Stock for ${stock.sku} (${stock.name}):
  On Hand:   ${stock.totalOnHand}
  Allocated: ${stock.totalAllocated}
  Available: ${stock.totalAvailable}
`);
            break;
          }
          case 'adjust': {
            const [skuArg, qtyStr, ...reasonParts] = actionArgs;
            const qty = parseInt(qtyStr, 10);
            const reason = reasonParts.join(' ');
            if (!skuArg || isNaN(qty) || !reason) {
              throw new Error('Usage: inventory adjust <sku> <quantity> <reason>');
            }
            await ensureWriteAllowed('inventory', 'adjust', `${skuArg} ${qty > 0 ? '+' : ''}${qty} (${reason})`);
            const sku = await resolveSku(skuArg);
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
            await ensureWriteAllowed('inventory', 'create', `${sku} (${name})`);
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
            const idArg = actionArgs[0];
            if (!idArg) throw new Error('Usage: returns get <id>');
            const id = await resolveId(idArg, 'returns');
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
            const idArg = actionArgs[0];
            if (!idArg) throw new Error('Usage: returns approve <id>');
            await ensureWriteAllowed('returns', 'approve', idArg);
            const id = await resolveId(idArg, 'returns');
            const ret = await commerce.returns.approve(id);
            output(jsonOutput ? ret : `Return ${ret.id} approved`);
            break;
          }
          case 'reject': {
            const [idArg, ...reasonParts] = actionArgs;
            const reason = reasonParts.join(' ');
            if (!idArg || !reason) {
              throw new Error('Usage: returns reject <id> <reason>');
            }
            await ensureWriteAllowed('returns', 'reject', `${idArg} (${reason})`);
            const id = await resolveId(idArg, 'returns');
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

      // ============================================================================
      // Vector Search
      // ============================================================================
      case 'vector': {
        const apiKey = process.env.OPENAI_API_KEY;
        if (!apiKey) {
          throw new Error('OPENAI_API_KEY not set. Vector search requires an OpenAI API key.');
        }

        const vector = commerce.vector(apiKey);

        switch (action) {
          case 'search': {
            const scope = normalizeVectorScope(actionArgs[0]);
            if (!scope) {
              throw new Error('Usage: vector search <products|customers|orders|inventory> <query> [limit]');
            }

            const { query, limit } = parseSearchArgs(actionArgs.slice(1));
            if (!query) {
              throw new Error('Usage: vector search <products|customers|orders|inventory> <query> [limit]');
            }

            let results = [];
            switch (scope) {
              case 'products':
                results = await vector.searchProducts(query, limit ?? 10);
                if (jsonOutput) {
                  output({ scope, query, count: results.length, results });
                } else {
                  output(formatTable(results.map(r => ({
                    id: r.product.id.slice(0, 8) + '...',
                    name: r.product.name,
                    score: r.score.toFixed(3),
                    distance: r.distance.toFixed(4)
                  })), ['id', 'name', 'score', 'distance']));
                }
                break;
              case 'customers':
                results = await vector.searchCustomers(query, limit ?? 10);
                if (jsonOutput) {
                  output({ scope, query, count: results.length, results });
                } else {
                  output(formatTable(results.map(r => ({
                    id: r.customer.id.slice(0, 8) + '...',
                    name: `${r.customer.firstName} ${r.customer.lastName}`,
                    email: r.customer.email,
                    score: r.score.toFixed(3)
                  })), ['id', 'name', 'email', 'score']));
                }
                break;
              case 'orders':
                results = await vector.searchOrders(query, limit ?? 10);
                if (jsonOutput) {
                  output({ scope, query, count: results.length, results });
                } else {
                  output(formatTable(results.map(r => ({
                    id: r.order.id.slice(0, 8) + '...',
                    number: r.order.orderNumber,
                    status: r.order.status,
                    total: `${r.order.currency} ${r.order.totalAmount.toFixed(2)}`,
                    score: r.score.toFixed(3)
                  })), ['id', 'number', 'status', 'total', 'score']));
                }
                break;
              case 'inventory':
                results = await vector.searchInventory(query, limit ?? 10);
                if (jsonOutput) {
                  output({ scope, query, count: results.length, results });
                } else {
                  output(formatTable(results.map(r => ({
                    id: String(r.item.id),
                    sku: r.item.sku,
                    name: r.item.name,
                    score: r.score.toFixed(3)
                  })), ['id', 'sku', 'name', 'score']));
                }
                break;
            }
            break;
          }

          case 'index': {
            const scope = normalizeVectorScope(actionArgs[0]);
            const id = actionArgs[1];
            if (!scope || !id) {
              throw new Error('Usage: vector index <products|customers|orders|inventory> <id>');
            }
            await ensureWriteAllowed('vector', 'index', `${scope} ${id}`);

            const resolvedId = scope === 'inventory' ? id : await resolveId(id, scope);

            switch (scope) {
              case 'products':
                await vector.indexProduct(resolvedId);
                break;
              case 'customers':
                await vector.indexCustomer(resolvedId);
                break;
              case 'orders':
                await vector.indexOrder(resolvedId);
                break;
              case 'inventory':
                await vector.indexInventoryItem(resolvedId);
                break;
            }

            output(jsonOutput ? { scope, id: resolvedId, indexed: true } : `Indexed ${scope} ${resolvedId}`);
            break;
          }

          case 'index-all': {
            const scope = normalizeVectorScope(actionArgs[0]);
            if (!scope) {
              throw new Error('Usage: vector index-all <products|customers|orders|inventory>');
            }
            await ensureWriteAllowed('vector', 'index-all', scope);

            let count = 0;
            switch (scope) {
              case 'products':
                count = await vector.indexAllProducts();
                break;
              case 'customers':
                count = await vector.indexAllCustomers();
                break;
              case 'orders':
                count = await vector.indexAllOrders();
                break;
              case 'inventory':
                count = await vector.indexAllInventory();
                break;
            }

            output(jsonOutput ? { scope, count } : `Indexed ${count} ${scope}`);
            break;
          }

          case 'stats': {
            const stats = await vector.stats();
            if (jsonOutput) {
              output(stats);
            } else {
              output([
                `Model: ${stats.model}`,
                `Dimensions: ${stats.dimensions}`,
                `Products: ${stats.productCount}`,
                `Customers: ${stats.customerCount}`,
                `Orders: ${stats.orderCount}`,
                `Inventory: ${stats.inventoryCount}`
              ].join('\n'));
            }
            break;
          }

          case 'clear': {
            const scope = normalizeVectorScope(actionArgs[0]);
            if (!scope) {
              throw new Error('Usage: vector clear <products|customers|orders|inventory>');
            }
            await ensureWriteAllowed('vector', 'clear', scope);
            const count = await vector.clear(scope);
            output(jsonOutput ? { scope, cleared: count } : `Cleared ${count} ${scope} embeddings`);
            break;
          }

          case 'clear-all': {
            await ensureWriteAllowed('vector', 'clear-all', 'all embeddings');
            const count = await vector.clearAll();
            output(jsonOutput ? { cleared: count } : `Cleared ${count} embeddings`);
            break;
          }

          default:
            throw new Error(
              `Unknown action: vector ${action}\n\n` +
              'Available actions:\n' +
              '  search <scope> <query> [limit]\n' +
              '  index <scope> <id>\n' +
              '  index-all <scope>\n' +
              '  stats\n' +
              '  clear <scope>\n' +
              '  clear-all'
            );
        }
        break;
      }

      default:
        throw new Error(`Unknown resource: ${resource}\nUse --help for available commands.`);
    }

    if (outputPath) {
      await fs.promises.writeFile(outputPath, outputLines.join('\n'));
      if (!jsonOutput) {
        console.log(`Output written to ${outputPath}`);
      }
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
