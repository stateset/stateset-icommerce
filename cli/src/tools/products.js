/**
 * Product Tools Module
 *
 * MCP tool definitions for product catalog operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

const variantInput = {
  sku: z.string().min(1).max(100).describe('Variant SKU'),
  name: z.string().max(255).optional().describe('Variant name'),
  price: z
    .union([z.string().regex(/^\d+(?:\.\d+)?$/), z.number().positive()])
    .describe(
      'Exact decimal string variant price (legacy numeric input is supported outside strict mode)',
    ),
  compareAtPrice: z
    .union([z.string().regex(/^\d+(?:\.\d+)?$/), z.number().positive()])
    .optional()
    .describe('Exact decimal string compare-at price'),
};

function productSummary(product) {
  return {
    id: product.id,
    name: product.name,
    slug: product.slug,
    status: product.status,
    createdAt: product.createdAt,
    updatedAt: product.updatedAt,
  };
}

/**
 * Product tool definitions
 */
export const productTools = withPolicyDomain('products', [
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

      return { success: true, product };
    },
  },

  {
    name: 'get_product_by_slug',
    description: 'Get a specific product by its URL slug.',
    inputSchema: {
      slug: z.string().min(1).describe('Product slug'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const product = await commerce.products.getBySlug(params.slug);

      if (!product) {
        return { success: false, error: `Product with slug ${params.slug} not found` };
      }

      return { success: true, product };
    },
  },

  {
    name: 'search_products',
    description: 'Search active products by name or description.',
    inputSchema: {
      query: z.string().min(1).describe('Search query'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const products = await commerce.products.search(params.query);
      return {
        success: true,
        count: products.length,
        products: products.map((p) => ({
          id: p.id,
          name: p.name,
          slug: p.slug,
          status: p.status,
        })),
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

      return { success: true, variant };
    },
  },

  {
    name: 'list_product_variants',
    description: 'List all variants for a product.',
    inputSchema: {
      productId: z.string().min(1).describe('Product ID (UUID)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const variants = await commerce.products.getVariants(params.productId);
      return { success: true, count: variants.length, variants };
    },
  },

  {
    name: 'create_product',
    description: 'Create a new product with optional variants.',
    inputSchema: {
      name: z.string().min(1).max(255).describe('Product name'),
      description: z.string().max(5000).optional().describe('Product description'),
      variants: z.array(z.object(variantInput)).max(100).optional().describe('Product variants'),
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

  {
    name: 'update_product',
    description:
      'Update an existing product. Only the fields provided are changed (name, slug, description, status).',
    inputSchema: {
      productId: z.string().min(1).describe('Product ID (UUID)'),
      name: z.string().min(1).max(255).optional().describe('New product name'),
      slug: z.string().min(1).max(255).optional().describe('New URL slug'),
      description: z.string().max(5000).optional().describe('New product description'),
      status: z.enum(['draft', 'active', 'archived']).optional().describe('New product status'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, autoIndexEntity }) => {
      if (!allowApply) {
        return applyRequired('Update product', params);
      }

      const { productId, ...update } = params;
      const product = await commerce.products.update(productId, update);
      if (autoIndexEntity) autoIndexEntity('product', product);

      return {
        success: true,
        message: 'Product updated successfully',
        product: productSummary(product),
      };
    },
  },

  {
    name: 'activate_product',
    description: 'Activate a product, making it available for purchase.',
    inputSchema: {
      productId: z.string().min(1).describe('Product ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Activate product', params);
      }

      const product = await commerce.products.activate(params.productId);
      return {
        success: true,
        message: 'Product activated',
        product: productSummary(product),
      };
    },
  },

  {
    name: 'archive_product',
    description: 'Archive a product, removing it from sale.',
    inputSchema: {
      productId: z.string().min(1).describe('Product ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Archive product', params);
      }

      const product = await commerce.products.archive(params.productId);
      return {
        success: true,
        message: 'Product archived',
        product: productSummary(product),
      };
    },
  },

  {
    name: 'delete_product',
    description: 'Delete a product (archives it).',
    inputSchema: {
      productId: z.string().min(1).describe('Product ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete product', params);
      }

      await commerce.products.delete(params.productId);
      return { success: true, message: 'Product deleted', productId: params.productId };
    },
  },

  {
    name: 'add_product_variant',
    description: 'Add a variant to an existing product.',
    inputSchema: {
      productId: z.string().min(1).describe('Product ID (UUID)'),
      ...variantInput,
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Add product variant', params);
      }

      const { productId, ...variant } = params;
      const created = await commerce.products.addVariant(productId, variant);
      return { success: true, message: 'Variant added', variant: created };
    },
  },

  {
    name: 'update_product_variant',
    description: 'Update an existing product variant.',
    inputSchema: {
      variantId: z.string().min(1).describe('Variant ID (UUID)'),
      ...variantInput,
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update product variant', params);
      }

      const { variantId, ...variant } = params;
      const updated = await commerce.products.updateVariant(variantId, variant);
      return { success: true, message: 'Variant updated', variant: updated };
    },
  },

  {
    name: 'delete_product_variant',
    description: 'Delete a product variant.',
    inputSchema: {
      variantId: z.string().min(1).describe('Variant ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete product variant', params);
      }

      await commerce.products.deleteVariant(params.variantId);
      return { success: true, message: 'Variant deleted', variantId: params.variantId };
    },
  },
]);

export default productTools;
