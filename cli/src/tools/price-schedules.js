/**
 * Price Schedule Tools Module
 *
 * MCP tool definitions for time-windowed scheduled pricing.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const priceScheduleTools = withPolicyDomain('price_schedules', [
  {
    name: 'check_price_schedules_supported',
    description: 'Check whether the price-schedules backend is available on this engine build.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const supported = await commerce.priceSchedules.isSupported();
      return { success: true, supported };
    },
  },
  {
    name: 'list_price_schedules',
    description: 'List price schedules with optional filtering.',
    inputSchema: {
      isActive: z.boolean().optional().describe('Filter by active flag'),
      limit: z.number().int().min(1).optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const priceSchedules = await commerce.priceSchedules.list({
        isActive: params.isActive,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: priceSchedules.length, priceSchedules };
    },
  },
  {
    name: 'get_price_schedule',
    description: 'Get a price schedule by ID.',
    inputSchema: {
      priceScheduleId: z.string().min(1).describe('Price schedule ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const priceSchedule = await commerce.priceSchedules.get(params.priceScheduleId);
      if (!priceSchedule) {
        return { success: false, error: 'Price schedule not found' };
      }
      return { success: true, priceSchedule };
    },
  },
  {
    name: 'create_price_schedule',
    description: 'Create a price schedule.',
    inputSchema: {
      name: z.string().min(1).describe('Schedule name'),
      code: z.string().min(1).optional().describe('Optional schedule code'),
      currency: z.string().min(1).optional().describe('Currency code, e.g. "USD"'),
      startsAt: z.string().optional().describe('Start timestamp in ISO 8601'),
      endsAt: z.string().optional().describe('End timestamp in ISO 8601'),
      priority: z
        .number()
        .int()
        .optional()
        .describe('Priority used to break ties (higher wins); default 0'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create price schedule', params);
      }

      const priceSchedule = await commerce.priceSchedules.create({
        name: params.name,
        code: params.code,
        currency: params.currency,
        startsAt: params.startsAt,
        endsAt: params.endsAt,
        priority: params.priority,
      });
      return { success: true, message: 'Price schedule created', priceSchedule };
    },
  },
  {
    name: 'update_price_schedule',
    description: 'Update a price schedule.',
    inputSchema: {
      priceScheduleId: z.string().min(1).describe('Price schedule ID'),
      name: z.string().min(1).optional().describe('New name'),
      code: z.string().min(1).optional().describe('New code'),
      startsAt: z.string().optional().describe('New start timestamp in ISO 8601'),
      endsAt: z.string().optional().describe('New end timestamp in ISO 8601'),
      isActive: z.boolean().optional().describe('Active flag'),
      priority: z.number().int().optional().describe('New priority'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update price schedule', params);
      }

      const priceSchedule = await commerce.priceSchedules.update(params.priceScheduleId, {
        name: params.name,
        code: params.code,
        startsAt: params.startsAt,
        endsAt: params.endsAt,
        isActive: params.isActive,
        priority: params.priority,
      });
      return { success: true, message: 'Price schedule updated', priceSchedule };
    },
  },
  {
    name: 'delete_price_schedule',
    description: 'Delete a price schedule and its entries.',
    inputSchema: {
      priceScheduleId: z.string().min(1).describe('Price schedule ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete price schedule', params);
      }

      await commerce.priceSchedules.delete(params.priceScheduleId);
      return { success: true, message: 'Price schedule deleted' };
    },
  },
  {
    name: 'set_price_schedule_entry',
    description: 'Upsert a per-product scheduled price on a price schedule.',
    inputSchema: {
      priceScheduleId: z.string().min(1).describe('Price schedule ID'),
      productId: z.string().min(1).describe('Product ID'),
      price: z.string().min(1).describe('Price as an exact decimal string, e.g. "19.99"'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Set price schedule entry', params);
      }

      const entry = await commerce.priceSchedules.setEntry(
        params.priceScheduleId,
        params.productId,
        params.price,
      );
      return { success: true, message: 'Price schedule entry set', entry };
    },
  },
  {
    name: 'delete_price_schedule_entry',
    description: 'Remove a per-product entry from a price schedule.',
    inputSchema: {
      priceScheduleId: z.string().min(1).describe('Price schedule ID'),
      productId: z.string().min(1).describe('Product ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete price schedule entry', params);
      }

      await commerce.priceSchedules.deleteEntry(params.priceScheduleId, params.productId);
      return { success: true, message: 'Price schedule entry deleted' };
    },
  },
  {
    name: 'list_price_schedule_entries',
    description: 'List per-product entries for a price schedule.',
    inputSchema: {
      priceScheduleId: z.string().min(1).describe('Price schedule ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const entries = await commerce.priceSchedules.listEntries(params.priceScheduleId);
      return { success: true, count: entries.length, entries };
    },
  },
  {
    name: 'resolve_scheduled_price',
    description:
      'Resolve the effective scheduled price for a product at an instant (defaults to now).',
    inputSchema: {
      productId: z.string().min(1).describe('Product ID'),
      at: z.string().optional().describe('Instant in ISO 8601 (defaults to now)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const price = await commerce.priceSchedules.resolvePrice(params.productId, params.at);
      return { success: true, price, applies: price !== null };
    },
  },
]);

export default priceScheduleTools;
