/**
 * Gift Cards Commands Module
 */

function parseAmount(value, usage) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(usage);
  }
  return parsed;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const [status, customerId] = args;
      const giftCards = await commerce.giftCards.list({ status, customerId });
      return formatGiftCards(giftCards, { output, jsonOutput });
    }

    case 'get': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: gift-cards get <giftCardId|code>');
      const giftCard = await commerce.giftCards.get(identifier);
      if (!giftCard) throw new Error(`Gift card not found: ${identifier}`);
      return formatGiftCard(giftCard, { jsonOutput });
    }

    case 'create': {
      const [
        initialBalanceRaw,
        currency = 'USD',
        customerId,
        recipientEmail,
        recipientName,
        ...messageParts
      ] = args;
      if (!initialBalanceRaw) {
        throw new Error(
          'Usage: gift-cards create <initialBalance> [currency] [customerId] [recipientEmail] [recipientName] [message]',
        );
      }
      const giftCard = await commerce.giftCards.create({
        initialBalance: String(
          parseAmount(
            initialBalanceRaw,
            'Usage: gift-cards create <initialBalance> [currency] [customerId] [recipientEmail] [recipientName] [message]',
          ),
        ),
        currency: currency.toUpperCase(),
        customerId: customerId || undefined,
        recipientEmail: recipientEmail || undefined,
        recipientName: recipientName || undefined,
        message: messageParts.join(' ') || undefined,
      });
      return {
        giftCard,
        formatted: `Created gift card ${giftCard.code || giftCard.id}`,
      };
    }

    case 'charge': {
      const [giftCardId, amountRaw, orderId, ...noteParts] = args;
      if (!giftCardId || !amountRaw) {
        throw new Error('Usage: gift-cards charge <giftCardId> <amount> [orderId] [note]');
      }
      const transaction = await commerce.giftCards.charge({
        giftCardId,
        amount: String(
          parseAmount(amountRaw, 'Usage: gift-cards charge <giftCardId> <amount> [orderId] [note]'),
        ),
        orderId: orderId || undefined,
        note: noteParts.join(' ') || undefined,
      });
      return {
        transaction,
        formatted: `Charged gift card ${giftCardId}`,
      };
    }

    case 'refund': {
      const [giftCardId, amountRaw, orderId, ...reasonParts] = args;
      if (!giftCardId || !amountRaw) {
        throw new Error('Usage: gift-cards refund <giftCardId> <amount> [orderId] [reason]');
      }
      const transaction = await commerce.giftCards.refund({
        giftCardId,
        amount: String(
          parseAmount(
            amountRaw,
            'Usage: gift-cards refund <giftCardId> <amount> [orderId] [reason]',
          ),
        ),
        orderId: orderId || undefined,
        reason: reasonParts.join(' ') || undefined,
      });
      return {
        transaction,
        formatted: `Refunded gift card ${giftCardId}`,
      };
    }

    case 'disable': {
      const [giftCardId, ...reasonParts] = args;
      if (!giftCardId) throw new Error('Usage: gift-cards disable <giftCardId> [reason]');
      const giftCard = await commerce.giftCards.disable(
        giftCardId,
        reasonParts.join(' ') || undefined,
      );
      return {
        giftCard,
        formatted: `Disabled gift card ${giftCard.code || giftCard.id}`,
      };
    }

    case 'balance': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: gift-cards balance <giftCardId|code>');
      const giftCard = await commerce.giftCards.get(identifier);
      if (!giftCard) throw new Error(`Gift card not found: ${identifier}`);
      return jsonOutput
        ? {
            giftCardId: giftCard.id,
            code: giftCard.code,
            currentBalance: giftCard.currentBalance,
            currency: giftCard.currency,
            status: giftCard.status,
          }
        : {
            giftCard,
            formatted: `Gift card ${giftCard.code} balance: ${giftCard.currentBalance} ${giftCard.currency}`,
          };
    }

    default:
      throw new Error(
        `Unknown action: gift-cards ${action}\n\n` +
          'Available actions:\n' +
          '  list [status] [customerId]                                               List gift cards\n' +
          '  get <giftCardId|code>                                                    Get gift card\n' +
          '  create <initialBalance> [currency] [customerId] [recipientEmail] [recipientName] [message]\n' +
          '  charge <giftCardId> <amount> [orderId] [note]                            Charge gift card\n' +
          '  refund <giftCardId> <amount> [orderId] [reason]                          Refund to gift card\n' +
          '  disable <giftCardId> [reason]                                            Disable gift card\n' +
          '  balance <giftCardId|code>                                                Check balance',
      );
  }
}

function formatGiftCards(giftCards, { output, jsonOutput }) {
  if (jsonOutput) return giftCards;
  if (giftCards.length === 0) return { formatted: 'No gift cards found.' };
  const formatted = output.table(giftCards, [
    { key: 'id', header: 'ID' },
    { key: 'code', header: 'Code' },
    { key: 'currentBalance', header: 'Balance', align: 'right' },
    { key: 'currency', header: 'Currency' },
    { key: 'status', header: 'Status' },
    { key: 'customerId', header: 'Customer' },
  ]);
  return { giftCards, formatted };
}

function formatGiftCard(giftCard, { jsonOutput }) {
  if (jsonOutput) return giftCard;
  return {
    giftCard,
    formatted:
      `Gift card: ${giftCard.code}\n` +
      `${'-'.repeat(34)}\n` +
      `ID:               ${giftCard.id}\n` +
      `Initial balance:  ${giftCard.initialBalance} ${giftCard.currency}\n` +
      `Current balance:  ${giftCard.currentBalance} ${giftCard.currency}\n` +
      `Status:           ${giftCard.status}\n` +
      `Customer:         ${giftCard.customerId || 'N/A'}`,
  };
}

export const metadata = {
  name: 'gift-cards',
  aliases: ['giftcard', 'gc'],
  description: 'Gift card issuance, balance, and redemption commands',
  actions: {
    list: { description: 'List gift cards', args: ['[status]', '[customerId]'] },
    get: { description: 'Get gift card', args: ['<giftCardId|code>'] },
    create: {
      description: 'Create gift card',
      args: [
        '<initialBalance>',
        '[currency]',
        '[customerId]',
        '[recipientEmail]',
        '[recipientName]',
        '[message]',
      ],
    },
    charge: {
      description: 'Charge gift card',
      args: ['<giftCardId>', '<amount>', '[orderId]', '[note]'],
    },
    refund: {
      description: 'Refund to gift card',
      args: ['<giftCardId>', '<amount>', '[orderId]', '[reason]'],
    },
    disable: { description: 'Disable gift card', args: ['<giftCardId>', '[reason]'] },
    balance: { description: 'Check gift card balance', args: ['<giftCardId|code>'] },
  },
};

export default { execute, metadata };
