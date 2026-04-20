/**
 * Cart/Checkout Tools Module (Agentic Commerce Protocol)
 *
 * MCP tool definitions for shopping cart and checkout operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const cartAddressInputSchema = {
  firstName: z.string().min(1).max(100).describe('First name'),
  lastName: z.string().min(1).max(100).describe('Last name'),
  line1: z.string().min(1).max(255).describe('Address line 1'),
  line2: z.string().max(255).optional().describe('Address line 2'),
  city: z.string().min(1).max(100).describe('City'),
  state: z.string().max(50).optional().describe('State/Province'),
  postalCode: z.string().min(1).max(20).describe('Postal/ZIP code'),
  country: z.string().min(2).max(3).describe('Country code (e.g., US)'),
  phone: z.string().max(30).optional().describe('Phone number'),
  email: z.string().email().optional().describe('Email address'),
};

function buildCartAddress(params) {
  return {
    firstName: params.firstName,
    lastName: params.lastName,
    line1: params.line1,
    line2: params.line2,
    city: params.city,
    state: params.state,
    postalCode: params.postalCode,
    country: params.country,
    phone: params.phone,
    email: params.email,
  };
}

export const cartTools = [
  {
    name: 'list_carts',
    description: 'List all shopping carts. Shows cart status, customer, totals, and item count.',
    inputSchema: {
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .default(50)
        .describe('Maximum number of carts to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { limit } = params;
      const carts = await commerce.carts.list();
      const count = await commerce.carts.count();
      const limitedCarts = carts.slice(0, limit);
      return {
        success: true,
        totalCount: count,
        returned: limitedCarts.length,
        carts: limitedCarts.map((c) => ({
          id: c.id,
          cartNumber: c.cartNumber,
          customerId: c.customerId,
          customerEmail: c.customerEmail,
          status: c.status,
          currency: c.currency,
          subtotal: c.subtotal,
          grandTotal: c.grandTotal,
          itemCount: c.itemCount,
          createdAt: c.createdAt,
        })),
      };
    },
  },

  {
    name: 'get_cart',
    description:
      'Get a specific cart by ID or cart number. Returns full cart details including items.',
    inputSchema: {
      identifier: z.string().min(1).describe('Cart ID (UUID) or cart number'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { identifier } = params;
      let cart;
      if (identifier.startsWith('CART-')) {
        cart = await commerce.carts.getByNumber(identifier);
      } else {
        cart = await commerce.carts.get(identifier);
      }
      if (!cart) return { success: false, error: 'Cart not found' };
      return {
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
          expiresAt: cart.expiresAt,
        },
      };
    },
  },

  {
    name: 'create_cart',
    description: 'Create a new shopping cart. Can be for a guest or authenticated customer.',
    inputSchema: {
      customerId: z.string().optional().describe('Customer ID (UUID) for authenticated checkout'),
      customerEmail: z.string().email().optional().describe('Customer email for guest checkout'),
      customerName: z.string().max(200).optional().describe('Customer name'),
      currency: z.string().max(10).optional().default('USD').describe('Currency code'),
      expiresInMinutes: z
        .number()
        .int()
        .positive()
        .optional()
        .describe('Cart expiration time in minutes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Create operation not allowed. The --apply flag must be set to create carts.',
          hint: 'Run with --apply to enable write operations.',
          wouldCreate: params,
        };
      }
      const cart = await commerce.carts.create(params);
      return {
        success: true,
        message: 'Cart created successfully',
        cart: {
          id: cart.id,
          cartNumber: cart.cartNumber,
          status: cart.status,
          currency: cart.currency,
        },
      };
    },
  },

  {
    name: 'update_cart',
    description: 'Update cart customer details, shipping method, coupon code, or notes.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
      customerEmail: z.string().email().optional().describe('Updated customer email'),
      customerPhone: z.string().max(30).optional().describe('Updated customer phone'),
      customerName: z.string().max(200).optional().describe('Updated customer name'),
      shippingMethod: z.string().min(1).optional().describe('Shipping method'),
      couponCode: z.string().min(1).optional().describe('Coupon code'),
      notes: z.string().max(2000).optional().describe('Cart notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { cartId, ...updates } = params;
      if (!allowApply) {
        return applyRequired('Update cart', { cartId, updates });
      }

      const cart = await commerce.carts.update(cartId, updates);
      return {
        success: true,
        message: 'Cart updated',
        cart: {
          id: cart.id,
          customerEmail: cart.customerEmail,
          customerPhone: cart.customerPhone,
          customerName: cart.customerName,
          shippingMethod: cart.shippingMethod,
          couponCode: cart.couponCode,
        },
      };
    },
  },

  {
    name: 'list_customer_carts',
    description: 'List carts for a specific customer.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID (UUID)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const carts = await commerce.carts.forCustomer(params.customerId);
      return {
        success: true,
        count: carts.length,
        carts: carts.map((c) => ({
          id: c.id,
          cartNumber: c.cartNumber,
          status: c.status,
          grandTotal: c.grandTotal,
          itemCount: c.itemCount,
          updatedAt: c.updatedAt,
        })),
      };
    },
  },

  {
    name: 'delete_cart',
    description: 'Delete a cart permanently.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'delete',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete cart', params);
      }

      await commerce.carts.delete(params.cartId);
      return { success: true, message: 'Cart deleted' };
    },
  },

  {
    name: 'add_cart_item',
    description: 'Add an item to a shopping cart.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
      sku: z.string().min(1).max(100).describe('Product SKU'),
      name: z.string().min(1).max(255).describe('Product name'),
      quantity: z.number().int().min(1).describe('Quantity to add'),
      unitPrice: z.number().positive().describe('Unit price'),
      description: z.string().max(1000).optional().describe('Item description'),
      imageUrl: z.string().url().optional().describe('Product image URL'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Add item operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldAdd: {
            cartId: params.cartId,
            sku: params.sku,
            name: params.name,
            quantity: params.quantity,
            unitPrice: params.unitPrice,
            lineTotal: params.quantity * params.unitPrice,
          },
        };
      }
      const item = await commerce.carts.addItem(params.cartId, {
        sku: params.sku,
        name: params.name,
        quantity: params.quantity,
        unitPrice: params.unitPrice,
        description: params.description,
        imageUrl: params.imageUrl,
      });
      return {
        success: true,
        message: 'Item added to cart',
        item: {
          id: item.id,
          sku: item.sku,
          name: item.name,
          quantity: item.quantity,
          unitPrice: item.unitPrice,
          total: item.total,
        },
      };
    },
  },

  {
    name: 'update_cart_item',
    description: 'Update the quantity of an item in the cart.',
    inputSchema: {
      itemId: z.string().min(1).describe('Cart item ID (UUID)'),
      quantity: z.number().int().min(1).describe('New quantity'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { itemId, quantity } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Update operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldUpdate: { itemId, newQuantity: quantity },
        };
      }
      const item = await commerce.carts.updateItem(itemId, { quantity });
      return {
        success: true,
        message: 'Cart item updated',
        item: { id: item.id, sku: item.sku, quantity: item.quantity, total: item.total },
      };
    },
  },

  {
    name: 'remove_cart_item',
    description: 'Remove an item from the cart.',
    inputSchema: {
      itemId: z.string().min(1).describe('Cart item ID (UUID)'),
    },
    permission: 'delete',
    handler: async ({ commerce, params, allowApply }) => {
      const { itemId } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Remove operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldRemove: { itemId },
        };
      }
      await commerce.carts.removeItem(itemId);
      return { success: true, message: 'Item removed from cart' };
    },
  },

  {
    name: 'list_cart_items',
    description: 'List items currently in a cart.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const items = await commerce.carts.getItems(params.cartId);
      return { success: true, count: items.length, items };
    },
  },

  {
    name: 'clear_cart_items',
    description: 'Remove all items from a cart.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'delete',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Clear cart items', params);
      }

      await commerce.carts.clearItems(params.cartId);
      return { success: true, message: 'Cart items cleared' };
    },
  },

  {
    name: 'set_cart_shipping_address',
    description: 'Set the shipping address for a cart.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
      ...cartAddressInputSchema,
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Set address operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldSet: {
            cartId: params.cartId,
            address: `${params.firstName} ${params.lastName}, ${params.line1}, ${params.city}, ${params.state} ${params.postalCode}, ${params.country}`,
          },
        };
      }
      const cart = await commerce.carts.setShippingAddress(params.cartId, buildCartAddress(params));
      return {
        success: true,
        message: 'Shipping address set',
        cart: { id: cart.id, shippingAddress: cart.shippingAddress },
      };
    },
  },

  {
    name: 'set_cart_shipping',
    description: 'Set shipping address and shipping selection for a cart.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
      ...cartAddressInputSchema,
      shippingMethod: z.string().min(1).optional().describe('Shipping method'),
      shippingCarrier: z.string().min(1).optional().describe('Shipping carrier'),
      shippingAmount: z.number().min(0).optional().describe('Shipping amount'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Set cart shipping', params);
      }

      const cart = await commerce.carts.setShipping(params.cartId, {
        shippingAddress: buildCartAddress(params),
        shippingMethod: params.shippingMethod,
        shippingCarrier: params.shippingCarrier,
        shippingAmount: params.shippingAmount,
      });
      return {
        success: true,
        message: 'Cart shipping updated',
        cart: {
          id: cart.id,
          shippingAddress: cart.shippingAddress,
          shippingMethod: cart.shippingMethod,
          shippingCarrier: cart.shippingCarrier,
          shippingAmount: cart.shippingAmount,
        },
      };
    },
  },

  {
    name: 'set_cart_billing_address',
    description: 'Set the billing address for a cart.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
      ...cartAddressInputSchema,
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Set cart billing address', params);
      }

      const cart = await commerce.carts.setBillingAddress(params.cartId, buildCartAddress(params));
      return {
        success: true,
        message: 'Billing address set',
        cart: { id: cart.id, billingAddress: cart.billingAddress },
      };
    },
  },

  {
    name: 'set_cart_payment',
    description: 'Set the payment method for a cart.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
      paymentMethod: z
        .string()
        .min(1)
        .describe('Payment method (e.g., credit_card, paypal, crypto)'),
      paymentToken: z.string().optional().describe('Payment token from payment provider'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { cartId, paymentMethod, paymentToken } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Set payment operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldSet: { cartId, paymentMethod },
        };
      }
      const cart = await commerce.carts.setPayment(cartId, { paymentMethod, paymentToken });
      return {
        success: true,
        message: 'Payment method set',
        cart: { id: cart.id, paymentMethod: cart.paymentMethod, paymentStatus: cart.paymentStatus },
      };
    },
  },

  {
    name: 'apply_cart_discount',
    description: 'Apply a coupon/discount code to the cart.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
      couponCode: z.string().min(1).describe('Coupon or discount code'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { cartId, couponCode } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Apply discount operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldApply: { cartId, couponCode },
        };
      }
      const cart = await commerce.carts.applyDiscount(cartId, couponCode);
      return {
        success: true,
        message: `Discount code "${couponCode}" applied`,
        cart: {
          id: cart.id,
          couponCode: cart.couponCode,
          discountAmount: cart.discountAmount,
          grandTotal: cart.grandTotal,
        },
      };
    },
  },

  {
    name: 'remove_cart_discount',
    description: 'Remove the coupon or discount from a cart.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'delete',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Remove cart discount', params);
      }

      const cart = await commerce.carts.removeDiscount(params.cartId);
      return {
        success: true,
        message: 'Cart discount removed',
        cart: {
          id: cart.id,
          couponCode: cart.couponCode,
          discountAmount: cart.discountAmount,
          grandTotal: cart.grandTotal,
        },
      };
    },
  },

  {
    name: 'get_shipping_rates',
    description: 'Get available shipping rates for a cart based on contents and address.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { cartId } = params;
      const rates = await commerce.carts.getShippingRates(cartId);
      return {
        success: true,
        rates: rates.map((r) => ({
          id: r.id,
          carrier: r.carrier,
          service: r.service,
          price: r.price,
          currency: r.currency,
          estimatedDays: r.estimatedDays,
        })),
      };
    },
  },

  {
    name: 'mark_cart_ready_for_payment',
    description: 'Mark a cart as ready for payment processing.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Mark cart ready for payment', params);
      }

      const cart = await commerce.carts.markReadyForPayment(params.cartId);
      return {
        success: true,
        message: 'Cart marked ready for payment',
        cart: { id: cart.id, status: cart.status, paymentStatus: cart.paymentStatus },
      };
    },
  },

  {
    name: 'begin_cart_checkout',
    description: 'Begin the checkout process for a cart.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Begin cart checkout', params);
      }

      const cart = await commerce.carts.beginCheckout(params.cartId);
      return {
        success: true,
        message: 'Cart checkout started',
        cart: { id: cart.id, status: cart.status, paymentStatus: cart.paymentStatus },
      };
    },
  },

  {
    name: 'complete_checkout',
    description:
      'Complete the checkout process and convert the cart to an order. This is the final step in the checkout flow.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { cartId } = params;
      if (!allowApply) {
        const cart = await commerce.carts.get(cartId);
        if (!cart) return { success: false, error: 'Cart not found' };
        return {
          success: false,
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
            paymentMethod: cart.paymentMethod,
          },
        };
      }
      const result = await commerce.carts.complete(cartId);
      return {
        success: true,
        message: 'Checkout completed successfully! Order created.',
        result: {
          orderId: result.orderId,
          orderNumber: result.orderNumber,
          cartId: result.cartId,
          totalCharged: result.totalCharged,
          currency: result.currency,
          paymentId: result.paymentId,
        },
      };
    },
  },

  {
    name: 'cancel_cart',
    description: 'Cancel a shopping cart.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'delete',
    handler: async ({ commerce, params, allowApply }) => {
      const { cartId } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Cancel operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldCancel: { cartId },
        };
      }
      const cart = await commerce.carts.cancel(cartId);
      return {
        success: true,
        message: 'Cart cancelled',
        cart: { id: cart.id, cartNumber: cart.cartNumber, status: cart.status },
      };
    },
  },

  {
    name: 'abandon_cart',
    description: 'Mark a cart as abandoned (for recovery campaigns).',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { cartId } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Abandon operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldAbandon: { cartId },
        };
      }
      const cart = await commerce.carts.abandon(cartId);
      return {
        success: true,
        message: 'Cart marked as abandoned',
        cart: { id: cart.id, cartNumber: cart.cartNumber, status: cart.status },
      };
    },
  },

  {
    name: 'expire_cart',
    description: 'Mark a cart as expired.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Expire cart', params);
      }

      const cart = await commerce.carts.expire(params.cartId);
      return {
        success: true,
        message: 'Cart expired',
        cart: { id: cart.id, cartNumber: cart.cartNumber, status: cart.status },
      };
    },
  },

  {
    name: 'reserve_cart_inventory',
    description: 'Reserve inventory for all cart items.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Reserve cart inventory', params);
      }

      const cart = await commerce.carts.reserveInventory(params.cartId);
      return {
        success: true,
        message: 'Cart inventory reserved',
        cart: { id: cart.id, inventoryReserved: cart.inventoryReserved },
      };
    },
  },

  {
    name: 'release_cart_inventory',
    description: 'Release reserved inventory for all cart items.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Release cart inventory', params);
      }

      const cart = await commerce.carts.releaseInventory(params.cartId);
      return {
        success: true,
        message: 'Cart inventory released',
        cart: { id: cart.id, inventoryReserved: cart.inventoryReserved },
      };
    },
  },

  {
    name: 'recalculate_cart',
    description: 'Recalculate cart totals after pricing or address changes.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Recalculate cart', params);
      }

      const cart = await commerce.carts.recalculate(params.cartId);
      return {
        success: true,
        message: 'Cart recalculated',
        cart: {
          id: cart.id,
          subtotal: cart.subtotal,
          taxAmount: cart.taxAmount,
          shippingAmount: cart.shippingAmount,
          discountAmount: cart.discountAmount,
          grandTotal: cart.grandTotal,
        },
      };
    },
  },

  {
    name: 'set_cart_tax',
    description: 'Set the tax amount for a cart explicitly.',
    inputSchema: {
      cartId: z.string().min(1).describe('Cart ID (UUID)'),
      taxAmount: z.number().min(0).describe('Tax amount'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Set cart tax', params);
      }

      const cart = await commerce.carts.setTax(params.cartId, params.taxAmount);
      return {
        success: true,
        message: 'Cart tax updated',
        cart: { id: cart.id, taxAmount: cart.taxAmount, grandTotal: cart.grandTotal },
      };
    },
  },

  {
    name: 'get_abandoned_carts',
    description: 'Get all abandoned carts for recovery campaigns.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const carts = await commerce.carts.getAbandoned();
      return {
        success: true,
        count: carts.length,
        carts: carts.map((c) => ({
          id: c.id,
          cartNumber: c.cartNumber,
          customerEmail: c.customerEmail,
          grandTotal: c.grandTotal,
          itemCount: c.itemCount,
          createdAt: c.createdAt,
          updatedAt: c.updatedAt,
        })),
      };
    },
  },

  {
    name: 'get_expired_carts',
    description: 'Get all expired carts.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const carts = await commerce.carts.getExpired();
      return {
        success: true,
        count: carts.length,
        carts: carts.map((c) => ({
          id: c.id,
          cartNumber: c.cartNumber,
          customerEmail: c.customerEmail,
          grandTotal: c.grandTotal,
          itemCount: c.itemCount,
          createdAt: c.createdAt,
          updatedAt: c.updatedAt,
        })),
      };
    },
  },
];

export default cartTools;
