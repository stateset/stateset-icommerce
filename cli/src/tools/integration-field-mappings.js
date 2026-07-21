/**
 * Integration Field Mapping Tools Module
 *
 * MCP tool definitions for integration field-path mappings.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

const transformSchema = z
  .enum(['none', 'uppercase', 'lowercase', 'trim'])
  .describe('Value transform');

export const integrationFieldMappingTools = withPolicyDomain('integration-field-mappings', [
  {
    name: 'list_integration_field_mappings',
    description: 'List integration field mappings.',
    inputSchema: {
      integrationAccount: z.string().min(1).optional().describe('Integration account'),
      mappingGroup: z.string().min(1).optional().describe('Mapping group'),
      sourceField: z.string().min(1).optional().describe('Source field path'),
      isActive: z.boolean().optional().describe('Filter by active flag'),
      limit: z.number().int().positive().optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Results to skip'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const mappings = await commerce.integrationFieldMappings.list({
        integrationAccount: params.integrationAccount,
        mappingGroup: params.mappingGroup,
        sourceField: params.sourceField,
        isActive: params.isActive,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: mappings.length, mappings };
    },
  },
  {
    name: 'get_integration_field_mapping',
    description: 'Get an integration field mapping by ID.',
    inputSchema: {
      id: z.string().min(1).describe('Integration field mapping ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const mapping = await commerce.integrationFieldMappings.get(params.id);
      if (!mapping) {
        return { success: false, error: 'Integration field mapping not found' };
      }
      return { success: true, mapping };
    },
  },
  {
    name: 'list_integration_mapping_groups',
    description: 'List the distinct mapping groups for an integration account.',
    inputSchema: {
      integrationAccount: z.string().min(1).describe('Integration account'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const groups = await commerce.integrationFieldMappings.distinctGroups(
        params.integrationAccount,
      );
      return { success: true, count: groups.length, groups };
    },
  },
  {
    name: 'create_integration_field_mapping',
    description: 'Create an integration field mapping.',
    inputSchema: {
      integrationAccount: z.string().min(1).describe('Integration account'),
      mappingGroup: z.string().min(1).describe('Mapping group'),
      sourceField: z.string().min(1).describe('Source field path'),
      destinationField: z.string().min(1).describe('Destination field path'),
      template: z.string().min(1).optional().describe('Optional value template'),
      transform: transformSchema.optional(),
      fallback: z.string().min(1).optional().describe('Fallback value'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create integration field mapping', params);
      }
      const mapping = await commerce.integrationFieldMappings.create({
        integrationAccount: params.integrationAccount,
        mappingGroup: params.mappingGroup,
        sourceField: params.sourceField,
        destinationField: params.destinationField,
        template: params.template,
        transform: params.transform,
        fallback: params.fallback,
      });
      return { success: true, message: 'Integration field mapping created', mapping };
    },
  },
  {
    name: 'update_integration_field_mapping',
    description: 'Update an integration field mapping.',
    inputSchema: {
      id: z.string().min(1).describe('Integration field mapping ID'),
      destinationField: z.string().min(1).optional().describe('New destination field path'),
      template: z.string().min(1).optional().describe('New value template'),
      transform: transformSchema.optional(),
      fallback: z.string().min(1).optional().describe('New fallback value'),
      isActive: z.boolean().optional().describe('New active flag'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update integration field mapping', params);
      }
      const mapping = await commerce.integrationFieldMappings.update(params.id, {
        destinationField: params.destinationField,
        template: params.template,
        transform: params.transform,
        fallback: params.fallback,
        isActive: params.isActive,
      });
      return { success: true, message: 'Integration field mapping updated', mapping };
    },
  },
  {
    name: 'bulk_create_integration_field_mappings',
    description: 'Bulk create integration field mappings.',
    inputSchema: {
      items: z
        .array(
          z.object({
            integrationAccount: z.string().min(1).describe('Integration account'),
            mappingGroup: z.string().min(1).describe('Mapping group'),
            sourceField: z.string().min(1).describe('Source field path'),
            destinationField: z.string().min(1).describe('Destination field path'),
            template: z.string().min(1).optional().describe('Optional value template'),
            transform: transformSchema.optional(),
            fallback: z.string().min(1).optional().describe('Fallback value'),
          }),
        )
        .min(1)
        .describe('Field mappings to create'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Bulk create integration field mappings', params);
      }
      const affected = await commerce.integrationFieldMappings.bulkCreate(params.items);
      return { success: true, message: 'Integration field mappings created', affected };
    },
  },
  {
    name: 'bulk_delete_integration_field_mappings',
    description: 'Bulk delete integration field mappings by ID.',
    inputSchema: {
      ids: z.array(z.string().min(1)).min(1).describe('Field mapping IDs'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Bulk delete integration field mappings', params);
      }
      const affected = await commerce.integrationFieldMappings.bulkDelete(params.ids);
      return { success: true, message: 'Integration field mappings deleted', affected };
    },
  },
  {
    name: 'delete_integration_field_mapping',
    description: 'Delete an integration field mapping.',
    inputSchema: {
      id: z.string().min(1).describe('Integration field mapping ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete integration field mapping', params);
      }
      await commerce.integrationFieldMappings.delete(params.id);
      return { success: true, message: 'Integration field mapping deleted' };
    },
  },
]);

export default integrationFieldMappingTools;
