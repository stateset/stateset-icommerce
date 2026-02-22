/**
 * Customer Tools Module
 *
 * MCP tool definitions for customer operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

/**
 * Customer tool definitions
 */
export const customerTools = [
  {
    name: 'list_customers',
    description:
      'List all customers in the database. Returns customer details including email, name, and status.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const customers = await commerce.customers.list();
      const count = await commerce.customers.count();

      return {
        success: true,
        count,
        customers: customers.map((c) => ({
          id: c.id,
          email: c.email,
          name: `${c.firstName} ${c.lastName}`,
          status: c.status,
          acceptsMarketing: c.acceptsMarketing,
          createdAt: c.createdAt,
        })),
      };
    },
  },

  {
    name: 'get_customer',
    description: 'Get a specific customer by ID or email address.',
    inputSchema: {
      identifier: z.string().min(1).describe('Customer ID (UUID) or email address'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { identifier } = params;

      let customer;
      if (identifier.includes('@')) {
        customer = await commerce.customers.getByEmail(identifier);
      } else {
        customer = await commerce.customers.get(identifier);
      }

      if (!customer) {
        return { success: false, error: 'Customer not found' };
      }

      return {
        success: true,
        customer: {
          id: customer.id,
          email: customer.email,
          firstName: customer.firstName,
          lastName: customer.lastName,
          phone: customer.phone,
          status: customer.status,
          acceptsMarketing: customer.acceptsMarketing,
          createdAt: customer.createdAt,
          updatedAt: customer.updatedAt,
        },
      };
    },
  },

  {
    name: 'create_customer',
    description: 'Create a new customer. Requires email, first name, and last name.',
    inputSchema: {
      email: z.string().email().describe('Customer email address'),
      firstName: z.string().min(1).max(100).describe('Customer first name'),
      lastName: z.string().min(1).max(100).describe('Customer last name'),
      phone: z.string().max(30).optional().describe('Customer phone number'),
      acceptsMarketing: z
        .boolean()
        .optional()
        .default(false)
        .describe('Whether customer accepts marketing'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, autoIndexEntity }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Create operation not allowed. The --apply flag must be set to create customers.',
          hint: 'Run with --apply to enable write operations.',
          wouldCreate: params,
        };
      }

      const customer = await commerce.customers.create(params);
      if (autoIndexEntity) autoIndexEntity('customer', customer);

      return {
        success: true,
        message: 'Customer created successfully',
        customer: {
          id: customer.id,
          email: customer.email,
          name: `${customer.firstName} ${customer.lastName}`,
        },
      };
    },
  },
];

export default customerTools;
