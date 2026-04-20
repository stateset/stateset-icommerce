/**
 * Accounts Receivable Commands Module
 */

function parseDays(value, usage) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 1) throw new Error(usage);
  return parsed;
}

function parseAmount(value, usage) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || parsed <= 0) throw new Error(usage);
  return parsed;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'aging': {
      const summary = await commerce.accountsReceivable.getAgingSummary();
      return jsonOutput
        ? summary
        : {
            summary,
            formatted:
              `Accounts receivable aging\n` +
              `${'-'.repeat(30)}\n` +
              `Current:      ${summary.current ?? 'N/A'}\n` +
              `1-30 days:    ${summary.days1to30 ?? 'N/A'}\n` +
              `31-60 days:   ${summary.days31to60 ?? 'N/A'}\n` +
              `61-90 days:   ${summary.days61to90 ?? 'N/A'}\n` +
              `90+ days:     ${summary.days90plus ?? 'N/A'}`,
          };
    }

    case 'outstanding': {
      const totalOutstanding = await commerce.accountsReceivable.getTotalOutstanding();
      return jsonOutput
        ? { totalOutstanding }
        : { formatted: `Accounts receivable total outstanding: ${totalOutstanding}` };
    }

    case 'dso': {
      const days = parseDays(args[0] || '30', 'Usage: accounts-receivable dso [days]');
      const dso = await commerce.accountsReceivable.getDso(days);
      return jsonOutput ? { days, dso } : { formatted: `DSO over ${days} days: ${dso}` };
    }

    case 'credit-memos': {
      const creditMemos = await commerce.accountsReceivable.listCreditMemos();
      return formatCreditMemos(creditMemos, { output, jsonOutput });
    }

    case 'credit-memo': {
      const creditMemoId = args[0];
      if (!creditMemoId) throw new Error('Usage: accounts-receivable credit-memo <creditMemoId>');
      const creditMemo = await commerce.accountsReceivable.getCreditMemo(creditMemoId);
      if (!creditMemo) throw new Error(`Credit memo not found: ${creditMemoId}`);
      return formatCreditMemo(creditMemo, { jsonOutput });
    }

    case 'create-credit-memo': {
      const [customerId, amountRaw, reason, originalInvoiceId, ...noteParts] = args;
      if (!customerId || !amountRaw || !reason) {
        throw new Error(
          'Usage: accounts-receivable create-credit-memo <customerId> <amount> <reason> [originalInvoiceId] [notes]',
        );
      }
      const creditMemo = await commerce.accountsReceivable.createCreditMemo({
        customerId,
        amount: parseAmount(
          amountRaw,
          'Usage: accounts-receivable create-credit-memo <customerId> <amount> <reason> [originalInvoiceId] [notes]',
        ),
        reason,
        originalInvoiceId: originalInvoiceId || undefined,
        notes: noteParts.join(' ') || undefined,
      });
      return {
        creditMemo,
        formatted: `Created credit memo ${creditMemo.id}`,
      };
    }

    case 'void-credit-memo': {
      const creditMemoId = args[0];
      if (!creditMemoId)
        throw new Error('Usage: accounts-receivable void-credit-memo <creditMemoId>');
      const creditMemo = await commerce.accountsReceivable.voidCreditMemo(creditMemoId);
      return { creditMemo, formatted: `Voided credit memo ${creditMemo.id}` };
    }

    case 'unapplied': {
      const customerId = args[0];
      if (!customerId) throw new Error('Usage: accounts-receivable unapplied <customerId>');
      const creditMemos = await commerce.accountsReceivable.getUnappliedCredits(customerId);
      return formatCreditMemos(creditMemos, { output, jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: accounts-receivable ${action}\n\n` +
          'Available actions:\n' +
          '  aging                                                                  Get AR aging summary\n' +
          '  outstanding                                                            Get AR total outstanding\n' +
          '  dso [days]                                                             Get days sales outstanding\n' +
          '  credit-memos                                                           List credit memos\n' +
          '  credit-memo <creditMemoId>                                             Get credit memo\n' +
          '  create-credit-memo <customerId> <amount> <reason> [originalInvoiceId] [notes]\n' +
          '  void-credit-memo <creditMemoId>                                        Void credit memo\n' +
          '  unapplied <customerId>                                                 List unapplied credits',
      );
  }
}

function formatCreditMemos(creditMemos, { output, jsonOutput }) {
  if (jsonOutput) return creditMemos;
  if (creditMemos.length === 0) return { formatted: 'No credit memos found.' };
  const formatted = output.table(creditMemos, [
    { key: 'id', header: 'ID' },
    { key: 'customerId', header: 'Customer' },
    { key: 'status', header: 'Status' },
    { key: 'amount', header: 'Amount', align: 'right' },
    { key: 'reason', header: 'Reason' },
  ]);
  return { creditMemos, formatted };
}

function formatCreditMemo(creditMemo, { jsonOutput }) {
  if (jsonOutput) return creditMemo;
  return {
    creditMemo,
    formatted:
      `Credit memo: ${creditMemo.id}\n` +
      `${'-'.repeat(34)}\n` +
      `Customer:       ${creditMemo.customerId}\n` +
      `Amount:         ${creditMemo.amount}\n` +
      `Status:         ${creditMemo.status}\n` +
      `Reason:         ${creditMemo.reason}\n` +
      `Invoice:        ${creditMemo.originalInvoiceId || 'N/A'}`,
  };
}

export const metadata = {
  name: 'accounts-receivable',
  aliases: ['ar', 'credit-memos'],
  description: 'Accounts receivable and credit memo commands',
  actions: {
    aging: { description: 'Get AR aging summary', args: [] },
    outstanding: { description: 'Get total outstanding', args: [] },
    dso: { description: 'Get days sales outstanding', args: ['[days]'] },
    'credit-memos': { description: 'List credit memos', args: [] },
    'credit-memo': { description: 'Get credit memo', args: ['<creditMemoId>'] },
    'create-credit-memo': {
      description: 'Create credit memo',
      args: ['<customerId>', '<amount>', '<reason>', '[originalInvoiceId]', '[notes]'],
    },
    'void-credit-memo': { description: 'Void credit memo', args: ['<creditMemoId>'] },
    unapplied: { description: 'List unapplied credits', args: ['<customerId>'] },
  },
};

export default { execute, metadata };
