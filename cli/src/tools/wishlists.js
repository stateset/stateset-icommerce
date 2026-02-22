/**
 * Wishlist Tools Module
 *
 * MCP tool definitions for wishlist creation, management, and cart conversion.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/**
 * Wishlist tool definitions
 */
export const wishlistTools = [
  {
    name: 'create_wishlist',
    description: 'Create a new wishlist for a customer.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
      name: z.string().min(1).max(255).optional().default('My Wishlist').describe('Wishlist name'),
      visibility: z
        .enum(['private', 'public', 'shared'])
        .optional()
        .default('private')
        .describe('Wishlist visibility'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create wishlist', params);
      }

      const wishlist = await commerce.wishlists.create({
        customerId: params.customerId,
        name: params.name || 'My Wishlist',
        visibility: params.visibility || 'private',
      });
      return { success: true, message: 'Wishlist created', wishlist };
    },
  },

  {
    name: 'get_wishlist',
    description: 'Get a wishlist by ID including all items.',
    inputSchema: {
      wishlistId: z.string().min(1).describe('Wishlist ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { wishlistId } = params;
      const wishlist = await commerce.wishlists.get(wishlistId);

      if (!wishlist) {
        return { success: false, error: 'Wishlist not found' };
      }

      return {
        success: true,
        wishlist: {
          id: wishlist.id,
          customerId: wishlist.customerId,
          name: wishlist.name,
          visibility: wishlist.visibility,
          itemCount: wishlist.itemCount,
          items: wishlist.items,
          createdAt: wishlist.createdAt,
          updatedAt: wishlist.updatedAt,
        },
      };
    },
  },

  {
    name: 'add_to_wishlist',
    description: 'Add a product to a wishlist.',
    inputSchema: {
      wishlistId: z.string().min(1).describe('Wishlist ID'),
      productId: z.string().min(1).describe('Product ID to add'),
      variantId: z.string().min(1).optional().describe('Specific variant ID'),
      note: z.string().max(500).optional().describe('Personal note about the item'),
      priority: z
        .number()
        .int()
        .min(1)
        .max(5)
        .optional()
        .describe('Priority level (1=highest, 5=lowest)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Add to wishlist', params);
      }

      const item = await commerce.wishlists.addItem(params.wishlistId, {
        productId: params.productId,
        variantId: params.variantId,
        note: params.note,
        priority: params.priority,
      });
      return { success: true, message: 'Item added to wishlist', item };
    },
  },

  {
    name: 'remove_from_wishlist',
    description: 'Remove a product from a wishlist.',
    inputSchema: {
      wishlistId: z.string().min(1).describe('Wishlist ID'),
      itemId: z.string().min(1).describe('Wishlist item ID to remove'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Remove from wishlist', params);
      }

      await commerce.wishlists.removeItem(params.wishlistId, params.itemId);
      return { success: true, message: 'Item removed from wishlist' };
    },
  },

  {
    name: 'list_wishlists',
    description: 'List wishlists for a customer.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
      limit: z
        .number()
        .int()
        .min(1)
        .max(100)
        .optional()
        .default(20)
        .describe('Maximum number of wishlists to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { customerId, limit } = params;
      const wishlists = await commerce.wishlists.list({ customerId });
      const limited = wishlists.slice(0, limit);

      return {
        success: true,
        customerId,
        returned: limited.length,
        wishlists: limited.map((w) => ({
          id: w.id,
          name: w.name,
          visibility: w.visibility,
          itemCount: w.itemCount,
          createdAt: w.createdAt,
          updatedAt: w.updatedAt,
        })),
      };
    },
  },

  {
    name: 'convert_wishlist_to_cart',
    description: 'Convert all items in a wishlist to a shopping cart.',
    inputSchema: {
      wishlistId: z.string().min(1).describe('Wishlist ID to convert'),
      clearWishlist: z
        .boolean()
        .optional()
        .default(false)
        .describe('Whether to clear the wishlist after conversion'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Convert wishlist to cart', params);
      }

      const result = await commerce.wishlists.convertToCart(params.wishlistId, {
        clearWishlist: params.clearWishlist || false,
      });
      return {
        success: true,
        message: 'Wishlist converted to cart',
        cartId: result.cartId,
        itemsAdded: result.itemsAdded,
        itemsUnavailable: result.itemsUnavailable,
      };
    },
  },
];

export default wishlistTools;
