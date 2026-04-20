/**
 * Receiving Commands Module
 */

function parseIntArg(value, usage) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(usage);
  }
  return parsed;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const receipts = await commerce.receiving.listReceipts();
      return formatReceipts(receipts, { output, jsonOutput });
    }

    case 'get': {
      const [identifier] = args;
      if (!identifier) throw new Error('Usage: receiving get <receiptId|receiptNumber>');
      const receipt = identifier.includes('-')
        ? await commerce.receiving.getReceipt(identifier)
        : await commerce.receiving.getReceiptByNumber(identifier);
      if (!receipt) throw new Error(`Receipt not found: ${identifier}`);
      return formatReceipt(receipt, { jsonOutput });
    }

    case 'create': {
      const [receiptType, warehouseIdRaw, purchaseOrderId, carrier, trackingNumber] = args;
      if (!receiptType || !warehouseIdRaw) {
        throw new Error(
          'Usage: receiving create <receiptType> <warehouseId> [purchaseOrderId] [carrier] [trackingNumber]',
        );
      }
      const receipt = await commerce.receiving.createReceipt({
        receiptType,
        warehouseId: parseIntArg(
          warehouseIdRaw,
          'Usage: receiving create <receiptType> <warehouseId> [purchaseOrderId] [carrier] [trackingNumber]',
        ),
        purchaseOrderId: purchaseOrderId || undefined,
        carrier: carrier || undefined,
        trackingNumber: trackingNumber || undefined,
      });
      return {
        receipt,
        formatted: `Created receipt ${receipt.receiptNumber || receipt.id}`,
      };
    }

    case 'from-po': {
      const [purchaseOrderId, warehouseIdRaw] = args;
      if (!purchaseOrderId || !warehouseIdRaw) {
        throw new Error('Usage: receiving from-po <purchaseOrderId> <warehouseId>');
      }
      const receipt = await commerce.receiving.createReceiptFromPo(
        purchaseOrderId,
        parseIntArg(warehouseIdRaw, 'Usage: receiving from-po <purchaseOrderId> <warehouseId>'),
      );
      return {
        receipt,
        formatted: `Created receipt ${receipt.receiptNumber || receipt.id} from purchase order ${purchaseOrderId}`,
      };
    }

    case 'start': {
      const [receiptId] = args;
      if (!receiptId) throw new Error('Usage: receiving start <receiptId>');
      const receipt = await commerce.receiving.startReceiving(receiptId);
      return {
        receipt,
        formatted: `Started receiving for receipt ${receipt.receiptNumber || receipt.id}`,
      };
    }

    case 'complete': {
      const [receiptId] = args;
      if (!receiptId) throw new Error('Usage: receiving complete <receiptId>');
      const receipt = await commerce.receiving.completeReceiving(receiptId);
      return {
        receipt,
        formatted: `Completed receiving for receipt ${receipt.receiptNumber || receipt.id}`,
      };
    }

    case 'cancel': {
      const [receiptId] = args;
      if (!receiptId) throw new Error('Usage: receiving cancel <receiptId>');
      const receipt = await commerce.receiving.cancelReceipt(receiptId);
      return {
        receipt,
        formatted: `Cancelled receipt ${receipt.receiptNumber || receipt.id}`,
      };
    }

    case 'count': {
      const count = await commerce.receiving.countReceipts();
      return { count, formatted: `Receipt count: ${count}` };
    }

    default:
      throw new Error(
        `Unknown action: receiving ${action}\n\n` +
          'Available actions:\n' +
          '  list                                                                     List receipts\n' +
          '  get <receiptId|receiptNumber>                                            Get receipt\n' +
          '  create <receiptType> <warehouseId> [purchaseOrderId] [carrier] [trackingNumber]\n' +
          '  from-po <purchaseOrderId> <warehouseId>                                  Create receipt from purchase order\n' +
          '  start <receiptId>                                                        Start receiving\n' +
          '  complete <receiptId>                                                     Complete receiving\n' +
          '  cancel <receiptId>                                                       Cancel receipt\n' +
          '  count                                                                    Count receipts',
      );
  }
}

function formatReceipts(receipts, { output, jsonOutput }) {
  if (jsonOutput) return receipts;
  if (receipts.length === 0) return { formatted: 'No receipts found.' };
  const formatted = output.table(receipts, [
    { key: 'id', header: 'ID' },
    { key: 'receiptNumber', header: 'Receipt #' },
    { key: 'receiptType', header: 'Type' },
    { key: 'warehouseId', header: 'Warehouse', align: 'right' },
    { key: 'status', header: 'Status' },
    { key: 'purchaseOrderId', header: 'PO' },
  ]);
  return { receipts, formatted };
}

function formatReceipt(receipt, { jsonOutput }) {
  if (jsonOutput) return receipt;
  return {
    receipt,
    formatted:
      `Receipt: ${receipt.receiptNumber || receipt.id}\n` +
      `${'-'.repeat(36)}\n` +
      `ID:            ${receipt.id}\n` +
      `Type:          ${receipt.receiptType}\n` +
      `Warehouse:     ${receipt.warehouseId}\n` +
      `Status:        ${receipt.status}\n` +
      `PO:            ${receipt.purchaseOrderId || 'N/A'}\n` +
      `Carrier:       ${receipt.carrier || 'N/A'}\n` +
      `Tracking:      ${receipt.trackingNumber || 'N/A'}`,
  };
}

export const metadata = {
  name: 'receiving',
  aliases: ['receipts', 'recv'],
  description: 'Inbound receipts and receiving workflow commands',
  actions: {
    list: { description: 'List receipts', args: [] },
    get: { description: 'Get receipt', args: ['<receiptId|receiptNumber>'] },
    create: {
      description: 'Create receipt',
      args: [
        '<receiptType>',
        '<warehouseId>',
        '[purchaseOrderId]',
        '[carrier]',
        '[trackingNumber]',
      ],
    },
    'from-po': {
      description: 'Create receipt from purchase order',
      args: ['<purchaseOrderId>', '<warehouseId>'],
    },
    start: { description: 'Start receiving', args: ['<receiptId>'] },
    complete: { description: 'Complete receiving', args: ['<receiptId>'] },
    cancel: { description: 'Cancel receipt', args: ['<receiptId>'] },
    count: { description: 'Count receipts', args: [] },
  },
};

export default { execute, metadata };
