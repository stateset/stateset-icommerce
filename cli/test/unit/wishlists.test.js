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

// Shape of the binding's WishlistOutput. Items have no id; they are keyed by
// productId.
const mockWishlist = {
  id: 'wl_001',
  customerId: 'cust_001',
  name: 'Birthday Ideas',
  isPublic: false,
  items: [
    { productId: 'prod_001', quantity: 1, addedAt: '2026-02-01T00:00:00Z' },
    { productId: 'prod_multi', quantity: 1, addedAt: '2026-02-01T00:00:00Z' },
    { productId: 'prod_draft', quantity: 1, addedAt: '2026-02-01T00:00:00Z' },
  ],
  createdAt: '2026-02-01T00:00:00Z',
  updatedAt: '2026-02-01T00:00:00Z',
};

// Shape of the binding's WishlistItemOutput.
const mockWishlistItem = {
  productId: 'prod_002',
  quantity: 1,
  addedAt: '2026-02-01T00:00:00Z',
};

const products = {
  prod_001: { id: 'prod_001', name: 'Widget', status: 'active' },
  prod_multi: { id: 'prod_multi', name: 'Gadget', status: 'active' },
  prod_draft: { id: 'prod_draft', name: 'Draft', status: 'draft' },
};
const variantsByProduct = {
  prod_001: [
    {
      id: 'var_w',
      productId: 'prod_001',
      sku: 'W-1',
      name: '',
      priceExact: '19.99',
      isDefault: true,
    },
  ],
  // Several variants and none marked default: cannot be priced honestly.
  prod_multi: [
    {
      id: 'var_g1',
      productId: 'prod_multi',
      sku: 'G-1',
      name: 'S',
      priceExact: '5.00',
      isDefault: false,
    },
    {
      id: 'var_g2',
      productId: 'prod_multi',
      sku: 'G-2',
      name: 'L',
      priceExact: '6.00',
      isDefault: false,
    },
  ],
  prod_draft: [
    {
      id: 'var_d',
      productId: 'prod_draft',
      sku: 'D-1',
      name: '',
      priceExact: '1.00',
      isDefault: true,
    },
  ],
};

// ============================================================================
// Mock commerce factory
// ============================================================================

