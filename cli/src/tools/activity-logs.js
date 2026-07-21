/**
 * Activity Log Tools Module
 *
 * MCP tool definitions for the internal activity/audit trail.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const activityLogTools = withPolicyDomain('activity_logs', [
  {
    name: 'check_activity_logs_supported',
    description: 'Check whether the activity-logs backend is available on this engine build.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const supported = await commerce.activityLogs.isSupported();
      return { success: true, supported };
    },
  },
  {
    name: 'list_activity_logs',
    description: 'List activity log entries with optional filtering.',
    inputSchema: {
      subjectType: z.string().min(1).optional().describe('Filter by subject type'),
      subjectId: z.string().min(1).optional().describe('Filter by subject ID'),
      action: z.string().min(1).optional().describe('Filter by action'),
      limit: z.number().int().min(1).optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const entries = await commerce.activityLogs.list({
        subjectType: params.subjectType,
        subjectId: params.subjectId,
        action: params.action,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: entries.length, entries };
    },
  },
  {
    name: 'get_activity_log',
    description: 'Get an activity log entry by ID.',
    inputSchema: {
      activityLogId: z.string().min(1).describe('Activity log entry ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const entry = await commerce.activityLogs.get(params.activityLogId);
      if (!entry) {
        return { success: false, error: 'Activity log entry not found' };
      }
      return { success: true, entry };
    },
  },
  {
    name: 'get_activity_history_for_subject',
    description: 'Get the activity history for a subject (e.g. an order or product).',
    inputSchema: {
      subjectType: z.string().min(1).describe('Subject type'),
      subjectId: z.string().min(1).describe('Subject ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const entries = await commerce.activityLogs.historyForSubject(
        params.subjectType,
        params.subjectId,
      );
      return { success: true, count: entries.length, entries };
    },
  },
  {
    name: 'record_activity',
    description: 'Record an activity log entry for a subject.',
    inputSchema: {
      subjectType: z.string().min(1).describe('Subject type (e.g. order, product)'),
      subjectId: z.string().min(1).describe('Subject ID'),
      action: z.string().min(1).describe('Action performed'),
      summary: z.string().min(1).max(2000).describe('Human-readable summary'),
      actorKind: z.string().min(1).describe('Actor kind (e.g. user, agent, system)'),
      actor: z.string().min(1).optional().describe('Optional actor identifier'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Record activity', params);
      }

      const entry = await commerce.activityLogs.record({
        subjectType: params.subjectType,
        subjectId: params.subjectId,
        action: params.action,
        summary: params.summary,
        actorKind: params.actorKind,
        actor: params.actor,
      });
      return { success: true, message: 'Activity recorded', entry };
    },
  },
]);

export default activityLogTools;
