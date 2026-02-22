/**
 * Segment Tools Module
 *
 * MCP tool definitions for customer segmentation and dynamic group management.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const conditionSchema = z.object({
  field: z
    .string()
    .min(1)
    .describe('Field to evaluate (e.g., totalSpent, orderCount, lastOrderAt)'),
  operator: z
    .enum(['eq', 'neq', 'gt', 'gte', 'lt', 'lte', 'in', 'not_in', 'contains', 'starts_with'])
    .describe('Comparison operator'),
  value: z
    .union([z.string(), z.number(), z.boolean(), z.array(z.string())])
    .describe('Value to compare against'),
});

/**
 * Segment tool definitions
 */
export const segmentTools = [
  {
    name: 'create_segment',
    description: 'Create a customer segment with filter conditions.',
    inputSchema: {
      name: z.string().min(1).max(255).describe('Segment name'),
      description: z.string().max(1000).optional().describe('Segment description'),
      type: z
        .enum(['static', 'dynamic'])
        .optional()
        .default('dynamic')
        .describe('Segment type (static: manual, dynamic: auto-evaluated)'),
      conditions: z
        .array(conditionSchema)
        .min(1)
        .max(20)
        .describe('Filter conditions for segment membership'),
      conditionLogic: z
        .enum(['all', 'any'])
        .optional()
        .default('all')
        .describe('Whether all or any conditions must match'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create segment', params);
      }

      const segment = await commerce.segments.create({
        name: params.name,
        description: params.description,
        type: params.type || 'dynamic',
        conditions: params.conditions,
        conditionLogic: params.conditionLogic || 'all',
      });
      return { success: true, message: 'Segment created', segment };
    },
  },

  {
    name: 'get_segment',
    description: 'Get a segment by ID including its conditions and member count.',
    inputSchema: {
      segmentId: z.string().min(1).describe('Segment ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { segmentId } = params;
      const segment = await commerce.segments.get(segmentId);

      if (!segment) {
        return { success: false, error: 'Segment not found' };
      }

      return {
        success: true,
        segment: {
          id: segment.id,
          name: segment.name,
          description: segment.description,
          type: segment.type,
          conditions: segment.conditions,
          conditionLogic: segment.conditionLogic,
          memberCount: segment.memberCount,
          status: segment.status,
          lastEvaluatedAt: segment.lastEvaluatedAt,
          createdAt: segment.createdAt,
          updatedAt: segment.updatedAt,
        },
      };
    },
  },

  {
    name: 'list_segments',
    description: 'List all customer segments.',
    inputSchema: {
      type: z.enum(['static', 'dynamic']).optional().describe('Filter by segment type'),
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .default(50)
        .describe('Maximum number of segments to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { type, limit } = params;
      const segments = await commerce.segments.list({ type });
      const count = await commerce.segments.count({ type });
      const limited = segments.slice(0, limit);

      return {
        success: true,
        totalCount: count,
        returned: limited.length,
        segments: limited.map((s) => ({
          id: s.id,
          name: s.name,
          type: s.type,
          memberCount: s.memberCount,
          status: s.status,
          lastEvaluatedAt: s.lastEvaluatedAt,
          createdAt: s.createdAt,
        })),
      };
    },
  },

  {
    name: 'update_segment',
    description: 'Update a segment name, description, or conditions.',
    inputSchema: {
      segmentId: z.string().min(1).describe('Segment ID'),
      name: z.string().min(1).max(255).optional().describe('Updated segment name'),
      description: z.string().max(1000).optional().describe('Updated description'),
      conditions: z
        .array(conditionSchema)
        .min(1)
        .max(20)
        .optional()
        .describe('Updated filter conditions'),
      conditionLogic: z.enum(['all', 'any']).optional().describe('Updated condition logic'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update segment', params);
      }

      const segment = await commerce.segments.update(params.segmentId, {
        name: params.name,
        description: params.description,
        conditions: params.conditions,
        conditionLogic: params.conditionLogic,
      });
      return { success: true, message: 'Segment updated', segment };
    },
  },

  {
    name: 'evaluate_segment_membership',
    description: 'Check whether a customer belongs to a segment.',
    inputSchema: {
      segmentId: z.string().min(1).describe('Segment ID'),
      customerId: z.string().min(1).describe('Customer ID to evaluate'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { segmentId, customerId } = params;
      const result = await commerce.segments.evaluateMembership(segmentId, customerId);

      return {
        success: true,
        segmentId,
        customerId,
        isMember: result.isMember,
        matchedConditions: result.matchedConditions,
        evaluatedAt: result.evaluatedAt,
      };
    },
  },

  {
    name: 'rebuild_dynamic_segment',
    description: 'Rebuild a dynamic segment by re-evaluating all customers against its conditions.',
    inputSchema: {
      segmentId: z.string().min(1).describe('Segment ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Rebuild dynamic segment', params);
      }

      const result = await commerce.segments.rebuild(params.segmentId);
      return {
        success: true,
        message: 'Segment rebuilt',
        segmentId: params.segmentId,
        memberCount: result.memberCount,
        added: result.added,
        removed: result.removed,
        evaluatedAt: result.evaluatedAt,
      };
    },
  },
];

export default segmentTools;
