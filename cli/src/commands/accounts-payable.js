/**
 * Accounts Payable Commands Module
 */

function parseIntArg(value, usage) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 0) throw new Error(usage);
  return parsed;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'bills': {
      const bills = await commerce.accountsPayable.listBills();
      return formatBills(bills, { output, jsonOutput });
    }

    case 'bill': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: accounts-payable bill <billId|billNumber>');
      const bill = identifier.includes('-')
        ? await commerce.accountsPayable.getBill(identifier)
        : await commerce.accountsPayable.getBillByNumber(identifier);
      if (!bill) throw new Error(`Bill not found: ${identifier}`);
      return formatBill(bill, { jsonOutput });
    }

    case 'create-bill': {
      const [supplierId, dueDate, paymentTerms, referenceNumber, ...noteParts] = args;
      if (!supplierId || !dueDate) {
        throw new Error(
          'Usage: accounts-payable create-bill <supplierId> <dueDate> [paymentTerms] [referenceNumber] [notes]',
        );
      }
      const bill = await commerce.accountsPayable.createBill({
        supplierId,
        dueDate,
        paymentTerms: paymentTerms || undefined,
        referenceNumber: referenceNumber || undefined,
        notes: noteParts.join(' ') || undefined,
      });
      return {
        bill,
        formatted: `Created AP bill ${bill.billNumber || bill.id}`,
      };
    }

    case 'approve-bill': {
      const billId = args[0];
      if (!billId) throw new Error('Usage: accounts-payable approve-bill <billId>');
      const bill = await commerce.accountsPayable.approveBill(billId);
      return { bill, formatted: `Approved AP bill ${bill.billNumber || bill.id}` };
    }

    case 'cancel-bill': {
      const billId = args[0];
      if (!billId) throw new Error('Usage: accounts-payable cancel-bill <billId>');
      const bill = await commerce.accountsPayable.cancelBill(billId);
      return { bill, formatted: `Cancelled AP bill ${bill.billNumber || bill.id}` };
    }

    case 'overdue': {
      const bills = await commerce.accountsPayable.getOverdueBills();
      return formatBills(bills, { output, jsonOutput });
    }

    case 'due-soon': {
      const daysRaw = args[0] || '7';
      const bills = await commerce.accountsPayable.getBillsDueSoon(
        parseIntArg(daysRaw, 'Usage: accounts-payable due-soon [days]'),
      );
      return formatBills(bills, { output, jsonOutput });
    }

    case 'aging': {
      const summary = await commerce.accountsPayable.getAgingSummary();
      return jsonOutput
        ? summary
        : {
            summary,
            formatted:
              `Accounts payable aging\n` +
              `${'-'.repeat(28)}\n` +
              `Current:      ${summary.current ?? 'N/A'}\n` +
              `1-30 days:    ${summary.days1to30 ?? 'N/A'}\n` +
              `31-60 days:   ${summary.days31to60 ?? 'N/A'}\n` +
              `61-90 days:   ${summary.days61to90 ?? 'N/A'}\n` +
              `90+ days:     ${summary.days90plus ?? 'N/A'}`,
          };
    }

    case 'outstanding': {
      const totalOutstanding = await commerce.accountsPayable.getTotalOutstanding();
      return jsonOutput
        ? { totalOutstanding }
        : { formatted: `Accounts payable total outstanding: ${totalOutstanding}` };
    }

    case 'count': {
      const count = await commerce.accountsPayable.countBills();
      return { count, formatted: `Accounts payable bill count: ${count}` };
    }

    default:
      throw new Error(
        `Unknown action: accounts-payable ${action}\n\n` +
          'Available actions:\n' +
          '  bills                                                                  List AP bills\n' +
          '  bill <billId|billNumber>                                               Get AP bill\n' +
          '  create-bill <supplierId> <dueDate> [paymentTerms] [referenceNumber] [notes]\n' +
          '  approve-bill <billId>                                                  Approve bill\n' +
          '  cancel-bill <billId>                                                   Cancel bill\n' +
          '  overdue                                                                List overdue bills\n' +
          '  due-soon [days]                                                        List bills due soon\n' +
          '  aging                                                                  Get AP aging summary\n' +
          '  outstanding                                                            Get AP total outstanding\n' +
          '  count                                                                  Count AP bills',
      );
  }
}

function formatBills(bills, { output, jsonOutput }) {
  if (jsonOutput) return bills;
  if (bills.length === 0) return { formatted: 'No accounts payable bills found.' };
  const formatted = output.table(bills, [
    { key: 'id', header: 'ID' },
    { key: 'billNumber', header: 'Bill #' },
    { key: 'supplierId', header: 'Supplier' },
    { key: 'status', header: 'Status' },
    { key: 'totalAmount', header: 'Total', align: 'right' },
    { key: 'dueDate', header: 'Due Date' },
  ]);
  return { bills, formatted };
}

function formatBill(bill, { jsonOutput }) {
  if (jsonOutput) return bill;
  return {
    bill,
    formatted:
      `AP bill: ${bill.billNumber || bill.id}\n` +
      `${'-'.repeat(34)}\n` +
      `ID:            ${bill.id}\n` +
      `Supplier:      ${bill.supplierId}\n` +
      `Status:        ${bill.status}\n` +
      `Due date:      ${bill.dueDate}\n` +
      `Total:         ${bill.totalAmount ?? 'N/A'}\n` +
      `Reference:     ${bill.referenceNumber || 'N/A'}`,
  };
}

export const metadata = {
  name: 'accounts-payable',
  aliases: ['ap', 'bills'],
  description: 'Accounts payable bills and aging commands',
  actions: {
    bills: { description: 'List AP bills', args: [] },
    bill: { description: 'Get AP bill', args: ['<billId|billNumber>'] },
    'create-bill': {
      description: 'Create AP bill',
      args: ['<supplierId>', '<dueDate>', '[paymentTerms]', '[referenceNumber]', '[notes]'],
    },
    'approve-bill': { description: 'Approve bill', args: ['<billId>'] },
    'cancel-bill': { description: 'Cancel bill', args: ['<billId>'] },
    overdue: { description: 'List overdue bills', args: [] },
    'due-soon': { description: 'List bills due soon', args: ['[days]'] },
    aging: { description: 'Get AP aging summary', args: [] },
    outstanding: { description: 'Get total outstanding', args: [] },
    count: { description: 'Count AP bills', args: [] },
  },
};

export default { execute, metadata };
