/**
 * Integration Mapping Tools Module
 *
 * MCP tool definitions for external↔internal value translation mappings.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const integrationMappingTools = withPolicyDomain('integration-mappings', [
  {
    name: 'list_integration_mappings',
    description: 'List integration value mappings.',
    inputSchema: {
      integration: z.string().min(1).optional().describe('Integration name'),
      mappingGroup: z.string().min(1).optional().describe('Mapping group'),
      fieldName: z.string().min(1).optional().describe('Field name'),
      isActive: z.boolean().optional().describe('Filter by active flag'),
      limit: z.number().int().positive().optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Results to skip'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const mappings = await commerce.integrationMappings.list({
        integration: params.integration,
        mappingGroup: params.mappingGroup,
        fieldName: params.fieldName,
        isActive: params.isActive,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: mappings.length, mappings };
    },
  },
  {
    name: 'get_integration_mapping',
    description: 'Get an integration mapping by ID.',
    inputSchema: {
      id: z.string().min(1).describe('Integration mapping ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const mapping = await commerce.integrationMappings.get(params.id);
      if (!mapping) {
        return { success: false, error: 'Integration mapping not found' };
      }
      return { success: true, mapping };
    },
  },
  {
    name: 'resolve_integration_mapping',
    description: 'Resolve the internal value for an external value.',
    inputSchema: {
      integration: z.string().min(1).describe('Integration name'),
      mappingGroup: z.string().min(1).describe('Mapping group'),
      fieldName: z.string().min(1).describe('Field name'),
      externalValue: z.string().min(1).describe('External value to translate'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const internalValue = await commerce.integrationMappings.resolve({
        integration: params.integration,
        mappingGroup: params.mappingGroup,
        fieldName: params.fieldName,
        externalValue: params.externalValue,
      });
      return { success: true, internalValue: internalValue ?? null };
    },
  },
  {
    name: 'create_integration_mapping',
    description: 'Create an integration value mapping.',
    inputSchema: {
      integration: z.string().min(1).describe('Integration name'),
      mappingGroup: z.string().min(1).describe('Mapping group'),
      fieldName: z.string().min(1).describe('Field name'),
      externalValue: z.string().min(1).describe('External value'),
      internalValue: z.string().min(1).describe('Internal value'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create integration mapping', params);
      }
      const mapping = await commerce.integrationMappings.create({
        integration: params.integration,
        mappingGroup: params.mappingGroup,
        fieldName: params.fieldName,
        externalValue: params.externalValue,
        internalValue: params.internalValue,
      });
      return { success: true, message: 'Integration mapping created', mapping };
    },
  },
  {
    name: 'update_integration_mapping',
    description: 'Update an integration mapping.',
    inputSchema: {
      id: z.string().min(1).describe('Integration mapping ID'),
      internalValue: z.string().min(1).optional().describe('New internal value'),
      isActive: z.boolean().optional().describe('New active flag'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update integration mapping', params);
      }
      const mapping = await commerce.integrationMappings.update(params.id, {
        internalValue: params.internalValue,
        isActive: params.isActive,
      });
      return { success: true, message: 'Integration mapping updated', mapping };
    },
  },
  {
    name: 'bulk_upsert_integration_mappings',
    description: 'Bulk upsert integration mappings.',
    inputSchema: {
      items: z
        .array(
          z.object({
            integration: z.string().min(1).describe('Integration name'),
            mappingGroup: z.string().min(1).describe('Mapping group'),
            fieldName: z.string().min(1).describe('Field name'),
            externalValue: z.string().min(1).describe('External value'),
            internalValue: z.string().min(1).describe('Internal value'),
          }),
        )
        .min(1)
        .describe('Mappings to upsert'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Bulk upsert integration mappings', params);
      }
      const affected = await commerce.integrationMappings.bulkUpsert(params.items);
      return { success: true, message: 'Integration mappings upserted', affected };
    },
  },
  {
    name: 'delete_integration_mapping',
    description: 'Delete an integration mapping.',
    inputSchema: {
      id: z.string().min(1).describe('Integration mapping ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete integration mapping', params);
      }
      await commerce.integrationMappings.delete(params.id);
      return { success: true, message: 'Integration mapping deleted' };
    },
  },
]);

export default integrationMappingTools;
