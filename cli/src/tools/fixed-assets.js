/**
 * Fixed Asset Tools Module
 *
 * MCP tool definitions for fixed assets and depreciation.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const fixedAssetTools = withPolicyDomain('fixed_assets', [
  {
    name: 'list_fixed_assets',
    description: 'List fixed assets.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const assets = await commerce.fixedAssets.list();
      return { success: true, count: assets.length, assets };
    },
  },
  {
    name: 'get_fixed_asset',
    description: 'Get a fixed asset by ID.',
    inputSchema: {
      assetId: z.string().min(1).describe('Fixed asset ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const asset = await commerce.fixedAssets.get(params.assetId);
      if (!asset) {
        return { success: false, error: 'Fixed asset not found' };
      }
      return { success: true, asset };
    },
  },
  {
    name: 'create_fixed_asset',
    description: 'Create a fixed asset.',
    inputSchema: {
      name: z.string().min(1).describe('Asset name'),
      assetType: z.string().min(1).describe('Asset type'),
      acquisitionCost: z.string().min(1).describe('Acquisition cost as an exact decimal string'),
      acquisitionDate: z.string().min(1).describe('Acquisition date in ISO 8601'),
      depreciationMethod: z.string().min(1).optional().describe('Depreciation method'),
      usefulLifeMonths: z.number().int().positive().optional().describe('Useful life in months'),
      salvageValue: z
        .string()
        .min(1)
        .optional()
        .describe('Salvage value as an exact decimal string'),
      description: z.string().max(2000).optional().describe('Optional description'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create fixed asset', params);
      }

      const asset = await commerce.fixedAssets.create({
        name: params.name,
        assetType: params.assetType,
        acquisitionCost: params.acquisitionCost,
        acquisitionDate: params.acquisitionDate,
        depreciationMethod: params.depreciationMethod,
        usefulLifeMonths: params.usefulLifeMonths,
        salvageValue: params.salvageValue,
        description: params.description,
      });
      return { success: true, message: 'Fixed asset created', asset };
    },
  },
  {
    name: 'place_asset_in_service',
    description: 'Place a fixed asset in service.',
    inputSchema: {
      assetId: z.string().min(1).describe('Fixed asset ID'),
      inServiceDate: z.string().min(1).optional().describe('In-service date in ISO 8601'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Place asset in service', params);
      }

      const asset = await commerce.fixedAssets.placeInService(params.assetId, params.inServiceDate);
      return { success: true, message: 'Asset placed in service', asset };
    },
  },
  {
    name: 'dispose_fixed_asset',
    description: 'Dispose of a fixed asset.',
    inputSchema: {
      assetId: z.string().min(1).describe('Fixed asset ID'),
      disposalDate: z.string().min(1).describe('Disposal date in ISO 8601'),
      disposalProceeds: z
        .string()
        .min(1)
        .optional()
        .describe('Disposal proceeds as an exact decimal string'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Dispose fixed asset', params);
      }

      const asset = await commerce.fixedAssets.dispose(params.assetId, {
        disposalDate: params.disposalDate,
        disposalProceeds: params.disposalProceeds,
        notes: params.notes,
      });
      return { success: true, message: 'Fixed asset disposed', asset };
    },
  },
  {
    name: 'write_off_fixed_asset',
    description: 'Write off a fixed asset.',
    inputSchema: {
      assetId: z.string().min(1).describe('Fixed asset ID'),
      reason: z.string().min(1).max(2000).describe('Write-off reason'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Write off fixed asset', params);
      }

      const asset = await commerce.fixedAssets.writeOff(params.assetId, params.reason);
      return { success: true, message: 'Fixed asset written off', asset };
    },
  },
  {
    name: 'generate_depreciation_schedule',
    description: 'Generate the depreciation schedule for a fixed asset.',
    inputSchema: {
      assetId: z.string().min(1).describe('Fixed asset ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Generate depreciation schedule', params);
      }

      const schedule = await commerce.fixedAssets.generateSchedule(params.assetId);
      return { success: true, message: 'Depreciation schedule generated', schedule };
    },
  },
  {
    name: 'get_depreciation_schedule',
    description: 'Get the depreciation schedule for a fixed asset.',
    inputSchema: {
      assetId: z.string().min(1).describe('Fixed asset ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const schedule = await commerce.fixedAssets.getSchedule(params.assetId);
      if (!schedule) {
        return { success: false, error: 'Depreciation schedule not found' };
      }
      return { success: true, schedule };
    },
  },
  {
    name: 'post_depreciation',
    description: 'Post depreciation for a period.',
    inputSchema: {
      assetId: z.string().min(1).describe('Fixed asset ID'),
      periodDate: z.string().min(1).describe('Period date in ISO 8601'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Post depreciation', params);
      }

      const result = await commerce.fixedAssets.postDepreciation(params.assetId, params.periodDate);
      return { success: true, message: 'Depreciation posted', result };
    },
  },
]);

export default fixedAssetTools;
