/**
 * Wishlist Tools Module
 *
 * MCP tool definitions for wishlist creation, management, and cart conversion.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/** Public projection of a binding `WishlistOutput`. */
function projectWishlist(w, { withItems = false } = {}) {
  return {
    id: w.id,
    customerId: w.customerId,
    name: w.name,
    visibility: w.isPublic ? 'public' : 'private',
    itemCount: w.items.length,
    ...(withItems ? { items: w.items } : {}),
    createdAt: w.createdAt,
    updatedAt: w.updatedAt,
  };
}

/**
 * Resolve the priced variant a wishlist item refers to. Returns
 * `{ product, variant }`, or `{ reason }` when the item cannot be priced
 * honestly (missing/inactive product, unknown variant, or no single default).
 */
async function resolvePricedVariant(commerce, item) {
  const product = await commerce.products.get(item.productId);
  if (!product) return { reason: 'product not found' };
  if (product.status !== 'active') return { reason: `product is ${product.status}` };

  if (item.variantId) {
    const variant = await commerce.products.getVariant(item.variantId);
    if (!variant || variant.productId !== item.productId) {
      return { reason: 'variant not found for product' };
    }
    return { product, variant };
  }

  const variants = await commerce.products.getVariants(item.productId);
  const variant =
    variants.find((v) => v.isDefault) ?? (variants.length === 1 ? variants[0] : undefined);
  if (!variant) {
    return {
      reason:
        variants.length === 0
          ? 'product has no priced variant'
          : 'product has several variants and none is default; wishlist item names no variant',
    };
  }
  return { product, variant };
}

/**
 * Create a cart for the wishlist's customer and add each wishlist item at its
 * catalog variant price. The binding has no convert-to-cart, so it is composed
 * here from `wishlists`, `products` and `carts`; items that cannot be priced
 * are reported, not added. Shared by the MCP tool and the `wishlists convert`
 * command.
 */
export async function convertWishlistToCart(commerce, wishlistId, { clearWishlist = false } = {}) {
  const wishlist = await commerce.wishlists.get(wishlistId);
  if (!wishlist) {
    return { success: false, error: 'Wishlist not found' };
  }

  // Price every line before creating anything, so an unpriceable wishlist
  // never leaves an empty cart behind.
  const lines = [];
  const itemsUnavailable = [];
  for (const item of wishlist.items) {
    const resolved = await resolvePricedVariant(commerce, item);
    if (resolved.reason) {
      itemsUnavailable.push({
        productId: item.productId,
        variantId: item.variantId,
        reason: resolved.reason,
      });
    } else {
      lines.push({ item, ...resolved });
    }
  }
  if (lines.length === 0) {
    return {
      success: false,
      error: 'No wishlist items could be priced; no cart was created',
      itemsAdded: 0,
      itemsUnavailable,
    };
  }

  const cart = await commerce.carts.create({ customerId: wishlist.customerId });
  const added = [];
  for (const { item, product, variant } of lines) {
    try {
      await commerce.carts.addItemExact(cart.id, {
        productId: product.id,
        variantId: variant.id,
        sku: variant.sku,
        name: variant.name ? `${product.name} - ${variant.name}` : product.name,
        quantity: item.quantity,
        unitPrice: variant.priceExact,
      });
      added.push(item);
    } catch (err) {
      itemsUnavailable.push({
        productId: item.productId,
        variantId: item.variantId,
        reason: `add to cart failed: ${err?.message ?? String(err)}`,
      });
    }
  }

  const removedFromWishlist = [];
  if (clearWishlist) {
    // Only the lines that actually reached the cart leave the wishlist.
    for (const item of added) {
      await commerce.wishlists.removeItem(wishlist.id, item.productId);
      removedFromWishlist.push(item.productId);
    }
  }

  return {
    success: added.length > 0,
    message:
      added.length > 0 ? 'Wishlist converted to cart' : 'Cart created but no items could be added',
    cartId: cart.id,
    itemsAdded: added.length,
    itemsUnavailable,
    removedFromWishlist,
  };
}

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

      const visibility = params.visibility || 'private';
      if (visibility === 'shared') {
        return {
          success: false,
          error: "visibility 'shared' is not supported by the engine; use 'private' or 'public'",
        };
      }

      const wishlist = await commerce.wishlists.create({
        customerId: params.customerId,
        name: params.name || 'My Wishlist',
        isPublic: visibility === 'public',
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

      return { success: true, wishlist: projectWishlist(wishlist, { withItems: true }) };
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
      itemId: z
        .string()
        .min(1)
        .describe(
          'Product ID of the wishlist item to remove (wishlist items are keyed by product)',
        ),
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
      const wishlists = await commerce.wishlists.list({ customerId, limit });

      return {
        success: true,
        customerId,
        returned: wishlists.length,
        wishlists: wishlists.map((w) => projectWishlist(w)),
      };
    },
  },

  {
    name: 'convert_wishlist_to_cart',
    description:
      "Create a cart for the wishlist's customer and add each wishlist item at its catalog variant price. Items that cannot be priced are reported, not added.",
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

      return convertWishlistToCart(commerce, params.wishlistId, {
        clearWishlist: params.clearWishlist,
      });
    },
  },
];

export default wishlistTools;
