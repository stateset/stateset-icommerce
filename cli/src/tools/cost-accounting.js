/**
 * Cost Accounting Tools Module
 *
 * MCP tool definitions for item costing and inventory valuation.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const costAccountingTools = withPolicyDomain('cost_accounting', [
  {
    name: 'list_item_costs',
    description: 'List item costs.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const itemCosts = await commerce.costAccounting.listItemCosts();
      return { success: true, count: itemCosts.length, itemCosts };
    },
  },
  {
    name: 'get_item_cost',
    description: 'Get item cost for a SKU.',
    inputSchema: {
      sku: z.string().min(1).describe('SKU'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const itemCost = await commerce.costAccounting.getItemCost(params.sku);
      if (!itemCost) {
        return { success: false, error: 'Item cost not found' };
      }
      return { success: true, itemCost };
    },
  },
  {
    name: 'set_item_cost',
    description: 'Set item cost inputs for a SKU.',
    inputSchema: {
      sku: z.string().min(1).describe('SKU'),
      costMethod: z.string().min(1).optional().describe('Costing method'),
      standardCost: z.number().positive().optional().describe('Standard cost'),
      materialCost: z.number().nonnegative().optional().describe('Material cost'),
      laborCost: z.number().nonnegative().optional().describe('Labor cost'),
      overheadCost: z.number().nonnegative().optional().describe('Overhead cost'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Set item cost', params);
      }

      const itemCost = await commerce.costAccounting.setItemCost({
        sku: params.sku,
        costMethod: params.costMethod,
        standardCost: params.standardCost,
        materialCost: params.materialCost,
        laborCost: params.laborCost,
        overheadCost: params.overheadCost,
      });
      return { success: true, message: 'Item cost updated', itemCost };
    },
  },
  {
    name: 'update_average_item_cost',
    description: 'Update average cost for a SKU from a quantity and unit cost.',
    inputSchema: {
      sku: z.string().min(1).describe('SKU'),
      quantity: z.number().positive().describe('Quantity'),
      unitCost: z.number().positive().describe('Unit cost'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update average item cost', params);
      }

      const itemCost = await commerce.costAccounting.updateAverageCost(
        params.sku,
        params.quantity,
        params.unitCost,
      );
      return { success: true, message: 'Average item cost updated', itemCost };
    },
  },
  {
    name: 'get_total_inventory_value',
    description: 'Get total inventory value.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const totalInventoryValue = await commerce.costAccounting.getTotalInventoryValue();
      return { success: true, totalInventoryValue };
    },
  },
]);

export default costAccountingTools;
