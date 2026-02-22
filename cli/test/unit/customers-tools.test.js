/**
 * Customer Tools Test Suite
 *
 * Tests for cli/src/tools/customers.js
 * Covers: list_customers, get_customer, create_customer
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { customerTools } from '../../src/tools/customers.js';

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

const mockCustomer = {
  id: 'cust_001',
  email: 'alice@example.com',
  firstName: 'Alice',
  lastName: 'Smith',
  phone: '+1-555-0100',
  status: 'active',
  acceptsMarketing: true,
  createdAt: '2026-02-21T00:00:00Z',
  updatedAt: '2026-02-21T00:00:00Z',
};

function makeCustomerCommerce(overrides = {}) {
  return {
    customers: {
      list: async () => [mockCustomer],
      count: async () => 1,
      get: async (_id) => mockCustomer,
      getByEmail: async (_email) => mockCustomer,
      create: async (data) => ({ ...mockCustomer, ...data }),
      ...overrides,
    },
  };
}

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Customer Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(customerTools));
  });

  it('has at least 3 tools', () => {
    assert.ok(customerTools.length >= 3, `Expected >= 3, got ${customerTools.length}`);
  });

  it('every tool has name, handler, and permission', () => {
    for (const tool of customerTools) {
      assert.ok(tool.name, 'tool missing name');
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });
});

// ============================================================================
// list_customers
// ============================================================================

describe('list_customers', () => {
  const tool = findTool(customerTools, 'list_customers');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns list with count', async () => {
    const result = await tool.handler({
      commerce: makeCustomerCommerce(),
      params: {},
    });
    assert.equal(result.success, true);
    assert.equal(result.count, 1);
    assert.equal(result.customers.length, 1);
    assert.equal(result.customers[0].id, 'cust_001');
  });

  it('maps all expected fields on each customer', async () => {
    const result = await tool.handler({
      commerce: makeCustomerCommerce(),
      params: {},
    });
    const c = result.customers[0];
    assert.ok('id' in c);
    assert.ok('email' in c);
    assert.ok('name' in c);
    assert.ok('status' in c);
    assert.ok('acceptsMarketing' in c);
    assert.ok('createdAt' in c);
  });

  it('concatenates firstName and lastName into name', async () => {
    const result = await tool.handler({
      commerce: makeCustomerCommerce(),
      params: {},
    });
    assert.equal(result.customers[0].name, 'Alice Smith');
  });

  it('returns error when list throws', async () => {
    const commerce = makeCustomerCommerce({
      list: async () => {
        throw new Error('DB error');
      },
    });
    try {
      await tool.handler({ commerce, params: {} });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('DB error'));
    }
  });
});

// ============================================================================
// get_customer
// ============================================================================

describe('get_customer', () => {
  const tool = findTool(customerTools, 'get_customer');

  it('is a read tool', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns customer for valid ID', async () => {
    const result = await tool.handler({
      commerce: makeCustomerCommerce(),
      params: { identifier: 'cust_001' },
    });
    assert.equal(result.success, true);
    assert.ok(result.customer);
    assert.equal(result.customer.id, 'cust_001');
    assert.equal(result.customer.email, 'alice@example.com');
    assert.equal(result.customer.firstName, 'Alice');
    assert.equal(result.customer.lastName, 'Smith');
  });

  it('looks up by email when identifier contains @', async () => {
    let usedMethod = null;
    const commerce = makeCustomerCommerce({
      get: async () => {
        usedMethod = 'get';
        return mockCustomer;
      },
      getByEmail: async () => {
        usedMethod = 'getByEmail';
        return mockCustomer;
      },
    });
    await tool.handler({
      commerce,
      params: { identifier: 'alice@example.com' },
    });
    assert.equal(usedMethod, 'getByEmail');
  });

  it('looks up by ID when identifier does not contain @', async () => {
    let usedMethod = null;
    const commerce = makeCustomerCommerce({
      get: async () => {
        usedMethod = 'get';
        return mockCustomer;
      },
      getByEmail: async () => {
        usedMethod = 'getByEmail';
        return mockCustomer;
      },
    });
    await tool.handler({
      commerce,
      params: { identifier: 'cust_001' },
    });
    assert.equal(usedMethod, 'get');
  });

  it('maps all expected fields on the returned customer', async () => {
    const result = await tool.handler({
      commerce: makeCustomerCommerce(),
      params: { identifier: 'cust_001' },
    });
    const c = result.customer;
    assert.ok('id' in c);
    assert.ok('email' in c);
    assert.ok('firstName' in c);
    assert.ok('lastName' in c);
    assert.ok('phone' in c);
    assert.ok('status' in c);
    assert.ok('acceptsMarketing' in c);
    assert.ok('createdAt' in c);
    assert.ok('updatedAt' in c);
  });

  it('returns success: false when customer not found', async () => {
    const commerce = makeCustomerCommerce({
      get: async () => null,
      getByEmail: async () => null,
    });
    const result = await tool.handler({
      commerce,
      params: { identifier: 'NONEXISTENT' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns success: false when email lookup not found', async () => {
    const commerce = makeCustomerCommerce({
      getByEmail: async () => null,
    });
    const result = await tool.handler({
      commerce,
      params: { identifier: 'nobody@example.com' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when get throws', async () => {
    const commerce = makeCustomerCommerce({
      get: async () => {
        throw new Error('lookup failed');
      },
    });
    try {
      await tool.handler({ commerce, params: { identifier: 'cust_001' } });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('lookup failed'));
    }
  });
});

// ============================================================================
// create_customer
// ============================================================================

describe('create_customer', () => {
  const tool = findTool(customerTools, 'create_customer');

  it('is a write tool', () => {
    assert.equal(tool.permission, 'write');
  });

  it('returns preview when allowApply is false', async () => {
    const result = await tool.handler({
      commerce: makeCustomerCommerce(),
      params: {
        email: 'bob@example.com',
        firstName: 'Bob',
        lastName: 'Jones',
      },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.hint, 'expected hint field');
    assert.ok(result.wouldCreate, 'expected wouldCreate preview');
    assert.equal(result.wouldCreate.email, 'bob@example.com');
  });

  it('creates customer with allowApply: true', async () => {
    const result = await tool.handler({
      commerce: makeCustomerCommerce(),
      params: {
        email: 'bob@example.com',
        firstName: 'Bob',
        lastName: 'Jones',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.toLowerCase().includes('created'));
    assert.ok(result.customer);
    assert.ok(result.customer.id);
    assert.ok(result.customer.email);
    assert.ok(result.customer.name);
  });

  it('concatenates firstName and lastName in response', async () => {
    const commerce = makeCustomerCommerce({
      create: async (data) => ({
        ...mockCustomer,
        firstName: data.firstName,
        lastName: data.lastName,
      }),
    });
    const result = await tool.handler({
      commerce,
      params: { email: 'test@example.com', firstName: 'Jane', lastName: 'Doe' },
      allowApply: true,
    });
    assert.equal(result.customer.name, 'Jane Doe');
  });

  it('calls autoIndexEntity when provided', async () => {
    let indexed = null;
    const result = await tool.handler({
      commerce: makeCustomerCommerce(),
      params: {
        email: 'index@example.com',
        firstName: 'Index',
        lastName: 'Test',
      },
      allowApply: true,
      autoIndexEntity: (type, entity) => {
        indexed = { type, entity };
      },
    });
    assert.equal(result.success, true);
    assert.equal(indexed.type, 'customer');
    assert.ok(indexed.entity);
  });

  it('returns error when commerce.customers.create throws', async () => {
    const commerce = makeCustomerCommerce({
      create: async () => {
        throw new Error('Duplicate email');
      },
    });
    try {
      await tool.handler({
        commerce,
        params: { email: 'dup@example.com', firstName: 'Dup', lastName: 'User' },
        allowApply: true,
      });
      assert.fail('expected throw');
    } catch (err) {
      assert.ok(err.message.includes('Duplicate email'));
    }
  });
});
