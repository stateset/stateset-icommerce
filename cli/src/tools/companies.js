/**
 * Company Tools Module
 *
 * MCP tool definitions for the B2B company and contact registry.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const companyTools = withPolicyDomain('companies', [
  {
    name: 'check_companies_supported',
    description: 'Check whether the companies backend is available on this engine build.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const supported = await commerce.companies.isSupported();
      return { success: true, supported };
    },
  },
  {
    name: 'list_companies',
    description: 'List B2B companies with optional filtering.',
    inputSchema: {
      status: z.string().min(1).optional().describe('Filter by status'),
      search: z.string().min(1).optional().describe('Search by name or reference'),
      limit: z.number().int().min(1).optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const companies = await commerce.companies.list({
        status: params.status,
        search: params.search,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: companies.length, companies };
    },
  },
  {
    name: 'get_company',
    description: 'Get a company by ID.',
    inputSchema: {
      companyId: z.string().min(1).describe('Company ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const company = await commerce.companies.get(params.companyId);
      if (!company) {
        return { success: false, error: 'Company not found' };
      }
      return { success: true, company };
    },
  },
  {
    name: 'create_company',
    description: 'Create a B2B company.',
    inputSchema: {
      name: z.string().min(1).describe('Company name'),
      reference: z.string().min(1).optional().describe('Optional external reference'),
      email: z.string().min(1).optional().describe('Optional contact email'),
      phone: z.string().min(1).optional().describe('Optional contact phone'),
      currency: z.string().length(3).optional().describe('Optional ISO 4217 currency code'),
      paymentTermsDays: z
        .number()
        .int()
        .min(0)
        .optional()
        .describe('Optional payment terms in days'),
      tags: z.array(z.string().min(1)).optional().describe('Optional tags'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create company', params);
      }

      const company = await commerce.companies.create({
        name: params.name,
        reference: params.reference,
        email: params.email,
        phone: params.phone,
        currency: params.currency,
        paymentTermsDays: params.paymentTermsDays,
        tags: params.tags,
      });
      return { success: true, message: 'Company created', company };
    },
  },
  {
    name: 'update_company',
    description: 'Update a B2B company.',
    inputSchema: {
      companyId: z.string().min(1).describe('Company ID'),
      name: z.string().min(1).optional().describe('New name'),
      reference: z.string().min(1).optional().describe('New external reference'),
      email: z.string().min(1).optional().describe('New contact email'),
      phone: z.string().min(1).optional().describe('New contact phone'),
      currency: z.string().length(3).optional().describe('New ISO 4217 currency code'),
      paymentTermsDays: z.number().int().min(0).optional().describe('New payment terms in days'),
      status: z.string().min(1).optional().describe('New status'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update company', params);
      }

      const company = await commerce.companies.update(params.companyId, {
        name: params.name,
        reference: params.reference,
        email: params.email,
        phone: params.phone,
        currency: params.currency,
        paymentTermsDays: params.paymentTermsDays,
        status: params.status,
      });
      return { success: true, message: 'Company updated', company };
    },
  },
  {
    name: 'list_company_addresses',
    description: 'List shipping addresses for a company.',
    inputSchema: {
      companyId: z.string().min(1).describe('Company ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const addresses = await commerce.companies.listAddresses(params.companyId);
      return { success: true, count: addresses.length, addresses };
    },
  },
  {
    name: 'list_company_contacts',
    description: 'List contacts for a company.',
    inputSchema: {
      companyId: z.string().min(1).describe('Company ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const contacts = await commerce.companies.listContacts(params.companyId);
      return { success: true, count: contacts.length, contacts };
    },
  },
  {
    name: 'create_company_contact',
    description: 'Create a contact linked to one or more companies.',
    inputSchema: {
      firstName: z.string().min(1).describe('Contact first name'),
      lastName: z.string().min(1).optional().describe('Optional last name'),
      email: z.string().min(1).optional().describe('Optional email'),
      phone: z.string().min(1).optional().describe('Optional phone'),
      title: z.string().min(1).optional().describe('Optional job title'),
      companyIds: z.array(z.string().min(1)).optional().describe('Company IDs to link'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create company contact', params);
      }

      const contact = await commerce.companies.createContact({
        firstName: params.firstName,
        lastName: params.lastName,
        email: params.email,
        phone: params.phone,
        title: params.title,
        companyIds: params.companyIds,
      });
      return { success: true, message: 'Contact created', contact };
    },
  },
  {
    name: 'delete_company',
    description: 'Delete a B2B company.',
    inputSchema: {
      companyId: z.string().min(1).describe('Company ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete company', params);
      }

      await commerce.companies.delete(params.companyId);
      return { success: true, message: 'Company deleted' };
    },
  },
]);

export default companyTools;
