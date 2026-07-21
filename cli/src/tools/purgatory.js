/**
 * Purgatory Tools Module
 *
 * MCP tool definitions for staged (pre-posting) order ingestion.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const purgatoryTools = withPolicyDomain('purgatory', [
  {
    name: 'list_purgatory_orders',
    description: 'List staged purgatory orders.',
    inputSchema: {
      channelId: z.string().min(1).optional().describe('Channel ID'),
      isPosted: z.boolean().optional().describe('Filter by posted state'),
      limit: z.number().int().positive().optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Results to skip'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const orders = await commerce.purgatory.list({
        channelId: params.channelId,
        isPosted: params.isPosted,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: orders.length, orders };
    },
  },
  {
    name: 'get_purgatory_order',
    description: 'Get a purgatory order by ID.',
    inputSchema: {
      id: z.string().min(1).describe('Purgatory order ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const order = await commerce.purgatory.get(params.id);
      if (!order) {
        return { success: false, error: 'Purgatory order not found' };
      }
      return { success: true, order };
    },
  },
  {
    name: 'ingest_purgatory_order',
    description: 'Ingest an external order into purgatory.',
    inputSchema: {
      externalOrderId: z.string().min(1).describe('External order ID'),
      channelId: z.string().min(1).optional().describe('Channel ID'),
      externalStatus: z.string().min(1).optional().describe('External status'),
      metadata: z.string().min(1).optional().describe('Metadata as a JSON string'),
      items: z
        .array(
          z.object({
            externalSku: z.string().min(1).describe('External SKU'),
            quantity: z.string().min(1).describe('Quantity (exact decimal string)'),
            productId: z.string().min(1).optional().describe('Resolved product ID'),
          }),
        )
        .min(1)
        .describe('Staged line items'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Ingest purgatory order', params);
      }
      const order = await commerce.purgatory.ingest({
        channelId: params.channelId,
        externalOrderId: params.externalOrderId,
        externalStatus: params.externalStatus,
        metadata: params.metadata,
        items: params.items,
      });
      return { success: true, message: 'Purgatory order ingested', order };
    },
  },
  {
    name: 'map_purgatory_line',
    description: 'Map a staged line to a product and/or toggle its flags.',
    inputSchema: {
      id: z.string().min(1).describe('Purgatory order ID'),
      lineId: z.string().min(1).describe('Purgatory line item ID'),
      productId: z.string().min(1).optional().describe('Product ID to map to'),
      ignoreItem: z.boolean().optional().describe('Ignore this line'),
      nonPhysical: z.boolean().optional().describe('Mark the line non-physical'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Map purgatory line', params);
      }
      const order = await commerce.purgatory.mapLine(params.id, params.lineId, {
        productId: params.productId,
        ignoreItem: params.ignoreItem,
        nonPhysical: params.nonPhysical,
      });
      return { success: true, message: 'Purgatory line mapped', order };
    },
  },
  {
    name: 'post_purgatory_order',
    description: 'Post a fully-resolved order out of purgatory.',
    inputSchema: {
      id: z.string().min(1).describe('Purgatory order ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Post purgatory order', params);
      }
      const order = await commerce.purgatory.post(params.id);
      return { success: true, message: 'Purgatory order posted', order };
    },
  },
  {
    name: 'delete_purgatory_order',
    description: 'Delete a purgatory order.',
    inputSchema: {
      id: z.string().min(1).describe('Purgatory order ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete purgatory order', params);
      }
      await commerce.purgatory.delete(params.id);
      return { success: true, message: 'Purgatory order deleted' };
    },
  },
]);

export default purgatoryTools;
