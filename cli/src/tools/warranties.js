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
    name: 'create_warranty',
    description: 'Create a warranty for a product.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID (required)'),
      orderId: z.string().optional().describe('Order ID'),
      productId: z.string().optional().describe('Product ID'),
      warrantyType: z
        .enum(['standard', 'extended', 'lifetime'])
        .optional()
        .describe('Warranty type'),
      durationMonths: z.number().int().positive().optional().describe('Duration in months'),
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
      });
      return { success: true, message: 'Warranty created', warranty };
    },
  },

  {
    name: 'create_warranty_claim',
    description: 'File a warranty claim.',
    inputSchema: {
      warrantyId: z.string().min(1).describe('Warranty ID'),
      description: z.string().min(1).max(1000).describe('Issue description'),
      claimType: z.enum(['repair', 'replacement', 'refund']).optional().describe('Claim type'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create claim', params);
      }

      const claim = await commerce.warranties.createClaim({
        warrantyId: params.warrantyId,
        description: params.description,
        claimType: params.claimType || 'replacement',
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
];

export default warrantyTools;
