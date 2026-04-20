/**
 * Warranty Tools Module
 *
 * MCP tool definitions for warranty creation and claim management.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/**
 * Warranty tool definitions
 */
export const warrantyTools = [
  {
    name: 'list_warranties',
    description: 'List all warranties.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const warranties = await commerce.warranties.list();
      const count = await commerce.warranties.count();
      return { success: true, count, warranties };
    },
  },

  {
    name: 'get_warranty',
    description: 'Get a warranty by ID.',
    inputSchema: {
      warrantyId: z.string().min(1).describe('Warranty ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const warranty = await commerce.warranties.get(params.warrantyId);
      if (!warranty) {
        return { success: false, error: 'Warranty not found' };
      }
      return { success: true, warranty };
    },
  },

  {
    name: 'create_warranty',
    description: 'Create a warranty for a product.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID (required)'),
      orderId: z.string().optional().describe('Order ID'),
      productId: z.string().optional().describe('Product ID'),
      warrantyType: z
        .enum(['standard', 'extended', 'limited', 'lifetime'])
        .optional()
        .describe('Warranty type'),
      durationMonths: z.number().int().positive().optional().describe('Duration in months'),
      serialNumber: z.string().min(1).optional().describe('Serial number covered by the warranty'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create warranty', params);
      }

      const warranty = await commerce.warranties.create({
        customerId: params.customerId,
        orderId: params.orderId,
        productId: params.productId,
        warrantyType: params.warrantyType || 'standard',
        durationMonths: params.durationMonths || 12,
        serialNumber: params.serialNumber,
      });
      return { success: true, message: 'Warranty created', warranty };
    },
  },

  {
    name: 'create_warranty_claim',
    description: 'File a warranty claim.',
    inputSchema: {
      warrantyId: z.string().min(1).describe('Warranty ID'),
      issueDescription: z.string().min(1).max(1000).optional().describe('Issue description'),
      description: z
        .string()
        .min(1)
        .max(1000)
        .optional()
        .describe('Deprecated alias for issueDescription'),
      contactEmail: z.string().email().optional().describe('Contact email for claim follow-up'),
      contactPhone: z
        .string()
        .min(1)
        .max(50)
        .optional()
        .describe('Contact phone for claim follow-up'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const issueDescription = params.issueDescription || params.description;
      if (!issueDescription) {
        return { success: false, error: 'issueDescription is required' };
      }

      if (!allowApply) {
        return applyRequired('Create claim', {
          ...params,
          issueDescription,
        });
      }

      const claim = await commerce.warranties.createClaim({
        warrantyId: params.warrantyId,
        issueDescription,
        contactEmail: params.contactEmail,
        contactPhone: params.contactPhone,
      });
      return { success: true, message: 'Claim filed', claim };
    },
  },

  {
    name: 'approve_warranty_claim',
    description: 'Approve a warranty claim.',
    inputSchema: {
      claimId: z.string().min(1).describe('Claim ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { claimId } = params;
      if (!allowApply) {
        return applyRequired('Approve claim', params);
      }

      const claim = await commerce.warranties.approveClaim(claimId);
      return { success: true, message: 'Claim approved', claim };
    },
  },

  {
    name: 'deny_warranty_claim',
    description: 'Deny a warranty claim with a reason.',
    inputSchema: {
      claimId: z.string().min(1).describe('Claim ID'),
      reason: z.string().min(1).max(1000).describe('Reason for denial'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Deny claim', params);
      }

      const claim = await commerce.warranties.denyClaim(params.claimId, params.reason);
      return { success: true, message: 'Claim denied', claim };
    },
  },

  {
    name: 'complete_warranty_claim',
    description: 'Complete a warranty claim with a final resolution.',
    inputSchema: {
      claimId: z.string().min(1).describe('Claim ID'),
      resolution: z
        .enum(['repair', 'replacement', 'refund', 'store_credit', 'denied'])
        .describe('Final claim resolution'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Complete claim', params);
      }

      const claim = await commerce.warranties.completeClaim(params.claimId, params.resolution);
      return { success: true, message: 'Claim completed', claim };
    },
  },
];

export default warrantyTools;
