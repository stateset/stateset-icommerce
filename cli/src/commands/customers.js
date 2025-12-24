/**
 * Customer Commands Module
 *
 * Handles all customer-related CLI operations for stateset-direct
 */

/**
 * Execute customer commands
 * @param {string} action - The action to perform
 * @param {Array} args - Command arguments
 * @param {Object} options - Command options
 * @returns {Promise<any>} Command result
 */
export async function execute(action, args, { commerce, output, jsonOutput, resolveId }) {
  switch (action) {
    case 'list': {
      const customers = await commerce.customers.list();
      return formatCustomerList(customers, { output, jsonOutput });
    }

    case 'get': {
      const idArg = args[0];
      if (!idArg) {
        throw new Error('Usage: customers get <id|email>\n\nProvide a customer ID or email address.');
      }

      const customer = idArg.includes('@')
        ? await commerce.customers.getByEmail(idArg)
        : await commerce.customers.get(await resolveId(idArg, 'customers'));

      if (!customer) {
        throw new Error(`Customer not found: ${idArg}\n\nTry 'stateset-direct customers list' to see all customers.`);
      }

      return formatCustomerDetail(customer, { output, jsonOutput });
    }

    case 'create': {
      const [email, firstName, lastName] = args;
      if (!email || !firstName || !lastName) {
        throw new Error(
          'Usage: customers create <email> <firstName> <lastName>\n\n' +
          'Example: stateset-direct customers create alice@example.com Alice Smith'
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
      const matches = customers.filter(c =>
        c.email.toLowerCase().includes(query.toLowerCase()) ||
        `${c.firstName} ${c.lastName}`.toLowerCase().includes(query.toLowerCase())
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
        '  search <query>    Search customers'
      );
  }
}

/**
 * Format customer list for output
 */
function formatCustomerList(customers, { output, jsonOutput }) {
  if (jsonOutput) {
    return customers;
  }

  if (customers.length === 0) {
    return { formatted: 'No customers found.' };
  }

  const formatted = output.table(
    customers.map(c => ({
      id: c.id.slice(0, 8) + '...',
      email: c.email,
      name: `${c.firstName} ${c.lastName}`,
      status: c.status
    })),
    [
      { key: 'id', header: 'ID' },
      { key: 'email', header: 'Email' },
      { key: 'name', header: 'Name' },
      { key: 'status', header: 'Status' }
    ]
  );

  return { customers, formatted };
}

/**
 * Format single customer detail
 */
function formatCustomerDetail(customer, { output, jsonOutput }) {
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

/**
 * Format customer created response
 */
function formatCustomerCreated(customer, { output, jsonOutput }) {
  if (jsonOutput) {
    return { success: true, customer };
  }

  return {
    customer,
    formatted: `Created customer: ${customer.id}\n  Email: ${customer.email}\n  Name: ${customer.firstName} ${customer.lastName}`
  };
}

/**
 * Command metadata for help/completion
 */
export const metadata = {
  name: 'customers',
  aliases: ['c', 'cust'],
  description: 'Customer management commands',
  actions: {
    list: { description: 'List all customers', args: [] },
    get: { description: 'Get customer by ID or email', args: ['<id|email>'] },
    create: { description: 'Create a customer', args: ['<email>', '<firstName>', '<lastName>'] },
    count: { description: 'Count customers', args: [] },
    search: { description: 'Search customers', args: ['<query>'] }
  }
};

export default { execute, metadata };
