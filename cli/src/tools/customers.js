/**
 * Customer Tools Module
 *
 * MCP tool definitions for customer operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

const addressInput = {
  addressType: z
    .enum(['shipping', 'billing', 'both'])
    .optional()
    .describe('Address type: shipping, billing, or both'),
  firstName: z.string().min(1).max(100).describe('Recipient first name'),
  lastName: z.string().min(1).max(100).describe('Recipient last name'),
  company: z.string().max(255).optional().describe('Company name'),
  line1: z.string().min(1).max(255).describe('Address line 1'),
  line2: z.string().max(255).optional().describe('Address line 2'),
  city: z.string().min(1).max(120).describe('City'),
  state: z.string().max(120).optional().describe('State/province'),
  postalCode: z.string().min(1).max(30).describe('Postal/ZIP code'),
  country: z.string().min(1).max(80).describe('Country'),
  phone: z.string().max(30).optional().describe('Contact phone number'),
  isDefault: z.boolean().optional().describe('Mark as default address'),
};

function customerSummary(customer) {
  return {
    id: customer.id,
    email: customer.email,
    firstName: customer.firstName,
    lastName: customer.lastName,
    phone: customer.phone,
    status: customer.status,
    acceptsMarketing: customer.acceptsMarketing,
    createdAt: customer.createdAt,
    updatedAt: customer.updatedAt,
  };
}

/**
 * Customer tool definitions
 */
export const customerTools = withPolicyDomain('customers', [
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

      return { success: true, customer: customerSummary(customer) };
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

  {
    name: 'update_customer',
    description:
      'Update an existing customer. Only the fields provided are changed (email, name, phone, status, marketing opt-in).',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID (UUID)'),
      email: z.string().email().optional().describe('New email address'),
      firstName: z.string().min(1).max(100).optional().describe('New first name'),
      lastName: z.string().min(1).max(100).optional().describe('New last name'),
      phone: z.string().max(30).optional().describe('New phone number'),
      status: z
        .enum(['active', 'inactive', 'suspended', 'deleted'])
        .optional()
        .describe('New customer status'),
      acceptsMarketing: z.boolean().optional().describe('Whether customer accepts marketing'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, autoIndexEntity }) => {
      if (!allowApply) {
        return applyRequired('Update customer', params);
      }

      const { customerId, ...update } = params;
      const customer = await commerce.customers.update(customerId, update);
      if (autoIndexEntity) autoIndexEntity('customer', customer);

      return {
        success: true,
        message: 'Customer updated successfully',
        customer: customerSummary(customer),
      };
    },
  },

  {
    name: 'delete_customer',
    description: 'Delete a customer (soft delete).',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete customer', params);
      }

      await commerce.customers.delete(params.customerId);
      return {
        success: true,
        message: 'Customer deleted successfully',
        customerId: params.customerId,
      };
    },
  },

  {
    name: 'find_or_create_customer',
    description:
      'Find a customer by email, or create one if none exists. Returns the existing or newly created customer.',
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
        return applyRequired('Find or create customer', params);
      }

      const customer = await commerce.customers.findOrCreate(params);
      if (autoIndexEntity) autoIndexEntity('customer', customer);

      return {
        success: true,
        message: 'Customer found or created',
        customer: customerSummary(customer),
      };
    },
  },

  {
    name: 'list_customer_addresses',
    description: 'List all addresses in a customer address book.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID (UUID)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const addresses = await commerce.customers.getAddresses(params.customerId);
      return { success: true, count: addresses.length, addresses };
    },
  },

  {
    name: 'add_customer_address',
    description: 'Add an address to a customer address book.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID (UUID)'),
      ...addressInput,
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Add customer address', params);
      }

      const address = await commerce.customers.addAddress(params);
      return { success: true, message: 'Address added', address };
    },
  },

  {
    name: 'update_customer_address',
    description: 'Update an existing customer address.',
    inputSchema: {
      addressId: z.string().min(1).describe('Address ID (UUID)'),
      customerId: z.string().min(1).describe('Owning customer ID (UUID)'),
      ...addressInput,
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update customer address', params);
      }

      const { addressId, ...input } = params;
      const address = await commerce.customers.updateAddress(addressId, input);
      return { success: true, message: 'Address updated', address };
    },
  },

  {
    name: 'delete_customer_address',
    description: 'Delete a customer address.',
    inputSchema: {
      addressId: z.string().min(1).describe('Address ID (UUID)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete customer address', params);
      }

      await commerce.customers.deleteAddress(params.addressId);
      return { success: true, message: 'Address deleted', addressId: params.addressId };
    },
  },

  {
    name: 'set_default_customer_address',
    description: 'Set a customer address as the default for shipping, billing, or both.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID (UUID)'),
      addressId: z.string().min(1).describe('Address ID (UUID)'),
      addressType: z
        .enum(['shipping', 'billing', 'both'])
        .default('both')
        .describe('Which default to set: shipping, billing, or both'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Set default customer address', params);
      }

      await commerce.customers.setDefaultAddress(
        params.customerId,
        params.addressId,
        params.addressType,
      );
      return {
        success: true,
        message: 'Default address set',
        customerId: params.customerId,
        addressId: params.addressId,
        addressType: params.addressType,
      };
    },
  },
]);

export default customerTools;
