/**
 * MCP Server for StateSet Commerce operations
 * Exposes tools for customers, orders, products, inventory, returns, and sync
 */

import { createSdkMcpServer, tool } from '@anthropic-ai/claude-agent-sdk';
import { z } from 'zod';
import { loadSyncConfig, SyncConfig, isSyncConfigured } from './sync/config.js';
import { createOutbox } from './sync/outbox.js';
import { createSyncEngine } from './sync/engine.js';
import { createSequencerClient } from './sync/client.js';

/**
 * Create the StateSet Commerce MCP server
 * @param {Object} options
 * @param {import('@stateset/embedded').Commerce} options.commerce - Commerce instance
 * @param {boolean} options.allowApply - Whether to allow destructive operations
 * @param {import('./telemetry.js').AgentTelemetry} options.telemetry - Telemetry instance
 * @param {import('./permissions.js').PermissionGate} options.permissionGate - Permission gate instance
 */
export function createStatesetMcpServer({ commerce, allowApply = false, telemetry = null, permissionGate = null }) {
  // Helper to check permissions before executing
  const checkPermission = async (toolName, params) => {
    if (!permissionGate) return { allowed: allowApply || isReadOnly(toolName) };
    return permissionGate.checkPermission(toolName, params);
  };

  // Helper to determine if a tool is read-only
  const isReadOnly = (toolName) => {
    const readOnlyTools = [
      'list_customers', 'get_customer',
      'list_orders', 'get_order',
      'list_products', 'get_product', 'get_product_variant',
      'get_stock',
      'list_returns', 'get_return',
      'list_carts', 'get_cart', 'get_shipping_rates', 'get_abandoned_carts',
      'get_sales_summary', 'get_top_products', 'get_customer_metrics',
      'get_top_customers', 'get_inventory_health', 'get_low_stock_items',
      'get_demand_forecast', 'get_revenue_forecast', 'get_order_status_breakdown',
      'get_return_metrics', 'get_exchange_rate', 'list_exchange_rates',
      'convert_currency', 'get_currency_settings', 'format_currency',
      // Tax tools
      'calculate_tax', 'get_tax_rate', 'list_tax_jurisdictions', 'list_tax_rates',
      'get_tax_settings', 'get_us_state_tax_info', 'get_customer_tax_exemptions',
      'calculate_cart_tax',
      // Promotions tools (read-only)
      'list_promotions', 'get_promotion', 'validate_coupon', 'list_coupons', 'get_active_promotions',
      // Subscriptions tools (read-only)
      'list_subscription_plans', 'get_subscription_plan', 'list_subscriptions', 'get_subscription',
      'list_billing_cycles', 'get_billing_cycle', 'get_subscription_events',
      // Sync tools (read-only)
      'sync_status', 'sync_pull', 'sync_outbox', 'sync_entity_history', 'sync_conflicts',
      // Manufacturing (read-only)
      'list_boms', 'get_bom', 'list_work_orders', 'get_work_order',
      // Payments (read-only)
      'list_payments', 'get_payment',
      // Shipments (read-only)
      'list_shipments',
      // Suppliers & Purchase Orders (read-only)
      'list_suppliers', 'list_purchase_orders',
      // Invoices (read-only)
      'list_invoices', 'get_overdue_invoices',
      // Warranties (read-only)
      'list_warranties'
    ];
    return readOnlyTools.includes(toolName);
  };

  // Helper to wrap tool execution with telemetry
  const wrapWithTelemetry = (toolName, fn) => {
    return async (params) => {
      const startTime = Date.now();
      try {
        const result = await fn(params);
        if (telemetry) {
          const duration = Date.now() - startTime;
          telemetry.logToolCall(toolName, params, result, duration);
        }
        return result;
      } catch (error) {
        if (telemetry) {
          const duration = Date.now() - startTime;
          telemetry.logToolCall(toolName, params, { error: error.message }, duration);
        }
        throw error;
      }
    };
  };

  return createSdkMcpServer({
    name: 'stateset-commerce',
    version: '1.0.0',
    tools: [
      // ============================================================================
      // Customer Tools
      // ============================================================================
      tool(
        'list_customers',
        'List all customers in the database. Returns customer details including email, name, and status.',
        {},
        async () => {
          try {
            const customers = await commerce.customers.list();
            const count = await commerce.customers.count();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count,
                  customers: customers.map(c => ({
                    id: c.id,
                    email: c.email,
                    name: `${c.firstName} ${c.lastName}`,
                    status: c.status,
                    acceptsMarketing: c.acceptsMarketing,
                    createdAt: c.createdAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_customer',
        'Get a specific customer by ID or email address.',
        {
          identifier: z.string().describe('Customer ID (UUID) or email address')
        },
        async ({ identifier }) => {
          try {
            let customer;
            if (identifier.includes('@')) {
              customer = await commerce.customers.getByEmail(identifier);
            } else {
              customer = await commerce.customers.get(identifier);
            }

            if (!customer) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: 'Customer not found' }) }] };
            }

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  customer: {
                    id: customer.id,
                    email: customer.email,
                    firstName: customer.firstName,
                    lastName: customer.lastName,
                    phone: customer.phone,
                    status: customer.status,
                    acceptsMarketing: customer.acceptsMarketing,
                    createdAt: customer.createdAt,
                    updatedAt: customer.updatedAt
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_customer',
        'Create a new customer. Requires email, first name, and last name.',
        {
          email: z.string().email().describe('Customer email address'),
          firstName: z.string().describe('Customer first name'),
          lastName: z.string().describe('Customer last name'),
          phone: z.string().optional().describe('Customer phone number'),
          acceptsMarketing: z.boolean().optional().default(false).describe('Whether customer accepts marketing')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Create operation not allowed. The --apply flag must be set to create customers.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldCreate: args
                })
              }]
            };
          }

          try {
            const customer = await commerce.customers.create(args);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Customer created successfully',
                  customer: {
                    id: customer.id,
                    email: customer.email,
                    name: `${customer.firstName} ${customer.lastName}`
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Order Tools
      // ============================================================================
      tool(
        'list_orders',
        'List all orders. Shows order number, status, customer, total amount, and item count.',
        {
          limit: z.number().optional().default(50).describe('Maximum number of orders to return')
        },
        async ({ limit }) => {
          try {
            const orders = await commerce.orders.list();
            const count = await commerce.orders.count();
            const limitedOrders = orders.slice(0, limit);

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  totalCount: count,
                  returned: limitedOrders.length,
                  orders: limitedOrders.map(o => ({
                    id: o.id,
                    orderNumber: o.orderNumber,
                    customerId: o.customerId,
                    status: o.status,
                    totalAmount: o.totalAmount,
                    currency: o.currency,
                    paymentStatus: o.paymentStatus,
                    fulfillmentStatus: o.fulfillmentStatus,
                    itemCount: o.items?.length || 0,
                    createdAt: o.createdAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_order',
        'Get a specific order by ID or order number. Returns full order details including line items.',
        {
          identifier: z.string().describe('Order ID (UUID) or order number')
        },
        async ({ identifier }) => {
          try {
            const order = await commerce.orders.get(identifier);

            if (!order) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: 'Order not found' }) }] };
            }

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  order: {
                    id: order.id,
                    orderNumber: order.orderNumber,
                    customerId: order.customerId,
                    status: order.status,
                    totalAmount: order.totalAmount,
                    currency: order.currency,
                    paymentStatus: order.paymentStatus,
                    fulfillmentStatus: order.fulfillmentStatus,
                    trackingNumber: order.trackingNumber,
                    items: order.items?.map(i => ({
                      id: i.id,
                      sku: i.sku,
                      name: i.name,
                      quantity: i.quantity,
                      unitPrice: i.unitPrice,
                      total: i.total
                    })),
                    createdAt: order.createdAt,
                    updatedAt: order.updatedAt
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_order',
        'Create a new order for a customer with line items.',
        {
          customerId: z.string().describe('Customer ID (UUID)'),
          items: z.array(z.object({
            sku: z.string().describe('Product SKU'),
            name: z.string().describe('Product name'),
            quantity: z.number().describe('Quantity'),
            unitPrice: z.number().describe('Unit price')
          })).describe('Order line items'),
          currency: z.string().optional().default('USD').describe('Currency code'),
          notes: z.string().optional().describe('Order notes')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Create operation not allowed. The --apply flag must be set to create orders.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldCreate: {
                    customerId: args.customerId,
                    itemCount: args.items.length,
                    estimatedTotal: args.items.reduce((sum, i) => sum + i.quantity * i.unitPrice, 0)
                  }
                })
              }]
            };
          }

          try {
            const order = await commerce.orders.create(args);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Order created successfully',
                  order: {
                    id: order.id,
                    orderNumber: order.orderNumber,
                    status: order.status,
                    totalAmount: order.totalAmount
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'update_order_status',
        'Update the status of an order. Valid statuses: pending, confirmed, processing, shipped, delivered, cancelled, refunded.',
        {
          orderId: z.string().describe('Order ID (UUID)'),
          status: z.enum(['pending', 'confirmed', 'processing', 'shipped', 'delivered', 'cancelled', 'refunded']).describe('New order status')
        },
        async ({ orderId, status }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Update operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldUpdate: { orderId, newStatus: status }
                })
              }]
            };
          }

          try {
            const order = await commerce.orders.updateStatus(orderId, status);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Order status updated to ${status}`,
                  order: {
                    id: order.id,
                    orderNumber: order.orderNumber,
                    status: order.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'ship_order',
        'Mark an order as shipped with optional tracking number.',
        {
          orderId: z.string().describe('Order ID (UUID)'),
          trackingNumber: z.string().optional().describe('Shipping tracking number')
        },
        async ({ orderId, trackingNumber }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Ship operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldShip: { orderId, trackingNumber }
                })
              }]
            };
          }

          try {
            const order = await commerce.orders.ship(orderId, trackingNumber);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Order shipped successfully',
                  order: {
                    id: order.id,
                    orderNumber: order.orderNumber,
                    status: order.status,
                    trackingNumber: order.trackingNumber
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'cancel_order',
        'Cancel an order. Only pending or confirmed orders can be cancelled.',
        {
          orderId: z.string().describe('Order ID (UUID)')
        },
        async ({ orderId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Cancel operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldCancel: { orderId }
                })
              }]
            };
          }

          try {
            const order = await commerce.orders.cancel(orderId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Order cancelled successfully',
                  order: {
                    id: order.id,
                    orderNumber: order.orderNumber,
                    status: order.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Product Tools
      // ============================================================================
      tool(
        'list_products',
        'List all products in the catalog.',
        {
          limit: z.number().optional().default(50).describe('Maximum number of products to return')
        },
        async ({ limit }) => {
          try {
            const products = await commerce.products.list();
            const count = await commerce.products.count();
            const limitedProducts = products.slice(0, limit);

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  totalCount: count,
                  returned: limitedProducts.length,
                  products: limitedProducts.map(p => ({
                    id: p.id,
                    name: p.name,
                    slug: p.slug,
                    status: p.status,
                    createdAt: p.createdAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_product',
        'Get a specific product by ID.',
        {
          productId: z.string().describe('Product ID (UUID)')
        },
        async ({ productId }) => {
          try {
            const product = await commerce.products.get(productId);

            if (!product) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: 'Product not found' }) }] };
            }

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  product
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_product_variant',
        'Get a product variant by SKU.',
        {
          sku: z.string().describe('Product variant SKU')
        },
        async ({ sku }) => {
          try {
            const variant = await commerce.products.getVariantBySku(sku);

            if (!variant) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: `Variant with SKU ${sku} not found` }) }] };
            }

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  variant
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_product',
        'Create a new product with optional variants.',
        {
          name: z.string().describe('Product name'),
          description: z.string().optional().describe('Product description'),
          variants: z.array(z.object({
            sku: z.string().describe('Variant SKU'),
            name: z.string().optional().describe('Variant name'),
            price: z.number().describe('Variant price'),
            compareAtPrice: z.number().optional().describe('Compare at price (original price)')
          })).optional().describe('Product variants')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Create operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldCreate: args
                })
              }]
            };
          }

          try {
            const product = await commerce.products.create(args);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Product created successfully',
                  product: {
                    id: product.id,
                    name: product.name,
                    slug: product.slug
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Inventory Tools
      // ============================================================================
      tool(
        'get_stock',
        'Get current stock level for a SKU. Shows on-hand, allocated, and available quantities.',
        {
          sku: z.string().describe('Product SKU')
        },
        async ({ sku }) => {
          try {
            const stock = await commerce.inventory.getStock(sku);

            if (!stock) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: `No inventory item found for SKU ${sku}` }) }] };
            }

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  stock: {
                    sku: stock.sku,
                    name: stock.name,
                    totalOnHand: stock.totalOnHand,
                    totalAllocated: stock.totalAllocated,
                    totalAvailable: stock.totalAvailable
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_inventory_item',
        'Create a new inventory item for a SKU.',
        {
          sku: z.string().describe('Product SKU'),
          name: z.string().describe('Item name'),
          description: z.string().optional().describe('Item description'),
          initialQuantity: z.number().optional().default(0).describe('Initial stock quantity'),
          reorderPoint: z.number().optional().describe('Reorder point threshold')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Create operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldCreate: args
                })
              }]
            };
          }

          try {
            const item = await commerce.inventory.createItem(args);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Inventory item created successfully',
                  item: {
                    id: item.id,
                    sku: item.sku,
                    name: item.name
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'adjust_inventory',
        'Adjust inventory quantity for a SKU. Use positive numbers to add stock, negative to remove.',
        {
          sku: z.string().describe('Product SKU'),
          quantity: z.number().describe('Quantity adjustment (positive to add, negative to subtract)'),
          reason: z.string().describe('Reason for adjustment (e.g., "Received shipment", "Damaged goods")')
        },
        async ({ sku, quantity, reason }) => {
          if (!allowApply) {
            // Preview the adjustment
            try {
              const stock = await commerce.inventory.getStock(sku);
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Adjust operation not allowed. The --apply flag must be set.',
                    hint: 'Run with --apply to enable write operations.',
                    wouldAdjust: {
                      sku,
                      currentOnHand: stock?.totalOnHand || 0,
                      adjustment: quantity,
                      newOnHand: (stock?.totalOnHand || 0) + quantity,
                      reason
                    }
                  })
                }]
              };
            } catch (error) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
            }
          }

          try {
            await commerce.inventory.adjust(sku, quantity, reason);
            const stock = await commerce.inventory.getStock(sku);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Inventory adjusted by ${quantity > 0 ? '+' : ''}${quantity}`,
                  stock: {
                    sku: stock.sku,
                    totalOnHand: stock.totalOnHand,
                    totalAvailable: stock.totalAvailable
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'reserve_inventory',
        'Reserve inventory for an order. Reserved stock is allocated but not yet deducted.',
        {
          sku: z.string().describe('Product SKU'),
          quantity: z.number().describe('Quantity to reserve'),
          referenceType: z.string().describe('Reference type (e.g., "order", "transfer")'),
          referenceId: z.string().describe('Reference ID (e.g., order ID)'),
          expiresInSeconds: z.number().optional().describe('Reservation expiry in seconds')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Reserve operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldReserve: args
                })
              }]
            };
          }

          try {
            const reservation = await commerce.inventory.reserve(
              args.sku,
              args.quantity,
              args.referenceType,
              args.referenceId,
              args.expiresInSeconds
            );
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Inventory reserved successfully',
                  reservation: {
                    id: reservation.id,
                    quantity: reservation.quantity,
                    status: reservation.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'confirm_reservation',
        'Confirm an inventory reservation, deducting the reserved quantity from stock.',
        {
          reservationId: z.string().describe('Reservation ID')
        },
        async ({ reservationId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Confirm operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldConfirm: { reservationId }
                })
              }]
            };
          }

          try {
            await commerce.inventory.confirmReservation(reservationId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Reservation confirmed and stock deducted'
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'release_reservation',
        'Release an inventory reservation, returning the reserved quantity to available stock.',
        {
          reservationId: z.string().describe('Reservation ID')
        },
        async ({ reservationId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Release operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldRelease: { reservationId }
                })
              }]
            };
          }

          try {
            await commerce.inventory.releaseReservation(reservationId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Reservation released and stock returned to available'
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Returns Tools
      // ============================================================================
      tool(
        'list_returns',
        'List all returns. Shows return status, order, and reason.',
        {
          limit: z.number().optional().default(50).describe('Maximum number of returns to show')
        },
        async ({ limit }) => {
          try {
            const returns = await commerce.returns.list();
            const count = await commerce.returns.count();
            const limitedReturns = returns.slice(0, limit);

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  totalCount: count,
                  returned: limitedReturns.length,
                  returns: limitedReturns.map(r => ({
                    id: r.id,
                    orderId: r.orderId,
                    status: r.status,
                    reason: r.reason,
                    createdAt: r.createdAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_return',
        'Get a specific return by ID.',
        {
          returnId: z.string().describe('Return ID (UUID)')
        },
        async ({ returnId }) => {
          try {
            const ret = await commerce.returns.get(returnId);

            if (!ret) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: 'Return not found' }) }] };
            }

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  return: ret
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_return',
        'Create a return request for an order.',
        {
          orderId: z.string().describe('Order ID (UUID)'),
          reason: z.enum(['defective', 'wrong_item', 'not_as_described', 'changed_mind', 'better_price_found', 'no_longer_needed', 'damaged', 'other']).describe('Return reason'),
          reasonDetails: z.string().optional().describe('Additional details about the return reason'),
          items: z.array(z.object({
            orderItemId: z.string().describe('Order item ID to return'),
            quantity: z.number().describe('Quantity to return')
          })).describe('Items to return')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Create operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldCreate: args
                })
              }]
            };
          }

          try {
            const ret = await commerce.returns.create(args);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Return created successfully',
                  return: {
                    id: ret.id,
                    orderId: ret.orderId,
                    status: ret.status,
                    reason: ret.reason
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'approve_return',
        'Approve a return request.',
        {
          returnId: z.string().describe('Return ID (UUID)')
        },
        async ({ returnId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Approve operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldApprove: { returnId }
                })
              }]
            };
          }

          try {
            const ret = await commerce.returns.approve(returnId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Return approved',
                  return: {
                    id: ret.id,
                    status: ret.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'reject_return',
        'Reject a return request with a reason.',
        {
          returnId: z.string().describe('Return ID (UUID)'),
          reason: z.string().describe('Reason for rejection')
        },
        async ({ returnId, reason }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Reject operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldReject: { returnId, reason }
                })
              }]
            };
          }

          try {
            const ret = await commerce.returns.reject(returnId, reason);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Return rejected',
                  return: {
                    id: ret.id,
                    status: ret.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Cart/Checkout Tools (Agentic Commerce Protocol)
      // ============================================================================
      tool(
        'list_carts',
        'List all shopping carts. Shows cart status, customer, totals, and item count.',
        {
          limit: z.number().optional().default(50).describe('Maximum number of carts to return')
        },
        async ({ limit }) => {
          try {
            const carts = await commerce.carts.list();
            const count = await commerce.carts.count();
            const limitedCarts = carts.slice(0, limit);

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  totalCount: count,
                  returned: limitedCarts.length,
                  carts: limitedCarts.map(c => ({
                    id: c.id,
                    cartNumber: c.cartNumber,
                    customerId: c.customerId,
                    customerEmail: c.customerEmail,
                    status: c.status,
                    currency: c.currency,
                    subtotal: c.subtotal,
                    grandTotal: c.grandTotal,
                    itemCount: c.itemCount,
                    createdAt: c.createdAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_cart',
        'Get a specific cart by ID or cart number. Returns full cart details including items.',
        {
          identifier: z.string().describe('Cart ID (UUID) or cart number')
        },
        async ({ identifier }) => {
          try {
            let cart;
            if (identifier.startsWith('CART-')) {
              cart = await commerce.carts.getByNumber(identifier);
            } else {
              cart = await commerce.carts.get(identifier);
            }

            if (!cart) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: 'Cart not found' }) }] };
            }

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  cart: {
                    id: cart.id,
                    cartNumber: cart.cartNumber,
                    customerId: cart.customerId,
                    customerEmail: cart.customerEmail,
                    customerName: cart.customerName,
                    status: cart.status,
                    paymentStatus: cart.paymentStatus,
                    currency: cart.currency,
                    subtotal: cart.subtotal,
                    taxAmount: cart.taxAmount,
                    shippingAmount: cart.shippingAmount,
                    discountAmount: cart.discountAmount,
                    grandTotal: cart.grandTotal,
                    paymentMethod: cart.paymentMethod,
                    shippingMethod: cart.shippingMethod,
                    couponCode: cart.couponCode,
                    items: cart.items,
                    itemCount: cart.itemCount,
                    shippingAddress: cart.shippingAddress,
                    billingAddress: cart.billingAddress,
                    createdAt: cart.createdAt,
                    updatedAt: cart.updatedAt,
                    expiresAt: cart.expiresAt
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_cart',
        'Create a new shopping cart. Can be for a guest or authenticated customer.',
        {
          customerId: z.string().optional().describe('Customer ID (UUID) for authenticated checkout'),
          customerEmail: z.string().email().optional().describe('Customer email for guest checkout'),
          customerName: z.string().optional().describe('Customer name'),
          currency: z.string().optional().default('USD').describe('Currency code'),
          expiresInMinutes: z.number().optional().describe('Cart expiration time in minutes')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Create operation not allowed. The --apply flag must be set to create carts.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldCreate: args
                })
              }]
            };
          }

          try {
            const cart = await commerce.carts.create(args);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Cart created successfully',
                  cart: {
                    id: cart.id,
                    cartNumber: cart.cartNumber,
                    status: cart.status,
                    currency: cart.currency
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'add_cart_item',
        'Add an item to a shopping cart.',
        {
          cartId: z.string().describe('Cart ID (UUID)'),
          sku: z.string().describe('Product SKU'),
          name: z.string().describe('Product name'),
          quantity: z.number().describe('Quantity to add'),
          unitPrice: z.number().describe('Unit price'),
          description: z.string().optional().describe('Item description'),
          imageUrl: z.string().optional().describe('Product image URL')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Add item operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldAdd: {
                    cartId: args.cartId,
                    sku: args.sku,
                    name: args.name,
                    quantity: args.quantity,
                    unitPrice: args.unitPrice,
                    lineTotal: args.quantity * args.unitPrice
                  }
                })
              }]
            };
          }

          try {
            const item = await commerce.carts.addItem(args.cartId, {
              sku: args.sku,
              name: args.name,
              quantity: args.quantity,
              unitPrice: args.unitPrice,
              description: args.description,
              imageUrl: args.imageUrl
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Item added to cart',
                  item: {
                    id: item.id,
                    sku: item.sku,
                    name: item.name,
                    quantity: item.quantity,
                    unitPrice: item.unitPrice,
                    total: item.total
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'update_cart_item',
        'Update the quantity of an item in the cart.',
        {
          itemId: z.string().describe('Cart item ID (UUID)'),
          quantity: z.number().describe('New quantity')
        },
        async ({ itemId, quantity }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Update operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldUpdate: { itemId, newQuantity: quantity }
                })
              }]
            };
          }

          try {
            const item = await commerce.carts.updateItem(itemId, { quantity });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Cart item updated',
                  item: {
                    id: item.id,
                    sku: item.sku,
                    quantity: item.quantity,
                    total: item.total
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'remove_cart_item',
        'Remove an item from the cart.',
        {
          itemId: z.string().describe('Cart item ID (UUID)')
        },
        async ({ itemId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Remove operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldRemove: { itemId }
                })
              }]
            };
          }

          try {
            await commerce.carts.removeItem(itemId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Item removed from cart'
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'set_cart_shipping_address',
        'Set the shipping address for a cart.',
        {
          cartId: z.string().describe('Cart ID (UUID)'),
          firstName: z.string().describe('First name'),
          lastName: z.string().describe('Last name'),
          line1: z.string().describe('Address line 1'),
          line2: z.string().optional().describe('Address line 2'),
          city: z.string().describe('City'),
          state: z.string().optional().describe('State/Province'),
          postalCode: z.string().describe('Postal/ZIP code'),
          country: z.string().describe('Country code (e.g., US)'),
          phone: z.string().optional().describe('Phone number'),
          email: z.string().email().optional().describe('Email address')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Set address operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldSet: {
                    cartId: args.cartId,
                    address: `${args.firstName} ${args.lastName}, ${args.line1}, ${args.city}, ${args.state} ${args.postalCode}, ${args.country}`
                  }
                })
              }]
            };
          }

          try {
            const { cartId, ...address } = args;
            const cart = await commerce.carts.setShippingAddress(cartId, address);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Shipping address set',
                  cart: {
                    id: cart.id,
                    shippingAddress: cart.shippingAddress
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'set_cart_payment',
        'Set the payment method for a cart.',
        {
          cartId: z.string().describe('Cart ID (UUID)'),
          paymentMethod: z.string().describe('Payment method (e.g., credit_card, paypal, crypto)'),
          paymentToken: z.string().optional().describe('Payment token from payment provider')
        },
        async ({ cartId, paymentMethod, paymentToken }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Set payment operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldSet: { cartId, paymentMethod }
                })
              }]
            };
          }

          try {
            const cart = await commerce.carts.setPayment(cartId, { paymentMethod, paymentToken });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Payment method set',
                  cart: {
                    id: cart.id,
                    paymentMethod: cart.paymentMethod,
                    paymentStatus: cart.paymentStatus
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'apply_cart_discount',
        'Apply a coupon/discount code to the cart.',
        {
          cartId: z.string().describe('Cart ID (UUID)'),
          couponCode: z.string().describe('Coupon or discount code')
        },
        async ({ cartId, couponCode }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Apply discount operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldApply: { cartId, couponCode }
                })
              }]
            };
          }

          try {
            const cart = await commerce.carts.applyDiscount(cartId, couponCode);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Discount code "${couponCode}" applied`,
                  cart: {
                    id: cart.id,
                    couponCode: cart.couponCode,
                    discountAmount: cart.discountAmount,
                    grandTotal: cart.grandTotal
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_shipping_rates',
        'Get available shipping rates for a cart based on contents and address.',
        {
          cartId: z.string().describe('Cart ID (UUID)')
        },
        async ({ cartId }) => {
          try {
            const rates = await commerce.carts.getShippingRates(cartId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  rates: rates.map(r => ({
                    id: r.id,
                    carrier: r.carrier,
                    service: r.service,
                    price: r.price,
                    currency: r.currency,
                    estimatedDays: r.estimatedDays
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'complete_checkout',
        'Complete the checkout process and convert the cart to an order. This is the final step in the checkout flow.',
        {
          cartId: z.string().describe('Cart ID (UUID)')
        },
        async ({ cartId }) => {
          if (!allowApply) {
            // Preview the checkout
            try {
              const cart = await commerce.carts.get(cartId);
              if (!cart) {
                return { content: [{ type: 'text', text: JSON.stringify({ error: 'Cart not found' }) }] };
              }
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Complete checkout operation not allowed. The --apply flag must be set.',
                    hint: 'Run with --apply to enable write operations.',
                    wouldCheckout: {
                      cartId: cart.id,
                      cartNumber: cart.cartNumber,
                      customerEmail: cart.customerEmail,
                      itemCount: cart.itemCount,
                      subtotal: cart.subtotal,
                      tax: cart.taxAmount,
                      shipping: cart.shippingAmount,
                      discount: cart.discountAmount,
                      grandTotal: cart.grandTotal,
                      currency: cart.currency,
                      paymentMethod: cart.paymentMethod
                    }
                  })
                }]
              };
            } catch (error) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
            }
          }

          try {
            const result = await commerce.carts.complete(cartId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Checkout completed successfully! Order created.',
                  result: {
                    orderId: result.orderId,
                    orderNumber: result.orderNumber,
                    cartId: result.cartId,
                    totalCharged: result.totalCharged,
                    currency: result.currency,
                    paymentId: result.paymentId
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'cancel_cart',
        'Cancel a shopping cart.',
        {
          cartId: z.string().describe('Cart ID (UUID)')
        },
        async ({ cartId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Cancel operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldCancel: { cartId }
                })
              }]
            };
          }

          try {
            const cart = await commerce.carts.cancel(cartId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Cart cancelled',
                  cart: {
                    id: cart.id,
                    cartNumber: cart.cartNumber,
                    status: cart.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'abandon_cart',
        'Mark a cart as abandoned (for recovery campaigns).',
        {
          cartId: z.string().describe('Cart ID (UUID)')
        },
        async ({ cartId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Abandon operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldAbandon: { cartId }
                })
              }]
            };
          }

          try {
            const cart = await commerce.carts.abandon(cartId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Cart marked as abandoned',
                  cart: {
                    id: cart.id,
                    cartNumber: cart.cartNumber,
                    status: cart.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_abandoned_carts',
        'Get all abandoned carts for recovery campaigns.',
        {},
        async () => {
          try {
            const carts = await commerce.carts.getAbandoned();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: carts.length,
                  carts: carts.map(c => ({
                    id: c.id,
                    cartNumber: c.cartNumber,
                    customerEmail: c.customerEmail,
                    grandTotal: c.grandTotal,
                    itemCount: c.itemCount,
                    createdAt: c.createdAt,
                    updatedAt: c.updatedAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Analytics & Forecasting Tools
      // ============================================================================
      tool(
        'get_sales_summary',
        'Get sales summary for a time period. Returns total revenue, order count, average order value, items sold, and unique customers.',
        {
          period: z.enum(['today', 'last7days', 'last30days', 'this_month', 'last_month', 'this_year', 'all_time']).optional().default('last30days').describe('Time period for the summary')
        },
        async ({ period }) => {
          try {
            const summary = await commerce.analytics.salesSummary({ period });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  period,
                  summary: {
                    totalRevenue: summary.totalRevenue,
                    orderCount: summary.orderCount,
                    averageOrderValue: summary.averageOrderValue,
                    itemsSold: summary.itemsSold,
                    uniqueCustomers: summary.uniqueCustomers
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_top_products',
        'Get top selling products by revenue or units sold.',
        {
          period: z.enum(['today', 'last7days', 'last30days', 'this_month', 'last_month', 'this_year', 'all_time']).optional().default('last30days').describe('Time period'),
          limit: z.number().optional().default(10).describe('Maximum number of products to return')
        },
        async ({ period, limit }) => {
          try {
            const products = await commerce.analytics.topProducts({ period, limit });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  period,
                  count: products.length,
                  products: products.map(p => ({
                    sku: p.sku,
                    name: p.name,
                    unitsSold: p.unitsSold,
                    revenue: p.revenue,
                    orderCount: p.orderCount
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_customer_metrics',
        'Get customer metrics including total customers, new customers, returning customers, and average lifetime value.',
        {
          period: z.enum(['today', 'last7days', 'last30days', 'this_month', 'last_month', 'this_year', 'all_time']).optional().default('last30days').describe('Time period')
        },
        async ({ period }) => {
          try {
            const metrics = await commerce.analytics.customerMetrics({ period });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  period,
                  metrics: {
                    totalCustomers: metrics.totalCustomers,
                    newCustomers: metrics.newCustomers,
                    returningCustomers: metrics.returningCustomers,
                    averageLifetimeValue: metrics.averageLifetimeValue,
                    averageOrdersPerCustomer: metrics.averageOrdersPerCustomer
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_top_customers',
        'Get top customers by total spend.',
        {
          period: z.enum(['today', 'last7days', 'last30days', 'this_month', 'last_month', 'this_year', 'all_time']).optional().default('all_time').describe('Time period'),
          limit: z.number().optional().default(10).describe('Maximum number of customers to return')
        },
        async ({ period, limit }) => {
          try {
            const customers = await commerce.analytics.topCustomers({ period, limit });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  period,
                  count: customers.length,
                  customers: customers.map(c => ({
                    customerId: c.customerId,
                    name: c.name,
                    email: c.email,
                    orderCount: c.orderCount,
                    totalSpent: c.totalSpent,
                    averageOrderValue: c.averageOrderValue
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_inventory_health',
        'Get inventory health summary showing total SKUs, in-stock, low stock, and out of stock counts.',
        {},
        async () => {
          try {
            const health = await commerce.analytics.inventoryHealth();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  health: {
                    totalSkus: health.totalSkus,
                    inStockSkus: health.inStockSkus,
                    lowStockSkus: health.lowStockSkus,
                    outOfStockSkus: health.outOfStockSkus,
                    totalValue: health.totalValue
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_low_stock_items',
        'Get items that are low in stock or approaching reorder point.',
        {
          threshold: z.number().optional().describe('Stock threshold to consider as low (default: 10)')
        },
        async ({ threshold }) => {
          try {
            const items = await commerce.analytics.lowStockItems(threshold);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: items.length,
                  items: items.map(i => ({
                    sku: i.sku,
                    name: i.name,
                    onHand: i.onHand,
                    allocated: i.allocated,
                    available: i.available,
                    reorderPoint: i.reorderPoint,
                    averageDailySales: i.averageDailySales,
                    daysOfStock: i.daysOfStock
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_demand_forecast',
        'Get demand forecast for inventory items based on historical sales. Predicts future demand and days until stockout.',
        {
          skus: z.array(z.string()).optional().describe('List of SKUs to forecast (all items if not specified)'),
          daysAhead: z.number().optional().default(30).describe('Number of days to forecast ahead')
        },
        async ({ skus, daysAhead }) => {
          try {
            const forecasts = await commerce.analytics.demandForecast(skus, daysAhead);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  daysAhead,
                  count: forecasts.length,
                  forecasts: forecasts.map(f => ({
                    sku: f.sku,
                    name: f.name,
                    averageDailyDemand: f.averageDailyDemand,
                    forecastedDemand: f.forecastedDemand,
                    confidence: f.confidence,
                    currentStock: f.currentStock,
                    daysUntilStockout: f.daysUntilStockout,
                    recommendedReorderQty: f.recommendedReorderQty,
                    trend: f.trend
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_revenue_forecast',
        'Get revenue forecast based on historical trends.',
        {
          periodsAhead: z.number().optional().default(3).describe('Number of periods to forecast'),
          granularity: z.enum(['day', 'week', 'month']).optional().default('month').describe('Time granularity')
        },
        async ({ periodsAhead, granularity }) => {
          try {
            const forecasts = await commerce.analytics.revenueForecast(periodsAhead, granularity);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  granularity,
                  count: forecasts.length,
                  forecasts: forecasts.map(f => ({
                    period: f.period,
                    forecastedRevenue: f.forecastedRevenue,
                    lowerBound: f.lowerBound,
                    upperBound: f.upperBound,
                    confidenceLevel: f.confidenceLevel,
                    basedOnPeriods: f.basedOnPeriods
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_order_status_breakdown',
        'Get breakdown of orders by status.',
        {
          period: z.enum(['today', 'last7days', 'last30days', 'this_month', 'last_month', 'this_year', 'all_time']).optional().default('last30days').describe('Time period')
        },
        async ({ period }) => {
          try {
            const breakdown = await commerce.analytics.orderStatusBreakdown({ period });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  period,
                  breakdown: {
                    pending: breakdown.pending,
                    confirmed: breakdown.confirmed,
                    processing: breakdown.processing,
                    shipped: breakdown.shipped,
                    delivered: breakdown.delivered,
                    cancelled: breakdown.cancelled,
                    refunded: breakdown.refunded
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_return_metrics',
        'Get return metrics including return rate and total refunds.',
        {
          period: z.enum(['today', 'last7days', 'last30days', 'this_month', 'last_month', 'this_year', 'all_time']).optional().default('last30days').describe('Time period')
        },
        async ({ period }) => {
          try {
            const metrics = await commerce.analytics.returnMetrics({ period });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  period,
                  metrics: {
                    totalReturns: metrics.totalReturns,
                    returnRatePercent: metrics.returnRatePercent,
                    totalRefunded: metrics.totalRefunded
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Currency & Exchange Rate Tools
      // ============================================================================
      tool(
        'get_exchange_rate',
        'Get the exchange rate between two currencies.',
        {
          from: z.string().describe('Source currency code (e.g., USD, EUR, GBP)'),
          to: z.string().describe('Target currency code (e.g., EUR, USD, GBP)')
        },
        async ({ from, to }) => {
          try {
            const rate = await commerce.currency.getRate(from.toUpperCase(), to.toUpperCase());
            if (!rate) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    success: false,
                    error: `No exchange rate found for ${from} to ${to}`
                  }, null, 2)
                }]
              };
            }
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  rate: {
                    baseCurrency: rate.baseCurrency,
                    quoteCurrency: rate.quoteCurrency,
                    rate: rate.rate,
                    source: rate.source,
                    rateAt: rate.rateAt
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'list_exchange_rates',
        'List all available exchange rates, optionally filtered by base currency.',
        {
          baseCurrency: z.string().optional().describe('Filter by base currency code (e.g., USD)')
        },
        async ({ baseCurrency }) => {
          try {
            let rates;
            if (baseCurrency) {
              rates = await commerce.currency.getRatesFor(baseCurrency.toUpperCase());
            } else {
              rates = await commerce.currency.listRates();
            }
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: rates.length,
                  rates: rates.map(r => ({
                    baseCurrency: r.baseCurrency,
                    quoteCurrency: r.quoteCurrency,
                    rate: r.rate,
                    source: r.source,
                    rateAt: r.rateAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'convert_currency',
        'Convert an amount from one currency to another using current exchange rates.',
        {
          from: z.string().describe('Source currency code (e.g., USD)'),
          to: z.string().describe('Target currency code (e.g., EUR)'),
          amount: z.number().describe('Amount to convert')
        },
        async ({ from, to, amount }) => {
          try {
            const result = await commerce.currency.convert({
              from: from.toUpperCase(),
              to: to.toUpperCase(),
              amount
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  conversion: {
                    originalAmount: result.originalAmount,
                    originalCurrency: result.originalCurrency,
                    convertedAmount: result.convertedAmount,
                    targetCurrency: result.targetCurrency,
                    rate: result.rate,
                    inverseRate: result.inverseRate,
                    rateAt: result.rateAt
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'set_exchange_rate',
        'Set or update an exchange rate between two currencies.',
        {
          baseCurrency: z.string().describe('Base currency code (e.g., USD)'),
          quoteCurrency: z.string().describe('Quote currency code (e.g., EUR)'),
          rate: z.number().describe('Exchange rate (e.g., 0.92 for USD to EUR)'),
          source: z.string().optional().default('manual').describe('Source of the rate (e.g., manual, api)')
        },
        async ({ baseCurrency, quoteCurrency, rate, source }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Write operations require --apply flag. Would set rate: 1 ' + baseCurrency + ' = ' + rate + ' ' + quoteCurrency,
                  preview: { baseCurrency, quoteCurrency, rate, source }
                }, null, 2)
              }]
            };
          }
          try {
            const result = await commerce.currency.setRate({
              baseCurrency: baseCurrency.toUpperCase(),
              quoteCurrency: quoteCurrency.toUpperCase(),
              rate,
              source
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Exchange rate set: 1 ${result.baseCurrency} = ${result.rate} ${result.quoteCurrency}`,
                  rate: {
                    id: result.id,
                    baseCurrency: result.baseCurrency,
                    quoteCurrency: result.quoteCurrency,
                    rate: result.rate,
                    source: result.source,
                    rateAt: result.rateAt
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_currency_settings',
        'Get the store currency settings including base currency and enabled currencies.',
        {},
        async () => {
          try {
            const settings = await commerce.currency.getSettings();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  settings: {
                    baseCurrency: settings.baseCurrency,
                    enabledCurrencies: settings.enabledCurrencies,
                    autoConvert: settings.autoConvert,
                    roundingMode: settings.roundingMode
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'set_base_currency',
        'Set the store\'s base currency.',
        {
          currency: z.string().describe('Currency code to set as base (e.g., USD, EUR)')
        },
        async ({ currency }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Write operations require --apply flag. Would set base currency to: ' + currency,
                  preview: { baseCurrency: currency.toUpperCase() }
                }, null, 2)
              }]
            };
          }
          try {
            const settings = await commerce.currency.setBaseCurrency(currency.toUpperCase());
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Base currency set to ${settings.baseCurrency}`,
                  settings: {
                    baseCurrency: settings.baseCurrency,
                    enabledCurrencies: settings.enabledCurrencies
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'enable_currencies',
        'Enable currencies for the store.',
        {
          currencies: z.array(z.string()).describe('List of currency codes to enable (e.g., ["USD", "EUR", "GBP"])')
        },
        async ({ currencies }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Write operations require --apply flag. Would enable currencies: ' + currencies.join(', '),
                  preview: { currencies: currencies.map(c => c.toUpperCase()) }
                }, null, 2)
              }]
            };
          }
          try {
            const settings = await commerce.currency.enableCurrencies(currencies.map(c => c.toUpperCase()));
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Enabled currencies: ${settings.enabledCurrencies.join(', ')}`,
                  settings: {
                    baseCurrency: settings.baseCurrency,
                    enabledCurrencies: settings.enabledCurrencies
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'format_currency',
        'Format an amount with currency symbol.',
        {
          amount: z.number().describe('Amount to format'),
          currency: z.string().describe('Currency code (e.g., USD, EUR)')
        },
        async ({ amount, currency }) => {
          try {
            const formatted = await commerce.currency.format(amount, currency.toUpperCase());
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  amount,
                  currency: currency.toUpperCase(),
                  formatted
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Tax Calculation Tools
      // ============================================================================
      tool(
        'calculate_tax',
        'Calculate tax for a transaction based on shipping address and line items. Supports US sales tax, EU VAT, and Canadian GST/HST/PST.',
        {
          items: z.array(z.object({
            id: z.string().describe('Line item identifier'),
            unitPrice: z.number().describe('Unit price per item'),
            quantity: z.number().describe('Quantity of items'),
            taxCategory: z.string().optional().default('standard').describe('Tax category: standard, reduced, exempt, digital, food, clothing, medical')
          })).describe('Line items to calculate tax for'),
          shippingAddress: z.object({
            country: z.string().describe('Country code (e.g., US, DE, CA)'),
            state: z.string().optional().describe('State/Province code (e.g., CA, TX, ON)'),
            city: z.string().optional().describe('City name'),
            postalCode: z.string().optional().describe('Postal/ZIP code')
          }).describe('Shipping address for tax jurisdiction determination'),
          shippingAmount: z.number().optional().describe('Shipping amount (may be taxable)'),
          customerId: z.string().optional().describe('Customer ID for exemption lookup')
        },
        async ({ items, shippingAddress, shippingAmount, customerId }) => {
          try {
            const result = await commerce.tax.calculate({
              lineItems: items.map(item => ({
                id: item.id,
                unitPrice: item.unitPrice,
                quantity: item.quantity,
                discountAmount: 0,
                taxCategory: item.taxCategory || 'standard'
              })),
              shippingAddress: {
                country: shippingAddress.country,
                state: shippingAddress.state,
                city: shippingAddress.city,
                postalCode: shippingAddress.postalCode
              },
              shippingAmount,
              customerId
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  calculation: {
                    subtotal: result.subtotal,
                    totalTax: result.totalTax,
                    shippingTax: result.shippingTax,
                    total: result.total,
                    exemptionsApplied: result.exemptionsApplied,
                    taxBreakdown: result.taxBreakdown.map(b => ({
                      jurisdictionName: b.jurisdictionName,
                      taxType: b.taxType,
                      rateName: b.rateName,
                      rate: b.rate,
                      taxableAmount: b.taxableAmount,
                      taxAmount: b.taxAmount
                    })),
                    lineItemTaxes: result.lineItemTaxes.map(lit => ({
                      lineItemId: lit.lineItemId,
                      taxableAmount: lit.taxableAmount,
                      taxAmount: lit.taxAmount,
                      effectiveRate: lit.effectiveRate,
                      isExempt: lit.isExempt
                    }))
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_tax_rate',
        'Get the effective tax rate for a shipping address and product category.',
        {
          country: z.string().describe('Country code (e.g., US, DE, CA)'),
          state: z.string().optional().describe('State/Province code (e.g., CA, TX, ON)'),
          city: z.string().optional().describe('City name'),
          taxCategory: z.string().optional().default('standard').describe('Product tax category: standard, reduced, exempt, digital, food, clothing, medical')
        },
        async ({ country, state, city, taxCategory }) => {
          try {
            const rate = await commerce.tax.getEffectiveRate(
              { country, state, city },
              taxCategory || 'standard'
            );
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  address: { country, state, city },
                  taxCategory: taxCategory || 'standard',
                  effectiveRate: rate,
                  effectiveRatePercent: (rate * 100).toFixed(2) + '%'
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'list_tax_jurisdictions',
        'List tax jurisdictions with optional filtering by country or level.',
        {
          countryCode: z.string().optional().describe('Filter by country code (e.g., US, DE, CA)'),
          level: z.string().optional().describe('Filter by level: country, state, county, city, district')
        },
        async ({ countryCode, level }) => {
          try {
            const jurisdictions = await commerce.tax.listJurisdictions({
              countryCode,
              level,
              activeOnly: true
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: jurisdictions.length,
                  jurisdictions: jurisdictions.map(j => ({
                    id: j.id,
                    code: j.code,
                    name: j.name,
                    level: j.level,
                    countryCode: j.countryCode,
                    stateCode: j.stateCode
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'list_tax_rates',
        'List tax rates for a jurisdiction or all active rates.',
        {
          jurisdictionId: z.string().optional().describe('Filter by jurisdiction ID'),
          taxType: z.string().optional().describe('Filter by tax type: sales_tax, vat, gst, hst, pst, qst'),
          productCategory: z.string().optional().describe('Filter by product category: standard, reduced, exempt, digital')
        },
        async ({ jurisdictionId, taxType, productCategory }) => {
          try {
            const rates = await commerce.tax.listRates({
              jurisdictionId,
              taxType,
              productCategory,
              activeOnly: true
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: rates.length,
                  rates: rates.map(r => ({
                    id: r.id,
                    jurisdictionId: r.jurisdictionId,
                    taxType: r.taxType,
                    productCategory: r.productCategory,
                    rate: r.rate,
                    ratePercent: (r.rate * 100).toFixed(2) + '%',
                    name: r.name,
                    isCompound: r.isCompound,
                    effectiveFrom: r.effectiveFrom
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_tax_settings',
        'Get the store tax calculation settings.',
        {},
        async () => {
          try {
            const settings = await commerce.tax.getSettings();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  settings: {
                    enabled: settings.enabled,
                    calculationMethod: settings.calculationMethod,
                    compoundMethod: settings.compoundMethod,
                    taxShipping: settings.taxShipping,
                    taxHandling: settings.taxHandling,
                    defaultProductCategory: settings.defaultProductCategory,
                    roundingMode: settings.roundingMode,
                    decimalPlaces: settings.decimalPlaces,
                    taxProvider: settings.taxProvider
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_us_state_tax_info',
        'Get pre-configured US state sales tax information including rates and rules.',
        {
          stateCode: z.string().describe('US state code (e.g., CA, TX, NY)')
        },
        async ({ stateCode }) => {
          try {
            // This is a static lookup, doesn't need commerce instance
            const info = {
              'CA': { stateCode: 'CA', stateName: 'California', stateRate: 0.0725, hasLocalTaxes: true, originBased: true, taxShipping: false, taxClothing: true, taxFood: false },
              'TX': { stateCode: 'TX', stateName: 'Texas', stateRate: 0.0625, hasLocalTaxes: true, originBased: true, taxShipping: true, taxClothing: true, taxFood: false },
              'NY': { stateCode: 'NY', stateName: 'New York', stateRate: 0.04, hasLocalTaxes: true, originBased: false, taxShipping: true, taxClothing: false, taxFood: false },
              'FL': { stateCode: 'FL', stateName: 'Florida', stateRate: 0.06, hasLocalTaxes: true, originBased: false, taxShipping: true, taxClothing: true, taxFood: false },
              'WA': { stateCode: 'WA', stateName: 'Washington', stateRate: 0.065, hasLocalTaxes: true, originBased: false, taxShipping: true, taxClothing: true, taxFood: false },
              'OR': { stateCode: 'OR', stateName: 'Oregon', stateRate: 0, hasLocalTaxes: false, originBased: false, taxShipping: false, taxClothing: false, taxFood: false },
              'DE': { stateCode: 'DE', stateName: 'Delaware', stateRate: 0, hasLocalTaxes: false, originBased: false, taxShipping: false, taxClothing: false, taxFood: false },
              'MT': { stateCode: 'MT', stateName: 'Montana', stateRate: 0, hasLocalTaxes: false, originBased: false, taxShipping: false, taxClothing: false, taxFood: false },
              'NH': { stateCode: 'NH', stateName: 'New Hampshire', stateRate: 0, hasLocalTaxes: false, originBased: false, taxShipping: false, taxClothing: false, taxFood: false },
              'AK': { stateCode: 'AK', stateName: 'Alaska', stateRate: 0, hasLocalTaxes: true, originBased: false, taxShipping: false, taxClothing: false, taxFood: false }
            };
            const stateInfo = info[stateCode.toUpperCase()];
            if (!stateInfo) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    success: false,
                    error: `State ${stateCode} not found. Try: CA, TX, NY, FL, WA, OR, DE, MT, NH, AK`
                  }, null, 2)
                }]
              };
            }
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  stateInfo: {
                    ...stateInfo,
                    stateRatePercent: (stateInfo.stateRate * 100).toFixed(2) + '%'
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_customer_tax_exemptions',
        'Get active tax exemptions for a customer.',
        {
          customerId: z.string().describe('Customer ID')
        },
        async ({ customerId }) => {
          try {
            const exemptions = await commerce.tax.getCustomerExemptions(customerId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: exemptions.length,
                  exemptions: exemptions.map(e => ({
                    id: e.id,
                    exemptionType: e.exemptionType,
                    certificateNumber: e.certificateNumber,
                    issuingAuthority: e.issuingAuthority,
                    effectiveFrom: e.effectiveFrom,
                    expiresAt: e.expiresAt,
                    verified: e.verified
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_tax_exemption',
        'Create a tax exemption certificate for a customer.',
        {
          customerId: z.string().describe('Customer ID'),
          exemptionType: z.string().describe('Type: resale, non_profit, government, educational, religious, medical, manufacturing, agricultural, export, diplomatic'),
          certificateNumber: z.string().optional().describe('Exemption certificate number'),
          issuingAuthority: z.string().optional().describe('Issuing authority (e.g., state name)'),
          expiresAt: z.string().optional().describe('Expiration date (YYYY-MM-DD)')
        },
        async ({ customerId, exemptionType, certificateNumber, issuingAuthority, expiresAt }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Write operations require --apply flag. Would create tax exemption for customer.',
                  preview: { customerId, exemptionType, certificateNumber, issuingAuthority }
                }, null, 2)
              }]
            };
          }
          try {
            const today = new Date().toISOString().split('T')[0];
            const exemption = await commerce.tax.createExemption({
              customerId,
              exemptionType,
              certificateNumber,
              issuingAuthority,
              effectiveFrom: today,
              expiresAt: expiresAt || null,
              jurisdictionIds: [],
              exemptCategories: []
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Tax exemption created for customer`,
                  exemption: {
                    id: exemption.id,
                    customerId: exemption.customerId,
                    exemptionType: exemption.exemptionType,
                    certificateNumber: exemption.certificateNumber,
                    effectiveFrom: exemption.effectiveFrom
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Cart Tax Calculation (Checkout Integration)
      // ============================================================================
      tool(
        'calculate_cart_tax',
        'Calculate and apply tax to a cart based on its shipping address. Must set shipping address first. Returns tax breakdown and updates cart totals.',
        {
          cartId: z.string().describe('Cart ID to calculate tax for')
        },
        async ({ cartId }) => {
          try {
            const result = await commerce.calculateCartTax(cartId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  cartId,
                  tax: {
                    subtotal: result.subtotal,
                    totalTax: result.totalTax,
                    total: result.total,
                    taxInclusive: result.taxInclusive,
                    breakdown: result.taxBreakdown?.map(b => ({
                      jurisdiction: b.jurisdictionName,
                      rate: `${(b.rate * 100).toFixed(2)}%`,
                      taxAmount: b.taxAmount
                    })) || []
                  },
                  lineItems: result.lineItemTaxes?.map(item => ({
                    id: item.lineItemId,
                    subtotal: item.subtotal,
                    taxAmount: item.taxAmount,
                    total: item.total
                  })) || []
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Promotions & Discounts Tools
      // ============================================================================
      tool(
        'list_promotions',
        'List all promotions. Shows active, paused, and scheduled promotions with their discount details.',
        {
          status: z.enum(['active', 'paused', 'draft', 'expired', 'scheduled']).optional().describe('Filter by promotion status'),
          type: z.enum(['percentage_off', 'fixed_amount_off', 'buy_x_get_y', 'free_shipping', 'tiered_discount']).optional().describe('Filter by promotion type')
        },
        async ({ status, type }) => {
          try {
            const filter = {};
            if (status) filter.status = status;
            if (type) filter.promotionType = type;

            const promotions = await commerce.promotions().list(filter);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: promotions.length,
                  promotions: promotions.map(p => ({
                    id: p.id,
                    code: p.code,
                    name: p.name,
                    type: p.promotionType,
                    status: p.status,
                    trigger: p.trigger,
                    percentageOff: p.percentageOff,
                    fixedAmountOff: p.fixedAmountOff,
                    startsAt: p.startsAt,
                    endsAt: p.endsAt,
                    usageCount: p.usageCount
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_promotion',
        'Get a promotion by ID or internal code.',
        {
          identifier: z.string().describe('Promotion ID (UUID) or internal code')
        },
        async ({ identifier }) => {
          try {
            let promotion;
            // Try as UUID first, then as code
            try {
              promotion = await commerce.promotions().get(identifier);
            } catch {
              promotion = await commerce.promotions().getByCode(identifier);
            }

            if (!promotion) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: 'Promotion not found' }) }] };
            }

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  promotion: {
                    id: promotion.id,
                    code: promotion.code,
                    name: promotion.name,
                    description: promotion.description,
                    type: promotion.promotionType,
                    status: promotion.status,
                    trigger: promotion.trigger,
                    target: promotion.target,
                    percentageOff: promotion.percentageOff,
                    fixedAmountOff: promotion.fixedAmountOff,
                    maxDiscount: promotion.maxDiscountAmount,
                    startsAt: promotion.startsAt,
                    endsAt: promotion.endsAt,
                    usageCount: promotion.usageCount,
                    usageLimit: promotion.totalUsageLimit,
                    conditions: promotion.conditions
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_promotion',
        'Create a new promotion. Supports percentage off, fixed amount off, BOGO, free shipping, and tiered discounts.',
        {
          name: z.string().describe('Promotion name (e.g., "Summer Sale")'),
          type: z.enum(['percentage_off', 'fixed_amount_off', 'buy_x_get_y', 'free_shipping', 'tiered_discount']).describe('Type of discount'),
          trigger: z.enum(['automatic', 'coupon_code', 'both']).default('automatic').describe('How the promotion is triggered'),
          percentageOff: z.number().min(0).max(1).optional().describe('Percentage discount (0.20 = 20% off)'),
          fixedAmountOff: z.number().optional().describe('Fixed amount discount in dollars'),
          maxDiscountAmount: z.number().optional().describe('Maximum discount cap'),
          description: z.string().optional().describe('Public description'),
          startsAt: z.string().optional().describe('Start date (ISO 8601)'),
          endsAt: z.string().optional().describe('End date (ISO 8601)')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Create operation not allowed. The --apply flag must be set to create promotions.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldCreate: args
                })
              }]
            };
          }

          try {
            // Map type to enum
            const typeMap = {
              'percentage_off': 'PercentageOff',
              'fixed_amount_off': 'FixedAmountOff',
              'buy_x_get_y': 'BuyXGetY',
              'free_shipping': 'FreeShipping',
              'tiered_discount': 'TieredDiscount'
            };

            const triggerMap = {
              'automatic': 'Automatic',
              'coupon_code': 'CouponCode',
              'both': 'Both'
            };

            const promotion = await commerce.promotions().create({
              name: args.name,
              description: args.description,
              promotionType: typeMap[args.type],
              trigger: triggerMap[args.trigger],
              target: 'Order',
              stacking: 'Stackable',
              percentageOff: args.percentageOff,
              fixedAmountOff: args.fixedAmountOff,
              maxDiscountAmount: args.maxDiscountAmount,
              startsAt: args.startsAt ? new Date(args.startsAt) : null,
              endsAt: args.endsAt ? new Date(args.endsAt) : null,
              priority: 1
            });

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Promotion created successfully (status: draft)',
                  hint: 'Use activate_promotion to make it live',
                  promotion: {
                    id: promotion.id,
                    code: promotion.code,
                    name: promotion.name,
                    type: promotion.promotionType,
                    status: promotion.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'activate_promotion',
        'Activate a promotion to make it available for use.',
        {
          promotionId: z.string().describe('Promotion ID to activate')
        },
        async ({ promotionId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Activate operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldActivate: promotionId
                })
              }]
            };
          }

          try {
            const promotion = await commerce.promotions().activate(promotionId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Promotion activated',
                  promotion: {
                    id: promotion.id,
                    name: promotion.name,
                    status: promotion.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'deactivate_promotion',
        'Pause/deactivate a promotion.',
        {
          promotionId: z.string().describe('Promotion ID to deactivate')
        },
        async ({ promotionId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Deactivate operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldDeactivate: promotionId
                })
              }]
            };
          }

          try {
            const promotion = await commerce.promotions().deactivate(promotionId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Promotion deactivated',
                  promotion: {
                    id: promotion.id,
                    name: promotion.name,
                    status: promotion.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_coupon',
        'Create a coupon code for a promotion.',
        {
          promotionId: z.string().describe('Promotion ID to create coupon for'),
          code: z.string().describe('Coupon code (e.g., "SUMMER25")'),
          usageLimit: z.number().optional().describe('Maximum number of times this coupon can be used'),
          perCustomerLimit: z.number().optional().describe('Max uses per customer'),
          startsAt: z.string().optional().describe('Coupon valid from (ISO 8601)'),
          endsAt: z.string().optional().describe('Coupon valid until (ISO 8601)')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Create operation not allowed. The --apply flag must be set to create coupons.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldCreate: args
                })
              }]
            };
          }

          try {
            const coupon = await commerce.promotions().createCoupon({
              promotionId: args.promotionId,
              code: args.code.toUpperCase(),
              usageLimit: args.usageLimit,
              perCustomerLimit: args.perCustomerLimit,
              startsAt: args.startsAt ? new Date(args.startsAt) : null,
              endsAt: args.endsAt ? new Date(args.endsAt) : null
            });

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Coupon code created',
                  coupon: {
                    id: coupon.id,
                    code: coupon.code,
                    promotionId: coupon.promotionId,
                    usageLimit: coupon.usageLimit,
                    usageCount: coupon.usageCount,
                    status: coupon.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'validate_coupon',
        'Check if a coupon code is valid and can be used.',
        {
          code: z.string().describe('Coupon code to validate')
        },
        async ({ code }) => {
          try {
            const coupon = await commerce.promotions().validateCoupon(code.toUpperCase());

            if (!coupon) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    success: true,
                    valid: false,
                    message: 'Invalid or expired coupon code'
                  }, null, 2)
                }]
              };
            }

            // Get the promotion to show discount details
            const promotion = await commerce.promotions().get(coupon.promotionId);

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  valid: true,
                  coupon: {
                    code: coupon.code,
                    promotionName: promotion?.name,
                    discountType: promotion?.promotionType,
                    percentageOff: promotion?.percentageOff,
                    fixedAmountOff: promotion?.fixedAmountOff,
                    usageRemaining: coupon.usageLimit ? coupon.usageLimit - coupon.usageCount : 'unlimited'
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'list_coupons',
        'List coupon codes with optional filters.',
        {
          promotionId: z.string().optional().describe('Filter by promotion ID'),
          status: z.enum(['active', 'expired', 'depleted', 'disabled']).optional().describe('Filter by status')
        },
        async ({ promotionId, status }) => {
          try {
            const filter = {};
            if (promotionId) filter.promotionId = promotionId;
            if (status) filter.status = status;

            const coupons = await commerce.promotions().listCoupons(filter);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: coupons.length,
                  coupons: coupons.map(c => ({
                    id: c.id,
                    code: c.code,
                    promotionId: c.promotionId,
                    status: c.status,
                    usageCount: c.usageCount,
                    usageLimit: c.usageLimit,
                    startsAt: c.startsAt,
                    endsAt: c.endsAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_active_promotions',
        'Get all currently active promotions.',
        {},
        async () => {
          try {
            const promotions = await commerce.promotions().getActive();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count: promotions.length,
                  promotions: promotions.map(p => ({
                    id: p.id,
                    name: p.name,
                    code: p.code,
                    type: p.promotionType,
                    trigger: p.trigger,
                    percentageOff: p.percentageOff,
                    fixedAmountOff: p.fixedAmountOff,
                    endsAt: p.endsAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'apply_cart_promotions',
        'Calculate and apply all applicable promotions to a cart. Uses coupon codes on the cart and automatic promotions.',
        {
          cartId: z.string().describe('Cart ID to apply promotions to')
        },
        async ({ cartId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Apply operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldApplyTo: cartId
                })
              }]
            };
          }

          try {
            const result = await commerce.applyCartPromotions(cartId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  cartId,
                  originalSubtotal: result.originalSubtotal,
                  totalDiscount: result.totalDiscount,
                  discountedSubtotal: result.discountedSubtotal,
                  shippingDiscount: result.shippingDiscount,
                  grandTotal: result.grandTotal,
                  appliedPromotions: result.appliedPromotions.map(p => ({
                    name: p.promotionName,
                    type: p.discountType,
                    discountAmount: p.discountAmount,
                    description: p.description,
                    couponCode: p.couponCode
                  })),
                  rejectedPromotions: result.rejectedPromotions?.map(p => ({
                    name: p.promotionName,
                    reason: p.rejectionReason
                  })) || []
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Subscription Tools
      // ============================================================================

      tool(
        'list_subscription_plans',
        'List all subscription plans. Filter by status (draft, active, archived) or billing interval.',
        {
          status: z.enum(['draft', 'active', 'archived']).optional().describe('Filter by plan status'),
          billingInterval: z.enum(['weekly', 'biweekly', 'monthly', 'bimonthly', 'quarterly', 'semiannual', 'annual']).optional().describe('Filter by billing interval')
        },
        async ({ status, billingInterval }) => {
          try {
            const plans = await commerce.listSubscriptionPlans({ status, billingInterval });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  count: plans.length,
                  plans: plans.map(p => ({
                    id: p.id,
                    code: p.code,
                    name: p.name,
                    status: p.status,
                    billingInterval: p.billingInterval,
                    price: p.price,
                    currency: p.currency,
                    trialDays: p.trialDays
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_subscription_plan',
        'Get details for a specific subscription plan.',
        {
          planId: z.string().describe('Plan ID or code')
        },
        async ({ planId }) => {
          try {
            const plan = await commerce.getSubscriptionPlan(planId);
            if (!plan) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: 'Plan not found' }) }] };
            }
            return { content: [{ type: 'text', text: JSON.stringify(plan, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_subscription_plan',
        'Create a new subscription plan. Requires --apply flag.',
        {
          name: z.string().describe('Plan name'),
          billingInterval: z.enum(['weekly', 'biweekly', 'monthly', 'bimonthly', 'quarterly', 'semiannual', 'annual']).describe('Billing interval'),
          price: z.number().describe('Price per billing cycle'),
          currency: z.string().optional().describe('Currency code (default: USD)'),
          trialDays: z.number().optional().describe('Trial period in days'),
          description: z.string().optional().describe('Plan description'),
          setupFee: z.number().optional().describe('One-time setup fee')
        },
        async ({ name, billingInterval, price, currency, trialDays, description, setupFee }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Create operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldCreate: { name, billingInterval, price }
                })
              }]
            };
          }

          try {
            const plan = await commerce.createSubscriptionPlan({
              name,
              billingInterval,
              price: price.toString(),
              currency,
              trialDays,
              description,
              setupFee: setupFee?.toString()
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Created subscription plan "${plan.name}"`,
                  plan
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'activate_subscription_plan',
        'Activate a subscription plan (make it available for new subscriptions). Requires --apply flag.',
        {
          planId: z.string().describe('Plan ID to activate')
        },
        async ({ planId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Activate operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldActivate: planId
                })
              }]
            };
          }

          try {
            const plan = await commerce.activateSubscriptionPlan(planId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Plan "${plan.name}" activated`,
                  plan
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'archive_subscription_plan',
        'Archive a subscription plan (no new subscriptions, existing ones continue). Requires --apply flag.',
        {
          planId: z.string().describe('Plan ID to archive')
        },
        async ({ planId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Archive operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldArchive: planId
                })
              }]
            };
          }

          try {
            const plan = await commerce.archiveSubscriptionPlan(planId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Plan "${plan.name}" archived`,
                  plan
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'list_subscriptions',
        'List subscriptions. Filter by customer, plan, or status.',
        {
          customerId: z.string().optional().describe('Filter by customer ID'),
          planId: z.string().optional().describe('Filter by plan ID'),
          status: z.enum(['trial', 'active', 'paused', 'past_due', 'cancelled', 'expired', 'pending']).optional().describe('Filter by status')
        },
        async ({ customerId, planId, status }) => {
          try {
            const subscriptions = await commerce.listSubscriptions({ customerId, planId, status });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  count: subscriptions.length,
                  subscriptions: subscriptions.map(s => ({
                    id: s.id,
                    subscriptionNumber: s.subscriptionNumber,
                    customerId: s.customerId,
                    planName: s.planName,
                    status: s.status,
                    price: s.price,
                    currency: s.currency,
                    nextBillingDate: s.nextBillingDate,
                    billingCycleCount: s.billingCycleCount
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_subscription',
        'Get details for a specific subscription.',
        {
          subscriptionId: z.string().describe('Subscription ID or number')
        },
        async ({ subscriptionId }) => {
          try {
            const subscription = await commerce.getSubscription(subscriptionId);
            if (!subscription) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: 'Subscription not found' }) }] };
            }
            return { content: [{ type: 'text', text: JSON.stringify(subscription, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_subscription',
        'Create a new subscription for a customer. Requires --apply flag.',
        {
          customerId: z.string().describe('Customer ID'),
          planId: z.string().describe('Plan ID'),
          paymentMethodId: z.string().optional().describe('Payment method ID from payment provider'),
          skipTrial: z.boolean().optional().describe('Skip trial period'),
          couponCode: z.string().optional().describe('Coupon code to apply')
        },
        async ({ customerId, planId, paymentMethodId, skipTrial, couponCode }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Subscribe operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldSubscribe: { customerId, planId }
                })
              }]
            };
          }

          try {
            const subscription = await commerce.createSubscription({
              customerId,
              planId,
              paymentMethodId,
              skipTrial,
              couponCode
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Created subscription ${subscription.subscriptionNumber}`,
                  subscription
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'pause_subscription',
        'Pause a subscription (stops billing, can resume later). Requires --apply flag.',
        {
          subscriptionId: z.string().describe('Subscription ID'),
          resumeAt: z.string().optional().describe('ISO date when to auto-resume'),
          reason: z.string().optional().describe('Reason for pausing')
        },
        async ({ subscriptionId, resumeAt, reason }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Pause operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldPause: subscriptionId
                })
              }]
            };
          }

          try {
            const subscription = await commerce.pauseSubscription(subscriptionId, {
              resumeAt: resumeAt ? new Date(resumeAt).toISOString() : undefined,
              reason
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Subscription ${subscription.subscriptionNumber} paused`,
                  subscription
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'resume_subscription',
        'Resume a paused subscription. Requires --apply flag.',
        {
          subscriptionId: z.string().describe('Subscription ID')
        },
        async ({ subscriptionId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Resume operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldResume: subscriptionId
                })
              }]
            };
          }

          try {
            const subscription = await commerce.resumeSubscription(subscriptionId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Subscription ${subscription.subscriptionNumber} resumed`,
                  subscription
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'cancel_subscription',
        'Cancel a subscription. By default cancels at end of period. Requires --apply flag.',
        {
          subscriptionId: z.string().describe('Subscription ID'),
          immediate: z.boolean().optional().describe('Cancel immediately (default: false, cancels at period end)'),
          reason: z.string().optional().describe('Reason for cancellation')
        },
        async ({ subscriptionId, immediate, reason }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Cancel operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldCancel: subscriptionId
                })
              }]
            };
          }

          try {
            const subscription = await commerce.cancelSubscription(subscriptionId, {
              immediate,
              reason
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: immediate
                    ? `Subscription ${subscription.subscriptionNumber} cancelled immediately`
                    : `Subscription ${subscription.subscriptionNumber} will cancel at period end`,
                  subscription
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'skip_billing_cycle',
        'Skip the next billing cycle for a subscription. Requires --apply flag.',
        {
          subscriptionId: z.string().describe('Subscription ID'),
          reason: z.string().optional().describe('Reason for skipping')
        },
        async ({ subscriptionId, reason }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Skip operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldSkip: subscriptionId
                })
              }]
            };
          }

          try {
            const subscription = await commerce.skipBillingCycle(subscriptionId, { reason });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Next billing cycle skipped for ${subscription.subscriptionNumber}`,
                  nextBillingDate: subscription.nextBillingDate,
                  subscription
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'list_billing_cycles',
        'List billing cycles for a subscription.',
        {
          subscriptionId: z.string().describe('Subscription ID'),
          status: z.enum(['scheduled', 'processing', 'paid', 'failed', 'skipped', 'refunded', 'voided']).optional().describe('Filter by status')
        },
        async ({ subscriptionId, status }) => {
          try {
            const cycles = await commerce.listBillingCycles({ subscriptionId, status });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  count: cycles.length,
                  cycles: cycles.map(c => ({
                    id: c.id,
                    cycleNumber: c.cycleNumber,
                    status: c.status,
                    periodStart: c.periodStart,
                    periodEnd: c.periodEnd,
                    total: c.total,
                    currency: c.currency,
                    billedAt: c.billedAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_billing_cycle',
        'Get details for a specific billing cycle.',
        {
          cycleId: z.string().describe('Billing cycle ID')
        },
        async ({ cycleId }) => {
          try {
            const cycle = await commerce.getBillingCycle(cycleId);
            if (!cycle) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: 'Billing cycle not found' }) }] };
            }
            return { content: [{ type: 'text', text: JSON.stringify(cycle, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_subscription_events',
        'Get event history (audit log) for a subscription.',
        {
          subscriptionId: z.string().describe('Subscription ID'),
          limit: z.number().optional().describe('Maximum events to return')
        },
        async ({ subscriptionId, limit }) => {
          try {
            const events = await commerce.getSubscriptionEvents(subscriptionId, limit);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  count: events.length,
                  events: events.map(e => ({
                    id: e.id,
                    eventType: e.eventType,
                    description: e.description,
                    triggeredBy: e.triggeredBy,
                    createdAt: e.createdAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Sync Tools (Verifiable Event Sync)
      // ============================================================================

      tool(
        'sync_status',
        'Get the current sync status between local database and remote sequencer. Shows pending events, sync lag, and connection status.',
        {},
        async () => {
          try {
            // Check if sync is configured
            if (!isSyncConfigured()) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    configured: false,
                    message: 'Sync not configured. Run "stateset-sync init" to set up sync.',
                    hint: 'stateset-sync init --sequencer-url <url> --tenant-id <uuid> --store-id <uuid>'
                  }, null, 2)
                }]
              };
            }

            const rawConfig = loadSyncConfig();
            const config = new SyncConfig(rawConfig);

            // Get outbox stats
            const outbox = createOutbox(commerce.db);
            const stats = outbox.getStats();
            const syncState = outbox.getSyncState();

            // Try to get remote head
            let remoteHead = syncState.headSequence;
            let connected = false;
            let connectionError = null;

            try {
              const client = createSequencerClient(config);
              await client.connect();
              const remoteState = await client.getHead();
              remoteHead = remoteState.headSequence;
              connected = true;
            } catch (error) {
              connectionError = error.message;
            }

            const lag = remoteHead - syncState.lastPulledSequence;

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  configured: true,
                  connected,
                  connectionError,
                  sequencer: config.sequencerUrl,
                  identity: {
                    tenantId: config.tenantId,
                    storeId: config.storeId,
                    agentId: config.agentId
                  },
                  localState: {
                    lastPushedSequence: syncState.lastPushedSequence,
                    lastPulledSequence: syncState.lastPulledSequence,
                    lastSyncAt: syncState.lastSyncAt
                  },
                  remoteHead,
                  lag,
                  outbox: {
                    total: stats.total,
                    pending: stats.pending,
                    synced: stats.synced,
                    failed: stats.failed,
                    rejected: stats.rejected,
                    oldestPending: stats.oldestPending,
                    lastSynced: stats.lastSynced
                  },
                  health: lag > 100 ? 'degraded' : connected ? 'healthy' : 'offline'
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'sync_push',
        'Push pending local events to the remote sequencer. Requires --apply flag for actual push.',
        {
          batchSize: z.number().optional().describe('Maximum events to push in one batch (default: 100)'),
          dryRun: z.boolean().optional().describe('Show what would be pushed without actually pushing')
        },
        async ({ batchSize = 100, dryRun = false }) => {
          try {
            if (!isSyncConfigured()) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Sync not configured',
                    hint: 'Run "stateset-sync init" to set up sync.'
                  })
                }]
              };
            }

            // Check permission for actual push
            if (!dryRun && !allowApply) {
              const outbox = createOutbox(commerce.db);
              const pending = outbox.getPending(batchSize);

              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Push operation not allowed. The --apply flag must be set.',
                    hint: 'Run with --apply to enable push, or use dryRun: true to preview.',
                    wouldPush: pending.length,
                    pendingEvents: pending.map(e => ({
                      eventId: e.eventId,
                      eventType: e.eventType,
                      entityType: e.entityType,
                      entityId: e.entityId,
                      createdAt: e.createdAt
                    }))
                  }, null, 2)
                }]
              };
            }

            const rawConfig = loadSyncConfig();
            const config = new SyncConfig(rawConfig);
            const engine = createSyncEngine({ db: commerce.db, config });

            await engine.initialize();
            const result = await engine.push({ batchSize, dryRun });
            await engine.shutdown();

            if (dryRun) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    dryRun: true,
                    wouldPush: result.pushed,
                    message: `Would push ${result.pushed} events to sequencer`
                  }, null, 2)
                }]
              };
            }

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: result.success,
                  pushed: result.pushed,
                  rejected: result.rejected,
                  receipt: result.receipt ? {
                    batchId: result.receipt.batchId,
                    sequenceStart: result.receipt.sequenceStart,
                    sequenceEnd: result.receipt.sequenceEnd
                  } : null,
                  error: result.error
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'sync_pull',
        'Pull events from the remote sequencer and store them locally.',
        {
          fromSequence: z.number().optional().describe('Start pulling from this sequence number'),
          limit: z.number().optional().describe('Maximum events to pull (default: 1000)')
        },
        async ({ fromSequence, limit = 1000 }) => {
          try {
            if (!isSyncConfigured()) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Sync not configured',
                    hint: 'Run "stateset-sync init" to set up sync.'
                  })
                }]
              };
            }

            const rawConfig = loadSyncConfig();
            const config = new SyncConfig(rawConfig);
            const engine = createSyncEngine({ db: commerce.db, config });

            await engine.initialize();
            const result = await engine.pull({ fromSequence, limit });
            await engine.shutdown();

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: result.success,
                  pulled: result.pulled,
                  applied: result.applied,
                  conflicts: result.conflicts,
                  error: result.error
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'sync_outbox',
        'List events in the local outbox. Shows pending, synced, failed, and rejected events.',
        {
          status: z.enum(['pending', 'synced', 'failed', 'rejected', 'all']).optional().describe('Filter by status (default: all)'),
          limit: z.number().optional().describe('Maximum events to return (default: 20)')
        },
        async ({ status = 'all', limit = 20 }) => {
          try {
            if (!isSyncConfigured()) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Sync not configured',
                    hint: 'Run "stateset-sync init" to set up sync.'
                  })
                }]
              };
            }

            const outbox = createOutbox(commerce.db);
            outbox.initialize();

            // Query based on status
            let query;
            if (status === 'pending') {
              query = 'SELECT * FROM _ves_outbox WHERE sync_status = ? ORDER BY local_seq DESC LIMIT ?';
            } else if (status === 'all') {
              query = 'SELECT * FROM _ves_outbox ORDER BY local_seq DESC LIMIT ?';
            } else {
              query = 'SELECT * FROM _ves_outbox WHERE sync_status = ? ORDER BY local_seq DESC LIMIT ?';
            }

            const stmt = status === 'all'
              ? commerce.db.prepare('SELECT * FROM _ves_outbox ORDER BY local_seq DESC LIMIT ?')
              : commerce.db.prepare('SELECT * FROM _ves_outbox WHERE sync_status = ? ORDER BY local_seq DESC LIMIT ?');

            const rows = status === 'all' ? stmt.all(limit) : stmt.all(status, limit);

            const events = rows.map(row => ({
              localSeq: row.local_seq,
              eventId: row.event_id,
              eventType: row.event_type,
              entityType: row.entity_type,
              entityId: row.entity_id,
              syncStatus: row.sync_status,
              remoteSequence: row.remote_sequence,
              createdAt: row.created_at,
              syncedAt: row.synced_at,
              rejectionReason: row.rejection_reason,
              retryCount: row.retry_count
            }));

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  count: events.length,
                  filter: status,
                  events
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'sync_retry_failed',
        'Reset failed events to pending status so they can be retried. Requires --apply flag.',
        {},
        async () => {
          try {
            if (!isSyncConfigured()) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Sync not configured',
                    hint: 'Run "stateset-sync init" to set up sync.'
                  })
                }]
              };
            }

            if (!allowApply) {
              const outbox = createOutbox(commerce.db);
              const stats = outbox.getStats();

              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Retry operation not allowed. The --apply flag must be set.',
                    hint: 'Run with --apply to enable retry.',
                    failedCount: stats.failed
                  })
                }]
              };
            }

            const outbox = createOutbox(commerce.db);
            const retriedCount = outbox.retryFailed();

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  retriedCount,
                  message: `Reset ${retriedCount} failed events to pending`
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'sync_entity_history',
        'Get the event history for a specific entity from the remote sequencer.',
        {
          entityType: z.string().describe('Entity type (order, customer, product, inventory, return, cart)'),
          entityId: z.string().describe('Entity ID')
        },
        async ({ entityType, entityId }) => {
          try {
            if (!isSyncConfigured()) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Sync not configured',
                    hint: 'Run "stateset-sync init" to set up sync.'
                  })
                }]
              };
            }

            const rawConfig = loadSyncConfig();
            const config = new SyncConfig(rawConfig);
            const client = createSequencerClient(config);

            await client.connect();
            const events = await client.getEntityHistory(entityType, entityId);

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  entityType,
                  entityId,
                  eventCount: events.length,
                  events: events.map(e => ({
                    sequenceNumber: e.sequenceNumber,
                    eventId: e.envelope.eventId,
                    eventType: e.envelope.eventType,
                    createdAt: e.envelope.createdAt,
                    sequencedAt: e.sequencedAt,
                    sourceAgent: e.envelope.sourceAgent
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'sync_full',
        'Perform a full sync: push pending events then pull new events. Requires --apply flag for push.',
        {
          pushBatchSize: z.number().optional().describe('Maximum events to push (default: 100)'),
          pullLimit: z.number().optional().describe('Maximum events to pull (default: 1000)')
        },
        async ({ pushBatchSize = 100, pullLimit = 1000 }) => {
          try {
            if (!isSyncConfigured()) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Sync not configured',
                    hint: 'Run "stateset-sync init" to set up sync.'
                  })
                }]
              };
            }

            const rawConfig = loadSyncConfig();
            const config = new SyncConfig(rawConfig);
            const engine = createSyncEngine({ db: commerce.db, config });

            await engine.initialize();

            // Push (if allowed)
            let pushResult = { success: true, pushed: 0, rejected: 0 };
            if (allowApply) {
              pushResult = await engine.push({ batchSize: pushBatchSize });
            } else {
              const outbox = createOutbox(commerce.db);
              pushResult = {
                success: false,
                pushed: 0,
                rejected: 0,
                skipped: true,
                pendingCount: outbox.getPendingCount(),
                message: 'Push skipped: --apply flag not set'
              };
            }

            // Pull (always allowed)
            const pullResult = await engine.pull({ limit: pullLimit });

            await engine.shutdown();

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  push: pushResult,
                  pull: pullResult
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Sync Conflict Resolution Tools
      // ============================================================================

      tool(
        'sync_conflicts',
        'List unresolved sync conflicts. Conflicts occur when local and remote events modify the same entity concurrently.',
        {},
        async () => {
          try {
            if (!isSyncConfigured()) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Sync not configured',
                    hint: 'Run "stateset-sync init" to set up sync.'
                  })
                }]
              };
            }

            const rawConfig = loadSyncConfig();
            const config = new SyncConfig(rawConfig);
            const engine = createSyncEngine({ db: commerce.db, config });

            await engine.initialize();
            const conflicts = await engine.getConflicts();
            await engine.shutdown();

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  count: conflicts.length,
                  conflicts: conflicts.map(c => ({
                    id: c.id,
                    type: c.type,
                    entityType: c.entityType,
                    entityId: c.entityId,
                    description: c.description,
                    suggestedStrategy: c.suggestedStrategy,
                    detectedAt: c.detectedAt,
                    localEvent: c.localEvent ? {
                      localSeq: c.localEvent.localSeq,
                      eventType: c.localEvent.eventType,
                      createdAt: c.localEvent.createdAt
                    } : null
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'sync_resolve',
        'Resolve a specific sync conflict using a resolution strategy. Requires --apply flag.',
        {
          conflictId: z.string().describe('The conflict ID to resolve'),
          strategy: z.enum(['remote-wins', 'local-wins', 'merge']).optional().describe('Resolution strategy (default: uses suggested strategy)')
        },
        async ({ conflictId, strategy }) => {
          try {
            if (!isSyncConfigured()) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Sync not configured',
                    hint: 'Run "stateset-sync init" to set up sync.'
                  })
                }]
              };
            }

            if (!allowApply) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Resolve operation not allowed. The --apply flag must be set.',
                    hint: 'Run with --apply to enable conflict resolution.',
                    conflictId,
                    wouldUseStrategy: strategy || 'suggested'
                  })
                }]
              };
            }

            const rawConfig = loadSyncConfig();
            const config = new SyncConfig(rawConfig);
            const engine = createSyncEngine({ db: commerce.db, config });

            await engine.initialize();
            const result = await engine.resolveConflict(conflictId, strategy);
            await engine.shutdown();

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: result.success,
                  conflictId: result.conflictId,
                  strategy: result.strategy,
                  result: result.result,
                  error: result.error
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'sync_rebase',
        'Resolve all sync conflicts using a resolution strategy. Requires --apply flag.',
        {
          strategy: z.enum(['remote-wins', 'local-wins', 'merge']).optional().describe('Resolution strategy for all conflicts (default: remote-wins)')
        },
        async ({ strategy = 'remote-wins' }) => {
          try {
            if (!isSyncConfigured()) {
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Sync not configured',
                    hint: 'Run "stateset-sync init" to set up sync.'
                  })
                }]
              };
            }

            const rawConfig = loadSyncConfig();
            const config = new SyncConfig(rawConfig);
            const engine = createSyncEngine({ db: commerce.db, config });

            await engine.initialize();
            const conflicts = await engine.getConflicts();

            if (!allowApply) {
              await engine.shutdown();
              return {
                content: [{
                  type: 'text',
                  text: JSON.stringify({
                    error: 'Rebase operation not allowed. The --apply flag must be set.',
                    hint: 'Run with --apply to enable rebase.',
                    wouldResolve: conflicts.length,
                    conflicts: conflicts.map(c => ({
                      id: c.id,
                      entityType: c.entityType,
                      entityId: c.entityId,
                      type: c.type
                    })),
                    strategy
                  }, null, 2)
                }]
              };
            }

            const result = await engine.rebase({ strategy });
            await engine.shutdown();

            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: result.success,
                  resolved: result.rebased,
                  failed: result.failed,
                  strategy,
                  errors: result.errors
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Manufacturing Tools - BOM & Work Orders
      // ============================================================================
      tool(
        'list_boms',
        'List all Bills of Materials (BOMs). BOMs define the components/ingredients needed to manufacture a product.',
        {},
        async () => {
          try {
            const boms = await commerce.bom.list();
            const count = await commerce.bom.count();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count,
                  boms: boms.map(b => ({
                    id: b.id,
                    bomNumber: b.bomNumber,
                    name: b.name,
                    productId: b.productId,
                    status: b.status,
                    revision: b.revision,
                    createdAt: b.createdAt
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_bom',
        'Get a Bill of Materials by ID, including all components/ingredients.',
        {
          bomId: z.string().describe('BOM ID or BOM number')
        },
        async ({ bomId }) => {
          try {
            const bom = await commerce.bom.get(bomId);
            if (!bom) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: 'BOM not found' }) }] };
            }
            const components = await commerce.bom.getComponents(bomId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  bom: { ...bom, components }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_bom',
        'Create a new Bill of Materials for a product. Defines what components/ingredients are needed.',
        {
          name: z.string().describe('BOM name (e.g., "Classic Pickled Onions Recipe")'),
          productId: z.string().describe('Product ID this BOM is for'),
          description: z.string().optional().describe('Description of this BOM'),
          revision: z.string().optional().describe('Revision number (default: A)')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Create BOM operation not allowed. The --apply flag must be set.',
                  hint: 'Run with --apply to enable write operations.',
                  wouldCreate: args
                })
              }]
            };
          }

          try {
            const bom = await commerce.bom.create({
              name: args.name,
              productId: args.productId,
              description: args.description,
              revision: args.revision
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'BOM created successfully',
                  bom: {
                    id: bom.id,
                    bomNumber: bom.bomNumber,
                    name: bom.name,
                    status: bom.status
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'add_bom_component',
        'Add a component/ingredient to a Bill of Materials.',
        {
          bomId: z.string().describe('BOM ID to add component to'),
          name: z.string().describe('Component name (e.g., "Yellow Onions")'),
          sku: z.string().optional().describe('Component SKU if from inventory'),
          quantity: z.number().describe('Quantity needed per unit produced'),
          unitOfMeasure: z.string().optional().describe('Unit (e.g., "kg", "lbs", "each", "ml")'),
          notes: z.string().optional().describe('Notes about this component')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Add component operation not allowed. The --apply flag must be set.',
                  wouldAdd: args
                })
              }]
            };
          }

          try {
            const component = await commerce.bom.addComponent(args.bomId, {
              name: args.name,
              componentSku: args.sku || null,
              quantity: String(args.quantity),
              unitOfMeasure: args.unitOfMeasure || 'each',
              notes: args.notes || null
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Component added to BOM',
                  component
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'activate_bom',
        'Activate a BOM to make it available for work orders.',
        {
          bomId: z.string().describe('BOM ID to activate')
        },
        async ({ bomId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Activate BOM operation not allowed. The --apply flag must be set.',
                  wouldActivate: bomId
                })
              }]
            };
          }

          try {
            const bom = await commerce.bom.activate(bomId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'BOM activated',
                  bom: { id: bom.id, name: bom.name, status: bom.status }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'list_work_orders',
        'List all manufacturing work orders. Work orders track production runs.',
        {},
        async () => {
          try {
            const workOrders = await commerce.workOrders.list();
            const count = await commerce.workOrders.count();
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  count,
                  workOrders: workOrders.map(wo => ({
                    id: wo.id,
                    workOrderNumber: wo.workOrderNumber,
                    productId: wo.productId,
                    status: wo.status,
                    priority: wo.priority,
                    quantityToBuild: wo.quantityToBuild,
                    quantityCompleted: wo.quantityCompleted,
                    scheduledStart: wo.scheduledStart,
                    scheduledEnd: wo.scheduledEnd
                  }))
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_work_order',
        'Get a work order by ID with full details.',
        {
          workOrderId: z.string().describe('Work order ID or number')
        },
        async ({ workOrderId }) => {
          try {
            const wo = await commerce.workOrders.get(workOrderId);
            if (!wo) {
              return { content: [{ type: 'text', text: JSON.stringify({ error: 'Work order not found' }) }] };
            }
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({ success: true, workOrder: wo }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_work_order',
        'Create a manufacturing work order to produce a quantity of product.',
        {
          productId: z.string().describe('Product ID to manufacture'),
          bomId: z.string().optional().describe('BOM ID to use (optional)'),
          quantityToBuild: z.number().describe('Number of units to produce'),
          priority: z.enum(['low', 'normal', 'high', 'urgent']).optional().describe('Priority level'),
          scheduledStart: z.string().optional().describe('Scheduled start date (ISO format)'),
          scheduledEnd: z.string().optional().describe('Scheduled end date (ISO format)'),
          notes: z.string().optional().describe('Production notes')
        },
        async (args) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Create work order operation not allowed. The --apply flag must be set.',
                  wouldCreate: args
                })
              }]
            };
          }

          try {
            const wo = await commerce.workOrders.create({
              productId: args.productId,
              bomId: args.bomId,
              quantityToBuild: args.quantityToBuild,
              priority: args.priority || 'normal',
              scheduledStart: args.scheduledStart,
              scheduledEnd: args.scheduledEnd,
              notes: args.notes
            });
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Work order created',
                  workOrder: {
                    id: wo.id,
                    workOrderNumber: wo.workOrderNumber,
                    status: wo.status,
                    quantityToBuild: wo.quantityToBuild
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'start_work_order',
        'Start a work order (begin production).',
        {
          workOrderId: z.string().describe('Work order ID to start')
        },
        async ({ workOrderId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Start work order operation not allowed. The --apply flag must be set.',
                  wouldStart: workOrderId
                })
              }]
            };
          }

          try {
            const wo = await commerce.workOrders.start(workOrderId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Work order started - production in progress',
                  workOrder: { id: wo.id, workOrderNumber: wo.workOrderNumber, status: wo.status }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'complete_work_order',
        'Complete a work order with the quantity produced.',
        {
          workOrderId: z.string().describe('Work order ID to complete'),
          quantityCompleted: z.number().describe('Number of units actually produced')
        },
        async ({ workOrderId, quantityCompleted }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Complete work order operation not allowed. The --apply flag must be set.',
                  wouldComplete: { workOrderId, quantityCompleted }
                })
              }]
            };
          }

          try {
            const wo = await commerce.workOrders.complete(workOrderId, quantityCompleted);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: `Work order completed - ${quantityCompleted} units produced`,
                  workOrder: {
                    id: wo.id,
                    workOrderNumber: wo.workOrderNumber,
                    status: wo.status,
                    quantityCompleted: wo.quantityCompleted
                  }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'cancel_work_order',
        'Cancel a work order.',
        {
          workOrderId: z.string().describe('Work order ID to cancel')
        },
        async ({ workOrderId }) => {
          if (!allowApply) {
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  error: 'Cancel work order operation not allowed. The --apply flag must be set.',
                  wouldCancel: workOrderId
                })
              }]
            };
          }

          try {
            const wo = await commerce.workOrders.cancel(workOrderId);
            return {
              content: [{
                type: 'text',
                text: JSON.stringify({
                  success: true,
                  message: 'Work order cancelled',
                  workOrder: { id: wo.id, workOrderNumber: wo.workOrderNumber, status: wo.status }
                }, null, 2)
              }]
            };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Payment Tools
      // ============================================================================
      tool(
        'list_payments',
        'List all payments in the system.',
        {},
        async () => {
          try {
            const payments = await commerce.payments.list();
            const count = await commerce.payments.count();
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, count, payments }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_payment',
        'Get a payment by ID.',
        { paymentId: z.string().describe('Payment ID') },
        async ({ paymentId }) => {
          try {
            const payment = await commerce.payments.get(paymentId);
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, payment }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_payment',
        'Create a payment for an order.',
        {
          orderId: z.string().describe('Order ID'),
          amount: z.number().describe('Payment amount'),
          currency: z.string().optional().describe('Currency (default: USD)'),
          method: z.string().optional().describe('Payment method: credit_card, paypal, bank_transfer, crypto')
        },
        async (args) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Create payment requires --apply flag.', wouldCreate: args }) }] };
          try {
            const payment = await commerce.payments.create({ orderId: args.orderId, amount: String(args.amount), currency: args.currency || 'USD', method: args.method || 'credit_card' });
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'Payment created', payment }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'complete_payment',
        'Mark a payment as completed.',
        { paymentId: z.string().describe('Payment ID') },
        async ({ paymentId }) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Complete payment requires --apply flag.' }) }] };
          try {
            const payment = await commerce.payments.markCompleted(paymentId);
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'Payment completed', payment }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_refund',
        'Create a refund for a payment.',
        {
          paymentId: z.string().describe('Payment ID to refund'),
          amount: z.number().describe('Refund amount'),
          reason: z.string().optional().describe('Refund reason')
        },
        async (args) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Create refund requires --apply flag.' }) }] };
          try {
            const refund = await commerce.payments.createRefund({ paymentId: args.paymentId, amount: String(args.amount), reason: args.reason });
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'Refund created', refund }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Shipment Tools
      // ============================================================================
      tool(
        'list_shipments',
        'List all shipments.',
        {},
        async () => {
          try {
            const shipments = await commerce.shipments.list();
            const count = await commerce.shipments.count();
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, count, shipments }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_shipment',
        'Create a shipment for an order.',
        {
          orderId: z.string().describe('Order ID'),
          carrier: z.string().optional().describe('Carrier: USPS, UPS, FedEx, DHL'),
          service: z.string().optional().describe('Service level')
        },
        async (args) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Create shipment requires --apply flag.' }) }] };
          try {
            const shipment = await commerce.shipments.create({ orderId: args.orderId, carrier: args.carrier, service: args.service });
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'Shipment created', shipment }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'deliver_shipment',
        'Mark a shipment as delivered.',
        { shipmentId: z.string().describe('Shipment ID') },
        async ({ shipmentId }) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Deliver shipment requires --apply flag.' }) }] };
          try {
            const shipment = await commerce.shipments.deliver(shipmentId);
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'Shipment delivered', shipment }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Supplier & Purchase Order Tools
      // ============================================================================
      tool(
        'list_suppliers',
        'List all suppliers.',
        {},
        async () => {
          try {
            const suppliers = await commerce.purchaseOrders.listSuppliers();
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, count: suppliers.length, suppliers }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_supplier',
        'Create a new supplier.',
        {
          name: z.string().describe('Supplier name'),
          email: z.string().optional().describe('Contact email'),
          phone: z.string().optional().describe('Phone number'),
          address: z.string().optional().describe('Address')
        },
        async (args) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Create supplier requires --apply flag.' }) }] };
          try {
            const supplier = await commerce.purchaseOrders.createSupplier({ name: args.name, email: args.email, phone: args.phone, address: args.address });
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'Supplier created', supplier }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'list_purchase_orders',
        'List all purchase orders.',
        {},
        async () => {
          try {
            const purchaseOrders = await commerce.purchaseOrders.list();
            const count = await commerce.purchaseOrders.count();
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, count, purchaseOrders }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_purchase_order',
        'Create a purchase order to a supplier.',
        {
          supplierId: z.string().describe('Supplier ID'),
          items: z.string().describe('JSON array: [{"sku":"X","name":"Y","quantity":10,"unitPrice":5.00}]'),
          notes: z.string().optional().describe('Notes')
        },
        async (args) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Create PO requires --apply flag.' }) }] };
          try {
            const items = JSON.parse(args.items);
            const po = await commerce.purchaseOrders.create({ supplierId: args.supplierId, items, notes: args.notes });
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'PO created', purchaseOrder: po }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'approve_purchase_order',
        'Approve a purchase order.',
        {
          purchaseOrderId: z.string().describe('PO ID'),
          approvedBy: z.string().describe('Approver name')
        },
        async ({ purchaseOrderId, approvedBy }) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Approve PO requires --apply flag.' }) }] };
          try {
            const po = await commerce.purchaseOrders.approve(purchaseOrderId, approvedBy);
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'PO approved', purchaseOrder: po }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'send_purchase_order',
        'Send a PO to the supplier.',
        { purchaseOrderId: z.string().describe('PO ID') },
        async ({ purchaseOrderId }) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Send PO requires --apply flag.' }) }] };
          try {
            const po = await commerce.purchaseOrders.send(purchaseOrderId);
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'PO sent to supplier', purchaseOrder: po }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Invoice Tools
      // ============================================================================
      tool(
        'list_invoices',
        'List all invoices.',
        {},
        async () => {
          try {
            const invoices = await commerce.invoices.list();
            const count = await commerce.invoices.count();
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, count, invoices }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_invoice',
        'Create an invoice for a customer.',
        {
          customerId: z.string().describe('Customer ID'),
          orderId: z.string().optional().describe('Order ID'),
          items: z.string().describe('JSON array: [{"description":"X","quantity":1,"unitPrice":10.00}]'),
          dueDate: z.string().optional().describe('Due date ISO'),
          notes: z.string().optional().describe('Notes')
        },
        async (args) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Create invoice requires --apply flag.' }) }] };
          try {
            const items = JSON.parse(args.items);
            const invoice = await commerce.invoices.create({ customerId: args.customerId, orderId: args.orderId, items, dueDate: args.dueDate, notes: args.notes });
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'Invoice created', invoice }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'send_invoice',
        'Send an invoice to the customer.',
        { invoiceId: z.string().describe('Invoice ID') },
        async ({ invoiceId }) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Send invoice requires --apply flag.' }) }] };
          try {
            const invoice = await commerce.invoices.send(invoiceId);
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'Invoice sent', invoice }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'record_invoice_payment',
        'Record payment on an invoice.',
        {
          invoiceId: z.string().describe('Invoice ID'),
          amount: z.number().describe('Amount paid'),
          paymentMethod: z.string().optional().describe('Payment method'),
          reference: z.string().optional().describe('Check/reference number')
        },
        async (args) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Record payment requires --apply flag.' }) }] };
          try {
            const invoice = await commerce.invoices.recordPayment(args.invoiceId, { amount: args.amount, paymentMethod: args.paymentMethod, reference: args.reference });
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'Payment recorded', invoice }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'get_overdue_invoices',
        'Get all overdue invoices.',
        {},
        async () => {
          try {
            const invoices = await commerce.invoices.getOverdue();
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, count: invoices.length, overdueInvoices: invoices }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      // ============================================================================
      // Warranty Tools
      // ============================================================================
      tool(
        'list_warranties',
        'List all warranties.',
        {},
        async () => {
          try {
            const warranties = await commerce.warranties.list();
            const count = await commerce.warranties.count();
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, count, warranties }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_warranty',
        'Create a warranty for a product.',
        {
          customerId: z.string().describe('Customer ID (required)'),
          orderId: z.string().optional().describe('Order ID'),
          productId: z.string().optional().describe('Product ID'),
          warrantyType: z.string().optional().describe('Type: standard, extended, lifetime'),
          durationMonths: z.number().optional().describe('Duration in months')
        },
        async (args) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Create warranty requires --apply flag.' }) }] };
          try {
            const warranty = await commerce.warranties.create({ customerId: args.customerId, orderId: args.orderId, productId: args.productId, warrantyType: args.warrantyType || 'standard', durationMonths: args.durationMonths || 12 });
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'Warranty created', warranty }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'create_warranty_claim',
        'File a warranty claim.',
        {
          warrantyId: z.string().describe('Warranty ID'),
          description: z.string().describe('Issue description'),
          claimType: z.string().optional().describe('Type: repair, replacement, refund')
        },
        async (args) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Create claim requires --apply flag.' }) }] };
          try {
            const claim = await commerce.warranties.createClaim({ warrantyId: args.warrantyId, description: args.description, claimType: args.claimType || 'replacement' });
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'Claim filed', claim }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      ),

      tool(
        'approve_warranty_claim',
        'Approve a warranty claim.',
        { claimId: z.string().describe('Claim ID') },
        async ({ claimId }) => {
          if (!allowApply) return { content: [{ type: 'text', text: JSON.stringify({ error: 'Approve claim requires --apply flag.' }) }] };
          try {
            const claim = await commerce.warranties.approveClaim(claimId);
            return { content: [{ type: 'text', text: JSON.stringify({ success: true, message: 'Claim approved', claim }, null, 2) }] };
          } catch (error) {
            return { content: [{ type: 'text', text: JSON.stringify({ error: error.message }) }] };
          }
        }
      )
    ]
  });
}

export const TOOL_NAMES = [
  // Customers
  'mcp__stateset-commerce__list_customers',
  'mcp__stateset-commerce__get_customer',
  'mcp__stateset-commerce__create_customer',
  // Orders
  'mcp__stateset-commerce__list_orders',
  'mcp__stateset-commerce__get_order',
  'mcp__stateset-commerce__create_order',
  'mcp__stateset-commerce__update_order_status',
  'mcp__stateset-commerce__ship_order',
  'mcp__stateset-commerce__cancel_order',
  // Products
  'mcp__stateset-commerce__list_products',
  'mcp__stateset-commerce__get_product',
  'mcp__stateset-commerce__get_product_variant',
  'mcp__stateset-commerce__create_product',
  // Inventory
  'mcp__stateset-commerce__get_stock',
  'mcp__stateset-commerce__create_inventory_item',
  'mcp__stateset-commerce__adjust_inventory',
  'mcp__stateset-commerce__reserve_inventory',
  'mcp__stateset-commerce__confirm_reservation',
  'mcp__stateset-commerce__release_reservation',
  // Returns
  'mcp__stateset-commerce__list_returns',
  'mcp__stateset-commerce__get_return',
  'mcp__stateset-commerce__create_return',
  'mcp__stateset-commerce__approve_return',
  'mcp__stateset-commerce__reject_return',
  // Carts/Checkout (Agentic Commerce Protocol)
  'mcp__stateset-commerce__list_carts',
  'mcp__stateset-commerce__get_cart',
  'mcp__stateset-commerce__create_cart',
  'mcp__stateset-commerce__add_cart_item',
  'mcp__stateset-commerce__update_cart_item',
  'mcp__stateset-commerce__remove_cart_item',
  'mcp__stateset-commerce__set_cart_shipping_address',
  'mcp__stateset-commerce__set_cart_payment',
  'mcp__stateset-commerce__apply_cart_discount',
  'mcp__stateset-commerce__get_shipping_rates',
  'mcp__stateset-commerce__complete_checkout',
  'mcp__stateset-commerce__cancel_cart',
  'mcp__stateset-commerce__abandon_cart',
  'mcp__stateset-commerce__get_abandoned_carts',
  // Analytics & Forecasting
  'mcp__stateset-commerce__get_sales_summary',
  'mcp__stateset-commerce__get_top_products',
  'mcp__stateset-commerce__get_customer_metrics',
  'mcp__stateset-commerce__get_top_customers',
  'mcp__stateset-commerce__get_inventory_health',
  'mcp__stateset-commerce__get_low_stock_items',
  'mcp__stateset-commerce__get_demand_forecast',
  'mcp__stateset-commerce__get_revenue_forecast',
  'mcp__stateset-commerce__get_order_status_breakdown',
  'mcp__stateset-commerce__get_return_metrics',
  // Currency & Exchange Rates
  'mcp__stateset-commerce__get_exchange_rate',
  'mcp__stateset-commerce__list_exchange_rates',
  'mcp__stateset-commerce__convert_currency',
  'mcp__stateset-commerce__set_exchange_rate',
  'mcp__stateset-commerce__get_currency_settings',
  'mcp__stateset-commerce__set_base_currency',
  'mcp__stateset-commerce__enable_currencies',
  'mcp__stateset-commerce__format_currency',
  // Tax
  'mcp__stateset-commerce__calculate_tax',
  'mcp__stateset-commerce__get_tax_rate',
  'mcp__stateset-commerce__list_tax_jurisdictions',
  'mcp__stateset-commerce__list_tax_rates',
  'mcp__stateset-commerce__get_tax_settings',
  'mcp__stateset-commerce__get_us_state_tax_info',
  'mcp__stateset-commerce__get_customer_tax_exemptions',
  'mcp__stateset-commerce__create_tax_exemption',
  'mcp__stateset-commerce__calculate_cart_tax',
  // Promotions & Discounts
  'mcp__stateset-commerce__list_promotions',
  'mcp__stateset-commerce__get_promotion',
  'mcp__stateset-commerce__create_promotion',
  'mcp__stateset-commerce__activate_promotion',
  'mcp__stateset-commerce__deactivate_promotion',
  'mcp__stateset-commerce__create_coupon',
  'mcp__stateset-commerce__validate_coupon',
  'mcp__stateset-commerce__list_coupons',
  'mcp__stateset-commerce__get_active_promotions',
  'mcp__stateset-commerce__apply_cart_promotions',
  // Subscriptions
  'mcp__stateset-commerce__list_subscription_plans',
  'mcp__stateset-commerce__get_subscription_plan',
  'mcp__stateset-commerce__create_subscription_plan',
  'mcp__stateset-commerce__activate_subscription_plan',
  'mcp__stateset-commerce__archive_subscription_plan',
  'mcp__stateset-commerce__list_subscriptions',
  'mcp__stateset-commerce__get_subscription',
  'mcp__stateset-commerce__create_subscription',
  'mcp__stateset-commerce__pause_subscription',
  'mcp__stateset-commerce__resume_subscription',
  'mcp__stateset-commerce__cancel_subscription',
  'mcp__stateset-commerce__skip_billing_cycle',
  'mcp__stateset-commerce__list_billing_cycles',
  'mcp__stateset-commerce__get_billing_cycle',
  'mcp__stateset-commerce__get_subscription_events',
  // Sync (Verifiable Event Sync)
  'mcp__stateset-commerce__sync_status',
  'mcp__stateset-commerce__sync_push',
  'mcp__stateset-commerce__sync_pull',
  'mcp__stateset-commerce__sync_outbox',
  'mcp__stateset-commerce__sync_retry_failed',
  'mcp__stateset-commerce__sync_entity_history',
  'mcp__stateset-commerce__sync_full',
  // Sync Conflict Resolution
  'mcp__stateset-commerce__sync_conflicts',
  'mcp__stateset-commerce__sync_resolve',
  'mcp__stateset-commerce__sync_rebase',
  // Manufacturing - BOM & Work Orders
  'mcp__stateset-commerce__list_boms',
  'mcp__stateset-commerce__get_bom',
  'mcp__stateset-commerce__create_bom',
  'mcp__stateset-commerce__add_bom_component',
  'mcp__stateset-commerce__activate_bom',
  'mcp__stateset-commerce__list_work_orders',
  'mcp__stateset-commerce__get_work_order',
  'mcp__stateset-commerce__create_work_order',
  'mcp__stateset-commerce__start_work_order',
  'mcp__stateset-commerce__complete_work_order',
  'mcp__stateset-commerce__cancel_work_order',
  // Payments
  'mcp__stateset-commerce__list_payments',
  'mcp__stateset-commerce__get_payment',
  'mcp__stateset-commerce__create_payment',
  'mcp__stateset-commerce__complete_payment',
  'mcp__stateset-commerce__create_refund',
  // Shipments
  'mcp__stateset-commerce__list_shipments',
  'mcp__stateset-commerce__create_shipment',
  'mcp__stateset-commerce__deliver_shipment',
  // Suppliers & Purchase Orders
  'mcp__stateset-commerce__list_suppliers',
  'mcp__stateset-commerce__create_supplier',
  'mcp__stateset-commerce__list_purchase_orders',
  'mcp__stateset-commerce__create_purchase_order',
  'mcp__stateset-commerce__approve_purchase_order',
  'mcp__stateset-commerce__send_purchase_order',
  // Invoices
  'mcp__stateset-commerce__list_invoices',
  'mcp__stateset-commerce__create_invoice',
  'mcp__stateset-commerce__send_invoice',
  'mcp__stateset-commerce__record_invoice_payment',
  'mcp__stateset-commerce__get_overdue_invoices',
  // Warranties
  'mcp__stateset-commerce__list_warranties',
  'mcp__stateset-commerce__create_warranty',
  'mcp__stateset-commerce__create_warranty_claim',
  'mcp__stateset-commerce__approve_warranty_claim'
];
