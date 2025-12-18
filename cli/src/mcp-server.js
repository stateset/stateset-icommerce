/**
 * MCP Server for StateSet Commerce operations
 * Exposes tools for customers, orders, products, inventory, and returns
 */

import { createSdkMcpServer, tool } from '@anthropic-ai/claude-agent-sdk';
import { z } from 'zod';

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
      'convert_currency', 'get_currency_settings', 'format_currency'
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
  'mcp__stateset-commerce__format_currency'
];
