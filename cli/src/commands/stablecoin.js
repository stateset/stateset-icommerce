/**
 * Stablecoin Commands Module
 */

async function getChains() {
  return import('../chains/index.js');
}

export async function execute(action, args, { jsonOutput }) {
  const chains = await getChains();

  switch (action) {
    case 'wallet': {
      const chain = args[0] || 'set_chain';
      const address = await chains.getWalletAddress('default', chain, { configDir: '.stateset' });
      const chainConfig = chains.getChain(chain);
      const defaultToken = chains.getDefaultPaymentToken(chain);
      const result = {
        chain: chainConfig?.name || chain,
        network: chainConfig?.network,
        address,
        defaultToken: defaultToken?.symbol,
        explorerUrl: chains.getExplorerAddressUrl(chain, address),
      };
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Agent wallet\n` +
              `${'-'.repeat(22)}\n` +
              `Chain:       ${result.chain}\n` +
              `Network:     ${result.network || 'N/A'}\n` +
              `Address:     ${result.address}\n` +
              `Token:       ${result.defaultToken || 'N/A'}`,
          };
    }

    case 'balance': {
      const [chain = 'set_chain', token] = args;
      const address = await chains.getWalletAddress('default', chain, { configDir: '.stateset' });
      const balance = await chains.getBalance(address, chain, token);
      return jsonOutput
        ? balance
        : {
            balance,
            formatted:
              `Wallet balance\n` +
              `${'-'.repeat(24)}\n` +
              `Chain:       ${chain}\n` +
              `Address:     ${address}\n` +
              `Balance:     ${balance.balance} ${balance.symbol}`,
          };
    }

    case 'pay': {
      const [toAddress, amountRaw, chain = 'set_chain', token, orderId, customerId, ...memoParts] =
        args;
      if (!toAddress || !amountRaw) {
        throw new Error(
          'Usage: stablecoin pay <toAddress> <amount> [chain] [token] [orderId] [customerId] [memo]',
        );
      }
      const amount = Number.parseFloat(amountRaw);
      if (!Number.isFinite(amount) || amount <= 0) {
        throw new Error(
          'Usage: stablecoin pay <toAddress> <amount> [chain] [token] [orderId] [customerId] [memo]',
        );
      }
      const result = await chains.executePayment(
        {
          agentId: 'default',
          chainId: chain,
          toAddress,
          amount,
          tokenSymbol: token || undefined,
          metadata: {
            order_id: orderId || null,
            customer_id: customerId || null,
            memo: memoParts.join(' ') || null,
          },
        },
        {
          configDir: '.stateset',
          simulate: false,
        },
      );
      if (!result.success) throw new Error(result.error || 'Stablecoin payment failed');
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Stablecoin payment sent\n` +
              `${'-'.repeat(32)}\n` +
              `Intent ID:    ${result.intentId}\n` +
              `Tx hash:      ${result.txHash}\n` +
              `Confirmations:${result.confirmations ?? 'N/A'}`,
          };
    }

    case 'chains': {
      const list = chains.listChains().map((id) => {
        const chain = chains.getChain(id);
        const defaultToken = chains.getDefaultPaymentToken(id);
        return {
          id,
          name: chain?.name,
          network: chain?.network,
          token: defaultToken?.symbol,
        };
      });
      return jsonOutput
        ? list
        : {
            chains: list,
            formatted:
              `Supported chains\n` +
              `${'-'.repeat(28)}\n` +
              list
                .map((entry) => `${entry.id}: ${entry.name || entry.id} (${entry.token || 'N/A'})`)
                .join('\n'),
          };
    }

    default:
      throw new Error(
        `Unknown action: stablecoin ${action}\n\n` +
          'Available actions:\n' +
          '  wallet [chain]                                           Get agent wallet\n' +
          '  balance [chain] [token]                                  Get wallet balance\n' +
          '  pay <toAddress> <amount> [chain] [token] [orderId] [customerId] [memo]  Send payment\n' +
          '  chains                                                   List supported chains',
      );
  }
}

export const metadata = {
  name: 'stablecoin',
  aliases: ['sc', 'stable'],
  description: 'Stablecoin and onchain payment commands',
  actions: {
    wallet: { description: 'Get agent wallet', args: ['[chain]'] },
    balance: { description: 'Get wallet balance', args: ['[chain]', '[token]'] },
    pay: {
      description: 'Send stablecoin payment',
      args: [
        '<toAddress>',
        '<amount>',
        '[chain]',
        '[token]',
        '[orderId]',
        '[customerId]',
        '[memo]',
      ],
    },
    chains: { description: 'List supported chains', args: [] },
  },
};

export default { execute, metadata };
