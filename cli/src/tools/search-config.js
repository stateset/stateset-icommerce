/**
 * Search Configuration Tools Module
 *
 * MCP tool definitions for search tuning profiles (fields, facets, synonyms,
 * boost rules) and activation.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

const searchFieldSchema = z.object({
  fieldName: z.string().min(1).describe('Field name (e.g. title, description, sku)'),
  weight: z.number().describe('Relative search weight'),
  tokenizer: z
    .enum(['standard', 'ngram', 'edge', 'keyword'])
    .optional()
    .describe('Tokenizer strategy'),
  enabled: z.boolean().optional().describe('Whether the field is searchable'),
});

const facetSchema = z.object({
  fieldName: z.string().min(1).describe('Field to facet on'),
  facetType: z.enum(['value', 'range', 'hierarchical']).optional().describe('Facet type'),
  displayName: z.string().min(1).describe('Display name shown in the facet panel'),
  sortOrder: z.number().int().optional().describe('Sort order in the facet panel'),
  maxValues: z.number().int().positive().optional().describe('Maximum facet values to return'),
});

const synonymSchema = z.object({
  canonical: z.string().min(1).describe('Canonical term'),
  synonyms: z.array(z.string().min(1)).describe('Terms that map to the canonical term'),
});

const boostRuleSchema = z.object({
  field: z.string().min(1).describe('Field to boost'),
  valueMatch: z.string().min(1).describe('Value pattern to match'),
  boostFactor: z.number().describe('Boost factor (>1 increases relevance)'),
});

export const searchConfigTools = withPolicyDomain('search-config', [
  {
    name: 'list_search_configs',
    description: 'List search configurations.',
    inputSchema: {
      isActive: z.boolean().optional().describe('Filter by active status'),
      name: z.string().min(1).optional().describe('Filter by name'),
      limit: z.number().int().positive().optional().describe('Maximum results'),
      offset: z.number().int().nonnegative().optional().describe('Pagination offset'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const configs = await commerce.searchConfig.list({
        isActive: params.isActive,
        name: params.name,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: configs.length, searchConfigs: configs };
    },
  },
  {
    name: 'get_search_config',
    description: 'Get a search configuration by ID.',
    inputSchema: {
      id: z.string().min(1).describe('Search configuration ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const config = await commerce.searchConfig.get(params.id);
      if (!config) {
        return { success: false, error: 'Search configuration not found' };
      }
      return { success: true, searchConfig: config };
    },
  },
  {
    name: 'get_active_search_config',
    description: 'Get the currently active search configuration.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const config = await commerce.searchConfig.getActive();
      if (!config) {
        return { success: false, error: 'No active search configuration' };
      }
      return { success: true, searchConfig: config };
    },
  },
  {
    name: 'create_search_config',
    description: 'Create a search configuration.',
    inputSchema: {
      name: z.string().min(1).describe('Configuration name'),
      description: z.string().min(1).optional().describe('Description'),
      searchableFields: z.array(searchFieldSchema).optional().describe('Searchable fields'),
      facets: z.array(facetSchema).optional().describe('Facet definitions'),
      synonyms: z.array(synonymSchema).optional().describe('Synonym groups'),
      boostRules: z.array(boostRuleSchema).optional().describe('Boost rules'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create search configuration', params);
      }

      const config = await commerce.searchConfig.create({
        name: params.name,
        description: params.description,
        searchableFields: params.searchableFields,
        facets: params.facets,
        synonyms: params.synonyms,
        boostRules: params.boostRules,
      });
      return { success: true, message: 'Search configuration created', searchConfig: config };
    },
  },
  {
    name: 'update_search_config',
    description: 'Update a search configuration. Collection fields replace the existing values.',
    inputSchema: {
      id: z.string().min(1).describe('Search configuration ID'),
      name: z.string().min(1).optional().describe('Updated name'),
      description: z.string().min(1).optional().describe('Updated description'),
      searchableFields: z.array(searchFieldSchema).optional().describe('Replacement fields'),
      facets: z.array(facetSchema).optional().describe('Replacement facets'),
      synonyms: z.array(synonymSchema).optional().describe('Replacement synonym groups'),
      boostRules: z.array(boostRuleSchema).optional().describe('Replacement boost rules'),
      isActive: z.boolean().optional().describe('Updated active status'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update search configuration', params);
      }

      const config = await commerce.searchConfig.update(params.id, {
        name: params.name,
        description: params.description,
        searchableFields: params.searchableFields,
        facets: params.facets,
        synonyms: params.synonyms,
        boostRules: params.boostRules,
        isActive: params.isActive,
      });
      return { success: true, message: 'Search configuration updated', searchConfig: config };
    },
  },
  {
    name: 'set_active_search_config',
    description: 'Make a search configuration active, deactivating the current one.',
    inputSchema: {
      id: z.string().min(1).describe('Search configuration ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Set active search configuration', params);
      }

      const config = await commerce.searchConfig.setActive(params.id);
      return { success: true, message: 'Search configuration activated', searchConfig: config };
    },
  },
  {
    name: 'delete_search_config',
    description: 'Delete a search configuration.',
    inputSchema: {
      id: z.string().min(1).describe('Search configuration ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete search configuration', params);
      }

      await commerce.searchConfig.delete(params.id);
      return { success: true, message: 'Search configuration deleted' };
    },
  },
]);

export default searchConfigTools;
