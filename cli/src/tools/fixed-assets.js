/**
 * Fixed Asset Tools Module
 *
 * MCP tool definitions for fixed assets and depreciation.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;
const DECIMAL = /^\d+(?:\.\d+)?$/;

const FIXED_ASSET_CATEGORIES = [
  'land',
  'building',
  'machinery',
  'equipment',
  'vehicle',
  'furniture_and_fixtures',
  'computer_hardware',
  'software',
  'leasehold_improvement',
  'other',
];

/** Today's date as the binding's `YYYY-MM-DD` string (UTC). */
function todayIsoDate() {
  return new Date().toISOString().slice(0, 10);
}

/**
 * Drop undefined/null keys: the binding's `Option<String>` fields accept an
 * absent key but refuse `null`.
 * @template {Record<string, unknown>} T
 * @param {T} input
 * @returns {T}
 */
function omitAbsent(input) {
  return /** @type {T} */ (
    Object.fromEntries(
      Object.entries(input).filter(([, value]) => value !== undefined && value !== null),
    )
  );
}

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
      category: z.enum(FIXED_ASSET_CATEGORIES).describe('Asset category'),
      acquisitionCost: z
        .string()
        .regex(DECIMAL, 'must be an exact decimal string')
        .describe('Acquisition cost as an exact decimal string (e.g. "10000.00")'),
      acquisitionDate: z
        .string()
        .regex(ISO_DATE, 'must be YYYY-MM-DD')
        .describe('Acquisition date (YYYY-MM-DD)'),
      depreciationMethod: z
        .enum(['straight_line', 'declining_balance', 'units_of_production'])
        .describe('Depreciation method (declining_balance also needs decliningBalanceRate)'),
      usefulLifeMonths: z.number().int().positive().describe('Useful life in months'),
      salvageValue: z
        .string()
        .regex(DECIMAL, 'must be an exact decimal string')
        .optional()
        .describe('Salvage value as an exact decimal string. Defaults to "0".'),
      decliningBalanceRate: z
        .string()
        .regex(DECIMAL, 'must be an exact decimal string')
        .optional()
        .describe('Periodic rate strictly between 0 and 1 (required for declining_balance)'),
      inServiceDate: z
        .string()
        .regex(ISO_DATE, 'must be YYYY-MM-DD')
        .optional()
        .describe('When supplied, the asset is created in service on this date (YYYY-MM-DD)'),
      currency: z.string().length(3).optional().describe('Currency code, e.g. USD'),
      description: z.string().max(2000).optional().describe('Optional description'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      // Binding CreateFixedAssetInput. salvageValue is required there; an
      // unstated salvage value is zero, the same default the HTTP API applies.
      const input = omitAbsent({
        name: params.name,
        description: params.description,
        category: params.category,
        acquisitionDate: params.acquisitionDate,
        acquisitionCost: params.acquisitionCost,
        salvageValue: params.salvageValue ?? '0',
        usefulLifeMonths: params.usefulLifeMonths,
        depreciationMethod: params.depreciationMethod,
        decliningBalanceRate: params.decliningBalanceRate,
        inServiceDate: params.inServiceDate,
        currency: params.currency,
      });
      if (!allowApply) {
        return applyRequired('Create fixed asset', input);
      }

      const asset = await commerce.fixedAssets.create(input);
      return { success: true, message: 'Fixed asset created', asset };
    },
  },
  {
    name: 'place_asset_in_service',
    description: 'Place a fixed asset in service.',
    inputSchema: {
      assetId: z.string().min(1).describe('Fixed asset ID'),
      inServiceDate: z
        .string()
        .regex(ISO_DATE, 'must be YYYY-MM-DD')
        .optional()
        .describe('In-service date (YYYY-MM-DD). Defaults to today (UTC).'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      // The binding requires the date; like its own dispose/writeOff, an
      // unstated date means today.
      const inServiceDate = params.inServiceDate ?? todayIsoDate();
      if (!allowApply) {
        return applyRequired('Place asset in service', { ...params, inServiceDate });
      }

      const asset = await commerce.fixedAssets.placeInService(params.assetId, inServiceDate);
      return { success: true, message: 'Asset placed in service', asset };
    },
  },
  {
    name: 'dispose_fixed_asset',
    description: 'Dispose of a fixed asset.',
    inputSchema: {
      assetId: z.string().min(1).describe('Fixed asset ID'),
      disposalProceeds: z
        .string()
        .regex(DECIMAL, 'must be an exact decimal string')
        .describe(
          'Disposal proceeds as an exact decimal string (use write_off_fixed_asset for no proceeds)',
        ),
      disposalDate: z
        .string()
        .regex(ISO_DATE, 'must be YYYY-MM-DD')
        .optional()
        .describe('Disposal date (YYYY-MM-DD). Defaults to today.'),
      notes: z.string().max(2000).optional().describe('Optional notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Dispose fixed asset', params);
      }

      // Binding: dispose(id, proceeds, date?, notes?) -- positional, not an object.
      const asset = await commerce.fixedAssets.dispose(
        params.assetId,
        params.disposalProceeds,
        params.disposalDate ?? undefined,
        params.notes ?? undefined,
      );
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

      // Binding: writeOff(id, date?, notes?) -- the reason is the notes; the
      // date defaults to today.
      const asset = await commerce.fixedAssets.writeOff(params.assetId, undefined, params.reason);
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
    description:
      'Post the next scheduled depreciation entries for a fixed asset (generate the schedule first).',
    inputSchema: {
      assetId: z.string().min(1).describe('Fixed asset ID'),
      periods: z
        .number()
        .int()
        .positive()
        .optional()
        .default(1)
        .describe('How many of the next scheduled periods to post (default 1)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const periods = params.periods ?? 1;
      if (!allowApply) {
        return applyRequired('Post depreciation', { ...params, periods });
      }

      const result = await commerce.fixedAssets.postDepreciation(params.assetId, periods);
      return { success: true, message: 'Depreciation posted', result };
    },
  },
]);

export default fixedAssetTools;
