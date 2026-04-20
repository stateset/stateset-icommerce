/**
 * Store Credits Commands Module
 */

function parseAmount(value, usage) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed)) {
    throw new Error(usage);
  }
  return parsed;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const [customerId, status] = args;
      const credits = await commerce.storeCredits.list({ customerId, status });
      return formatCredits(credits, { output, jsonOutput });
    }

    case 'get': {
      const creditId = args[0];
      if (!creditId) throw new Error('Usage: store-credits get <creditId>');
      const credit = await commerce.storeCredits.get(creditId);
      if (!credit) throw new Error(`Store credit not found: ${creditId}`);
      return formatCredit(credit, { jsonOutput });
    }

    case 'create': {
      const [customerId, amountRaw, currency = 'USD', reason = 'other', expiresAt, ...noteParts] =
        args;
      if (!customerId || !amountRaw) {
        throw new Error(
          'Usage: store-credits create <customerId> <amount> [currency] [reason] [expiresAt] [note]',
        );
      }
      const credit = await commerce.storeCredits.create({
        customerId,
        amount: String(
          parseAmount(
            amountRaw,
            'Usage: store-credits create <customerId> <amount> [currency] [reason] [expiresAt] [note]',
          ),
        ),
        currency: currency.toUpperCase(),
        reason,
        expiresAt: expiresAt || undefined,
        note: noteParts.join(' ') || undefined,
      });
      return {
        credit,
        formatted: `Issued store credit ${credit.id}`,
      };
    }

    case 'adjust': {
      const [creditId, amountRaw, ...reasonParts] = args;
      if (!creditId || !amountRaw || reasonParts.length === 0) {
        throw new Error('Usage: store-credits adjust <creditId> <amount> <reason>');
      }
      const credit = await commerce.storeCredits.adjust({
        creditId,
        amount: String(
          parseAmount(amountRaw, 'Usage: store-credits adjust <creditId> <amount> <reason>'),
        ),
        reason: reasonParts.join(' '),
      });
      return {
        credit,
        formatted: `Adjusted store credit ${credit.id}`,
      };
    }

    case 'apply': {
      const [creditId, orderId, amountRaw] = args;
      if (!creditId || !orderId || !amountRaw) {
        throw new Error('Usage: store-credits apply <creditId> <orderId> <amount>');
      }
      const transaction = await commerce.storeCredits.apply({
        creditId,
        orderId,
        amount: String(
          parseAmount(amountRaw, 'Usage: store-credits apply <creditId> <orderId> <amount>'),
        ),
      });
      return {
        transaction,
        formatted: `Applied store credit ${creditId} to order ${orderId}`,
      };
    }

    default:
      throw new Error(
        `Unknown action: store-credits ${action}\n\n` +
          'Available actions:\n' +
          '  list [customerId] [status]                                     List store credits\n' +
          '  get <creditId>                                                 Get store credit\n' +
          '  create <customerId> <amount> [currency] [reason] [expiresAt] [note]\n' +
          '  adjust <creditId> <amount> <reason>                            Adjust credit balance\n' +
          '  apply <creditId> <orderId> <amount>                            Apply credit to order',
      );
  }
}

function formatCredits(credits, { output, jsonOutput }) {
  if (jsonOutput) return credits;
  if (credits.length === 0) return { formatted: 'No store credits found.' };
  const formatted = output.table(credits, [
    { key: 'id', header: 'ID' },
    { key: 'customerId', header: 'Customer' },
    { key: 'currentBalance', header: 'Balance', align: 'right' },
    { key: 'currency', header: 'Currency' },
    { key: 'reason', header: 'Reason' },
    { key: 'status', header: 'Status' },
  ]);
  return { credits, formatted };
}

function formatCredit(credit, { jsonOutput }) {
  if (jsonOutput) return credit;
  return {
    credit,
    formatted:
      `Store credit: ${credit.id}\n` +
      `${'-'.repeat(36)}\n` +
      `Customer:          ${credit.customerId}\n` +
      `Original amount:   ${credit.originalAmount} ${credit.currency}\n` +
      `Current balance:   ${credit.currentBalance} ${credit.currency}\n` +
      `Reason:            ${credit.reason}\n` +
      `Status:            ${credit.status}`,
  };
}

export const metadata = {
  name: 'store-credits',
  aliases: ['credits', 'credit'],
  description: 'Store credit issuance and application commands',
  actions: {
    list: { description: 'List store credits', args: ['[customerId]', '[status]'] },
    get: { description: 'Get store credit', args: ['<creditId>'] },
    create: {
      description: 'Issue store credit',
      args: ['<customerId>', '<amount>', '[currency]', '[reason]', '[expiresAt]', '[note]'],
    },
    adjust: { description: 'Adjust store credit', args: ['<creditId>', '<amount>', '<reason>'] },
    apply: { description: 'Apply store credit', args: ['<creditId>', '<orderId>', '<amount>'] },
  },
};

export default { execute, metadata };
