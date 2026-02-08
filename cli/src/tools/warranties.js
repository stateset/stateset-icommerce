/**
 * Warranty Tools Module
 *
 * MCP tool definitions for warranty creation and claim management.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

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
      customerId: z.string().describe('Customer ID (required)'),
      orderId: z.string().optional().describe('Order ID'),
      productId: z.string().optional().describe('Product ID'),
      warrantyType: z.string().optional().describe('Type: standard, extended, lifetime'),
      durationMonths: z.number().optional().describe('Duration in months'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return { error: 'Create warranty requires --apply flag.' };
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
      warrantyId: z.string().describe('Warranty ID'),
      description: z.string().describe('Issue description'),
      claimType: z.string().optional().describe('Type: repair, replacement, refund'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return { error: 'Create claim requires --apply flag.' };
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
      claimId: z.string().describe('Claim ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { claimId } = params;
      if (!allowApply) {
        return { error: 'Approve claim requires --apply flag.' };
      }

      const claim = await commerce.warranties.approveClaim(claimId);
      return { success: true, message: 'Claim approved', claim };
    },
  },
];

/**
 * Get all warranty tools
 */
export function getWarrantyTools() {
  return warrantyTools;
}

/**
 * Get warranty tool by name
 */
export function getWarrantyTool(name) {
  return warrantyTools.find((t) => t.name === name);
}

export default warrantyTools;
