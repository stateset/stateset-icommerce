/**
 * Shipping Zone Tools Module
 *
 * MCP tool definitions for shipping zone, method, and rate management.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

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
      regions: z
        .array(z.string().min(1).max(100))
        .optional()
        .describe('State/province/region codes (e.g., ["CA", "NY"])'),
      postalCodeRanges: z
        .array(
          z.object({
            from: z.string().min(1).describe('Start of postal code range'),
            to: z.string().min(1).describe('End of postal code range'),
          }),
        )
        .optional()
        .describe('Postal code ranges for the zone'),
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
        postalCodeRanges: params.postalCodeRanges,
      });
      return { success: true, message: 'Shipping zone created', zone };
    },
  },

  {
    name: 'get_shipping_zone',
    description: 'Get a shipping zone by ID.',
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

      return {
        success: true,
        zone: {
          id: zone.id,
          name: zone.name,
          countries: zone.countries,
          regions: zone.regions,
          postalCodeRanges: zone.postalCodeRanges,
          methods: zone.methods,
          status: zone.status,
          createdAt: zone.createdAt,
          updatedAt: zone.updatedAt,
        },
      };
    },
  },

  {
    name: 'list_shipping_zones',
    description: 'List all shipping zones.',
    inputSchema: {
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
      const { limit } = params;
      const zones = await commerce.shippingZones.list();
      const count = await commerce.shippingZones.count();
      const limited = zones.slice(0, limit);

      return {
        success: true,
        totalCount: count,
        returned: limited.length,
        zones: limited.map((z) => ({
          id: z.id,
          name: z.name,
          countries: z.countries,
          methodCount: z.methodCount,
          status: z.status,
          createdAt: z.createdAt,
        })),
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
      regions: z.array(z.string().min(1).max(100)).optional().describe('Updated region codes'),
      postalCodeRanges: z
        .array(
          z.object({
            from: z.string().min(1).describe('Start of postal code range'),
            to: z.string().min(1).describe('End of postal code range'),
          }),
        )
        .optional()
        .describe('Updated postal code ranges'),
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
        postalCodeRanges: params.postalCodeRanges,
      });
      return { success: true, message: 'Shipping zone updated', zone };
    },
  },

  {
    name: 'create_shipping_method',
    description: 'Create a shipping method within a zone (e.g., Standard, Express, Overnight).',
    inputSchema: {
      zoneId: z.string().min(1).describe('Shipping zone ID'),
      name: z.string().min(1).max(255).describe('Shipping method name'),
      carrier: z
        .string()
        .min(1)
        .max(100)
        .optional()
        .describe('Carrier name (e.g., USPS, FedEx, UPS, DHL)'),
      minDeliveryDays: z.number().int().positive().optional().describe('Minimum delivery days'),
      maxDeliveryDays: z.number().int().positive().optional().describe('Maximum delivery days'),
      baseRate: z.number().positive().describe('Base shipping rate'),
      perItemRate: z.number().min(0).optional().default(0).describe('Additional rate per item'),
      freeShippingThreshold: z
        .number()
        .positive()
        .optional()
        .describe('Order amount for free shipping'),
      currency: z.string().min(1).max(10).optional().default('USD').describe('Currency code'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create shipping method', params);
      }

      const method = await commerce.shippingZones.createMethod(params.zoneId, {
        name: params.name,
        carrier: params.carrier,
        minDeliveryDays: params.minDeliveryDays,
        maxDeliveryDays: params.maxDeliveryDays,
        baseRate: String(params.baseRate),
        perItemRate: String(params.perItemRate || 0),
        freeShippingThreshold: params.freeShippingThreshold
          ? String(params.freeShippingThreshold)
          : undefined,
        currency: params.currency || 'USD',
      });
      return { success: true, message: 'Shipping method created', method };
    },
  },

  {
    name: 'calculate_shipping_rate',
    description: 'Calculate shipping rate for a destination address and cart items.',
    inputSchema: {
      country: z.string().min(2).max(3).describe('Destination country code (ISO)'),
      region: z.string().min(1).max(100).optional().describe('Destination state/province'),
      postalCode: z.string().min(1).max(20).optional().describe('Destination postal code'),
      items: z
        .array(
          z.object({
            sku: z.string().min(1).describe('Product SKU'),
            quantity: z.number().int().positive().describe('Item quantity'),
            weight: z.number().min(0).optional().describe('Item weight in grams'),
          }),
        )
        .min(1)
        .max(100)
        .describe('Cart items to calculate shipping for'),
      currency: z.string().min(1).max(10).optional().default('USD').describe('Currency code'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const rates = await commerce.shippingZones.calculateRates({
        country: params.country,
        region: params.region,
        postalCode: params.postalCode,
        items: params.items,
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
          isFreeShipping: r.isFreeShipping,
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
      const methods = await commerce.shippingZones.listMethods(zoneId);

      return {
        success: true,
        zoneId,
        count: methods.length,
        methods: methods.map((m) => ({
          id: m.id,
          name: m.name,
          carrier: m.carrier,
          baseRate: m.baseRate,
          perItemRate: m.perItemRate,
          freeShippingThreshold: m.freeShippingThreshold,
          minDeliveryDays: m.minDeliveryDays,
          maxDeliveryDays: m.maxDeliveryDays,
          currency: m.currency,
          status: m.status,
        })),
      };
    },
  },
];

export default shippingZoneTools;
