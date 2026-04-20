/**
 * Quality Tools Module
 *
 * MCP tool definitions for inspections, NCRs, and quality holds.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const qualityTools = withPolicyDomain('quality', [
  {
    name: 'list_inspections',
    description: 'List quality inspections.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const inspections = await commerce.quality.listInspections();
      return { success: true, count: inspections.length, inspections };
    },
  },
  {
    name: 'get_inspection',
    description: 'Get a quality inspection by ID.',
    inputSchema: {
      inspectionId: z.string().min(1).describe('Inspection ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const inspection = await commerce.quality.getInspection(params.inspectionId);
      if (!inspection) {
        return { success: false, error: 'Inspection not found' };
      }
      return { success: true, inspection };
    },
  },
  {
    name: 'create_inspection',
    description: 'Create a quality inspection.',
    inputSchema: {
      inspectionType: z.string().min(1).describe('Inspection type'),
      referenceType: z.string().min(1).describe('Reference entity type'),
      referenceId: z.string().min(1).describe('Reference entity ID'),
      warehouseId: z.number().int().optional().describe('Optional warehouse ID'),
      assignedTo: z.string().min(1).optional().describe('Optional assignee'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create inspection', params);
      }

      const inspection = await commerce.quality.createInspection({
        inspectionType: params.inspectionType,
        referenceType: params.referenceType,
        referenceId: params.referenceId,
        warehouseId: params.warehouseId,
        assignedTo: params.assignedTo,
        notes: params.notes,
      });
      return { success: true, message: 'Inspection created', inspection };
    },
  },
  {
    name: 'start_inspection',
    description: 'Start a quality inspection.',
    inputSchema: {
      inspectionId: z.string().min(1).describe('Inspection ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Start inspection', params);
      }

      const inspection = await commerce.quality.startInspection(params.inspectionId);
      return { success: true, message: 'Inspection started', inspection };
    },
  },
  {
    name: 'complete_inspection',
    description: 'Complete a quality inspection.',
    inputSchema: {
      inspectionId: z.string().min(1).describe('Inspection ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Complete inspection', params);
      }

      const inspection = await commerce.quality.completeInspection(params.inspectionId);
      return { success: true, message: 'Inspection completed', inspection };
    },
  },
  {
    name: 'list_ncrs',
    description: 'List non-conformance reports.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const ncrs = await commerce.quality.listNcrs();
      return { success: true, count: ncrs.length, ncrs };
    },
  },
  {
    name: 'get_ncr',
    description: 'Get a non-conformance report by ID.',
    inputSchema: {
      ncrId: z.string().min(1).describe('NCR ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const ncr = await commerce.quality.getNcr(params.ncrId);
      if (!ncr) {
        return { success: false, error: 'NCR not found' };
      }
      return { success: true, ncr };
    },
  },
  {
    name: 'create_ncr',
    description: 'Create a non-conformance report.',
    inputSchema: {
      source: z.string().min(1).describe('NCR source'),
      severity: z.string().min(1).describe('Severity level'),
      sku: z.string().min(1).describe('SKU'),
      quantityAffected: z.number().positive().describe('Affected quantity'),
      description: z.string().min(1).max(4000).describe('Issue description'),
      lotNumber: z.string().min(1).optional().describe('Optional lot number'),
      locationId: z.number().int().optional().describe('Optional location ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create NCR', params);
      }

      const ncr = await commerce.quality.createNcr({
        source: params.source,
        severity: params.severity,
        sku: params.sku,
        quantityAffected: params.quantityAffected,
        description: params.description,
        lotNumber: params.lotNumber,
        locationId: params.locationId,
      });
      return { success: true, message: 'NCR created', ncr };
    },
  },
  {
    name: 'close_ncr',
    description: 'Close a non-conformance report.',
    inputSchema: {
      ncrId: z.string().min(1).describe('NCR ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Close NCR', params);
      }

      const ncr = await commerce.quality.closeNcr(params.ncrId);
      return { success: true, message: 'NCR closed', ncr };
    },
  },
  {
    name: 'list_quality_holds',
    description: 'List quality holds.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const holds = await commerce.quality.listHolds();
      return { success: true, count: holds.length, holds };
    },
  },
  {
    name: 'get_quality_hold',
    description: 'Get a quality hold by ID.',
    inputSchema: {
      holdId: z.string().min(1).describe('Quality hold ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const hold = await commerce.quality.getHold(params.holdId);
      if (!hold) {
        return { success: false, error: 'Quality hold not found' };
      }
      return { success: true, hold };
    },
  },
  {
    name: 'create_quality_hold',
    description: 'Create a quality hold.',
    inputSchema: {
      sku: z.string().min(1).describe('SKU'),
      lotNumber: z.string().min(1).optional().describe('Optional lot number'),
      quantityHeld: z.number().positive().describe('Quantity to hold'),
      reason: z.string().min(1).max(2000).describe('Hold reason'),
      holdType: z.string().min(1).describe('Hold type'),
      placedBy: z.string().min(1).optional().describe('User placing the hold'),
      locationId: z.number().int().optional().describe('Optional location ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create quality hold', params);
      }

      const hold = await commerce.quality.createHold({
        sku: params.sku,
        lotNumber: params.lotNumber,
        quantityHeld: params.quantityHeld,
        reason: params.reason,
        holdType: params.holdType,
        placedBy: params.placedBy,
        locationId: params.locationId,
      });
      return { success: true, message: 'Quality hold created', hold };
    },
  },
  {
    name: 'release_quality_hold',
    description: 'Release a quality hold.',
    inputSchema: {
      holdId: z.string().min(1).describe('Quality hold ID'),
      releasedBy: z.string().min(1).describe('User releasing the hold'),
      notes: z.string().max(2000).optional().describe('Optional release notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Release quality hold', params);
      }

      const hold = await commerce.quality.releaseHold(
        params.holdId,
        params.releasedBy,
        params.notes,
      );
      return { success: true, message: 'Quality hold released', hold };
    },
  },
  {
    name: 'list_active_quality_holds',
    description: 'List active quality holds.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const holds = await commerce.quality.getActiveHolds();
      return { success: true, count: holds.length, holds };
    },
  },
  {
    name: 'count_active_quality_holds',
    description: 'Count active quality holds.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const count = await commerce.quality.countActiveHolds();
      return { success: true, count };
    },
  },
]);

export default qualityTools;
