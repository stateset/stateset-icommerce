/**
 * Product Commands Module
 *
 * Handles all product-related CLI operations for stateset-direct
 */

/**
 * Execute product commands
 * @param {string} action - The action to perform
 * @param {Array} args - Command arguments
 * @param {Object} options - Command options
 * @returns {Promise<any>} Command result
 */
export async function execute(
  action,
  args,
  { commerce, output, jsonOutput, resolveId, resolveSku },
) {
  switch (action) {
    case 'list': {
      const products = await commerce.products.list();
      return formatProductList(products, { output, jsonOutput });
    }

    case 'get': {
      const idArg = args[0];
      if (!idArg) {
        throw new Error('Usage: products get <id>\n\nProvide a product ID.');
      }

      const id = await resolveId(idArg, 'products');
      const product = await commerce.products.get(id);

      if (!product) {
        throw new Error(
          `Product not found: ${idArg}\n\nTry 'stateset-direct products list' to see all products.`,
        );
      }

      return formatProductDetail(product, { output, jsonOutput });
    }

    case 'variant': {
      const skuArg = args[0];
      if (!skuArg) {
        throw new Error('Usage: products variant <sku>\n\nProvide a product variant SKU.');
      }

      const sku = await resolveSku(skuArg);
      const variant = await commerce.products.getVariantBySku(sku);

      if (!variant) {
        throw new Error(
          `Variant not found: ${skuArg}\n\nTry 'stateset-direct products list' to see all products and their variants.`,
        );
      }

      return formatVariantDetail(variant, { output, jsonOutput });
    }

    case 'count': {
      const count = await commerce.products.count();
      return { count, formatted: `Product count: ${count}` };
    }

    case 'search': {
      const query = args.join(' ');
      if (!query) {
        throw new Error('Usage: products search <query>\n\nSearch by name or slug.');
      }

      const products = await commerce.products.list();
      const matches = products.filter(
        (p) =>
          p.name.toLowerCase().includes(query.toLowerCase()) ||
          (p.slug && p.slug.toLowerCase().includes(query.toLowerCase())),
      );

      return formatProductList(matches, { output, jsonOutput });
    }

    case 'variants': {
      const idArg = args[0];
      if (!idArg) {
        throw new Error('Usage: products variants <id>\n\nList all variants for a product.');
      }

      const id = await resolveId(idArg, 'products');
      const product = await commerce.products.get(id);

      if (!product) {
        throw new Error(`Product not found: ${idArg}`);
      }

      return formatVariantList(product, { output, jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: products ${action}\n\n` +
          'Available actions:\n' +
          '  list              List all products\n' +
          '  get <id>          Get product details\n' +
          '  variant <sku>     Get variant by SKU\n' +
          '  variants <id>     List variants for product\n' +
          '  count             Count products\n' +
          '  search <query>    Search products',
      );
  }
}

/**
 * Format product list for output
 */
function formatProductList(products, { output, jsonOutput }) {
  if (jsonOutput) {
    return products;
  }

  if (products.length === 0) {
    return { formatted: 'No products found.' };
  }

  const formatted = output.table(
    products.map((p) => ({
      id: p.id.slice(0, 8) + '...',
      name: p.name.length > 30 ? p.name.slice(0, 27) + '...' : p.name,
      slug: p.slug || 'N/A',
      status: p.status,
      variants: p.variants?.length || 0,
    })),
    [
      { key: 'id', header: 'ID' },
      { key: 'name', header: 'Name' },
      { key: 'slug', header: 'Slug' },
      { key: 'status', header: 'Status' },
      { key: 'variants', header: 'Variants', align: 'right' },
    ],
  );

  return { products, formatted };
}

/**
 * Format single product detail
 */
function formatProductDetail(product, { output: _output, jsonOutput }) {
  if (jsonOutput) {
    return product;
  }

  const variantLines =
    product.variants?.map((v) => `  - ${v.sku}: ${v.name} @ ${v.price}`).join('\n') ||
    '  (no variants)';

  const formatted = `
Product: ${product.name}
${'-'.repeat(40)}
ID:          ${product.id}
Slug:        ${product.slug || 'N/A'}
Status:      ${product.status}
Description: ${product.description || 'N/A'}
Created:     ${product.createdAt}

Variants:
${variantLines}
`;

  return { product, formatted };
}

/**
 * Format variant detail
 */
function formatVariantDetail(variant, { output: _output, jsonOutput }) {
  if (jsonOutput) {
    return variant;
  }

  const formatted = `
Variant: ${variant.name}
${'-'.repeat(40)}
SKU:         ${variant.sku}
Price:       ${variant.price}
Compare At:  ${variant.compareAtPrice || 'N/A'}
Default:     ${variant.isDefault ? 'Yes' : 'No'}
`;

  return { variant, formatted };
}

/**
 * Format variant list for a product
 */
function formatVariantList(product, { output, jsonOutput }) {
  const variants = product.variants || [];

  if (jsonOutput) {
    return variants;
  }

  if (variants.length === 0) {
    return { formatted: `No variants found for product: ${product.name}` };
  }

  const formatted = output.table(
    variants.map((v) => ({
      sku: v.sku,
      name: v.name,
      price: v.price,
      default: v.isDefault ? 'Yes' : 'No',
    })),
    [
      { key: 'sku', header: 'SKU' },
      { key: 'name', header: 'Name' },
      { key: 'price', header: 'Price', align: 'right' },
      { key: 'default', header: 'Default' },
    ],
  );

  return { variants, formatted: `Variants for ${product.name}:\n\n${formatted}` };
}

/**
 * Command metadata for help/completion
 */
export const metadata = {
  name: 'products',
  aliases: ['p', 'prod'],
  description: 'Product management commands',
  actions: {
    list: { description: 'List all products', args: [] },
    get: { description: 'Get product by ID', args: ['<id>'] },
    variant: { description: 'Get variant by SKU', args: ['<sku>'] },
    variants: { description: 'List variants for product', args: ['<id>'] },
    count: { description: 'Count products', args: [] },
    search: { description: 'Search products', args: ['<query>'] },
  },
};

export default { execute, metadata };
