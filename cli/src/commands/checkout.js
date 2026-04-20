/**
 * Checkout Commands Module
 */

let checkoutSvcPromise = null;

async function getCheckoutSvc() {
  if (!checkoutSvcPromise) {
    checkoutSvcPromise = (async () => {
      const { A2AStore } = await import('../a2a/store.js');
      const { createExpressCheckout } = await import('../checkout/express.js');
      const store = new A2AStore();
      store.init();
      return createExpressCheckout(store);
    })();
  }
  return checkoutSvcPromise;
}

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

function parseNumber(value, usage) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(usage);
  }
  return parsed;
}

export async function execute(action, args, { output, jsonOutput }) {
  const svc = await getCheckoutSvc();

  switch (action) {
    case 'create-link': {
      const [itemsJson, currency = 'USD', expiresInRaw, customerId] = args;
      if (!itemsJson) {
        throw new Error(
          'Usage: checkout create-link <itemsJson> [currency] [expiresIn] [customerId]',
        );
      }
      const result = svc.createPaymentLink({
        items: parseJsonArg(itemsJson, 'items'),
        currency: currency.toUpperCase(),
        expiresIn:
          expiresInRaw !== undefined
            ? parseNumber(
                expiresInRaw,
                'Usage: checkout create-link <itemsJson> [currency] [expiresIn] [customerId]',
              )
            : undefined,
        customerId: customerId || undefined,
      });
      return formatLinkCreation(result, { jsonOutput });
    }

    case 'resolve': {
      const linkId = args[0];
      if (!linkId) throw new Error('Usage: checkout resolve <linkId|shortCode>');
      const result = svc.resolvePaymentLink(linkId);
      if (!result) throw new Error(`Payment link not found: ${linkId}`);
      return formatResolvedLink(result, { output, jsonOutput });
    }

    case 'express': {
      const [linkId, customerId, paymentMethod] = args;
      if (!linkId)
        throw new Error('Usage: checkout express <linkId|shortCode> [customerId] [paymentMethod]');
      const result = svc.expressCheckout({
        linkId,
        customerId: customerId || undefined,
        paymentMethod: paymentMethod || undefined,
      });
      return formatExpressCheckout(result, { jsonOutput });
    }

    case 'agent': {
      const [buyerAgent, sellerAgent, itemsJson, paymentMethod = 'a2a', currency = 'USD'] = args;
      if (!buyerAgent || !sellerAgent || !itemsJson) {
        throw new Error(
          'Usage: checkout agent <buyerAgent> <sellerAgent> <itemsJson> [paymentMethod] [currency]',
        );
      }
      const result = svc.agentCheckout({
        buyerAgent,
        sellerAgent,
        items: parseJsonArg(itemsJson, 'items'),
        paymentMethod,
        currency: currency.toUpperCase(),
      });
      return formatAgentCheckout(result, { jsonOutput });
    }

    case 'status': {
      const linkId = args[0];
      if (!linkId) throw new Error('Usage: checkout status <linkId|shortCode>');
      const result = svc.getPaymentLinkStatus(linkId);
      if (!result) throw new Error(`Payment link not found: ${linkId}`);
      return formatLinkStatus(result, { jsonOutput });
    }

    case 'list': {
      const [status, customerId, limitRaw, offsetRaw] = args;
      const links = svc.listPaymentLinks({
        status: status || undefined,
        customerId: customerId || undefined,
        limit:
          limitRaw !== undefined
            ? parseNumber(limitRaw, 'Usage: checkout list [status] [customerId] [limit] [offset]')
            : undefined,
        offset:
          offsetRaw !== undefined
            ? parseNumber(offsetRaw, 'Usage: checkout list [status] [customerId] [limit] [offset]')
            : undefined,
      });
      return formatLinkList(links, { output, jsonOutput });
    }

    case 'revoke': {
      const linkId = args[0];
      if (!linkId) throw new Error('Usage: checkout revoke <linkId|shortCode>');
      const result = svc.revokePaymentLink(linkId);
      return formatRevocation(result, { jsonOutput });
    }

    case 'crypto': {
      const [linkId, walletAddress, network = 'set_chain', customerId] = args;
      if (!linkId || !walletAddress) {
        throw new Error(
          'Usage: checkout crypto <linkId|shortCode> <walletAddress> [network] [customerId]',
        );
      }
      const result = svc.expressCheckout({
        linkId,
        customerId: customerId || undefined,
        walletAddress,
        paymentMethod: `crypto:${network}`,
      });
      return formatCryptoCheckout(result, walletAddress, network, { jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: checkout ${action}\n\n` +
          'Available actions:\n' +
          '  create-link <itemsJson> [currency] [expiresIn] [customerId]  Create payment link\n' +
          '  resolve <linkId|shortCode>                                   Resolve payment link\n' +
          '  express <linkId|shortCode> [customerId] [paymentMethod]      Express checkout\n' +
          '  agent <buyerAgent> <sellerAgent> <itemsJson> [paymentMethod] [currency]  Agent checkout\n' +
          '  status <linkId|shortCode>                                    Get payment link status\n' +
          '  list [status] [customerId] [limit] [offset]                  List payment links\n' +
          '  revoke <linkId|shortCode>                                    Revoke payment link\n' +
          '  crypto <linkId|shortCode> <walletAddress> [network] [customerId]  Crypto checkout',
      );
  }
}

function formatLinkCreation(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    result,
    formatted:
      `Payment link created\n` +
      `${'-'.repeat(28)}\n` +
      `Link ID:     ${result.linkId}\n` +
      `Short code:  ${result.shortCode}\n` +
      `URL:         ${result.url}\n` +
      `Total:       ${result.total}\n` +
      `Status:      ${result.status}\n` +
      `Expires:     ${result.expiresAt || 'never'}`,
  };
}

function formatResolvedLink(result, { output, jsonOutput }) {
  if (jsonOutput) return result;
  const itemsTable =
    result.items.length === 0
      ? 'No items'
      : output.table(result.items, [
          { key: 'name', header: 'Name' },
          { key: 'sku', header: 'SKU' },
          { key: 'quantity', header: 'Qty', align: 'right' },
          { key: 'unitPrice', header: 'Unit', align: 'right' },
        ]);
  return {
    result,
    formatted:
      `Payment link ${result.link.short_code}\n` +
      `${'-'.repeat(32)}\n` +
      `Link ID:     ${result.link.id}\n` +
      `Status:      ${result.status}\n` +
      `Expired:     ${result.expired ? 'yes' : 'no'}\n` +
      `Total:       ${result.total} ${result.link.currency}\n` +
      `Views:       ${result.link.views}\n\n` +
      itemsTable,
  };
}

function formatExpressCheckout(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    result,
    formatted:
      `Express checkout complete\n` +
      `${'-'.repeat(32)}\n` +
      `Order ID:    ${result.orderId}\n` +
      `Payment ID:  ${result.paymentId}\n` +
      `Short code:  ${result.shortCode}`,
  };
}

function formatAgentCheckout(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    result,
    formatted:
      `Agent checkout complete\n` +
      `${'-'.repeat(30)}\n` +
      `Order ID:    ${result.orderId}\n` +
      `Escrow ID:   ${result.escrowId}\n` +
      `Link ID:     ${result.linkId}`,
  };
}

function formatLinkStatus(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    result,
    formatted:
      `Payment link status\n` +
      `${'-'.repeat(30)}\n` +
      `Link ID:      ${result.link.id}\n` +
      `Short code:   ${result.link.short_code}\n` +
      `Status:       ${result.status}\n` +
      `Views:        ${result.views}\n` +
      `Conversions:  ${result.conversions}`,
  };
}

function formatLinkList(links, { output, jsonOutput }) {
  if (jsonOutput) return links;
  if (links.length === 0) return { formatted: 'No payment links found.' };
  const formatted = output.table(
    links.map((link) => ({
      id: link.id,
      shortCode: link.short_code,
      status: link.status,
      total: link.total,
      currency: link.currency,
      customerId: link.customer_id,
    })),
    [
      { key: 'id', header: 'ID' },
      { key: 'shortCode', header: 'Short Code' },
      { key: 'status', header: 'Status' },
      { key: 'total', header: 'Total', align: 'right' },
      { key: 'currency', header: 'Currency' },
      { key: 'customerId', header: 'Customer' },
    ],
  );
  return { links, formatted };
}

function formatRevocation(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    result,
    formatted: `Revoked payment link ${result.link.id}`,
  };
}

function formatCryptoCheckout(result, walletAddress, network, { jsonOutput }) {
  if (jsonOutput) return { ...result, walletAddress, network };
  return {
    result,
    formatted:
      `Crypto checkout complete\n` +
      `${'-'.repeat(30)}\n` +
      `Order ID:    ${result.orderId}\n` +
      `Payment ID:  ${result.paymentId}\n` +
      `Short code:  ${result.shortCode}\n` +
      `Wallet:      ${walletAddress}\n` +
      `Network:     ${network}`,
  };
}

export const metadata = {
  name: 'checkout',
  aliases: ['cko', 'paylink'],
  description: 'Express checkout and payment link commands',
  actions: {
    'create-link': {
      description: 'Create payment link',
      args: ['<itemsJson>', '[currency]', '[expiresIn]', '[customerId]'],
    },
    resolve: { description: 'Resolve payment link', args: ['<linkId|shortCode>'] },
    express: {
      description: 'Run express checkout',
      args: ['<linkId|shortCode>', '[customerId]', '[paymentMethod]'],
    },
    agent: {
      description: 'Run agent checkout',
      args: ['<buyerAgent>', '<sellerAgent>', '<itemsJson>', '[paymentMethod]', '[currency]'],
    },
    status: { description: 'Get payment link status', args: ['<linkId|shortCode>'] },
    list: {
      description: 'List payment links',
      args: ['[status]', '[customerId]', '[limit]', '[offset]'],
    },
    revoke: { description: 'Revoke payment link', args: ['<linkId|shortCode>'] },
    crypto: {
      description: 'Run crypto checkout',
      args: ['<linkId|shortCode>', '<walletAddress>', '[network]', '[customerId]'],
    },
  },
};

export default { execute, metadata };
