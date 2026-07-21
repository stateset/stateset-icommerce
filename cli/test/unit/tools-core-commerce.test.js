/**
 * Core Commerce Tools Test Suite
 *
 * Tests the expanded customer, product, and returns MCP tool surfaces:
 * registration, policy domains, permissions, apply-guard on writes, and
 * handler behavior against a mocked commerce instance.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { customerTools } from '../../src/tools/customers.js';
import { productTools } from '../../src/tools/products.js';
import { returnTools } from '../../src/tools/returns.js';
import { DOMAIN_TOOL_ARRAYS, TOOL_POLICY_DOMAIN_BY_NAME } from '../../src/tools/domain-registry.js';

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// REGISTRY
// ============================================================================

describe('Core commerce domain registry', () => {
  it('registers the modules', () => {
    assert.equal(DOMAIN_TOOL_ARRAYS.customers, customerTools);
    assert.equal(DOMAIN_TOOL_ARRAYS.products, productTools);
    assert.equal(DOMAIN_TOOL_ARRAYS.returns, returnTools);
  });

  it('assigns policy domains', () => {
    assert.equal(TOOL_POLICY_DOMAIN_BY_NAME.update_customer, 'customers');
    assert.equal(TOOL_POLICY_DOMAIN_BY_NAME.add_customer_address, 'customers');
    assert.equal(TOOL_POLICY_DOMAIN_BY_NAME.update_product, 'products');
    assert.equal(TOOL_POLICY_DOMAIN_BY_NAME.add_product_variant, 'products');
    assert.equal(TOOL_POLICY_DOMAIN_BY_NAME.complete_return, 'returns');
    assert.equal(TOOL_POLICY_DOMAIN_BY_NAME.list_returns_for_order, 'returns');
  });

  it('exposes the expected customer tool names', () => {
    assert.deepEqual(
      customerTools.map((t) => t.name),
      [
        'list_customers',
        'get_customer',
        'create_customer',
        'update_customer',
        'delete_customer',
        'find_or_create_customer',
        'list_customer_addresses',
        'add_customer_address',
        'update_customer_address',
        'delete_customer_address',
        'set_default_customer_address',
      ],
    );
  });

  it('exposes the expected product tool names', () => {
    assert.deepEqual(
      productTools.map((t) => t.name),
      [
        'list_products',
        'get_product',
        'get_product_by_slug',
        'search_products',
        'get_product_variant',
        'list_product_variants',
        'create_product',
        'update_product',
        'activate_product',
        'archive_product',
        'delete_product',
        'add_product_variant',
        'update_product_variant',
        'delete_product_variant',
      ],
    );
  });

  it('exposes the expected return tool names', () => {
    assert.deepEqual(
      returnTools.map((t) => t.name),
      [
        'list_returns',
        'get_return',
        'list_returns_for_order',
        'list_returns_for_customer',
        'list_pending_returns',
        'create_return',
        'approve_return',
        'reject_return',
        'mark_return_received',
        'complete_return',
        'cancel_return',
        'add_return_tracking',
      ],
    );
  });

  it('marks read and write tools correctly', () => {
    const reads = new Set([
      'list_customers',
      'get_customer',
      'list_customer_addresses',
      'list_products',
      'get_product',
      'get_product_by_slug',
      'search_products',
      'get_product_variant',
      'list_product_variants',
      'list_returns',
      'get_return',
      'list_returns_for_order',
      'list_returns_for_customer',
      'list_pending_returns',
    ]);
    for (const tool of [...customerTools, ...productTools, ...returnTools]) {
      assert.equal(tool.permission, reads.has(tool.name) ? 'read' : 'write', tool.name);
    }
  });
});

// ============================================================================
// APPLY GUARD
// ============================================================================

describe('Core commerce apply guard', () => {
  it('write tools refuse to mutate without allowApply', async () => {
    const writeTools = [...customerTools, ...productTools, ...returnTools].filter(
      (t) => t.permission === 'write',
    );

    for (const tool of writeTools) {
      let called = false;
      const trap = new Proxy(
        {},
        {
          get: () => async () => {
            called = true;
            return {};
          },
        },
      );
      const commerce = { customers: trap, products: trap, returns: trap };
      const result = await tool.handler({ commerce, params: {}, allowApply: false });
      assert.equal(result.success, false, tool.name);
      assert.equal(called, false, `${tool.name} must not call the API without allowApply`);
    }
  });
});

// ============================================================================
// CUSTOMERS
// ============================================================================

describe('Customer Tools', () => {
  const mockCustomer = {
    id: 'cust_001',
    email: 'alice@example.com',
    firstName: 'Alice',
    lastName: 'Smith',
    phone: '+1-555-0100',
    status: 'active',
    acceptsMarketing: false,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-02T00:00:00Z',
  };
  const mockAddress = {
    id: 'addr_001',
    customerId: 'cust_001',
    line1: '1 Main St',
    isDefault: true,
  };

  function makeCommerce(overrides = {}) {
    return {
      customers: {
        list: async () => [mockCustomer],
        count: async () => 1,
        get: async () => mockCustomer,
        getByEmail: async () => mockCustomer,
        create: async (data) => ({ ...mockCustomer, ...data }),
        update: async (id, data) => ({ ...mockCustomer, id, ...data }),
        delete: async () => undefined,
        findOrCreate: async (data) => ({ ...mockCustomer, ...data }),
        getAddresses: async () => [mockAddress],
        addAddress: async (data) => ({ ...mockAddress, ...data }),
        updateAddress: async (id, data) => ({ ...mockAddress, id, ...data }),
        deleteAddress: async () => undefined,
        setDefaultAddress: async () => undefined,
        ...overrides,
      },
    };
  }

  it('update_customer forwards fields and returns summary', async () => {
    let received;
    const commerce = makeCommerce({
      update: async (id, data) => {
        received = { id, data };
        return { ...mockCustomer, ...data };
      },
    });
    const result = await findTool(customerTools, 'update_customer').handler({
      commerce,
      params: { customerId: 'cust_001', firstName: 'Alicia', status: 'inactive' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(received.id, 'cust_001');
    assert.equal(received.data.firstName, 'Alicia');
    assert.equal(received.data.customerId, undefined);
    assert.equal(result.customer.firstName, 'Alicia');
  });

  it('delete_customer returns success', async () => {
    const result = await findTool(customerTools, 'delete_customer').handler({
      commerce: makeCommerce(),
      params: { customerId: 'cust_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.customerId, 'cust_001');
  });

  it('find_or_create_customer returns the customer', async () => {
    const result = await findTool(customerTools, 'find_or_create_customer').handler({
      commerce: makeCommerce(),
      params: { email: 'bob@example.com', firstName: 'Bob', lastName: 'Jones' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.customer.email, 'bob@example.com');
  });

  it('list_customer_addresses returns addresses with count', async () => {
    const result = await findTool(customerTools, 'list_customer_addresses').handler({
      commerce: makeCommerce(),
      params: { customerId: 'cust_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(result.addresses[0].id, 'addr_001');
  });

  it('add_customer_address forwards the address payload', async () => {
    let received;
    const commerce = makeCommerce({
      addAddress: async (data) => {
        received = data;
        return { ...mockAddress, ...data };
      },
    });
    const result = await findTool(customerTools, 'add_customer_address').handler({
      commerce,
      params: {
        customerId: 'cust_001',
        firstName: 'Alice',
        lastName: 'Smith',
        line1: '1 Main St',
        city: 'Anytown',
        postalCode: '90210',
        country: 'US',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(received.customerId, 'cust_001');
    assert.equal(received.line1, '1 Main St');
  });

  it('update_customer_address strips addressId from the payload', async () => {
    let received;
    const commerce = makeCommerce({
      updateAddress: async (id, data) => {
        received = { id, data };
        return { ...mockAddress, id };
      },
    });
    await findTool(customerTools, 'update_customer_address').handler({
      commerce,
      params: {
        addressId: 'addr_001',
        customerId: 'cust_001',
        firstName: 'Alice',
        lastName: 'Smith',
        line1: '2 Main St',
        city: 'Anytown',
        postalCode: '90210',
        country: 'US',
      },
      allowApply: true,
    });
    assert.equal(received.id, 'addr_001');
    assert.equal(received.data.addressId, undefined);
    assert.equal(received.data.line1, '2 Main St');
  });

  it('set_default_customer_address forwards all three args', async () => {
    let received;
    const commerce = makeCommerce({
      setDefaultAddress: async (customerId, addressId, addressType) => {
        received = { customerId, addressId, addressType };
      },
    });
    const result = await findTool(customerTools, 'set_default_customer_address').handler({
      commerce,
      params: { customerId: 'cust_001', addressId: 'addr_001', addressType: 'shipping' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.deepEqual(received, {
      customerId: 'cust_001',
      addressId: 'addr_001',
      addressType: 'shipping',
    });
  });
});

// ============================================================================
// PRODUCTS
// ============================================================================

describe('Product Tools', () => {
  const mockProduct = {
    id: 'prod_001',
    name: 'Widget',
    slug: 'widget',
    status: 'active',
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-02T00:00:00Z',
  };
  const mockVariant = { id: 'var_001', productId: 'prod_001', sku: 'WIDGET-001', price: 9.99 };

  function makeCommerce(overrides = {}) {
    return {
      products: {
        list: async () => [mockProduct],
        count: async () => 1,
        get: async () => mockProduct,
        getBySlug: async () => mockProduct,
        search: async () => [mockProduct],
        getVariantBySku: async () => mockVariant,
        getVariants: async () => [mockVariant],
        create: async (data) => ({ ...mockProduct, ...data }),
        update: async (id, data) => ({ ...mockProduct, id, ...data }),
        activate: async (id) => ({ ...mockProduct, id, status: 'active' }),
        archive: async (id) => ({ ...mockProduct, id, status: 'archived' }),
        delete: async () => undefined,
        addVariant: async (productId, data) => ({ ...mockVariant, productId, ...data }),
        updateVariant: async (id, data) => ({ ...mockVariant, id, ...data }),
        deleteVariant: async () => undefined,
        ...overrides,
      },
    };
  }

  it('get_product_by_slug returns the product', async () => {
    const result = await findTool(productTools, 'get_product_by_slug').handler({
      commerce: makeCommerce(),
      params: { slug: 'widget' },
    });
    assert.equal(result.success, true);
    assert.equal(result.product.slug, 'widget');
  });

  it('get_product_by_slug returns error when missing', async () => {
    const result = await findTool(productTools, 'get_product_by_slug').handler({
      commerce: makeCommerce({ getBySlug: async () => null }),
      params: { slug: 'nope' },
    });
    assert.equal(result.success, false);
  });

  it('search_products returns matches', async () => {
    let received;
    const commerce = makeCommerce({
      search: async (q) => {
        received = q;
        return [mockProduct];
      },
    });
    const result = await findTool(productTools, 'search_products').handler({
      commerce,
      params: { query: 'widget' },
    });
    assert.equal(result.success, true);
    assert.equal(received, 'widget');
    assert.equal(result.count, 1);
  });

  it('update_product strips productId and forwards update', async () => {
    let received;
    const commerce = makeCommerce({
      update: async (id, data) => {
        received = { id, data };
        return { ...mockProduct, ...data };
      },
    });
    await findTool(productTools, 'update_product').handler({
      commerce,
      params: { productId: 'prod_001', name: 'Widget Pro', status: 'draft' },
      allowApply: true,
    });
    assert.equal(received.id, 'prod_001');
    assert.equal(received.data.productId, undefined);
    assert.equal(received.data.name, 'Widget Pro');
  });

  it('activate_product / archive_product return updated status', async () => {
    const activated = await findTool(productTools, 'activate_product').handler({
      commerce: makeCommerce(),
      params: { productId: 'prod_001' },
      allowApply: true,
    });
    assert.equal(activated.product.status, 'active');
    const archived = await findTool(productTools, 'archive_product').handler({
      commerce: makeCommerce(),
      params: { productId: 'prod_001' },
      allowApply: true,
    });
    assert.equal(archived.product.status, 'archived');
  });

  it('list_product_variants returns variants', async () => {
    const result = await findTool(productTools, 'list_product_variants').handler({
      commerce: makeCommerce(),
      params: { productId: 'prod_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(result.variants[0].sku, 'WIDGET-001');
  });

  it('add_product_variant strips productId and forwards variant', async () => {
    let received;
    const commerce = makeCommerce({
      addVariant: async (productId, data) => {
        received = { productId, data };
        return { ...mockVariant, ...data };
      },
    });
    await findTool(productTools, 'add_product_variant').handler({
      commerce,
      params: { productId: 'prod_001', sku: 'WIDGET-002', price: 12.5 },
      allowApply: true,
    });
    assert.equal(received.productId, 'prod_001');
    assert.equal(received.data.productId, undefined);
    assert.equal(received.data.sku, 'WIDGET-002');
  });

  it('delete_product_variant returns success', async () => {
    const result = await findTool(productTools, 'delete_product_variant').handler({
      commerce: makeCommerce(),
      params: { variantId: 'var_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.variantId, 'var_001');
  });
});

// ============================================================================
// RETURNS
// ============================================================================

describe('Return Tools', () => {
  const mockReturn = {
    id: 'ret_001',
    orderId: 'ord_001',
    status: 'requested',
    reason: 'defective',
    createdAt: '2026-01-01T00:00:00Z',
  };

  function makeCommerce(overrides = {}) {
    return {
      returns: {
        list: async () => [mockReturn],
        count: async () => 1,
        get: async () => mockReturn,
        listForOrder: async () => [mockReturn],
        listForCustomer: async () => [mockReturn],
        listPending: async () => [mockReturn],
        create: async (data) => ({ ...mockReturn, ...data }),
        approve: async (id) => ({ ...mockReturn, id, status: 'approved' }),
        reject: async (id) => ({ ...mockReturn, id, status: 'rejected' }),
        markReceived: async (id) => ({ ...mockReturn, id, status: 'received' }),
        complete: async (id) => ({ ...mockReturn, id, status: 'completed' }),
        cancel: async (id) => ({ ...mockReturn, id, status: 'cancelled' }),
        addTracking: async (id) => ({ ...mockReturn, id, status: 'in_transit' }),
        ...overrides,
      },
    };
  }

  it('list_returns_for_order forwards the order id', async () => {
    let received;
    const commerce = makeCommerce({
      listForOrder: async (id) => {
        received = id;
        return [mockReturn];
      },
    });
    const result = await findTool(returnTools, 'list_returns_for_order').handler({
      commerce,
      params: { orderId: 'ord_001' },
    });
    assert.equal(result.success, true);
    assert.equal(received, 'ord_001');
    assert.equal(result.count, 1);
  });

  it('list_returns_for_customer forwards the customer id', async () => {
    const result = await findTool(returnTools, 'list_returns_for_customer').handler({
      commerce: makeCommerce(),
      params: { customerId: 'cust_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.returns[0].id, 'ret_001');
  });

  it('list_pending_returns returns pending returns', async () => {
    const result = await findTool(returnTools, 'list_pending_returns').handler({
      commerce: makeCommerce(),
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
  });

  it('mark_return_received transitions status', async () => {
    const result = await findTool(returnTools, 'mark_return_received').handler({
      commerce: makeCommerce(),
      params: { returnId: 'ret_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.return.status, 'received');
  });

  it('complete_return processes the refund', async () => {
    const result = await findTool(returnTools, 'complete_return').handler({
      commerce: makeCommerce(),
      params: { returnId: 'ret_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.return.status, 'completed');
  });

  it('cancel_return transitions status', async () => {
    const result = await findTool(returnTools, 'cancel_return').handler({
      commerce: makeCommerce(),
      params: { returnId: 'ret_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.return.status, 'cancelled');
  });

  it('add_return_tracking forwards id and tracking number', async () => {
    let received;
    const commerce = makeCommerce({
      addTracking: async (id, tracking) => {
        received = { id, tracking };
        return { ...mockReturn, id, status: 'in_transit' };
      },
    });
    const result = await findTool(returnTools, 'add_return_tracking').handler({
      commerce,
      params: { returnId: 'ret_001', trackingNumber: 'TRK123' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.deepEqual(received, { id: 'ret_001', tracking: 'TRK123' });
  });
});
