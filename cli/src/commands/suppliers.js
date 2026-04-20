/**
 * Suppliers Commands Module
 */

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const suppliers = await commerce.purchaseOrders.listSuppliers();
      return formatSupplierList(suppliers, { output, jsonOutput });
    }

    case 'get': {
      const supplierId = args[0];
      if (!supplierId) throw new Error('Usage: suppliers get <supplierId>');
      const supplier = await commerce.purchaseOrders.getSupplier(supplierId);
      if (!supplier) throw new Error(`Supplier not found: ${supplierId}`);
      return formatSupplierDetail(supplier, { jsonOutput });
    }

    case 'create': {
      const [name, email, phone, address] = args;
      if (!name) throw new Error('Usage: suppliers create <name> [email] [phone] [address]');
      const supplier = await commerce.purchaseOrders.createSupplier({
        name,
        email,
        phone,
        address,
      });
      return {
        supplier,
        formatted: `Created supplier ${supplier.name || supplier.id}`,
      };
    }

    case 'orders': {
      const [supplierId, status] = args;
      const purchaseOrders = await commerce.purchaseOrders.list();
      const filtered = purchaseOrders.filter(
        (purchaseOrder) =>
          (!supplierId || purchaseOrder.supplierId === supplierId) &&
          (!status || purchaseOrder.status === status),
      );
      return formatPurchaseOrderList(filtered, { output, jsonOutput });
    }

    case 'order': {
      const purchaseOrderId = args[0];
      if (!purchaseOrderId) throw new Error('Usage: suppliers order <purchaseOrderId>');
      const purchaseOrder = await commerce.purchaseOrders.get(purchaseOrderId);
      if (!purchaseOrder) throw new Error(`Purchase order not found: ${purchaseOrderId}`);
      return formatPurchaseOrderDetail(purchaseOrder, { jsonOutput });
    }

    case 'create-order': {
      const [supplierId, itemsJson, ...noteParts] = args;
      if (!supplierId || !itemsJson) {
        throw new Error('Usage: suppliers create-order <supplierId> <itemsJson> [notes]');
      }
      const purchaseOrder = await commerce.purchaseOrders.create({
        supplierId,
        items: parseJsonArg(itemsJson, 'items'),
        notes: noteParts.join(' ') || undefined,
      });
      return {
        purchaseOrder,
        formatted: `Created purchase order ${purchaseOrder.id}`,
      };
    }

    case 'submit': {
      const purchaseOrderId = args[0];
      if (!purchaseOrderId) throw new Error('Usage: suppliers submit <purchaseOrderId>');
      const purchaseOrder = await commerce.purchaseOrders.submit(purchaseOrderId);
      return {
        purchaseOrder,
        formatted: `Submitted purchase order ${purchaseOrder.id}`,
      };
    }

    case 'approve': {
      const [purchaseOrderId, approvedBy] = args;
      if (!purchaseOrderId || !approvedBy) {
        throw new Error('Usage: suppliers approve <purchaseOrderId> <approvedBy>');
      }
      const purchaseOrder = await commerce.purchaseOrders.approve(purchaseOrderId, approvedBy);
      return {
        purchaseOrder,
        formatted: `Approved purchase order ${purchaseOrder.id}`,
      };
    }

    case 'send': {
      const purchaseOrderId = args[0];
      if (!purchaseOrderId) throw new Error('Usage: suppliers send <purchaseOrderId>');
      const purchaseOrder = await commerce.purchaseOrders.send(purchaseOrderId);
      return {
        purchaseOrder,
        formatted: `Sent purchase order ${purchaseOrder.id}`,
      };
    }

    case 'cancel': {
      const purchaseOrderId = args[0];
      if (!purchaseOrderId) throw new Error('Usage: suppliers cancel <purchaseOrderId>');
      const purchaseOrder = await commerce.purchaseOrders.cancel(purchaseOrderId);
      return {
        purchaseOrder,
        formatted: `Cancelled purchase order ${purchaseOrder.id}`,
      };
    }

    default:
      throw new Error(
        `Unknown action: suppliers ${action}\n\n` +
          'Available actions:\n' +
          '  list                                         List suppliers\n' +
          '  get <supplierId>                             Get supplier details\n' +
          '  create <name> [email] [phone] [address]      Create supplier\n' +
          '  orders [supplierId] [status]                 List purchase orders\n' +
          '  order <purchaseOrderId>                      Get purchase order details\n' +
          '  create-order <supplierId> <itemsJson> [notes] Create purchase order\n' +
          '  submit <purchaseOrderId>                     Submit purchase order\n' +
          '  approve <purchaseOrderId> <approvedBy>       Approve purchase order\n' +
          '  send <purchaseOrderId>                       Send purchase order\n' +
          '  cancel <purchaseOrderId>                     Cancel purchase order',
      );
  }
}

