/**
 * Wishlist Tools Test Suite
 *
 * Tests for the wishlistTools module (cli/src/tools/wishlists.js):
 * - create_wishlist (write)
 * - get_wishlist (read)
 * - add_to_wishlist (write)
 * - remove_from_wishlist (write)
 * - list_wishlists (read)
 * - convert_wishlist_to_cart (write)
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { wishlistTools } from '../../src/tools/wishlists.js';

// ============================================================================
// Helper: find tool by name from a tools array
// ============================================================================

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock data
// ============================================================================

const mockWishlist = {
  id: 'wl_001',
  customerId: 'cust_001',
  name: 'Birthday Ideas',
  visibility: 'private',
  isPublic: false,
  itemCount: 3,
  items: [{ id: 'wli_001', productId: 'prod_001', variantId: null, note: null, priority: 1 }],
  createdAt: '2026-02-01T00:00:00Z',
  updatedAt: '2026-02-01T00:00:00Z',
};

const mockCartResult = {
  cartId: 'cart_001',
  itemsAdded: 3,
  itemsUnavailable: 0,
};

const mockWishlistItem = {
  id: 'wli_002',
  wishlistId: 'wl_001',
  productId: 'prod_002',
  variantId: null,
  note: null,
  priority: null,
  addedAt: '2026-02-01T00:00:00Z',
};

// ============================================================================
// Mock commerce factory
// ============================================================================

function makeWishlistCommerce(overrides = {}) {
  return {
    wishlists: {
      create: async (data) => ({ ...mockWishlist, ...data }),
      get: async (id) => (id === 'wl_001' ? mockWishlist : null),
      addItem: async (wishlistId, data) => ({ ...mockWishlistItem, wishlistId, ...data }),
      removeItem: async (_wishlistId, _itemId) => undefined,
      list: async () => [mockWishlist],
      convertToCart: async (_wishlistId, _opts) => mockCartResult,
      ...overrides,
    },
  };
}

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Wishlist Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(wishlistTools));
  });

  it('exports exactly 6 tools', () => {
    assert.equal(wishlistTools.length, 6);
  });

  it('every tool has name, handler, and permission', () => {
    for (const tool of wishlistTools) {
      assert.ok(tool.name, `missing name`);
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });

  it('write tool permissions are correct', () => {
    const writeTools = [
      'create_wishlist',
      'add_to_wishlist',
      'remove_from_wishlist',
      'convert_wishlist_to_cart',
    ];
    for (const name of writeTools) {
      const tool = findTool(wishlistTools, name);
      assert.equal(tool.permission, 'write', `${name} should have write permission`);
    }
  });

  it('read tool permissions are correct', () => {
    const readTools = ['get_wishlist', 'list_wishlists'];
    for (const name of readTools) {
      const tool = findTool(wishlistTools, name);
      assert.equal(tool.permission, 'read', `${name} should have read permission`);
    }
  });
});

// ============================================================================
// create_wishlist
// ============================================================================

describe('create_wishlist', () => {
  const tool = findTool(wishlistTools, 'create_wishlist');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { customerId: 'cust_001', name: 'Birthday Ideas' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
    assert.ok(result.hint);
  });

  it('creates wishlist with --apply and returns success: true', async () => {
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { customerId: 'cust_001', name: 'Birthday Ideas', visibility: 'private' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('created'));
    assert.ok(result.wishlist);
    assert.equal(result.wishlist.customerId, 'cust_001');
  });

  it('passes customerId, name, visibility to commerce.wishlists.create', async () => {
    let calledWith;
    const commerce = makeWishlistCommerce({
      create: async (data) => {
        calledWith = data;
        return { ...mockWishlist, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: { customerId: 'cust_002', name: 'Gift Ideas', visibility: 'public' },
      allowApply: true,
    });
    assert.equal(calledWith.customerId, 'cust_002');
    assert.equal(calledWith.name, 'Gift Ideas');
    assert.equal(calledWith.visibility, 'public');
  });

  it('uses default name and visibility when not provided', async () => {
    let calledWith;
    const commerce = makeWishlistCommerce({
      create: async (data) => {
        calledWith = data;
        return { ...mockWishlist, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: { customerId: 'cust_001' },
      allowApply: true,
    });
    assert.ok(calledWith.name);
    assert.ok(calledWith.visibility);
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeWishlistCommerce({
      create: async () => {
        throw new Error('create failed');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { customerId: 'cust_001' }, allowApply: true }),
      /create failed/,
    );
  });
});

// ============================================================================
// get_wishlist
// ============================================================================

describe('get_wishlist', () => {
  const tool = findTool(wishlistTools, 'get_wishlist');

  it('returns wishlist with items for valid ID', async () => {
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { wishlistId: 'wl_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.wishlist.id, 'wl_001');
    assert.equal(result.wishlist.customerId, 'cust_001');
    assert.equal(result.wishlist.name, 'Birthday Ideas');
    assert.ok(Array.isArray(result.wishlist.items));
  });

  it('returns success: false for unknown wishlist ID', async () => {
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { wishlistId: 'wl_nope' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeWishlistCommerce({
      get: async () => {
        throw new Error('DB unavailable');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { wishlistId: 'wl_001' } }),
      /DB unavailable/,
    );
  });
});

// ============================================================================
// add_to_wishlist
// ============================================================================

describe('add_to_wishlist', () => {
  const tool = findTool(wishlistTools, 'add_to_wishlist');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { wishlistId: 'wl_001', productId: 'prod_002' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('adds item with --apply and returns success: true', async () => {
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { wishlistId: 'wl_001', productId: 'prod_002' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('added'));
    assert.ok(result.item);
  });

  it('passes all optional fields to commerce.wishlists.addItem', async () => {
    let calledWishlistId, calledData;
    const commerce = makeWishlistCommerce({
      addItem: async (wid, data) => {
        calledWishlistId = wid;
        calledData = data;
        return { ...mockWishlistItem, wishlistId: wid, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: {
        wishlistId: 'wl_001',
        productId: 'prod_003',
        variantId: 'var_001',
        note: 'Would love this',
        priority: 2,
      },
      allowApply: true,
    });
    assert.equal(calledWishlistId, 'wl_001');
    assert.equal(calledData.productId, 'prod_003');
    assert.equal(calledData.variantId, 'var_001');
    assert.equal(calledData.note, 'Would love this');
    assert.equal(calledData.priority, 2);
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeWishlistCommerce({
      addItem: async () => {
        throw new Error('wishlist not found');
      },
    });
    await assert.rejects(
      () =>
        tool.handler({
          commerce,
          params: { wishlistId: 'wl_x', productId: 'p' },
          allowApply: true,
        }),
      /wishlist not found/,
    );
  });
});

// ============================================================================
// remove_from_wishlist
// ============================================================================

describe('remove_from_wishlist', () => {
  const tool = findTool(wishlistTools, 'remove_from_wishlist');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { wishlistId: 'wl_001', itemId: 'wli_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('removes item with --apply and returns success: true', async () => {
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { wishlistId: 'wl_001', itemId: 'wli_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('removed'));
  });

  it('calls commerce.wishlists.removeItem with correct args', async () => {
    let calledWishlistId, calledItemId;
    const commerce = makeWishlistCommerce({
      removeItem: async (wid, iid) => {
        calledWishlistId = wid;
        calledItemId = iid;
      },
    });
    await tool.handler({
      commerce,
      params: { wishlistId: 'wl_001', itemId: 'wli_001' },
      allowApply: true,
    });
    assert.equal(calledWishlistId, 'wl_001');
    assert.equal(calledItemId, 'wli_001');
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeWishlistCommerce({
      removeItem: async () => {
        throw new Error('item not found');
      },
    });
    await assert.rejects(
      () =>
        tool.handler({
          commerce,
          params: { wishlistId: 'wl_001', itemId: 'wli_x' },
          allowApply: true,
        }),
      /item not found/,
    );
  });
});

// ============================================================================
// list_wishlists
// ============================================================================

describe('list_wishlists', () => {
  const tool = findTool(wishlistTools, 'list_wishlists');

  it('returns wishlists for customer', async () => {
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { customerId: 'cust_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.customerId, 'cust_001');
    assert.equal(result.returned, 1);
    assert.ok(Array.isArray(result.wishlists));
    assert.equal(result.wishlists[0].id, 'wl_001');
    assert.equal(result.wishlists[0].name, 'Birthday Ideas');
  });

  it('passes customerId filter to commerce.wishlists.list', async () => {
    let calledFilter;
    const commerce = makeWishlistCommerce({
      list: async (filter) => {
        calledFilter = filter;
        return [];
      },
    });
    await tool.handler({ commerce, params: { customerId: 'cust_999' } });
    assert.equal(calledFilter.customerId, 'cust_999');
  });

  it('slices results to limit', async () => {
    const manyWishlists = Array.from({ length: 30 }, (_, i) => ({
      ...mockWishlist,
      id: `wl_${String(i).padStart(3, '0')}`,
    }));
    const commerce = makeWishlistCommerce({
      list: async () => manyWishlists,
    });
    const result = await tool.handler({ commerce, params: { customerId: 'cust_001', limit: 5 } });
    assert.equal(result.returned, 5);
    assert.equal(result.wishlists.length, 5);
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeWishlistCommerce({
      list: async () => {
        throw new Error('list query failed');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { customerId: 'cust_001' } }),
      /list query failed/,
    );
  });
});

// ============================================================================
// convert_wishlist_to_cart
// ============================================================================

describe('convert_wishlist_to_cart', () => {
  const tool = findTool(wishlistTools, 'convert_wishlist_to_cart');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { wishlistId: 'wl_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('converts wishlist to cart with --apply and returns success: true', async () => {
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { wishlistId: 'wl_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('converted'));
    assert.equal(result.cartId, 'cart_001');
    assert.equal(result.itemsAdded, 3);
    assert.equal(result.itemsUnavailable, 0);
  });

  it('passes clearWishlist option to commerce.wishlists.convertToCart', async () => {
    let calledWishlistId, calledOpts;
    const commerce = makeWishlistCommerce({
      convertToCart: async (wid, opts) => {
        calledWishlistId = wid;
        calledOpts = opts;
        return mockCartResult;
      },
    });
    await tool.handler({
      commerce,
      params: { wishlistId: 'wl_001', clearWishlist: true },
      allowApply: true,
    });
    assert.equal(calledWishlistId, 'wl_001');
    assert.equal(calledOpts.clearWishlist, true);
  });

  it('defaults clearWishlist to false', async () => {
    let calledOpts;
    const commerce = makeWishlistCommerce({
      convertToCart: async (_wid, opts) => {
        calledOpts = opts;
        return mockCartResult;
      },
    });
    await tool.handler({
      commerce,
      params: { wishlistId: 'wl_001' },
      allowApply: true,
    });
    assert.equal(calledOpts.clearWishlist, false);
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeWishlistCommerce({
      convertToCart: async () => {
        throw new Error('cart creation failed');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { wishlistId: 'wl_001' }, allowApply: true }),
      /cart creation failed/,
    );
  });
});
