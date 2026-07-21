/**
 * Channel Tools Module
 *
 * MCP tool definitions for sales-channel administration.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const channelTools = withPolicyDomain('channels', [
  {
    name: 'check_channels_supported',
    description: 'Check whether the channels backend is available on this engine build.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const supported = await commerce.channels.isSupported();
      return { success: true, supported };
    },
  },
  {
    name: 'list_channels',
    description: 'List sales channels with optional filtering.',
    inputSchema: {
      channelType: z.string().min(1).optional().describe('Filter by channel type'),
      status: z.string().min(1).optional().describe('Filter by status'),
      integration: z.string().min(1).optional().describe('Filter by integration'),
      limit: z.number().int().min(1).optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const channels = await commerce.channels.list({
        channelType: params.channelType,
        status: params.status,
        integration: params.integration,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: channels.length, channels };
    },
  },
  {
    name: 'get_channel',
    description: 'Get a sales channel by ID.',
    inputSchema: {
      channelId: z.string().min(1).describe('Channel ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const channel = await commerce.channels.get(params.channelId);
      if (!channel) {
        return { success: false, error: 'Channel not found' };
      }
      return { success: true, channel };
    },
  },
  {
    name: 'create_channel',
    description: 'Create a sales channel.',
    inputSchema: {
      name: z.string().min(1).describe('Channel name'),
      channelType: z.string().min(1).describe('Channel type'),
      integration: z.string().min(1).optional().describe('Optional integration identifier'),
      defaultWarehouseId: z.string().min(1).optional().describe('Optional default warehouse ID'),
      tags: z.array(z.string().min(1)).optional().describe('Optional tags'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create channel', params);
      }

      const channel = await commerce.channels.create({
        name: params.name,
        channelType: params.channelType,
        integration: params.integration,
        defaultWarehouseId: params.defaultWarehouseId,
        tags: params.tags,
      });
      return { success: true, message: 'Channel created', channel };
    },
  },
  {
    name: 'update_channel',
    description: 'Update a sales channel.',
    inputSchema: {
      channelId: z.string().min(1).describe('Channel ID'),
      name: z.string().min(1).optional().describe('New name'),
      integration: z.string().min(1).optional().describe('New integration identifier'),
      status: z.string().min(1).optional().describe('New status'),
      defaultWarehouseId: z.string().min(1).optional().describe('New default warehouse ID'),
      tags: z.array(z.string().min(1)).optional().describe('New tags'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update channel', params);
      }

      const channel = await commerce.channels.update(params.channelId, {
        name: params.name,
        integration: params.integration,
        status: params.status,
        defaultWarehouseId: params.defaultWarehouseId,
        tags: params.tags,
      });
      return { success: true, message: 'Channel updated', channel };
    },
  },
  {
    name: 'set_channel_lock',
    description: 'Lock or unlock a sales channel for API writes.',
    inputSchema: {
      channelId: z.string().min(1).describe('Channel ID'),
      locked: z.boolean().describe('True to lock the channel, false to unlock'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Set channel lock', params);
      }

      const channel = await commerce.channels.setLock(params.channelId, params.locked);
      return {
        success: true,
        message: params.locked ? 'Channel locked' : 'Channel unlocked',
        channel,
      };
    },
  },
  {
    name: 'list_channel_product_mappings',
    description: 'List product mappings for a sales channel.',
    inputSchema: {
      channelId: z.string().min(1).describe('Channel ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const mappings = await commerce.channels.listProductMappings(params.channelId);
      return { success: true, count: mappings.length, mappings };
    },
  },
  {
    name: 'delete_channel',
    description: 'Delete a sales channel.',
    inputSchema: {
      channelId: z.string().min(1).describe('Channel ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete channel', params);
      }

      await commerce.channels.delete(params.channelId);
      return { success: true, message: 'Channel deleted' };
    },
  },
]);

export default channelTools;