function formatSupplierList(suppliers, { output, jsonOutput }) {
  if (jsonOutput) return suppliers;
  if (suppliers.length === 0) return { formatted: 'No suppliers found.' };
  const formatted = output.table(suppliers, [
    { key: 'id', header: 'ID' },
    { key: 'name', header: 'Name' },
    { key: 'email', header: 'Email' },
    { key: 'phone', header: 'Phone' },
  ]);
  return { suppliers, formatted };
}

function formatSupplierDetail(supplier, { jsonOutput }) {
  if (jsonOutput) return supplier;
  return {
    supplier,
    formatted:
      `Supplier: ${supplier.name}\n` +
      `${'-'.repeat(36)}\n` +
      `ID:          ${supplier.id}\n` +
      `Email:       ${supplier.email || 'N/A'}\n` +
      `Phone:       ${supplier.phone || 'N/A'}\n` +
      `Address:     ${supplier.address || 'N/A'}`,
  };
}

function formatPurchaseOrderList(purchaseOrders, { output, jsonOutput }) {
  if (jsonOutput) return purchaseOrders;
  if (purchaseOrders.length === 0) return { formatted: 'No purchase orders found.' };
  const formatted = output.table(purchaseOrders, [
    { key: 'id', header: 'PO' },
    { key: 'supplierId', header: 'Supplier' },
    { key: 'status', header: 'Status' },
    { key: 'totalAmount', header: 'Total', align: 'right' },
    { key: 'currency', header: 'Currency' },
  ]);
  return { purchaseOrders, formatted };
}

function formatPurchaseOrderDetail(purchaseOrder, { jsonOutput }) {
  if (jsonOutput) return purchaseOrder;
  return {
    purchaseOrder,
    formatted:
      `Purchase order: ${purchaseOrder.id}\n` +
      `${'-'.repeat(42)}\n` +
      `Supplier:     ${purchaseOrder.supplierId}\n` +
      `Status:       ${purchaseOrder.status}\n` +
      `Total:        ${purchaseOrder.totalAmount || 'N/A'} ${purchaseOrder.currency || ''}`.trimEnd() +
      `\nCreated:      ${purchaseOrder.createdAt || 'N/A'}`,
  };
}

export const metadata = {
  name: 'suppliers',
  aliases: ['supp', 'po'],
  description: 'Suppliers and purchase order commands',
  actions: {
    list: { description: 'List suppliers', args: [] },
    get: { description: 'Get supplier', args: ['<supplierId>'] },
    create: { description: 'Create supplier', args: ['<name>', '[email]', '[phone]', '[address]'] },
    orders: { description: 'List purchase orders', args: ['[supplierId]', '[status]'] },
    order: { description: 'Get purchase order', args: ['<purchaseOrderId>'] },
    'create-order': {
      description: 'Create purchase order',
      args: ['<supplierId>', '<itemsJson>', '[notes]'],
    },
    submit: { description: 'Submit purchase order', args: ['<purchaseOrderId>'] },
    approve: { description: 'Approve purchase order', args: ['<purchaseOrderId>', '<approvedBy>'] },
    send: { description: 'Send purchase order', args: ['<purchaseOrderId>'] },
    cancel: { description: 'Cancel purchase order', args: ['<purchaseOrderId>'] },
  },
};

export default { execute, metadata };
