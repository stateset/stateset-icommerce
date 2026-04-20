/**
 * Manufacturing Commands Module
 */

function parsePositiveNumber(value, usage) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || parsed <= 0) throw new Error(usage);
  return parsed;
}

function parseNonNegativeInt(value, usage) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 0) throw new Error(usage);
  return parsed;
}

function parsePositiveInt(value, usage) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0) throw new Error(usage);
  return parsed;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'boms': {
      const boms = await commerce.bom.list();
      return formatBomList(boms, { output, jsonOutput });
    }

    case 'bom': {
      const bomId = args[0];
      if (!bomId) throw new Error('Usage: manufacturing bom <bomId>');
      const bom = await commerce.bom.get(bomId);
      if (!bom) throw new Error(`BOM not found: ${bomId}`);
      const components = await commerce.bom.getComponents(bomId);
      return formatBomDetail({ ...bom, components }, { output, jsonOutput });
    }

    case 'create-bom': {
      const [name, productId, description, revision] = args;
      if (!name || !productId) {
        throw new Error(
          'Usage: manufacturing create-bom <name> <productId> [description] [revision]',
        );
      }
      const bom = await commerce.bom.create({
        name,
        productId,
        description: description || undefined,
        revision: revision || undefined,
      });
      return { bom, formatted: `Created BOM ${bom.bomNumber || bom.id}` };
    }

    case 'add-component': {
      const [bomId, name, quantityRaw, sku, unitOfMeasure, ...noteParts] = args;
      if (!bomId || !name || !quantityRaw) {
        throw new Error(
          'Usage: manufacturing add-component <bomId> <name> <quantity> [sku] [unitOfMeasure] [notes]',
        );
      }
      const component = await commerce.bom.addComponent(bomId, {
        name,
        componentSku: sku || null,
        quantity: String(
          parsePositiveNumber(
            quantityRaw,
            'Usage: manufacturing add-component <bomId> <name> <quantity> [sku] [unitOfMeasure] [notes]',
          ),
        ),
        unitOfMeasure: unitOfMeasure || 'each',
        notes: noteParts.join(' ') || null,
      });
      return { component, formatted: `Added component ${component.id || name} to BOM ${bomId}` };
    }

    case 'activate-bom': {
      const bomId = args[0];
      if (!bomId) throw new Error('Usage: manufacturing activate-bom <bomId>');
      const bom = await commerce.bom.activate(bomId);
      return { bom, formatted: `Activated BOM ${bom.bomNumber || bom.id}` };
    }

    case 'work-orders': {
      const workOrders = await commerce.workOrders.list();
      return formatWorkOrderList(workOrders, { output, jsonOutput });
    }

    case 'work-order': {
      const workOrderId = args[0];
      if (!workOrderId) throw new Error('Usage: manufacturing work-order <workOrderId>');
      const workOrder = await commerce.workOrders.get(workOrderId);
      if (!workOrder) throw new Error(`Work order not found: ${workOrderId}`);
      return formatWorkOrderDetail(workOrder, { jsonOutput });
    }

    case 'create-work-order': {
      const [
        productId,
        quantityToBuildRaw,
        bomId,
        priority,
        scheduledStart,
        scheduledEnd,
        ...noteParts
      ] = args;
      if (!productId || !quantityToBuildRaw) {
        throw new Error(
          'Usage: manufacturing create-work-order <productId> <quantityToBuild> [bomId] [priority] [scheduledStart] [scheduledEnd] [notes]',
        );
      }
      const workOrder = await commerce.workOrders.create({
        productId,
        bomId: bomId || undefined,
        quantityToBuild: parsePositiveInt(
          quantityToBuildRaw,
          'Usage: manufacturing create-work-order <productId> <quantityToBuild> [bomId] [priority] [scheduledStart] [scheduledEnd] [notes]',
        ),
        priority: priority || 'normal',
        scheduledStart: scheduledStart || undefined,
        scheduledEnd: scheduledEnd || undefined,
        notes: noteParts.join(' ') || undefined,
      });
      return {
        workOrder,
        formatted: `Created work order ${workOrder.workOrderNumber || workOrder.id}`,
      };
    }

    case 'start-work-order': {
      const workOrderId = args[0];
      if (!workOrderId) throw new Error('Usage: manufacturing start-work-order <workOrderId>');
      const workOrder = await commerce.workOrders.start(workOrderId);
      return {
        workOrder,
        formatted: `Started work order ${workOrder.workOrderNumber || workOrder.id}`,
      };
    }

    case 'complete-work-order': {
      const [workOrderId, quantityCompletedRaw] = args;
      if (!workOrderId || quantityCompletedRaw === undefined) {
        throw new Error(
          'Usage: manufacturing complete-work-order <workOrderId> <quantityCompleted>',
        );
      }
      const workOrder = await commerce.workOrders.complete(
        workOrderId,
        parseNonNegativeInt(
          quantityCompletedRaw,
          'Usage: manufacturing complete-work-order <workOrderId> <quantityCompleted>',
        ),
      );
      return {
        workOrder,
        formatted: `Completed work order ${workOrder.workOrderNumber || workOrder.id}`,
      };
    }

    case 'cancel-work-order': {
      const workOrderId = args[0];
      if (!workOrderId) throw new Error('Usage: manufacturing cancel-work-order <workOrderId>');
      const workOrder = await commerce.workOrders.cancel(workOrderId);
      return {
        workOrder,
        formatted: `Cancelled work order ${workOrder.workOrderNumber || workOrder.id}`,
      };
    }

    default:
      throw new Error(
        `Unknown action: manufacturing ${action}\n\n` +
          'Available actions:\n' +
          '  boms                                                                        List BOMs\n' +
          '  bom <bomId>                                                                 Get BOM\n' +
          '  create-bom <name> <productId> [description] [revision]                      Create BOM\n' +
          '  add-component <bomId> <name> <quantity> [sku] [unitOfMeasure] [notes]      Add BOM component\n' +
          '  activate-bom <bomId>                                                        Activate BOM\n' +
          '  work-orders                                                                 List work orders\n' +
          '  work-order <workOrderId>                                                    Get work order\n' +
          '  create-work-order <productId> <quantityToBuild> [bomId] [priority] [scheduledStart] [scheduledEnd] [notes]\n' +
          '  start-work-order <workOrderId>                                              Start work order\n' +
          '  complete-work-order <workOrderId> <quantityCompleted>                       Complete work order\n' +
          '  cancel-work-order <workOrderId>                                             Cancel work order',
      );
  }
}

