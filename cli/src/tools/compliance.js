/**
 * Compliance & Regulatory Tools Module
 *
 * MCP tool definitions for compliance exports, tax reporting,
 * GDPR data portability/erasure, and SOC2 evidence gathering.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

import { scopedService } from './scoped-store.js';
import { createIsolatedComplianceService } from '../compliance/isolated.js';

/** Use a worker for file stores; never initialize a second SQLite driver here. */
async function getComplianceSvc(ctx) {
  const dbPath = ctx.a2aStore?.dbPath ?? ctx.commerce?._store?.dbPath ?? ctx.dbPath;
  if (dbPath && dbPath !== ':memory:' && !dbPath.startsWith('file::memory:')) {
    return createIsolatedComplianceService(dbPath);
  }
  // In-memory A2A stores cannot be reopened in another process. Their local
  // connection remains the only source; this does not expose native :memory:.
  return scopedService(ctx, 'compliance', async (store) => {
    const { createComplianceService } = await import('../compliance/exports.js');
    return createComplianceService(store, { commerceDb: store.db });
  });
}

/**
 * Compliance tool definitions
 */
export const complianceTools = [
  // ==========================================================================
  // Audit Trail
  // ==========================================================================
  {
    name: 'export_audit_trail',
    description:
      'Export a complete audit trail of agent transactions and events for compliance review. Supports JSON and CSV formats with date range, agent, and event type filters.',
    inputSchema: {
      from: z.string().optional().describe('Start date (ISO 8601)'),
      to: z.string().optional().describe('End date (ISO 8601)'),
      format: z.enum(['json', 'csv']).default('json').describe('Output format'),
      agentName: z.string().optional().describe('Filter by agent name or address'),
      eventType: z.string().optional().describe('Filter by event type'),
      limit: z
        .number()
        .int()
        .positive()
        .max(10000)
        .default(1000)
        .describe('Maximum number of records to return'),
    },
    permission: 'admin',
    handler: async ({ params, ...ctx }) => {
      try {
        const svc = await getComplianceSvc(ctx);
        const result = await svc.exportAuditTrail(params);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // 1099-K Tax Report
  // ==========================================================================
  {
    name: 'generate_1099k',
    description:
      'Generate a 1099-K tax report for an agent. Summarizes gross payment amounts, transaction counts, and monthly breakdowns for a given tax year.',
    inputSchema: {
      year: z.number().int().min(2020).max(2100).describe('Tax year (e.g. 2025)'),
      agentAddress: z.string().min(1).describe('Agent wallet address (payee)'),
    },
    permission: 'admin',
    handler: async ({ params, ...ctx }) => {
      try {
        const svc = await getComplianceSvc(ctx);
        const result = await svc.generate1099K(params);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // GDPR Data Export
  // ==========================================================================
  {
    name: 'export_gdpr_data',
    description:
      'Export all personal data for a customer or agent (GDPR Article 20 — data portability). Returns personal data, payments, communications, and disputes.',
    inputSchema: {
      customerId: z
        .string()
        .min(1)
        .describe('Customer or agent identifier (wallet address, agent ID, or name)'),
    },
    permission: 'admin',
    handler: async ({ params, ...ctx }) => {
      try {
        const svc = await getComplianceSvc(ctx);
        const result = await svc.generateGDPRExport(params.customerId);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // GDPR Data Deletion
  // ==========================================================================
  {
    name: 'delete_gdpr_data',
    description:
      'Delete personal data for GDPR right to erasure (Article 17). Optionally retains anonymized transaction records for legal/accounting requirements.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer or agent identifier to delete'),
      keepTransactions: z
        .boolean()
        .default(false)
        .describe('If true, keep payment/dispute records but anonymize personal fields'),
    },
    permission: 'admin',
    handler: async ({ params, ...ctx }) => {
      if (!ctx.allowApply) return applyRequired('Delete GDPR data', params);
      try {
        const svc = await getComplianceSvc(ctx);
        const result = await svc.deleteGDPRData(params.customerId, {
          keepTransactions: params.keepTransactions,
        });
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // Compliance Summary
  // ==========================================================================
  {
    name: 'compliance_summary',
    description:
      'Generate a compliance dashboard summary with transaction volume, dispute rates, policy violations, and top agents for a given period.',
    inputSchema: {
      period: z
        .enum(['day', 'week', 'month', 'quarter', 'year'])
        .default('month')
        .describe('Reporting period'),
      agentName: z.string().optional().describe('Filter by agent name or address'),
    },
    permission: 'read',
    handler: async ({ params, ...ctx }) => {
      try {
        const svc = await getComplianceSvc(ctx);
        const result = await svc.generateComplianceSummary(params);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // SOC2 Evidence
  // ==========================================================================
  {
    name: 'soc2_evidence',
    description:
      'Generate a SOC2 audit evidence package. Gathers structured evidence for requested controls: access_control, change_management, encryption, monitoring, incident_response.',
    inputSchema: {
      controls: z
        .array(
          z.enum([
            'access_control',
            'change_management',
            'encryption',
            'monitoring',
            'incident_response',
          ]),
        )
        .min(1)
        .describe('SOC2 control IDs to gather evidence for'),
    },
    permission: 'admin',
    handler: async ({ params, ...ctx }) => {
      try {
        const svc = await getComplianceSvc(ctx);
        const result = await svc.generateSOC2Evidence(params);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },
];

export default complianceTools;
