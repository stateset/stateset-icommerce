/**
 * Price Level Tools Module
 *
 * MCP tool definitions for customer / segment price levels.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const priceLevelTools = withPolicyDomain('price_levels', [
  {
    name: 'check_price_levels_supported',
    description: 'Check whether the price-levels backend is available on this engine build.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const supported = await commerce.priceLevels.isSupported();
      return { success: true, supported };
    },
  },
  {
    name: 'list_price_levels',
    description: 'List price levels with optional filtering.',
    inputSchema: {
      isActive: z.boolean().optional().describe('Filter by active flag'),
      limit: z.number().int().min(1).optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const priceLevels = await commerce.priceLevels.list({
        isActive: params.isActive,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: priceLevels.length, priceLevels };
    },
  },
  {
    name: 'get_price_level',
    description: 'Get a price level by ID.',
    inputSchema: {
      priceLevelId: z.string().min(1).describe('Price level ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const priceLevel = await commerce.priceLevels.get(params.priceLevelId);
      if (!priceLevel) {
        return { success: false, error: 'Price level not found' };
      }
      return { success: true, priceLevel };
    },
  },
  {
    name: 'create_price_level',
    description: 'Create a price level.',
    inputSchema: {
      name: z.string().min(1).describe('Price level name'),
      code: z.string().min(1).describe('Price level code'),
      description: z.string().max(2000).optional().describe('Optional description'),
      adjustmentType: z
        .enum(['none', 'percentage_discount', 'percentage_markup'])
        .optional()
        .describe('Adjustment type (default none)'),
      adjustmentValue: z
        .string()
        .min(1)
        .optional()
        .describe('Percentage as an exact decimal string, e.g. "10" for 10%; default "0"'),
      currency: z.string().min(1).optional().describe('Currency code, e.g. "USD"'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create price level', params);
      }

      const priceLevel = await commerce.priceLevels.create({
        name: params.name,
        code: params.code,
        description: params.description,
        adjustmentType: params.adjustmentType,
        adjustmentValue: params.adjustmentValue,
        currency: params.currency,
      });
      return { success: true, message: 'Price level created', priceLevel };
    },
  },
  {
    name: 'update_price_level',
    description: 'Update a price level.',
    inputSchema: {
      priceLevelId: z.string().min(1).describe('Price level ID'),
      name: z.string().min(1).optional().describe('New name'),
      description: z.string().max(2000).optional().describe('New description'),
      adjustmentType: z
        .enum(['none', 'percentage_discount', 'percentage_markup'])
        .optional()
        .describe('New adjustment type'),
      adjustmentValue: z
        .string()
        .min(1)
        .optional()
        .describe('New percentage as an exact decimal string'),
      isActive: z.boolean().optional().describe('Active flag'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update price level', params);
      }

      const priceLevel = await commerce.priceLevels.update(params.priceLevelId, {
        name: params.name,
        description: params.description,
        adjustmentType: params.adjustmentType,
        adjustmentValue: params.adjustmentValue,
        isActive: params.isActive,
      });
      return { success: true, message: 'Price level updated', priceLevel };
    },
  },
  {
    name: 'delete_price_level',
    description: 'Delete a price level and its entries.',
    inputSchema: {
      priceLevelId: z.string().min(1).describe('Price level ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete price level', params);
      }

      await commerce.priceLevels.delete(params.priceLevelId);
      return { success: true, message: 'Price level deleted' };
    },
  },
  {
    name: 'set_price_level_entry',
    description: 'Upsert a per-product fixed price entry on a price level.',
    inputSchema: {
      priceLevelId: z.string().min(1).describe('Price level ID'),
      productId: z.string().min(1).describe('Product ID'),
      price: z.string().min(1).describe('Price as an exact decimal string, e.g. "19.99"'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Set price level entry', params);
      }

      const entry = await commerce.priceLevels.setEntry(
        params.priceLevelId,
        params.productId,
        params.price,
      );
      return { success: true, message: 'Price level entry set', entry };
    },
  },
  {
    name: 'delete_price_level_entry',
    description: 'Remove a per-product entry from a price level.',
    inputSchema: {
      priceLevelId: z.string().min(1).describe('Price level ID'),
      productId: z.string().min(1).describe('Product ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete price level entry', params);
      }

      await commerce.priceLevels.deleteEntry(params.priceLevelId, params.productId);
      return { success: true, message: 'Price level entry deleted' };
    },
  },
  {
    name: 'list_price_level_entries',
    description: 'List per-product entries for a price level.',
    inputSchema: {
      priceLevelId: z.string().min(1).describe('Price level ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const entries = await commerce.priceLevels.listEntries(params.priceLevelId);
      return { success: true, count: entries.length, entries };
    },
  },
]);

export default priceLevelTools;
