/**
 * Cycle Count Tools Module
 *
 * MCP tool definitions for inventory cycle counting.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

/** A non-negative quantity: an exact decimal string, or an integer. */
const quantityInput = z.union([
  z.string().regex(/^\d+(?:\.\d+)?$/, 'must be a non-negative exact decimal string'),
  z.number().int().min(0),
]);

/**
 * Drop undefined/null keys: the binding's optional fields accept an absent
 * key but refuse `null`.
 * @template {Record<string, unknown>} T
 * @param {T} input
 * @returns {T}
 */
function omitAbsent(input) {
  return /** @type {T} */ (
    Object.fromEntries(
      Object.entries(input).filter(([, value]) => value !== undefined && value !== null),
    )
  );
}

/**
 * Normalise an optional ISO-8601 date/datetime to RFC 3339; absent stays absent.
 * @param {string | undefined} value
 * @returns {string | undefined}
 */
function toRfc3339(value) {
  if (value === undefined || value === null) return undefined;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    throw new Error(`scheduledDate must be an ISO 8601 date or datetime, got "${value}"`);
  }
  return date.toISOString();
}

export const cycleCountTools = withPolicyDomain('cycle_counts', [
  {
    name: 'list_cycle_counts',
    description: 'List cycle counts.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const cycleCounts = await commerce.cycleCounts.list();
      return { success: true, count: cycleCounts.length, cycleCounts };
    },
  },
  {
    name: 'get_cycle_count',
    description: 'Get a cycle count by ID.',
    inputSchema: {
      cycleCountId: z.string().min(1).describe('Cycle count ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const cycleCount = await commerce.cycleCounts.get(params.cycleCountId);
      if (!cycleCount) {
        return { success: false, error: 'Cycle count not found' };
      }
      return { success: true, cycleCount };
    },
  },
  {
    name: 'create_cycle_count',
    description:
      'Create a draft cycle count for a warehouse with the SKUs to count and the quantity the system expects for each.',
    inputSchema: {
      warehouseId: z.number().int().positive().describe('Warehouse ID to count in'),
      lines: z
        .array(
          z.object({
            sku: z.string().min(1).describe('SKU to count'),
            expectedQuantity: quantityInput.describe(
              'Quantity the system expects on hand (exact decimal string or integer)',
            ),
            lotId: z.string().min(1).optional().describe('Lot ID (UUID) for lot-tracked stock'),
          }),
        )
        .min(1)
        .describe('Lines to count'),
      locationId: z
        .number()
        .int()
        .positive()
        .optional()
        .describe('Single location scope; omit to count across the warehouse'),
      scheduledDate: z.string().min(1).optional().describe('Scheduled date/time (ISO 8601)'),
      countedBy: z.string().min(1).optional().describe('Who will perform the count'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create cycle count', params);
      }

      // Binding CreateCycleCountInput: quantities are exact decimal strings,
      // scheduledDate an RFC 3339 timestamp; absent optionals omitted.
      const cycleCount = await commerce.cycleCounts.create(
        omitAbsent({
          warehouseId: params.warehouseId,
          locationId: params.locationId,
          scheduledDate: toRfc3339(params.scheduledDate),
          countedBy: params.countedBy,
          lines: params.lines.map((line) =>
            omitAbsent({
              sku: line.sku,
              lotId: line.lotId,
              expectedQuantity: String(line.expectedQuantity),
            }),
          ),
        }),
      );
      return { success: true, message: 'Cycle count created', cycleCount };
    },
  },
  {
    name: 'start_cycle_count',
    description: 'Start a cycle count.',
    inputSchema: {
      cycleCountId: z.string().min(1).describe('Cycle count ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Start cycle count', params);
      }

      const cycleCount = await commerce.cycleCounts.start(params.cycleCountId);
      return { success: true, message: 'Cycle count started', cycleCount };
    },
  },
  {
    name: 'record_cycle_counts',
    description: 'Record counted quantities for a cycle count.',
    inputSchema: {
      cycleCountId: z.string().min(1).describe('Cycle count ID'),
      counts: z
        .array(
          z.object({
            sku: z.string().min(1).describe('SKU'),
            countedQuantity: quantityInput.describe(
              'Counted quantity (exact decimal string or integer)',
            ),
            lotId: z.string().min(1).optional().describe('Lot ID (UUID) for lot-tracked stock'),
          }),
        )
        .min(1)
        .describe('Counted line items'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Record cycle counts', params);
      }

      // Binding RecordCycleCountLineInput.countedQuantity is an exact decimal string.
      const cycleCount = await commerce.cycleCounts.recordCounts(
        params.cycleCountId,
        params.counts.map((line) =>
          omitAbsent({
            sku: line.sku,
            lotId: line.lotId,
            countedQuantity: String(line.countedQuantity),
          }),
        ),
      );
      return { success: true, message: 'Cycle counts recorded', cycleCount };
    },
  },
  {
    name: 'complete_cycle_count',
    description: 'Complete a cycle count.',
    inputSchema: {
      cycleCountId: z.string().min(1).describe('Cycle count ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Complete cycle count', params);
      }

      const cycleCount = await commerce.cycleCounts.complete(params.cycleCountId);
      return { success: true, message: 'Cycle count completed', cycleCount };
    },
  },
  {
    name: 'cancel_cycle_count',
    description: 'Cancel a cycle count.',
    inputSchema: {
      cycleCountId: z.string().min(1).describe('Cycle count ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel cycle count', params);
      }

      const cycleCount = await commerce.cycleCounts.cancel(params.cycleCountId);
      return { success: true, message: 'Cycle count canceled', cycleCount };
    },
  },
]);

export default cycleCountTools;
