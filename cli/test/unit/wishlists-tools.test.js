/**
 * Wishlist Tools Test Suite
 *
 * Tests for cli/src/tools/wishlists.js
 * Covers: create_wishlist, get_wishlist, add_to_wishlist,
 *         remove_from_wishlist, list_wishlists, convert_wishlist_to_cart
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { wishlistTools } from '../../src/tools/wishlists.js';

// ============================================================================
// Helper: find tool by name
// ============================================================================

function findTool(name) {
  const tool = wishlistTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock factory
// ============================================================================

const mockWishlistItem = {
  id: 'wi_001',
  productId: 'prod_001',
  variantId: 'var_001',
  note: 'Love this one',
  priority: 1,
};

const mockWishlist = {
  id: 'wl_001',
  customerId: 'cust_001',
  name: 'Birthday List',
  visibility: 'private',
  itemCount: 1,
  items: [mockWishlistItem],
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-02-01T00:00:00Z',
};

function makeWishlistCommerce(overrides = {}) {
  return {
    wishlists: {
      create: async (data) => ({ ...mockWishlist, ...data }),
      get: async (_id) => mockWishlist,
      list: async (_filters) => [mockWishlist],
      addItem: async (_wlId, data) => ({ ...mockWishlistItem, ...data }),
      removeItem: async (_wlId, _itemId) => undefined,
      convertToCart: async (_wlId, _opts) => ({
        cartId: 'cart_001',
        itemsAdded: 1,
        itemsUnavailable: 0,
      }),
      ...overrides,
    },
  };
}

// ============================================================================
// Module exports
// ============================================================================

describe('wishlistTools -- module exports', () => {
  it('exports an array of 6 tools', () => {
    assert.ok(Array.isArray(wishlistTools));
    assert.equal(wishlistTools.length, 6);
  });

  it('exports expected tool names in order', () => {
    const names = wishlistTools.map((t) => t.name);
    assert.deepStrictEqual(names, [
      'create_wishlist',
      'get_wishlist',
      'add_to_wishlist',
      'remove_from_wishlist',
      'list_wishlists',
      'convert_wishlist_to_cart',
    ]);
  });

  it('all tools have handler functions', () => {
    for (const tool of wishlistTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of wishlistTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of wishlistTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });

  it('all tools have inputSchema objects', () => {
    for (const tool of wishlistTools) {
      assert.equal(typeof tool.inputSchema, 'object', `${tool.name} missing inputSchema`);
    }
  });
});

// ============================================================================
// Input schema validation
// ============================================================================

describe('wishlistTools -- input schemas', () => {
  it('create_wishlist has customerId, name, visibility', () => {
    const schema = findTool('create_wishlist').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('customerId'));
    assert.ok(keys.includes('name'));
    assert.ok(keys.includes('visibility'));
  });

  it('get_wishlist has wishlistId', () => {
    const schema = findTool('get_wishlist').inputSchema;
    assert.ok(Object.keys(schema).includes('wishlistId'));
  });

  it('add_to_wishlist has wishlistId, productId, variantId, note, priority', () => {
    const schema = findTool('add_to_wishlist').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('wishlistId'));
    assert.ok(keys.includes('productId'));
    assert.ok(keys.includes('variantId'));
    assert.ok(keys.includes('note'));
    assert.ok(keys.includes('priority'));
  });

  it('remove_from_wishlist has wishlistId, itemId', () => {
    const schema = findTool('remove_from_wishlist').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('wishlistId'));
    assert.ok(keys.includes('itemId'));
  });

  it('list_wishlists has customerId, limit', () => {
    const schema = findTool('list_wishlists').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('customerId'));
    assert.ok(keys.includes('limit'));
  });

  it('convert_wishlist_to_cart has wishlistId, clearWishlist', () => {
    const schema = findTool('convert_wishlist_to_cart').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('wishlistId'));
    assert.ok(keys.includes('clearWishlist'));
  });
});

// ============================================================================
// Permission checks
// ============================================================================

describe('wishlistTools -- permissions', () => {
  it('read tools have read permission', () => {
    assert.equal(findTool('get_wishlist').permission, 'read');
    assert.equal(findTool('list_wishlists').permission, 'read');
  });

  it('write tools have write permission', () => {
    assert.equal(findTool('create_wishlist').permission, 'write');
    assert.equal(findTool('add_to_wishlist').permission, 'write');
    assert.equal(findTool('remove_from_wishlist').permission, 'write');
    assert.equal(findTool('convert_wishlist_to_cart').permission, 'write');
  });
});

// ============================================================================
// Handler apply-guard (write tools without --apply)
// ============================================================================

describe('wishlistTools -- apply-guard', () => {
  it('create_wishlist requires --apply', async () => {
    const tool = findTool('create_wishlist');
    const result = await tool.handler({
      params: { customerId: 'cust_001' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('add_to_wishlist requires --apply', async () => {
    const tool = findTool('add_to_wishlist');
    const result = await tool.handler({
      params: { wishlistId: 'wl_001', productId: 'prod_001' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('remove_from_wishlist requires --apply', async () => {
    const tool = findTool('remove_from_wishlist');
    const result = await tool.handler({
      params: { wishlistId: 'wl_001', itemId: 'wi_001' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('convert_wishlist_to_cart requires --apply', async () => {
    const tool = findTool('convert_wishlist_to_cart');
    const result = await tool.handler({
      params: { wishlistId: 'wl_001' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('apply-guard returns hint about --apply', async () => {
    const tool = findTool('create_wishlist');
    const result = await tool.handler({
      params: { customerId: 'cust_001' },
      allowApply: false,
      commerce: {},
    });
    assert.ok(result.hint);
    assert.ok(result.hint.includes('--apply'));
  });

  it('apply-guard returns preview (wouldDo) with params', async () => {
    const params = { wishlistId: 'wl_001', productId: 'prod_002', priority: 2 };
    const tool = findTool('add_to_wishlist');
    const result = await tool.handler({ params, allowApply: false, commerce: {} });
    assert.equal(result.success, false);
    assert.deepStrictEqual(result.wouldDo, params);
  });
});

// ============================================================================
// Handler success paths (with mocked commerce)
// ============================================================================

describe('wishlistTools -- create_wishlist handler', () => {
  it('creates wishlist when allowApply is true', async () => {
    const tool = findTool('create_wishlist');
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { customerId: 'cust_001', name: 'Holiday Gifts' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Wishlist created');
    assert.ok(result.wishlist);
  });
});

describe('wishlistTools -- get_wishlist handler', () => {
  it('returns wishlist with expected fields', async () => {
    const tool = findTool('get_wishlist');
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { wishlistId: 'wl_001' },
    });
    assert.equal(result.success, true);
    assert.ok(result.wishlist);
    assert.equal(result.wishlist.id, 'wl_001');
    assert.equal(result.wishlist.customerId, 'cust_001');
    assert.equal(result.wishlist.name, 'Birthday List');
    assert.equal(result.wishlist.visibility, 'private');
    assert.equal(result.wishlist.itemCount, 1);
    assert.ok(Array.isArray(result.wishlist.items));
    assert.ok(result.wishlist.createdAt);
    assert.ok(result.wishlist.updatedAt);
  });

  it('returns not found when wishlist is null', async () => {
    const tool = findTool('get_wishlist');
    const result = await tool.handler({
      commerce: makeWishlistCommerce({ get: async () => null }),
      params: { wishlistId: 'wl_missing' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'Wishlist not found');
  });
});

describe('wishlistTools -- add_to_wishlist handler', () => {
  it('adds item to wishlist when allowApply is true', async () => {
    const tool = findTool('add_to_wishlist');
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { wishlistId: 'wl_001', productId: 'prod_002' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Item added to wishlist');
    assert.ok(result.item);
  });
});

describe('wishlistTools -- remove_from_wishlist handler', () => {
  it('removes item from wishlist when allowApply is true', async () => {
    const tool = findTool('remove_from_wishlist');
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { wishlistId: 'wl_001', itemId: 'wi_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Item removed from wishlist');
  });
});

describe('wishlistTools -- list_wishlists handler', () => {
  it('returns list with customerId and returned count', async () => {
    const tool = findTool('list_wishlists');
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { customerId: 'cust_001', limit: 20 },
    });
    assert.equal(result.success, true);
    assert.equal(result.customerId, 'cust_001');
    assert.equal(result.returned, 1);
    assert.ok(Array.isArray(result.wishlists));
    assert.equal(result.wishlists[0].id, 'wl_001');
  });

  it('maps expected fields on each wishlist', async () => {
    const tool = findTool('list_wishlists');
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { customerId: 'cust_001' },
    });
    const w = result.wishlists[0];
    const expectedKeys = ['id', 'name', 'visibility', 'itemCount', 'createdAt', 'updatedAt'];
    for (const key of expectedKeys) {
      assert.ok(key in w, `missing key: ${key}`);
    }
  });
});

describe('wishlistTools -- convert_wishlist_to_cart handler', () => {
  it('converts wishlist to cart when allowApply is true', async () => {
    const tool = findTool('convert_wishlist_to_cart');
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { wishlistId: 'wl_001', clearWishlist: true },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Wishlist converted to cart');
    assert.equal(result.cartId, 'cart_001');
    assert.equal(result.itemsAdded, 1);
    assert.equal(result.itemsUnavailable, 0);
  });
});

// ============================================================================
// Handler error paths (commerce object missing methods)
// ============================================================================

describe('wishlistTools -- error paths', () => {
  it('get_wishlist throws when commerce.wishlists is undefined', async () => {
    const tool = findTool('get_wishlist');
    await assert.rejects(
      () => tool.handler({ commerce: {}, params: { wishlistId: 'wl_001' } }),
      (err) => err instanceof TypeError,
    );
  });

  it('list_wishlists throws when commerce.wishlists is undefined', async () => {
    const tool = findTool('list_wishlists');
    await assert.rejects(
      () => tool.handler({ commerce: {}, params: { customerId: 'cust_001' } }),
      (err) => err instanceof TypeError,
    );
  });

  it('create_wishlist throws when commerce.wishlists.create is missing', async () => {
    const tool = findTool('create_wishlist');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { wishlists: {} },
          params: { customerId: 'cust_001' },
          allowApply: true,
        }),
      (err) => err instanceof TypeError,
    );
  });

  it('add_to_wishlist throws when commerce.wishlists.addItem is missing', async () => {
    const tool = findTool('add_to_wishlist');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { wishlists: {} },
          params: { wishlistId: 'wl_001', productId: 'prod_001' },
          allowApply: true,
        }),
      (err) => err instanceof TypeError,
    );
  });

  it('remove_from_wishlist throws when commerce.wishlists.removeItem is missing', async () => {
    const tool = findTool('remove_from_wishlist');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { wishlists: {} },
          params: { wishlistId: 'wl_001', itemId: 'wi_001' },
          allowApply: true,
        }),
      (err) => err instanceof TypeError,
    );
  });

  it('convert_wishlist_to_cart throws when commerce.wishlists.convertToCart is missing', async () => {
    const tool = findTool('convert_wishlist_to_cart');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { wishlists: {} },
          params: { wishlistId: 'wl_001' },
          allowApply: true,
        }),
      (err) => err instanceof TypeError,
    );
  });
});
