/**
 * Product Tools Test Suite
 *
 * Tests for cli/src/tools/products.js
 * Covers: list_products, get_product, get_product_variant, create_product
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { productTools } from '../../src/tools/products.js';

// ============================================================================
// Helper: find tool by name
// ============================================================================

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock factory
// ============================================================================

const mockProduct = {
  id: 'prod_001',
  name: 'Widget Pro',
  slug: 'widget-pro',
  status: 'active',
  description: 'A premium widget',
  createdAt: '2026-02-21T00:00:00Z',
};

const mockVariant = {
  id: 'var_001',
  sku: 'WIDGET-PRO-SM',
  name: 'Small',
  price: '29.99',
  compareAtPrice: '39.99',
  productId: 'prod_001',
};

function makeProductCommerce(overrides = {}) {
  return {
    products: {
      list: async () => [mockProduct],
      count: async () => 1,
      get: async (_id) => mockProduct,
      getVariantBySku: async (_sku) => mockVariant,
      create: async (data) => ({ ...mockProduct, ...data }),
      ...overrides,
    },
  };
}

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Product Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(productTools));
  });

  it('has at least 4 tools', () => {
    assert.ok(productTools.length >= 4, `Expected >= 4, got ${productTools.length}`);
  });

  it('every tool has name, handler, and permission', () => {
    for (const tool of productTools) {
      assert.ok(tool.name, 'tool missing name');
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });
});

// ============================================================================
// list_products
// ============================================================================

describe('list_products', () => {
  const tool = findTool(productTools, 'list_products');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns list with totalCount and returned', async () => {
    const result = await tool.handler({
      commerce: makeProductCommerce(),
      params: { limit: 50 },
    });
    assert.equal(result.success, true);
    assert.equal(result.totalCount, 1);
    assert.equal(result.returned, 1);
    assert.equal(result.products.length, 1);
    assert.equal(result.products[0].id, 'prod_001');
  });

  it('maps all expected fields on each product', async () => {
    const result = await tool.handler({
      commerce: makeProductCommerce(),
      params: { limit: 50 },
    });
    const p = result.products[0];
    assert.ok('id' in p);
    assert.ok('name' in p);
    assert.ok('slug' in p);
    assert.ok('status' in p);
    assert.ok('createdAt' in p);
  });

  it('respects limit parameter', async () => {
    const manyProducts = Array.from({ length: 10 }, (_, i) => ({
      ...mockProduct,
      id: `prod_${i}`,
    }));
    const commerce = makeProductCommerce({
      list: async () => manyProducts,
      count: async () => 10,
    });
    const result = await tool.handler({
      commerce,
      params: { limit: 3 },
    });
    assert.equal(result.totalCount, 10);
    assert.equal(result.returned, 3);
    assert.equal(result.products.length, 3);
  });

  it('returns error when list throws', async () => {
    const commerce = makeProductCommerce({
      list: async () => {
        throw new Error('DB error');
      },
    });
    try {
      await tool.handler({ commerce, params: { limit: 50 } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('DB error'));
    }
  });
});

// ============================================================================
// get_product
// ============================================================================

describe('get_product', () => {
  const tool = findTool(productTools, 'get_product');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns product for valid ID', async () => {
    const result = await tool.handler({
      commerce: makeProductCommerce(),
      params: { productId: 'prod_001' },
    });
    assert.equal(result.success, true);
    assert.ok(result.product);
    assert.equal(result.product.id, 'prod_001');
    assert.equal(result.product.name, 'Widget Pro');
  });

  it('returns success: false when product not found', async () => {
    const commerce = makeProductCommerce({ get: async () => null });
    const result = await tool.handler({
      commerce,
      params: { productId: 'NONEXISTENT' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when get throws', async () => {
    const commerce = makeProductCommerce({
      get: async () => {
        throw new Error('lookup failed');
      },
    });
    try {
      await tool.handler({ commerce, params: { productId: 'prod_001' } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('lookup failed'));
    }
  });
});

// ============================================================================
// get_product_variant
// ============================================================================

describe('get_product_variant', () => {
  const tool = findTool(productTools, 'get_product_variant');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns variant for valid SKU', async () => {
    const result = await tool.handler({
      commerce: makeProductCommerce(),
      params: { sku: 'WIDGET-PRO-SM' },
    });
    assert.equal(result.success, true);
    assert.ok(result.variant);
    assert.equal(result.variant.sku, 'WIDGET-PRO-SM');
    assert.equal(result.variant.price, '29.99');
  });

  it('returns success: false when variant not found', async () => {
    const commerce = makeProductCommerce({ getVariantBySku: async () => null });
    const result = await tool.handler({
      commerce,
      params: { sku: 'NONEXISTENT-SKU' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when getVariantBySku throws', async () => {
    const commerce = makeProductCommerce({
      getVariantBySku: async () => {
        throw new Error('SKU lookup failed');
      },
    });
    try {
      await tool.handler({ commerce, params: { sku: 'BAD-SKU' } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('SKU lookup failed'));
    }
  });
});

// ============================================================================
// create_product
// ============================================================================

describe('create_product', () => {
  const tool = findTool(productTools, 'create_product');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeProductCommerce(),
      params: { name: 'New Widget', description: 'A new widget' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field');
    assert.ok(result.wouldCreate, 'expected wouldCreate preview');
  });

  it('creates product with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeProductCommerce(),
      params: { name: 'New Widget', description: 'A new widget' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('created'));
    assert.ok(result.product);
    assert.ok(result.product.id);
    assert.ok(result.product.name);
    assert.ok(result.product.slug);
  });

  it('calls autoIndexEntity when provided', async () => {
    let indexed = null;
    const result = await tool.handler({
      commerce: makeProductCommerce(),
      params: { name: 'Indexed Widget' },
      allowApply: true,
      autoIndexEntity: (type, entity) => {
        indexed = { type, entity };
      },
    });
    assert.equal(result.success, true);
    assert.equal(indexed.type, 'product');
    assert.ok(indexed.entity);
  });

  it('returns error when commerce.products.create throws', async () => {
    const commerce = makeProductCommerce({
      create: async () => {
        throw new Error('Duplicate product name');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { name: 'Duplicate Widget' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Duplicate product name'));
    }
  });
});
