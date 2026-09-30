/**
 * Shipping Zone Tools Module
 *
 * MCP tool definitions for shipping zone, method, and rate management.
 * Every call here targets the real `ShippingZones` class of
 * `@stateset/embedded`: `create`, `get`, `update`, `list`, `createMethod`,
 * `listMethods`, `calculateRates`. Money crosses the binding as exact decimal
 * strings.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/**
 * Page size used to count every zone. The engine caps any single list at 1000
 * rows (default 500 when no limit is given), so one unpaged `list()` would
 * silently truncate the total.
 */
const LIST_PAGE_SIZE = 500;

const METHOD_TYPES = ['flat', 'weight_based', 'price_based', 'calculated', 'free'];

/** A non-negative decimal amount, as a number or an exact decimal string. */
const decimalAmount = z.union([
  z.number().min(0),
  z.string().regex(/^\d+(\.\d+)?$/, 'must be a non-negative decimal string'),
]);

/** @param {number|string|undefined} value */
function toDecimalString(value) {
  return value === undefined ? undefined : String(value);
}

/**
 * Read every shipping zone, page by page.
 * @param {any} commerce
 * @param {object} filter
 */
async function listAllZones(commerce, filter) {
  const all = [];
  for (let offset = 0; ; offset += LIST_PAGE_SIZE) {
    const page = await commerce.shippingZones.list({ ...filter, limit: LIST_PAGE_SIZE, offset });
    all.push(...page);
    if (page.length < LIST_PAGE_SIZE) return all;
  }
}

function summarizeZone(zone) {
  return {
    id: zone.id,
    name: zone.name,
    countries: zone.countries,
    regions: zone.regions,
    postalCodes: zone.postalCodes,
    priority: zone.priority,
    isActive: zone.isActive,
    createdAt: zone.createdAt,
    updatedAt: zone.updatedAt,
  };
}

function summarizeMethod(method) {
  return {
    id: method.id,
    zoneId: method.zoneId,
    name: method.name,
    carrier: method.carrier,
    methodType: method.methodType,
    baseRate: method.baseRate,
    currency: method.currency,
    minDeliveryDays: method.minDeliveryDays,
    maxDeliveryDays: method.maxDeliveryDays,
    conditions: method.conditions,
    isActive: method.isActive,
  };
}

const zoneGeographySchema = {
  regions: z
    .array(z.string().min(1).max(100))
    .optional()
    .describe('State/province/region codes (e.g., ["CA", "NY"])'),
  postalCodes: z
    .array(z.string().min(1).max(20))
    .optional()
    .describe('Postal codes (or postal code patterns) included in the zone'),
  priority: z
    .number()
    .int()
    .optional()
    .describe('Match priority when several zones cover a destination'),
};

/**
 * Shipping zone tool definitions
 */
