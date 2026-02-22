/**
 * Manufacturing Tools Module
 *
 * MCP tool definitions for Bill of Materials (BOM) and Work Order operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

/**
 * Manufacturing tool definitions
 */
export const manufacturingTools = [
  {
    name: 'list_boms',
    description:
      'List all Bills of Materials (BOMs). BOMs define the components/ingredients needed to manufacture a product.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const boms = await commerce.bom.list();
      const count = await commerce.bom.count();
      return {
        success: true,
        count,
        boms: boms.map((b) => ({
          id: b.id,
          bomNumber: b.bomNumber,
          name: b.name,
          productId: b.productId,
          status: b.status,
          revision: b.revision,
          createdAt: b.createdAt,
        })),
      };
    },
  },

  {
    name: 'get_bom',
    description: 'Get a Bill of Materials by ID, including all components/ingredients.',
    inputSchema: {
      bomId: z.string().min(1).describe('BOM ID or BOM number'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { bomId } = params;
      const bom = await commerce.bom.get(bomId);
      if (!bom) {
        return { success: false, error: 'BOM not found' };
      }
      const components = await commerce.bom.getComponents(bomId);
      return {
        success: true,
        bom: { ...bom, components },
      };
    },
  },

  {
    name: 'create_bom',
    description:
      'Create a new Bill of Materials for a product. Defines what components/ingredients are needed.',
    inputSchema: {
      name: z.string().min(1).describe('BOM name (e.g., "Classic Pickled Onions Recipe")'),
      productId: z.string().min(1).describe('Product ID this BOM is for'),
      description: z.string().optional().describe('Description of this BOM'),
      revision: z.string().optional().describe('Revision number (default: A)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Create BOM operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldCreate: params,
        };
      }

      const bom = await commerce.bom.create({
        name: params.name,
        productId: params.productId,
        description: params.description,
        revision: params.revision,
      });
      return {
        success: true,
        message: 'BOM created successfully',
        bom: {
          id: bom.id,
          bomNumber: bom.bomNumber,
          name: bom.name,
          status: bom.status,
        },
      };
    },
  },

  {
    name: 'add_bom_component',
    description: 'Add a component/ingredient to a Bill of Materials.',
    inputSchema: {
      bomId: z.string().min(1).describe('BOM ID to add component to'),
      name: z.string().min(1).describe('Component name (e.g., "Yellow Onions")'),
      sku: z.string().optional().describe('Component SKU if from inventory'),
      quantity: z.number().positive().describe('Quantity needed per unit produced'),
      unitOfMeasure: z.string().optional().describe('Unit (e.g., "kg", "lbs", "each", "ml")'),
      notes: z.string().optional().describe('Notes about this component'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Add component operation not allowed. The --apply flag must be set.',
          wouldAdd: params,
        };
      }

      const component = await commerce.bom.addComponent(params.bomId, {
        name: params.name,
        componentSku: params.sku || null,
        quantity: String(params.quantity),
        unitOfMeasure: params.unitOfMeasure || 'each',
        notes: params.notes || null,
      });
      return {
        success: true,
        message: 'Component added to BOM',
        component,
      };
    },
  },

  {
    name: 'activate_bom',
    description: 'Activate a BOM to make it available for work orders.',
    inputSchema: {
      bomId: z.string().min(1).describe('BOM ID to activate'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { bomId } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Activate BOM operation not allowed. The --apply flag must be set.',
          wouldActivate: bomId,
        };
      }

      const bom = await commerce.bom.activate(bomId);
      return {
        success: true,
        message: 'BOM activated',
        bom: { id: bom.id, name: bom.name, status: bom.status },
      };
    },
  },

  {
    name: 'list_work_orders',
    description: 'List all manufacturing work orders. Work orders track production runs.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const workOrders = await commerce.workOrders.list();
      const count = await commerce.workOrders.count();
      return {
        success: true,
        count,
        workOrders: workOrders.map((wo) => ({
          id: wo.id,
          workOrderNumber: wo.workOrderNumber,
          productId: wo.productId,
          status: wo.status,
          priority: wo.priority,
          quantityToBuild: wo.quantityToBuild,
          quantityCompleted: wo.quantityCompleted,
          scheduledStart: wo.scheduledStart,
          scheduledEnd: wo.scheduledEnd,
        })),
      };
    },
  },

  {
    name: 'get_work_order',
    description: 'Get a work order by ID with full details.',
    inputSchema: {
      workOrderId: z.string().min(1).describe('Work order ID or number'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { workOrderId } = params;
      const wo = await commerce.workOrders.get(workOrderId);
      if (!wo) {
        return { success: false, error: 'Work order not found' };
      }
      return { success: true, workOrder: wo };
    },
  },

  {
    name: 'create_work_order',
    description: 'Create a manufacturing work order to produce a quantity of product.',
    inputSchema: {
      productId: z.string().min(1).describe('Product ID to manufacture'),
      bomId: z.string().optional().describe('BOM ID to use (optional)'),
      quantityToBuild: z.number().int().min(1).describe('Number of units to produce'),
      priority: z.enum(['low', 'normal', 'high', 'urgent']).optional().describe('Priority level'),
      scheduledStart: z.string().optional().describe('Scheduled start date (ISO format)'),
      scheduledEnd: z.string().optional().describe('Scheduled end date (ISO format)'),
      notes: z.string().optional().describe('Production notes'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Create work order operation not allowed. The --apply flag must be set.',
          wouldCreate: params,
        };
      }

      const wo = await commerce.workOrders.create({
        productId: params.productId,
        bomId: params.bomId,
        quantityToBuild: params.quantityToBuild,
        priority: params.priority || 'normal',
        scheduledStart: params.scheduledStart,
        scheduledEnd: params.scheduledEnd,
        notes: params.notes,
      });
      return {
        success: true,
        message: 'Work order created',
        workOrder: {
          id: wo.id,
          workOrderNumber: wo.workOrderNumber,
          status: wo.status,
          quantityToBuild: wo.quantityToBuild,
        },
      };
    },
  },

  {
    name: 'start_work_order',
    description: 'Start a work order (begin production).',
    inputSchema: {
      workOrderId: z.string().min(1).describe('Work order ID to start'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { workOrderId } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Start work order operation not allowed. The --apply flag must be set.',
          wouldStart: workOrderId,
        };
      }

      const wo = await commerce.workOrders.start(workOrderId);
      return {
        success: true,
        message: 'Work order started - production in progress',
        workOrder: {
          id: wo.id,
          workOrderNumber: wo.workOrderNumber,
          status: wo.status,
        },
      };
    },
  },

  {
    name: 'complete_work_order',
    description: 'Complete a work order with the quantity produced.',
    inputSchema: {
      workOrderId: z.string().min(1).describe('Work order ID to complete'),
      quantityCompleted: z.number().int().min(0).describe('Number of units actually produced'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { workOrderId, quantityCompleted } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Complete work order operation not allowed. The --apply flag must be set.',
          wouldComplete: { workOrderId, quantityCompleted },
        };
      }

      const wo = await commerce.workOrders.complete(workOrderId, quantityCompleted);
      return {
        success: true,
        message: `Work order completed - ${quantityCompleted} units produced`,
        workOrder: {
          id: wo.id,
          workOrderNumber: wo.workOrderNumber,
          status: wo.status,
          quantityCompleted: wo.quantityCompleted,
        },
      };
    },
  },

  {
    name: 'cancel_work_order',
    description: 'Cancel a work order.',
    inputSchema: {
      workOrderId: z.string().min(1).describe('Work order ID to cancel'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { workOrderId } = params;
      if (!allowApply) {
        return {
          success: false,
          error: 'Cancel work order operation not allowed. The --apply flag must be set.',
          wouldCancel: workOrderId,
        };
      }

      const wo = await commerce.workOrders.cancel(workOrderId);
      return {
        success: true,
        message: 'Work order cancelled',
        workOrder: {
          id: wo.id,
          workOrderNumber: wo.workOrderNumber,
          status: wo.status,
        },
      };
    },
  },
];

export default manufacturingTools;
