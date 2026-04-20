/**
 * Cost Accounting Commands Module
 */

function parseNumber(value, usage, { allowZero = false } = {}) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || (!allowZero && parsed <= 0) || (allowZero && parsed < 0)) {
    throw new Error(usage);
  }
  return parsed;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const itemCosts = await commerce.costAccounting.listItemCosts();
      return formatItemCosts(itemCosts, { output, jsonOutput });
    }

    case 'get': {
      const sku = args[0];
      if (!sku) throw new Error('Usage: cost-accounting get <sku>');
      const itemCost = await commerce.costAccounting.getItemCost(sku);
      if (!itemCost) throw new Error(`Item cost not found: ${sku}`);
      return formatItemCost(itemCost, { jsonOutput });
    }

    case 'set': {
      const [sku, costMethod, standardCostRaw, materialCostRaw, laborCostRaw, overheadCostRaw] =
        args;
      if (!sku) {
        throw new Error(
          'Usage: cost-accounting set <sku> [costMethod] [standardCost] [materialCost] [laborCost] [overheadCost]',
        );
      }
      const itemCost = await commerce.costAccounting.setItemCost({
        sku,
        costMethod: costMethod || undefined,
        standardCost: standardCostRaw
          ? parseNumber(standardCostRaw, 'standardCost must be positive')
          : undefined,
        materialCost: materialCostRaw
          ? parseNumber(materialCostRaw, 'materialCost must be non-negative', { allowZero: true })
          : undefined,
        laborCost: laborCostRaw
          ? parseNumber(laborCostRaw, 'laborCost must be non-negative', { allowZero: true })
          : undefined,
        overheadCost: overheadCostRaw
          ? parseNumber(overheadCostRaw, 'overheadCost must be non-negative', { allowZero: true })
          : undefined,
      });
      return { itemCost, formatted: `Updated item cost for ${sku}` };
    }

    case 'average': {
      const [sku, quantityRaw, unitCostRaw] = args;
      if (!sku || !quantityRaw || !unitCostRaw) {
        throw new Error('Usage: cost-accounting average <sku> <quantity> <unitCost>');
      }
      const itemCost = await commerce.costAccounting.updateAverageCost(
        sku,
        parseNumber(quantityRaw, 'Usage: cost-accounting average <sku> <quantity> <unitCost>'),
        parseNumber(unitCostRaw, 'Usage: cost-accounting average <sku> <quantity> <unitCost>'),
      );
      return { itemCost, formatted: `Updated average item cost for ${sku}` };
    }

    case 'inventory-value': {
      const totalInventoryValue = await commerce.costAccounting.getTotalInventoryValue();
      return jsonOutput
        ? { totalInventoryValue }
        : { formatted: `Total inventory value: ${totalInventoryValue}` };
    }

    default:
      throw new Error(
        `Unknown action: cost-accounting ${action}\n\n` +
          'Available actions:\n' +
          '  list                                                                   List item costs\n' +
          '  get <sku>                                                              Get item cost\n' +
          '  set <sku> [costMethod] [standardCost] [materialCost] [laborCost] [overheadCost]\n' +
          '  average <sku> <quantity> <unitCost>                                    Update average cost\n' +
          '  inventory-value                                                        Get total inventory value',
      );
  }
}

function formatItemCosts(itemCosts, { output, jsonOutput }) {
  if (jsonOutput) return itemCosts;
  if (itemCosts.length === 0) return { formatted: 'No item costs found.' };
  const formatted = output.table(itemCosts, [
    { key: 'sku', header: 'SKU' },
    { key: 'costMethod', header: 'Method' },
    { key: 'standardCost', header: 'Standard', align: 'right' },
    { key: 'materialCost', header: 'Material', align: 'right' },
    { key: 'laborCost', header: 'Labor', align: 'right' },
    { key: 'overheadCost', header: 'Overhead', align: 'right' },
  ]);
  return { itemCosts, formatted };
}

function formatItemCost(itemCost, { jsonOutput }) {
  if (jsonOutput) return itemCost;
  return {
    itemCost,
    formatted:
      `Item cost: ${itemCost.sku}\n` +
      `${'-'.repeat(30)}\n` +
      `Method:         ${itemCost.costMethod || 'N/A'}\n` +
      `Standard cost:  ${itemCost.standardCost ?? 'N/A'}\n` +
      `Material cost:  ${itemCost.materialCost ?? 'N/A'}\n` +
      `Labor cost:     ${itemCost.laborCost ?? 'N/A'}\n` +
      `Overhead cost:  ${itemCost.overheadCost ?? 'N/A'}`,
  };
}

export const metadata = {
  name: 'cost-accounting',
  aliases: ['costs', 'costing'],
  description: 'Item costing and inventory valuation commands',
  actions: {
    list: { description: 'List item costs', args: [] },
    get: { description: 'Get item cost', args: ['<sku>'] },
    set: {
      description: 'Set item cost inputs',
      args: [
        '<sku>',
        '[costMethod]',
        '[standardCost]',
        '[materialCost]',
        '[laborCost]',
        '[overheadCost]',
      ],
    },
    average: { description: 'Update average cost', args: ['<sku>', '<quantity>', '<unitCost>'] },
    'inventory-value': { description: 'Get total inventory value', args: [] },
  },
};

export default { execute, metadata };