function formatBomList(boms, { output, jsonOutput }) {
  if (jsonOutput) return boms;
  if (boms.length === 0) return { formatted: 'No BOMs found.' };
  const formatted = output.table(boms, [
    { key: 'id', header: 'ID' },
    { key: 'bomNumber', header: 'BOM #' },
    { key: 'name', header: 'Name' },
    { key: 'productId', header: 'Product' },
    { key: 'status', header: 'Status' },
  ]);
  return { boms, formatted };
}

function formatBomDetail(bom, { output, jsonOutput }) {
  if (jsonOutput) return bom;
  const components = Array.isArray(bom.components) ? bom.components : [];
  const componentsTable =
    components.length === 0
      ? 'No components'
      : output.table(components, [
          { key: 'id', header: 'Component' },
          { key: 'name', header: 'Name' },
          { key: 'componentSku', header: 'SKU' },
          { key: 'quantity', header: 'Qty', align: 'right' },
          { key: 'unitOfMeasure', header: 'Unit' },
        ]);
  return {
    bom,
    formatted:
      `BOM: ${bom.name}\n` +
      `${'-'.repeat(28)}\n` +
      `ID:           ${bom.id}\n` +
      `BOM number:   ${bom.bomNumber || 'N/A'}\n` +
      `Product:      ${bom.productId}\n` +
      `Revision:     ${bom.revision || 'N/A'}\n` +
      `Status:       ${bom.status}\n\n` +
      componentsTable,
  };
}

function formatWorkOrderList(workOrders, { output, jsonOutput }) {
  if (jsonOutput) return workOrders;
  if (workOrders.length === 0) return { formatted: 'No work orders found.' };
  const formatted = output.table(workOrders, [
    { key: 'id', header: 'ID' },
    { key: 'workOrderNumber', header: 'WO #' },
    { key: 'productId', header: 'Product' },
    { key: 'status', header: 'Status' },
    { key: 'priority', header: 'Priority' },
    { key: 'quantityToBuild', header: 'Planned', align: 'right' },
  ]);
  return { workOrders, formatted };
}

function formatWorkOrderDetail(workOrder, { jsonOutput }) {
  if (jsonOutput) return workOrder;
  return {
    workOrder,
    formatted:
      `Work order: ${workOrder.workOrderNumber || workOrder.id}\n` +
      `${'-'.repeat(36)}\n` +
      `Product:       ${workOrder.productId}\n` +
      `BOM:           ${workOrder.bomId || 'N/A'}\n` +
      `Status:        ${workOrder.status}\n` +
      `Priority:      ${workOrder.priority}\n` +
      `To build:      ${workOrder.quantityToBuild}\n` +
      `Completed:     ${workOrder.quantityCompleted}`,
  };
}

export const metadata = {
  name: 'manufacturing',
  aliases: ['mfg', 'bom'],
  description: 'BOM and work-order management commands',
  actions: {
    boms: { description: 'List BOMs', args: [] },
    bom: { description: 'Get BOM', args: ['<bomId>'] },
    'create-bom': {
      description: 'Create BOM',
      args: ['<name>', '<productId>', '[description]', '[revision]'],
    },
    'add-component': {
      description: 'Add BOM component',
      args: ['<bomId>', '<name>', '<quantity>', '[sku]', '[unitOfMeasure]', '[notes]'],
    },
    'activate-bom': { description: 'Activate BOM', args: ['<bomId>'] },
    'work-orders': { description: 'List work orders', args: [] },
    'work-order': { description: 'Get work order', args: ['<workOrderId>'] },
    'create-work-order': {
      description: 'Create work order',
      args: [
        '<productId>',
        '<quantityToBuild>',
        '[bomId]',
        '[priority]',
        '[scheduledStart]',
        '[scheduledEnd]',
        '[notes]',
      ],
    },
    'start-work-order': { description: 'Start work order', args: ['<workOrderId>'] },
    'complete-work-order': {
      description: 'Complete work order',
      args: ['<workOrderId>', '<quantityCompleted>'],
    },
    'cancel-work-order': { description: 'Cancel work order', args: ['<workOrderId>'] },
  },
};

export default { execute, metadata };
