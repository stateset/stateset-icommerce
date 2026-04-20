/**
 * Backorders Commands Module
 */

function parseQuantity(value, usage) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || parsed <= 0) throw new Error(usage);
  return parsed;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const backorders = await commerce.backorder.listBackorders();
      return formatBackorders(backorders, { output, jsonOutput });
    }

    case 'get': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: backorders get <backorderId|backorderNumber>');
      const backorder = identifier.includes('-')
        ? await commerce.backorder.getBackorder(identifier)
        : await commerce.backorder.getBackorderByNumber(identifier);
      if (!backorder) throw new Error(`Backorder not found: ${identifier}`);
      return formatBackorder(backorder, { jsonOutput });
    }

    case 'create': {
      const [orderId, customerId, sku, quantityRaw, priority, ...noteParts] = args;
      if (!orderId || !customerId || !sku || !quantityRaw) {
        throw new Error(
          'Usage: backorders create <orderId> <customerId> <sku> <quantity> [priority] [notes]',
        );
      }
      const backorder = await commerce.backorder.createBackorder({
        orderId,
        customerId,
        sku,
        quantity: parseQuantity(
          quantityRaw,
          'Usage: backorders create <orderId> <customerId> <sku> <quantity> [priority] [notes]',
        ),
        priority: priority || undefined,
        notes: noteParts.join(' ') || undefined,
      });
      return {
        backorder,
        formatted: `Created backorder ${backorder.backorderNumber || backorder.id}`,
      };
    }

    case 'cancel': {
      const backorderId = args[0];
      if (!backorderId) throw new Error('Usage: backorders cancel <backorderId>');
      const backorder = await commerce.backorder.cancelBackorder(backorderId);
      return {
        backorder,
        formatted: `Cancelled backorder ${backorder.backorderNumber || backorder.id}`,
      };
    }

    case 'order': {
      const orderId = args[0];
      if (!orderId) throw new Error('Usage: backorders order <orderId>');
      const backorders = await commerce.backorder.getBackordersForOrder(orderId);
      return formatBackorders(backorders, { output, jsonOutput });
    }

    case 'sku': {
      const sku = args[0];
      if (!sku) throw new Error('Usage: backorders sku <sku>');
      const backorders = await commerce.backorder.getBackordersForSku(sku);
      return formatBackorders(backorders, { output, jsonOutput });
    }

    case 'overdue': {
      const backorders = await commerce.backorder.getOverdueBackorders();
      return formatBackorders(backorders, { output, jsonOutput });
    }

    case 'summary': {
      const summary = await commerce.backorder.getSummary();
      return jsonOutput
        ? summary
        : {
            summary,
            formatted:
              `Backorder summary\n` +
              `${'-'.repeat(24)}\n` +
              `Pending:      ${summary.pending ?? 'N/A'}\n` +
              `Overdue:      ${summary.overdue ?? 'N/A'}\n` +
              `Total qty:    ${summary.totalQuantity ?? 'N/A'}`,
          };
    }

    case 'count': {
      const count = await commerce.backorder.countPending();
      return { count, formatted: `Pending backorder count: ${count}` };
    }

    default:
      throw new Error(
        `Unknown action: backorders ${action}\n\n` +
          'Available actions:\n' +
          '  list                                                                 List backorders\n' +
          '  get <backorderId|backorderNumber>                                    Get backorder\n' +
          '  create <orderId> <customerId> <sku> <quantity> [priority] [notes]    Create backorder\n' +
          '  cancel <backorderId>                                                 Cancel backorder\n' +
          '  order <orderId>                                                      List backorders for order\n' +
          '  sku <sku>                                                            List backorders for SKU\n' +
          '  overdue                                                              List overdue backorders\n' +
          '  summary                                                              Get backorder summary\n' +
          '  count                                                                Count pending backorders',
      );
  }
}

function formatBackorders(backorders, { output, jsonOutput }) {
  if (jsonOutput) return backorders;
  if (backorders.length === 0) return { formatted: 'No backorders found.' };
  const formatted = output.table(backorders, [
    { key: 'id', header: 'ID' },
    { key: 'backorderNumber', header: 'Backorder #' },
    { key: 'orderId', header: 'Order' },
    { key: 'sku', header: 'SKU' },
    { key: 'quantity', header: 'Qty', align: 'right' },
    { key: 'status', header: 'Status' },
  ]);
  return { backorders, formatted };
}

function formatBackorder(backorder, { jsonOutput }) {
  if (jsonOutput) return backorder;
  return {
    backorder,
    formatted:
      `Backorder: ${backorder.backorderNumber || backorder.id}\n` +
      `${'-'.repeat(38)}\n` +
      `Order:          ${backorder.orderId}\n` +
      `Customer:       ${backorder.customerId}\n` +
      `SKU:            ${backorder.sku}\n` +
      `Quantity:       ${backorder.quantity}\n` +
      `Priority:       ${backorder.priority || 'N/A'}\n` +
      `Status:         ${backorder.status}`,
  };
}

export const metadata = {
  name: 'backorders',
  aliases: ['bo', 'backorder'],
  description: 'Backorder management commands',
  actions: {
    list: { description: 'List backorders', args: [] },
    get: { description: 'Get backorder', args: ['<backorderId|backorderNumber>'] },
    create: {
      description: 'Create backorder',
      args: ['<orderId>', '<customerId>', '<sku>', '<quantity>', '[priority]', '[notes]'],
    },
    cancel: { description: 'Cancel backorder', args: ['<backorderId>'] },
    order: { description: 'List backorders for order', args: ['<orderId>'] },
    sku: { description: 'List backorders for SKU', args: ['<sku>'] },
    overdue: { description: 'List overdue backorders', args: [] },
    summary: { description: 'Get backorder summary', args: [] },
    count: { description: 'Count pending backorders', args: [] },
  },
};

export default { execute, metadata };