function makeWishlistCommerce(overrides = {}, cartOverrides = {}) {
  const calls = [];
  // Only methods the real Wishlists / Products / Carts binding classes have.
  return {
    calls,
    wishlists: {
      create: async (data) => ({ ...mockWishlist, ...data }),
      get: async (id) => (id === 'wl_001' ? mockWishlist : null),
      addItem: async (_wishlistId, data) => ({ ...mockWishlistItem, ...data }),
      removeItem: async (wishlistId, productId) => {
        calls.push(['wishlists.removeItem', wishlistId, productId]);
      },
      list: async () => [mockWishlist],
      ...overrides,
    },
    products: {
      get: async (id) => products[id] ?? null,
      getVariant: async (id) =>
        Object.values(variantsByProduct)
          .flat()
          .find((v) => v.id === id) ?? null,
      getVariants: async (productId) => variantsByProduct[productId] ?? [],
    },
    carts: {
      create: async (input) => {
        calls.push(['carts.create', input]);
        return { id: 'cart_001', customerId: input.customerId };
      },
      addItemExact: async (cartId, item) => {
        calls.push(['carts.addItemExact', cartId, item]);
        return { id: `ci_${item.sku}`, cartId, ...item };
      },
      ...cartOverrides,
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
    assert.equal(calledWith.isPublic, true);
    assert.ok(!('visibility' in calledWith), 'CreateWishlistInput has no visibility');
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
    assert.equal(calledWith.isPublic, false);
  });

  it("refuses visibility 'shared', which the engine cannot represent", async () => {
    let called = false;
    const commerce = makeWishlistCommerce({
      create: async () => {
        called = true;
      },
    });
    const result = await tool.handler({
      commerce,
      params: { customerId: 'cust_001', visibility: 'shared' },
      allowApply: true,
    });
    assert.equal(result.success, false);
    assert.match(result.error, /shared/);
    assert.equal(called, false);
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
    let calledWishlistId, calledProductId;
    const commerce = makeWishlistCommerce({
      removeItem: async (wid, productId) => {
        calledWishlistId = wid;
        calledProductId = productId;
      },
    });
    // Wishlist items are keyed by product; itemId carries the product ID.
    await tool.handler({
      commerce,
      params: { wishlistId: 'wl_001', itemId: 'prod_001' },
      allowApply: true,
    });
    assert.equal(calledWishlistId, 'wl_001');
    assert.equal(calledProductId, 'prod_001');
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
    await tool.handler({ commerce, params: { customerId: 'cust_999', limit: 20 } });
    assert.deepStrictEqual(calledFilter, { customerId: 'cust_999', limit: 20 });
  });

  it('derives visibility and itemCount from the real output fields', async () => {
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { customerId: 'cust_001', limit: 5 },
    });
    assert.equal(result.wishlists[0].visibility, 'private');
    assert.equal(result.wishlists[0].itemCount, 3);
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

  it('prices each line from the catalog and reports unpriceable items', async () => {
    const commerce = makeWishlistCommerce();
    const result = await tool.handler({
      commerce,
      params: { wishlistId: 'wl_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('converted'));
    assert.equal(result.cartId, 'cart_001');
    assert.equal(result.itemsAdded, 1);
    assert.deepStrictEqual(
      result.itemsUnavailable.map((u) => u.productId),
      ['prod_multi', 'prod_draft'],
    );
    assert.match(result.itemsUnavailable[0].reason, /none is default/);
    assert.match(result.itemsUnavailable[1].reason, /draft/);
    assert.deepStrictEqual(commerce.calls, [
      ['carts.create', { customerId: 'cust_001' }],
      [
        'carts.addItemExact',
        'cart_001',
        {
          productId: 'prod_001',
          variantId: 'var_w',
          sku: 'W-1',
          name: 'Widget',
          quantity: 1,
          unitPrice: '19.99',
        },
      ],
    ]);
    assert.deepStrictEqual(result.removedFromWishlist, []);
  });

  it('uses the wishlist item variant when one is named', async () => {
    const commerce = makeWishlistCommerce({
      get: async () => ({
        ...mockWishlist,
        items: [{ productId: 'prod_multi', variantId: 'var_g2', quantity: 3, addedAt: 'x' }],
      }),
    });
    const result = await tool.handler({
      commerce,
      params: { wishlistId: 'wl_001' },
      allowApply: true,
    });
    assert.equal(result.itemsAdded, 1);
    const [, , line] = commerce.calls.find((c) => c[0] === 'carts.addItemExact');
    assert.equal(line.sku, 'G-2');
    assert.equal(line.unitPrice, '6.00');
    assert.equal(line.quantity, 3);
  });

  it('clearWishlist removes only the lines that reached the cart', async () => {
    const commerce = makeWishlistCommerce();
    const result = await tool.handler({
      commerce,
      params: { wishlistId: 'wl_001', clearWishlist: true },
      allowApply: true,
    });
    assert.deepStrictEqual(result.removedFromWishlist, ['prod_001']);
    assert.deepStrictEqual(
      commerce.calls.filter((c) => c[0] === 'wishlists.removeItem'),
      [['wishlists.removeItem', 'wl_001', 'prod_001']],
    );
  });

  it('creates no cart when no item can be priced', async () => {
    const commerce = makeWishlistCommerce({
      get: async () => ({
        ...mockWishlist,
        items: [{ productId: 'prod_draft', quantity: 1, addedAt: 'x' }],
      }),
    });
    const result = await tool.handler({
      commerce,
      params: { wishlistId: 'wl_001' },
      allowApply: true,
    });
    assert.equal(result.success, false);
    assert.equal(result.itemsAdded, 0);
    assert.equal(commerce.calls.length, 0);
  });

  it('returns not found for a missing wishlist', async () => {
    const result = await tool.handler({
      commerce: makeWishlistCommerce(),
      params: { wishlistId: 'wl_missing' },
      allowApply: true,
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'Wishlist not found');
  });

  it('propagates a cart creation failure', async () => {
    const commerce = makeWishlistCommerce(
      {},
      {
        create: async () => {
          throw new Error('cart creation failed');
        },
      },
    );
    await assert.rejects(
      () => tool.handler({ commerce, params: { wishlistId: 'wl_001' }, allowApply: true }),
      /cart creation failed/,
    );
  });
});
