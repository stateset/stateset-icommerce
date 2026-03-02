/**
 * Audit Export & Compliance Tools Module
 *
 * MCP tool definitions for querying, exporting, and managing the persistent
 * audit log. Wraps the AuditStore class for MCP consumption.
 */

import { z } from 'zod';

/**
 * Audit tool definitions
 */
export const auditTools = [
  {
    name: 'audit_query',
    description:
      'Query the audit log with optional filters. Returns recent permission checks and tool executions.',
    inputSchema: {
      tool: z.string().min(1).optional().describe('Filter by tool name'),
      result: z
        .enum(['allowed', 'denied', 'executed'])
        .optional()
        .describe('Filter by result type'),
      since: z
        .string()
        .min(1)
        .optional()
        .describe('ISO 8601 timestamp to filter from (e.g. 2026-03-01T00:00:00Z)'),
      limit: z
        .number()
        .int()
        .positive()
        .max(500)
        .optional()
        .describe('Maximum entries to return (default: 50)'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const { getAuditStore } = await import('../audit-store.js');
        const store = getAuditStore();
        const entries = store.query({
          tool: params.tool || null,
          result: params.result || null,
          since: params.since || null,
          limit: params.limit || 50,
        });
        return {
          success: true,
          count: entries.length,
          entries,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'audit_summary',
    description:
      'Get a summary of audit activity including total entries, breakdown by result type, and most active tools.',
    inputSchema: {
      since: z.string().min(1).optional().describe('ISO 8601 timestamp to summarize from'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const { getAuditStore } = await import('../audit-store.js');
        const store = getAuditStore();
        const totalCount = store.count();

        // Query recent entries for breakdown
        const entries = store.query({
          since: params.since || null,
          limit: 10000,
        });

        const byResult = {};
        const byTool = {};
        for (const entry of entries) {
          byResult[entry.result] = (byResult[entry.result] || 0) + 1;
          byTool[entry.tool] = (byTool[entry.tool] || 0) + 1;
        }

        // Top 10 most active tools
        const topTools = Object.entries(byTool)
          .sort((a, b) => b[1] - a[1])
          .slice(0, 10)
          .map(([tool, count]) => ({ tool, count }));

        return {
          success: true,
          totalEntries: totalCount,
          queriedEntries: entries.length,
          since: params.since || null,
          byResult,
          topTools,
          denialRate:
            entries.length > 0
              ? `${(((byResult.denied || 0) / entries.length) * 100).toFixed(1)}%`
              : '0%',
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'audit_export',
    description:
      'Export the full audit log for compliance purposes. Returns all entries with metadata for external archival.',
    inputSchema: {
      since: z.string().min(1).optional().describe('ISO 8601 timestamp to export from'),
      limit: z
        .number()
        .int()
        .positive()
        .max(50000)
        .optional()
        .describe('Maximum entries to export (default: 10000)'),
      format: z.enum(['json', 'csv']).optional().describe('Export format (default: json)'),
    },
    permission: 'admin',
    handler: async ({ params }) => {
      try {
        const { getAuditStore } = await import('../audit-store.js');
        const store = getAuditStore();
        const exported = store.export({
          since: params.since || null,
          limit: params.limit || 10000,
        });

        if (params.format === 'csv') {
          const headers = 'id,timestamp,tool,result,reason,level,session_id,agent';
          const rows = exported.entries.map((e) =>
            [
              e.id,
              e.timestamp,
              e.tool,
              e.result,
              (e.reason || '').replace(/,/g, ';'),
              e.level,
              e.session_id || '',
              e.agent || '',
            ].join(','),
          );
          return {
            success: true,
            format: 'csv',
            exportedAt: exported.exportedAt,
            totalEntries: exported.totalEntries,
            exportedEntries: exported.entries.length,
            csv: [headers, ...rows].join('\n'),
          };
        }

        return {
          success: true,
          format: 'json',
          ...exported,
          exportedEntries: exported.entries.length,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'audit_retention',
    description:
      'Run audit log retention cleanup. Removes entries older than the configured retention period (default: 90 days).',
    inputSchema: {},
    permission: 'admin',
    handler: async ({ allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Audit retention cleanup requires --apply flag.',
          hint: 'Run with --apply to remove old audit entries.',
          wouldDo: 'Delete audit log entries older than the retention period',
        };
      }
      try {
        const { getAuditStore } = await import('../audit-store.js');
        const store = getAuditStore();
        const beforeCount = store.count();
        store.cleanup();
        const afterCount = store.count();
        return {
          success: true,
          message: 'Audit log cleanup completed',
          entriesBefore: beforeCount,
          entriesAfter: afterCount,
          entriesRemoved: beforeCount - afterCount,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },
];

export default auditTools;
