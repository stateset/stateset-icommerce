/**
 * Warehouse Tools Module
 *
 * MCP tool definitions for warehouses and storage locations.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const warehouseTools = withPolicyDomain('warehouse', [
  {
    name: 'list_warehouses',
    description: 'List warehouses.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const warehouses = await commerce.warehouse.listWarehouses();
      return { success: true, count: warehouses.length, warehouses };
    },
  },
  {
    name: 'get_warehouse',
    description: 'Get a warehouse by ID or code.',
    inputSchema: {
      warehouseId: z.number().int().optional().describe('Warehouse ID'),
      code: z.string().min(1).optional().describe('Warehouse code'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const warehouse =
        params.warehouseId !== undefined
          ? await commerce.warehouse.getWarehouse(params.warehouseId)
          : params.code
            ? await commerce.warehouse.getWarehouseByCode(params.code)
            : null;

      if (!warehouse) {
        return { success: false, error: 'Warehouse not found' };
      }
      return { success: true, warehouse };
    },
  },
  {
    name: 'create_warehouse',
    description: 'Create a warehouse.',
    inputSchema: {
      code: z.string().min(1).describe('Warehouse code'),
      name: z.string().min(1).describe('Warehouse name'),
      warehouseType: z.string().min(1).optional().describe('Warehouse type'),
      timezone: z.string().min(1).optional().describe('IANA timezone'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create warehouse', params);
      }

      const warehouse = await commerce.warehouse.createWarehouse({
        code: params.code,
        name: params.name,
        warehouseType: params.warehouseType,
        timezone: params.timezone,
      });
      return { success: true, message: 'Warehouse created', warehouse };
    },
  },
  {
    name: 'create_location',
    description: 'Create a warehouse location.',
    inputSchema: {
      warehouseId: z.number().int().describe('Warehouse ID'),
      locationType: z.string().min(1).describe('Location type'),
      zone: z.string().min(1).optional().describe('Optional zone'),
      aisle: z.string().min(1).optional().describe('Optional aisle'),
      rack: z.string().min(1).optional().describe('Optional rack'),
      bin: z.string().min(1).optional().describe('Optional bin'),
      isPickable: z.boolean().optional().describe('Whether location is pickable'),
      isReceivable: z.boolean().optional().describe('Whether location is receivable'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create location', params);
      }

      const location = await commerce.warehouse.createLocation({
        warehouseId: params.warehouseId,
        locationType: params.locationType,
        zone: params.zone,
        aisle: params.aisle,
        rack: params.rack,
        bin: params.bin,
        isPickable: params.isPickable,
        isReceivable: params.isReceivable,
      });
      return { success: true, message: 'Location created', location };
    },
  },
  {
    name: 'get_location',
    description: 'Get a warehouse location by ID.',
    inputSchema: {
      locationId: z.number().int().describe('Location ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const location = await commerce.warehouse.getLocation(params.locationId);
      if (!location) {
        return { success: false, error: 'Location not found' };
      }
      return { success: true, location };
    },
  },
  {
    name: 'list_locations',
    description: 'List warehouse locations.',
    inputSchema: {
      warehouseId: z.number().int().optional().describe('Optional warehouse ID filter'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const locations = await commerce.warehouse.listLocations(params.warehouseId);
      return { success: true, count: locations.length, locations };
    },
  },
  {
    name: 'list_pickable_locations',
    description: 'List pickable locations for a SKU in a warehouse.',
    inputSchema: {
      warehouseId: z.number().int().describe('Warehouse ID'),
      sku: z.string().min(1).describe('SKU'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const locations = await commerce.warehouse.getPickableLocations(
        params.warehouseId,
        params.sku,
      );
      return { success: true, count: locations.length, locations };
    },
  },
  {
    name: 'get_warehouse_sku_available_quantity',
    description: 'Get total available quantity for a SKU in a warehouse.',
    inputSchema: {
      warehouseId: z.number().int().describe('Warehouse ID'),
      sku: z.string().min(1).describe('SKU'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const quantity = await commerce.warehouse.getTotalAvailable(params.warehouseId, params.sku);
      return { success: true, warehouseId: params.warehouseId, sku: params.sku, quantity };
    },
  },
  {
    name: 'count_warehouses',
    description: 'Count warehouses.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const count = await commerce.warehouse.countWarehouses();
      return { success: true, count };
    },
  },
]);

export default warehouseTools;
