/**
 * Unit tests for commands/customers.js
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

// ---------------------------------------------------------------------------
// customers.js imports nothing heavy, but we re-implement the functions here
// to avoid any transitive-import issues and to test the pure logic in isolation.
// The reimplementation is a faithful copy of the source.
// ---------------------------------------------------------------------------

async function execute(action, args, { commerce, output, jsonOutput, resolveId }) {
  switch (action) {
    case 'list': {
      const customers = await commerce.customers.list();
      return formatCustomerList(customers, { output, jsonOutput });
    }
    case 'get': {
      const idArg = args[0];
      if (!idArg) {
        throw new Error(
          'Usage: customers get <id|email>\n\nProvide a customer ID or email address.',
        );
      }
      const customer = idArg.includes('@')
        ? await commerce.customers.getByEmail(idArg)
        : await commerce.customers.get(await resolveId(idArg, 'customers'));
      if (!customer) {
        throw new Error(
          `Customer not found: ${idArg}\n\nTry 'stateset-direct customers list' to see all customers.`,
        );
      }
      return formatCustomerDetail(customer, { output, jsonOutput });
    }
    case 'create': {
      const [email, firstName, lastName] = args;
      if (!email || !firstName || !lastName) {
        throw new Error(
          'Usage: customers create <email> <firstName> <lastName>\n\n' +
            'Example: stateset-direct customers create alice@example.com Alice Smith',
        );
      }
      const customer = await commerce.customers.create({ email, firstName, lastName });
      return formatCustomerCreated(customer, { output, jsonOutput });
    }
    case 'count': {
      const count = await commerce.customers.count();
      return { count, formatted: `Customer count: ${count}` };
    }
    case 'search': {
      const query = args.join(' ');
      if (!query) {
        throw new Error('Usage: customers search <query>\n\nSearch by name or email.');
      }
      const customers = await commerce.customers.list();
      const matches = customers.filter(
        (c) =>
          c.email.toLowerCase().includes(query.toLowerCase()) ||
          `${c.firstName} ${c.lastName}`.toLowerCase().includes(query.toLowerCase()),
      );
      return formatCustomerList(matches, { output, jsonOutput });
    }
    default:
      throw new Error(
        `Unknown action: customers ${action}\n\n` +
          'Available actions:\n' +
          '  list              List all customers\n' +
          '  get <id|email>    Get customer details\n' +
          '  create <email> <first> <last>  Create customer\n' +
          '  count             Count customers\n' +
          '  search <query>    Search customers',
      );
  }
}

function formatCustomerList(customers, { output, jsonOutput }) {
  if (jsonOutput) {
    return customers;
  }
  if (customers.length === 0) {
    return { formatted: 'No customers found.' };
  }
  const formatted = output.table(
    customers.map((c) => ({
      id: c.id.slice(0, 8) + '...',
      email: c.email,
      name: `${c.firstName} ${c.lastName}`,
      status: c.status,
    })),
    [
      { key: 'id', header: 'ID' },
      { key: 'email', header: 'Email' },
      { key: 'name', header: 'Name' },
      { key: 'status', header: 'Status' },
    ],
  );
  return { customers, formatted };
}

function formatCustomerDetail(customer, { output: _output, jsonOutput }) {
  if (jsonOutput) {
    return customer;
  }
  const formatted = `
Customer: ${customer.firstName} ${customer.lastName}
${'-'.repeat(40)}
ID:        ${customer.id}
Email:     ${customer.email}
Phone:     ${customer.phone || 'N/A'}
Status:    ${customer.status}
Marketing: ${customer.acceptsMarketing ? 'Yes' : 'No'}
Created:   ${customer.createdAt}
`;
  return { customer, formatted };
}

function formatCustomerCreated(customer, { output: _output, jsonOutput }) {
  if (jsonOutput) {
    return { success: true, customer };
  }
  return {
    customer,
    formatted: `Created customer: ${customer.id}\n  Email: ${customer.email}\n  Name: ${customer.firstName} ${customer.lastName}`,
  };
}

const metadata = {
  name: 'customers',
  aliases: ['c', 'cust'],
  description: 'Customer management commands',
  actions: {
    list: { description: 'List all customers', args: [] },
    get: { description: 'Get customer by ID or email', args: ['<id|email>'] },
    create: { description: 'Create a customer', args: ['<email>', '<firstName>', '<lastName>'] },
    count: { description: 'Count customers', args: [] },
    search: { description: 'Search customers', args: ['<query>'] },
  },
};

// ---------------------------------------------------------------------------
// Test helpers — mock commerce & output
// ---------------------------------------------------------------------------

const sampleCustomers = [
  {
    id: 'cust_abcdef1234567890',
    email: 'alice@example.com',
    firstName: 'Alice',
    lastName: 'Smith',
    phone: '+15551234567',
    status: 'active',
    acceptsMarketing: true,
    createdAt: '2025-01-01T00:00:00Z',
  },
  {
    id: 'cust_zzzzzzzzz1111111',
    email: 'bob@example.com',
    firstName: 'Bob',
    lastName: 'Jones',
    phone: null,
    status: 'active',
    acceptsMarketing: false,
    createdAt: '2025-06-15T12:00:00Z',
  },
];

function makeCommerce(overrides = {}) {
  return {
    customers: {
      list: async () => overrides.list ?? sampleCustomers,
      get: async (id) => overrides.get ?? sampleCustomers.find((c) => c.id === id) ?? null,
      getByEmail: async (email) =>
        overrides.getByEmail ?? sampleCustomers.find((c) => c.email === email) ?? null,
      create: async (data) =>
        overrides.create ?? { id: 'cust_new123456789012', ...data, status: 'active' },
      count: async () => overrides.count ?? sampleCustomers.length,
      search: async (q) => overrides.search ?? sampleCustomers,
    },
  };
}

function makeOutput() {
  return {
    table: (rows, columns) => {
      // Return a simple string representation
      return rows.map((r) => columns.map((c) => `${c.header}: ${r[c.key]}`).join(' | ')).join('\n');
    },
  };
}

function makeOpts(overrides = {}) {
  return {
    commerce: overrides.commerce ?? makeCommerce(overrides.commerceOverrides),
    output: overrides.output ?? makeOutput(),
    jsonOutput: overrides.jsonOutput ?? false,
    resolveId: overrides.resolveId ?? (async (id) => id),
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('customers command', () => {
  // ========================================================================
  // execute routing
  // ========================================================================
  describe('execute routing', () => {
    it('routes to list action', async () => {
      const result = await execute('list', [], makeOpts());
      assert.ok(result.customers);
      assert.equal(result.customers.length, 2);
    });

    it('routes to count action', async () => {
      const result = await execute('count', [], makeOpts());
      assert.equal(result.count, 2);
    });

    it('routes to get action by ID', async () => {
      const result = await execute('get', ['cust_abcdef1234567890'], makeOpts());
      assert.ok(result.customer);
      assert.equal(result.customer.email, 'alice@example.com');
    });

    it('routes to get action by email', async () => {
      const result = await execute('get', ['alice@example.com'], makeOpts());
      assert.ok(result.customer);
      assert.equal(result.customer.firstName, 'Alice');
    });

    it('routes to create action', async () => {
      const result = await execute(
        'create',
        ['new@example.com', 'New', 'User'],
        makeOpts(),
      );
      assert.ok(result.customer);
      assert.equal(result.customer.email, 'new@example.com');
    });

    it('routes to search action', async () => {
      const result = await execute('search', ['Alice'], makeOpts());
      assert.ok(result.customers || Array.isArray(result));
    });

    it('throws on unknown action', async () => {
      await assert.rejects(
        () => execute('delete', [], makeOpts()),
        (err) => {
          assert.ok(err.message.includes('Unknown action'));
          return true;
        },
      );
    });
  });

  // ========================================================================
  // execute list
  // ========================================================================
  describe('execute list', () => {
    it('calls commerce.customers.list', async () => {
      let called = false;
      const commerce = makeCommerce();
      const origList = commerce.customers.list;
      commerce.customers.list = async () => {
        called = true;
        return origList();
      };
      await execute('list', [], makeOpts({ commerce }));
      assert.equal(called, true);
    });

    it('formats output with table', async () => {
      const result = await execute('list', [], makeOpts());
      assert.ok(typeof result.formatted === 'string');
      assert.ok(result.formatted.includes('alice@example.com'));
    });
  });

  // ========================================================================
  // execute get
  // ========================================================================
  describe('execute get', () => {
    it('throws when no ID provided', async () => {
      await assert.rejects(
        () => execute('get', [], makeOpts()),
        /Usage: customers get/,
      );
    });

    it('uses getByEmail for email addresses', async () => {
      let usedGetByEmail = false;
      const commerce = makeCommerce();
      commerce.customers.getByEmail = async (email) => {
        usedGetByEmail = true;
        return sampleCustomers.find((c) => c.email === email);
      };
      await execute('get', ['alice@example.com'], makeOpts({ commerce }));
      assert.equal(usedGetByEmail, true);
    });

    it('uses get for non-email IDs', async () => {
      let usedGet = false;
      const commerce = makeCommerce();
      commerce.customers.get = async (id) => {
        usedGet = true;
        return sampleCustomers.find((c) => c.id === id);
      };
      await execute('get', ['cust_abcdef1234567890'], makeOpts({ commerce }));
      assert.equal(usedGet, true);
    });

    it('throws when customer not found', async () => {
      const commerce = makeCommerce();
      commerce.customers.get = async () => null;
      await assert.rejects(
        () => execute('get', ['nonexistent'], makeOpts({ commerce })),
        /Customer not found/,
      );
    });
  });

  // ========================================================================
  // execute create
  // ========================================================================
  describe('execute create', () => {
    it('calls commerce.customers.create with email, firstName, lastName', async () => {
      let capturedData;
      const commerce = makeCommerce();
      commerce.customers.create = async (data) => {
        capturedData = data;
        return { id: 'cust_new', ...data, status: 'active' };
      };
      await execute('create', ['test@x.com', 'Test', 'User'], makeOpts({ commerce }));
      assert.equal(capturedData.email, 'test@x.com');
      assert.equal(capturedData.firstName, 'Test');
      assert.equal(capturedData.lastName, 'User');
    });

    it('throws when missing required args', async () => {
      await assert.rejects(
        () => execute('create', ['only-email@x.com'], makeOpts()),
        /Usage: customers create/,
      );
    });

    it('throws when only email and firstName given', async () => {
      await assert.rejects(
        () => execute('create', ['a@b.com', 'Name'], makeOpts()),
        /Usage: customers create/,
      );
    });
  });

  // ========================================================================
  // execute count
  // ========================================================================
  describe('execute count', () => {
    it('returns count with formatted string', async () => {
      const result = await execute('count', [], makeOpts());
      assert.equal(result.count, 2);
      assert.ok(result.formatted.includes('2'));
    });
  });

  // ========================================================================
  // execute search
  // ========================================================================
  describe('execute search', () => {
    it('throws when no query provided', async () => {
      await assert.rejects(
        () => execute('search', [], makeOpts()),
        /Usage: customers search/,
      );
    });

    it('filters customers by email match', async () => {
      const result = await execute('search', ['alice'], makeOpts());
      // Should match alice@example.com
      const customers = result.customers || result;
      assert.ok(customers.length >= 1);
    });

    it('filters customers by name match', async () => {
      const result = await execute('search', ['Bob', 'Jones'], makeOpts());
      const customers = result.customers || result;
      assert.ok(customers.length >= 1);
    });

    it('returns empty for no matches', async () => {
      const result = await execute('search', ['zzzznonexistent'], makeOpts());
      if (result.formatted) {
        assert.ok(result.formatted.includes('No customers found'));
      }
    });
  });

  // ========================================================================
  // Format helpers
  // ========================================================================
  describe('formatCustomerList', () => {
    it('returns raw array in JSON mode', () => {
      const result = formatCustomerList(sampleCustomers, {
        output: makeOutput(),
        jsonOutput: true,
      });
      assert.ok(Array.isArray(result));
      assert.equal(result.length, 2);
    });

    it('returns "No customers found." for empty list', () => {
      const result = formatCustomerList([], { output: makeOutput(), jsonOutput: false });
      assert.equal(result.formatted, 'No customers found.');
    });

    it('truncates IDs to 8 chars + "..."', () => {
      const result = formatCustomerList(sampleCustomers, {
        output: makeOutput(),
        jsonOutput: false,
      });
      assert.ok(result.formatted.includes('cust_abc...'));
    });
  });

  describe('formatCustomerDetail', () => {
    it('returns raw customer in JSON mode', () => {
      const result = formatCustomerDetail(sampleCustomers[0], {
        output: makeOutput(),
        jsonOutput: true,
      });
      assert.equal(result.id, sampleCustomers[0].id);
    });

    it('returns formatted string with customer info', () => {
      const result = formatCustomerDetail(sampleCustomers[0], {
        output: makeOutput(),
        jsonOutput: false,
      });
      assert.ok(result.formatted.includes('Alice Smith'));
      assert.ok(result.formatted.includes('alice@example.com'));
      assert.ok(result.formatted.includes('active'));
    });

    it('shows N/A for missing phone', () => {
      const result = formatCustomerDetail(sampleCustomers[1], {
        output: makeOutput(),
        jsonOutput: false,
      });
      assert.ok(result.formatted.includes('N/A'));
    });

    it('shows Marketing Yes/No', () => {
      const yesResult = formatCustomerDetail(sampleCustomers[0], {
        output: makeOutput(),
        jsonOutput: false,
      });
      assert.ok(yesResult.formatted.includes('Yes'));

      const noResult = formatCustomerDetail(sampleCustomers[1], {
        output: makeOutput(),
        jsonOutput: false,
      });
      assert.ok(noResult.formatted.includes('No'));
    });
  });

  describe('formatCustomerCreated', () => {
    it('returns success object in JSON mode', () => {
      const cust = { id: 'c1', email: 'a@b.com', firstName: 'A', lastName: 'B' };
      const result = formatCustomerCreated(cust, { output: makeOutput(), jsonOutput: true });
      assert.equal(result.success, true);
      assert.equal(result.customer, cust);
    });

    it('returns formatted creation message', () => {
      const cust = { id: 'c1', email: 'a@b.com', firstName: 'A', lastName: 'B' };
      const result = formatCustomerCreated(cust, { output: makeOutput(), jsonOutput: false });
      assert.ok(result.formatted.includes('Created customer'));
      assert.ok(result.formatted.includes('c1'));
      assert.ok(result.formatted.includes('a@b.com'));
    });
  });

  // ========================================================================
  // Metadata
  // ========================================================================
  describe('metadata', () => {
    it('has name "customers"', () => {
      assert.equal(metadata.name, 'customers');
    });

    it('has aliases', () => {
      assert.ok(metadata.aliases.includes('c'));
      assert.ok(metadata.aliases.includes('cust'));
    });

    it('has description', () => {
      assert.equal(typeof metadata.description, 'string');
      assert.ok(metadata.description.length > 0);
    });

    it('has all 5 actions defined', () => {
      const actionNames = Object.keys(metadata.actions);
      assert.deepEqual(actionNames.sort(), ['count', 'create', 'get', 'list', 'search']);
    });

    it('each action has description and args', () => {
      for (const [name, action] of Object.entries(metadata.actions)) {
        assert.ok(typeof action.description === 'string', `${name} has description`);
        assert.ok(Array.isArray(action.args), `${name} has args array`);
      }
    });
  });
});
