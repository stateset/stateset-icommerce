/**
 * Product Tools Module
 *
 * MCP tool definitions for product catalog operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

/**
 * Product tool definitions
 */
export const productTools = [
  {
    name: 'list_products',
    description: 'List all products in the catalog.',
    inputSchema: {
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .default(50)
        .describe('Maximum number of products to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { limit } = params;
      const products = await commerce.products.list();
      const count = await commerce.products.count();
      const limitedProducts = products.slice(0, limit);

      return {
        success: true,
        totalCount: count,
        returned: limitedProducts.length,
        products: limitedProducts.map((p) => ({
          id: p.id,
          name: p.name,
          slug: p.slug,
          status: p.status,
          createdAt: p.createdAt,
        })),
      };
    },
  },

  {
    name: 'get_product',
    description: 'Get a specific product by ID.',
    inputSchema: {
      productId: z.string().min(1).describe('Product ID (UUID)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { productId } = params;
      const product = await commerce.products.get(productId);

      if (!product) {
        return { success: false, error: 'Product not found' };
      }

      return {
        success: true,
        product,
      };
    },
  },

  {
    name: 'get_product_variant',
    description: 'Get a product variant by SKU.',
    inputSchema: {
      sku: z.string().min(1).describe('Product variant SKU'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { sku } = params;
      const variant = await commerce.products.getVariantBySku(sku);

      if (!variant) {
        return { success: false, error: `Variant with SKU ${sku} not found` };
      }

      return {
        success: true,
        variant,
      };
    },
  },

  {
    name: 'create_product',
    description: 'Create a new product with optional variants.',
    inputSchema: {
      name: z.string().min(1).max(255).describe('Product name'),
      description: z.string().max(5000).optional().describe('Product description'),
      variants: z
        .array(
          z.object({
            sku: z.string().min(1).max(100).describe('Variant SKU'),
            name: z.string().max(255).optional().describe('Variant name'),
            price: z.number().positive().describe('Variant price'),
            compareAtPrice: z
              .number()
              .positive()
              .optional()
              .describe('Compare at price (original price)'),
          }),
        )
        .max(100)
        .optional()
        .describe('Product variants'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, autoIndexEntity }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Create operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldCreate: params,
        };
      }

      const product = await commerce.products.create(params);
      if (autoIndexEntity) autoIndexEntity('product', product);
      return {
        success: true,
        message: 'Product created successfully',
        product: {
          id: product.id,
          name: product.name,
          slug: product.slug,
        },
      };
    },
  },
];

export default productTools;
