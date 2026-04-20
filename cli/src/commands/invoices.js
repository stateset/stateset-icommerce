/**
 * Invoices Commands Module
 */

function parseAmount(value) {
  const amount = Number.parseFloat(value);
  if (!Number.isFinite(amount) || amount <= 0) {
    throw new Error('Amount must be a positive number');
  }
  return amount;
}

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
      const [customerId, status] = args;
      const invoices = await commerce.invoices.list();
      const filtered = invoices.filter(
        (invoice) =>
          (!customerId || invoice.customerId === customerId) &&
          (!status || invoice.status === status),
      );
      return formatInvoiceList(filtered, { output, jsonOutput });
    }

    case 'get': {
      const invoiceId = args[0];
      if (!invoiceId) throw new Error('Usage: invoices get <invoiceId>');
      const invoice = await commerce.invoices.get(invoiceId);
      if (!invoice) throw new Error(`Invoice not found: ${invoiceId}`);
      return formatInvoiceDetail(invoice, { jsonOutput });
    }

    case 'create': {
      const [customerId, itemsJson, orderId, dueDate, ...noteParts] = args;
      if (!customerId || !itemsJson) {
        throw new Error(
          'Usage: invoices create <customerId> <itemsJson> [orderId] [dueDate] [notes]',
        );
      }
      const invoice = await commerce.invoices.create({
        customerId,
        items: parseJsonArg(itemsJson, 'items'),
        orderId,
        dueDate,
        notes: noteParts.join(' ') || undefined,
      });
      return {
        invoice,
        formatted: `Created invoice ${invoice.id}`,
      };
    }

    case 'send': {
      const invoiceId = args[0];
      if (!invoiceId) throw new Error('Usage: invoices send <invoiceId>');
      const invoice = await commerce.invoices.send(invoiceId);
      return {
        invoice,
        formatted: `Sent invoice ${invoice.id}`,
      };
    }

    case 'void': {
      const invoiceId = args[0];
      if (!invoiceId) throw new Error('Usage: invoices void <invoiceId>');
      const invoice = await commerce.invoices.void(invoiceId);
      return {
        invoice,
        formatted: `Voided invoice ${invoice.id}`,
      };
    }

    case 'pay': {
      const [invoiceId, amountRaw, paymentMethod, reference] = args;
      if (!invoiceId || !amountRaw) {
        throw new Error('Usage: invoices pay <invoiceId> <amount> [paymentMethod] [reference]');
      }
      const invoice = await commerce.invoices.recordPayment(invoiceId, {
        amount: parseAmount(amountRaw),
        paymentMethod,
        reference,
      });
      return {
        invoice,
        formatted: `Recorded payment on invoice ${invoice.id}`,
      };
    }

    case 'overdue': {
      const invoices = await commerce.invoices.getOverdue();
      return formatInvoiceList(invoices, { output, jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: invoices ${action}\n\n` +
          'Available actions:\n' +
          '  list [customerId] [status]                    List invoices\n' +
          '  get <invoiceId>                               Get invoice details\n' +
          '  create <customerId> <itemsJson> [orderId] [dueDate] [notes] Create invoice\n' +
          '  send <invoiceId>                              Send invoice\n' +
          '  void <invoiceId>                              Void invoice\n' +
          '  pay <invoiceId> <amount> [paymentMethod] [reference] Record payment\n' +
          '  overdue                                       List overdue invoices',
      );
  }
}

function formatInvoiceList(invoices, { output, jsonOutput }) {
  if (jsonOutput) return invoices;
  if (invoices.length === 0) return { formatted: 'No invoices found.' };
  const formatted = output.table(invoices, [
    { key: 'id', header: 'Invoice' },
    { key: 'customerId', header: 'Customer' },
    { key: 'status', header: 'Status' },
    { key: 'total', header: 'Total', align: 'right' },
    { key: 'currency', header: 'Currency' },
    { key: 'dueDate', header: 'Due' },
  ]);
  return { invoices, formatted };
}

function formatInvoiceDetail(invoice, { jsonOutput }) {
  if (jsonOutput) return invoice;
  return {
    invoice,
    formatted:
      `Invoice: ${invoice.id}\n` +
      `${'-'.repeat(34)}\n` +
      `Customer:     ${invoice.customerId}\n` +
      `Status:       ${invoice.status}\n` +
      `Total:        ${invoice.total || 'N/A'} ${invoice.currency || ''}`.trimEnd() +
      `\nOrder:        ${invoice.orderId || 'N/A'}\n` +
      `Due date:     ${invoice.dueDate || 'N/A'}`,
  };
}

export const metadata = {
  name: 'invoices',
  aliases: ['invc', 'bill'],
  description: 'Invoice issuance and payment commands',
  actions: {
    list: { description: 'List invoices', args: ['[customerId]', '[status]'] },
    get: { description: 'Get invoice', args: ['<invoiceId>'] },
    create: {
      description: 'Create invoice',
      args: ['<customerId>', '<itemsJson>', '[orderId]', '[dueDate]', '[notes]'],
    },
    send: { description: 'Send invoice', args: ['<invoiceId>'] },
    void: { description: 'Void invoice', args: ['<invoiceId>'] },
    pay: {
      description: 'Record invoice payment',
      args: ['<invoiceId>', '<amount>', '[paymentMethod]', '[reference]'],
    },
    overdue: { description: 'List overdue invoices', args: [] },
  },
};

export default { execute, metadata };
