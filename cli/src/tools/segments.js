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
    .describe(
      'Customer field: email, first_name, last_name, phone, status, tags, accepts_marketing, ' +
        'email_verified, order_count, total_spent:<CURRENCY> (e.g. total_spent:USD), ' +
        'created_at or last_order_at',
    ),
  operator: z
    .enum([
      'eq',
      'neq',
      'gt',
      'gte',
      'lt',
      'lte',
      'between',
      'in',
      'not_in',
      'contains',
      'starts_with',
      'ends_with',
    ])
    .describe('Comparison operator'),
  value: z
    .union([z.string(), z.number(), z.boolean(), z.array(z.union([z.string(), z.number()]))])
    .describe('Value to compare against'),
});

/** Operators the engine's `SegmentOperator` accepts. */
const ENGINE_OPERATORS = new Set([
  'eq',
  'neq',
  'gt',
  'gte',
  'lt',
  'lte',
  'between',
  'contains',
  'in',
  'starts_with',
  'ends_with',
]);

/** The engine caps a single `segments.list` page at 1000 rows (default 500). */
const SEGMENT_PAGE_SIZE = 1000;

/**
 * Map tool conditions onto the binding's `SegmentRuleInput[]`. Rule values are
 * strings in the engine (JSON-encoded for non-string values). Returns an error
 * string for anything the engine cannot represent rather than dropping it.
 */
function toEngineRules(conditions, conditionLogic) {
  if (conditionLogic === 'any') {
    return { error: "conditionLogic 'any' is not supported by the engine; rules are all-of" };
  }
  const rules = [];
  for (const c of conditions) {
    if (!ENGINE_OPERATORS.has(c.operator)) {
      return { error: `operator '${c.operator}' is not supported by the engine` };
    }
    rules.push({
      field: c.field,
      operator: c.operator,
      value: typeof c.value === 'string' ? c.value : JSON.stringify(c.value),
    });
  }
  return { rules };
}

/** Collect every segment matching `filter`, paging past the engine's per-call limit. */
async function listAllSegments(commerce, filter) {
  const all = [];
  for (let offset = 0; ; offset += SEGMENT_PAGE_SIZE) {
    const page = await commerce.segments.list({ ...filter, limit: SEGMENT_PAGE_SIZE, offset });
    all.push(...page);
    if (page.length < SEGMENT_PAGE_SIZE) return all;
  }
}

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
        .max(20)
        .optional()
        .default([])
        .describe(
          'Rules a customer must all match (dynamic segments only; required for dynamic, ' +
            'not allowed for static)',
        ),
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

      const segmentType = params.type || 'dynamic';
      const conditions = params.conditions ?? [];
      if (segmentType === 'dynamic' && conditions.length === 0) {
        return { success: false, error: 'a dynamic segment needs at least one condition' };
      }
      if (segmentType === 'static' && conditions.length > 0) {
        return {
          success: false,
          error:
            'a static segment takes no conditions (add members explicitly); ' +
            "use type 'dynamic' to select members by rule",
        };
      }
      const mapped = toEngineRules(conditions, params.conditionLogic);
      if (mapped.error) return { success: false, error: mapped.error };

      const segment = await commerce.segments.create({
        name: params.name,
        description: params.description,
        segmentType,
        rules: mapped.rules,
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
          type: segment.segmentType,
          rules: segment.rules,
          memberCount: segment.memberCount,
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
      const segments = await listAllSegments(commerce, { segmentType: type });
      const limited = segments.slice(0, limit);

      return {
        success: true,
        totalCount: segments.length,
        returned: limited.length,
        segments: limited.map((s) => ({
          id: s.id,
          name: s.name,
          type: s.segmentType,
          memberCount: s.memberCount,
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

      let rules;
      if (params.conditions !== undefined || params.conditionLogic !== undefined) {
        if (params.conditions === undefined) {
          return {
            success: false,
            error:
              'conditionLogic cannot be changed without conditions; the engine stores rules only',
          };
        }
        const mapped = toEngineRules(params.conditions, params.conditionLogic);
        if (mapped.error) return { success: false, error: mapped.error };
        rules = mapped.rules;
      }

      const segment = await commerce.segments.update(params.segmentId, {
        name: params.name,
        description: params.description,
        rules,
      });
      return { success: true, message: 'Segment updated', segment };
    },
  },

  {
    name: 'evaluate_segment_membership',
    description:
      'Check whether a customer is a member of a segment. Dynamic segments evaluate their rules against the customer now; static segments read the curated member list.',
    inputSchema: {
      segmentId: z.string().min(1).describe('Segment ID'),
      customerId: z.string().min(1).describe('Customer ID to evaluate'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { segmentId, customerId } = params;
      const isMember = await commerce.segments.isMember(segmentId, customerId);

      return {
        success: true,
        segmentId,
        customerId,
        isMember,
        basis: 'engine_membership',
      };
    },
  },
];

export default segmentTools;