export const shippingZoneTools = [
  {
    name: 'create_shipping_zone',
    description: 'Create a shipping zone with country/region rules.',
    inputSchema: {
      name: z
        .string()
        .min(1)
        .max(255)
        .describe('Shipping zone name (e.g., "Domestic", "EU", "Asia-Pacific")'),
      countries: z
        .array(z.string().min(2).max(3))
        .min(1)
        .max(250)
        .describe('ISO country codes included in the zone'),
      ...zoneGeographySchema,
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create shipping zone', params);
      }

      const zone = await commerce.shippingZones.create({
        name: params.name,
        countries: params.countries,
        regions: params.regions,
        postalCodes: params.postalCodes,
        priority: params.priority,
      });
      return { success: true, message: 'Shipping zone created', zone };
    },
  },

  {
    name: 'get_shipping_zone',
    description: 'Get a shipping zone by ID, including its shipping methods.',
    inputSchema: {
      zoneId: z.string().min(1).describe('Shipping zone ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { zoneId } = params;
      const zone = await commerce.shippingZones.get(zoneId);

      if (!zone) {
        return { success: false, error: 'Shipping zone not found' };
      }

      const methods = await commerce.shippingZones.listMethods({ zoneId });
      return {
        success: true,
        zone: { ...summarizeZone(zone), methods: methods.map(summarizeMethod) },
      };
    },
  },

  {
    name: 'list_shipping_zones',
    description: 'List all shipping zones.',
    inputSchema: {
      country: z.string().min(2).max(3).optional().describe('Only zones covering this country'),
      isActive: z.boolean().optional().describe('Filter by active flag'),
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .default(50)
        .describe('Maximum number of zones to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { country, isActive, limit } = params;
      const zones = await listAllZones(commerce, { country, isActive });
      const limited = zones.slice(0, limit);

      return {
        success: true,
        totalCount: zones.length,
        returned: limited.length,
        zones: limited.map(summarizeZone),
      };
    },
  },

  {
    name: 'update_shipping_zone',
    description: 'Update a shipping zone name, countries, or regions.',
    inputSchema: {
      zoneId: z.string().min(1).describe('Shipping zone ID'),
      name: z.string().min(1).max(255).optional().describe('Updated zone name'),
      countries: z
        .array(z.string().min(2).max(3))
        .min(1)
        .max(250)
        .optional()
        .describe('Updated country codes'),
      ...zoneGeographySchema,
      isActive: z.boolean().optional().describe('Activate or deactivate the zone'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update shipping zone', params);
      }

      const zone = await commerce.shippingZones.update(params.zoneId, {
        name: params.name,
        countries: params.countries,
        regions: params.regions,
        postalCodes: params.postalCodes,
        priority: params.priority,
        isActive: params.isActive,
      });
      return { success: true, message: 'Shipping zone updated', zone };
    },
  },

  {
    name: 'create_shipping_method',
    description:
      'Create a shipping method within a zone (e.g., Standard, Express, Overnight). ' +
      'weight_based and price_based methods pick their rate from `conditions`; ' +
      'free always rates 0; flat and calculated use baseRate.',
    inputSchema: {
      zoneId: z.string().min(1).describe('Shipping zone ID'),
      name: z.string().min(1).max(255).describe('Shipping method name'),
      carrier: z
        .string()
        .min(1)
        .max(100)
        .optional()
        .describe('Carrier name (e.g., USPS, FedEx, UPS, DHL)'),
      methodType: z
        .enum(METHOD_TYPES)
        .optional()
        .default('flat')
        .describe('Rating model (default: flat)'),
      minDeliveryDays: z.number().int().positive().optional().describe('Minimum delivery days'),
      maxDeliveryDays: z.number().int().positive().optional().describe('Maximum delivery days'),
      baseRate: decimalAmount.describe('Base shipping rate'),
      conditions: z
        .array(
          z.object({
            minWeight: decimalAmount.optional(),
            maxWeight: decimalAmount.optional(),
            minPrice: decimalAmount.optional().describe('Inclusive lower order-total bound'),
            maxPrice: decimalAmount.optional().describe('Inclusive upper order-total bound'),
            rate: decimalAmount.describe('Rate when this condition matches'),
          }),
        )
        .optional()
        .describe(
          'Rate tiers for weight_based / price_based methods (e.g. free shipping over 75: ' +
            '[{minPrice: 75, rate: 0}, {minPrice: 0, maxPrice: 74.99, rate: 5.99}])',
        ),
      currency: z.string().min(1).max(10).optional().default('USD').describe('Currency code'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create shipping method', params);
      }

      const method = await commerce.shippingZones.createMethod({
        zoneId: params.zoneId,
        name: params.name,
        carrier: params.carrier,
        methodType: params.methodType || 'flat',
        baseRate: toDecimalString(params.baseRate),
        currency: params.currency || 'USD',
        minDeliveryDays: params.minDeliveryDays,
        maxDeliveryDays: params.maxDeliveryDays,
        conditions: params.conditions?.map((c) => ({
          minWeight: toDecimalString(c.minWeight),
          maxWeight: toDecimalString(c.maxWeight),
          minPrice: toDecimalString(c.minPrice),
          maxPrice: toDecimalString(c.maxPrice),
          rate: toDecimalString(c.rate),
        })),
      });
      return { success: true, message: 'Shipping method created', method };
    },
  },

  {
    name: 'calculate_shipping_rate',
    description: 'Calculate available shipping rates for a destination address.',
    inputSchema: {
      country: z.string().min(2).max(3).describe('Destination country code (ISO)'),
      region: z.string().min(1).max(100).optional().describe('Destination state/province'),
      postalCode: z.string().min(1).max(20).optional().describe('Destination postal code'),
      weight: decimalAmount
        .optional()
        .describe('Total shipment weight (used by weight_based methods)'),
      orderTotal: decimalAmount.optional().describe('Order total (used by price_based methods)'),
      currency: z.string().min(1).max(10).optional().default('USD').describe('Currency code'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const rates = await commerce.shippingZones.calculateRates({
        country: params.country,
        region: params.region,
        postalCode: params.postalCode,
        weight: toDecimalString(params.weight),
        orderTotal: toDecimalString(params.orderTotal),
        currency: params.currency || 'USD',
      });

      return {
        success: true,
        destination: {
          country: params.country,
          region: params.region,
          postalCode: params.postalCode,
        },
        rates: rates.map((r) => ({
          methodId: r.methodId,
          methodName: r.methodName,
          carrier: r.carrier,
          rate: r.rate,
          currency: r.currency,
          minDeliveryDays: r.minDeliveryDays,
          maxDeliveryDays: r.maxDeliveryDays,
        })),
      };
    },
  },

  {
    name: 'list_shipping_methods',
    description: 'List shipping methods for a specific zone.',
    inputSchema: {
      zoneId: z.string().min(1).describe('Shipping zone ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { zoneId } = params;
      const methods = await commerce.shippingZones.listMethods({ zoneId });

      return {
        success: true,
        zoneId,
        count: methods.length,
        methods: methods.map(summarizeMethod),
      };
    },
  },
];

export default shippingZoneTools;
