/**
 * Treasury Commands Module
 */

function parsePositiveNumber(value, usage) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || parsed <= 0) throw new Error(usage);
  return parsed;
}

function parsePositiveInt(value, usage) {
  if (value === undefined) return undefined;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0) throw new Error(usage);
  return parsed;
}

function parseNonNegativeInt(value, usage) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 0) throw new Error(usage);
  return parsed;
}

async function getTreasury() {
  return import('../treasury/index.js');
}

export async function execute(action, args, { output, jsonOutput }) {
  const treasury = await getTreasury();
  const ctx = await treasury.loadTreasuryContext({});

  switch (action) {
    case 'balance': {
      const [agentId = 'default', chainId, token] = args;
      if (token && !chainId) throw new Error('Usage: treasury balance [agentId] [chainId] [token]');
      if (token) {
        const resolved = treasury.resolveToken(chainId, token, ctx.registry);
        if (!resolved) throw new Error(`Unknown token ${token} on ${chainId}`);
        const balance = ctx.store.getBalance({
          agentId,
          chainId,
          tokenSymbol: resolved.symbol,
          tokenDecimals: resolved.decimals,
        });
        const result = {
          agentId,
          chainId,
          token: resolved.symbol,
          balance: treasury.computeBalanceDisplay(balance.balanceSmallest, resolved.decimals),
          balanceSmallest: balance.balanceSmallest.toString(),
        };
        return jsonOutput
          ? result
          : {
              result,
              formatted:
                `Treasury balance\n` +
                `${'-'.repeat(28)}\n` +
                `Agent:       ${agentId}\n` +
                `Chain:       ${chainId}\n` +
                `Token:       ${result.token}\n` +
                `Balance:     ${result.balance}`,
            };
      }
      const balances = ctx.store.getBalances({ agentId, chainId: chainId || null });
      return formatBalances(
        balances.map((entry) => ({
          chainId: entry.chainId,
          token: entry.tokenSymbol,
          balance: treasury.computeBalanceDisplay(entry.balanceSmallest, entry.tokenDecimals || 0),
          balanceSmallest: entry.balanceSmallest.toString(),
        })),
        { output, jsonOutput },
      );
    }

    case 'ledger': {
      const [agentId, chainId, token, taskId, requestId, limitRaw] = args;
      if (!agentId) {
        throw new Error(
          'Usage: treasury ledger <agentId> [chainId] [token] [taskId] [requestId] [limit]',
        );
      }
      const entries = ctx.store.list({
        agentId,
        chainId: chainId || null,
        tokenSymbol: token ? token.toUpperCase() : null,
        taskId: taskId || null,
        requestId: requestId || null,
        limit:
          parsePositiveInt(
            limitRaw,
            'Usage: treasury ledger <agentId> [chainId] [token] [taskId] [requestId] [limit]',
          ) || 25,
      });
      return formatLedger(entries, { output, jsonOutput });
    }

    case 'deposit': {
      const [agentId, chainId, token, amountRaw, txId, fromAddress] = args;
      if (!agentId || !chainId || !token || !amountRaw) {
        throw new Error(
          'Usage: treasury deposit <agentId> <chainId> <token> <amount> [txId] [fromAddress]',
        );
      }
      const entry = await treasury.recordDeposit(
        {
          agentId,
          chainId,
          tokenSymbol: token,
          amount: parsePositiveNumber(
            amountRaw,
            'Usage: treasury deposit <agentId> <chainId> <token> <amount> [txId] [fromAddress]',
          ),
          txId: txId || null,
          fromAddress: fromAddress || null,
          source: 'cli',
        },
        ctx,
      );
      return { entry, formatted: `Recorded treasury deposit ${entry.id || txId || token}` };
    }

    case 'buy': {
      const [agentId, chainId, toToken, amountRaw, fromToken, priceUsdRaw, slippagePctRaw] = args;
      if (!agentId || !chainId || !toToken || !amountRaw) {
        throw new Error(
          'Usage: treasury buy <agentId> <chainId> <toToken> <amount> [fromToken] [priceUsd] [slippagePct]',
        );
      }
      const result = await treasury.buyTokens(
        {
          agentId,
          chainId,
          fromSymbol: fromToken || null,
          toSymbol: toToken,
          amount: parsePositiveNumber(
            amountRaw,
            'Usage: treasury buy <agentId> <chainId> <toToken> <amount> [fromToken] [priceUsd] [slippagePct]',
          ),
          priceUsd: priceUsdRaw
            ? parsePositiveNumber(priceUsdRaw, 'priceUsd must be positive')
            : undefined,
          slippagePct: slippagePctRaw
            ? parsePositiveNumber(slippagePctRaw, 'slippagePct must be positive')
            : undefined,
        },
        ctx,
      );
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Treasury buy executed\n` +
              `${'-'.repeat(30)}\n` +
              `Agent:       ${agentId}\n` +
              `Chain:       ${chainId}\n` +
              `Bought:      ${toToken}\n` +
              `Spent:       ${amountRaw}`,
          };
    }

    case 'tokens': {
      const chainId = args[0];
      const tokens = treasury.listTokens(chainId || null, ctx.registry);
      return formatTokens(tokens, { output, jsonOutput });
    }

    case 'register-token': {
      const [symbol, chainId, decimalsRaw, address, priceUsdRaw, issuerAgentId] = args;
      if (!symbol || !chainId || decimalsRaw === undefined) {
        throw new Error(
          'Usage: treasury register-token <symbol> <chainId> <decimals> [address] [priceUsd] [issuerAgentId]',
        );
      }
      const updated = await treasury.addRegistryToken(ctx.registryPath, ctx.registry, {
        symbol,
        chainId,
        decimals: parseNonNegativeInt(
          decimalsRaw,
          'Usage: treasury register-token <symbol> <chainId> <decimals> [address] [priceUsd] [issuerAgentId]',
        ),
        address: address || null,
        priceUsd: priceUsdRaw
          ? parsePositiveNumber(priceUsdRaw, 'priceUsd must be positive')
          : null,
        issuerAgentId: issuerAgentId || null,
      });
      return {
        tokens: updated.tokens,
        formatted: `Registered token ${symbol} on ${chainId}`,
      };
    }

    default:
      throw new Error(
        `Unknown action: treasury ${action}\n\n` +
          'Available actions:\n' +
          '  balance [agentId] [chainId] [token]                           Get treasury balance\n' +
          '  ledger <agentId> [chainId] [token] [taskId] [requestId] [limit]  List treasury ledger\n' +
          '  deposit <agentId> <chainId> <token> <amount> [txId] [fromAddress] Record treasury deposit\n' +
          '  buy <agentId> <chainId> <toToken> <amount> [fromToken] [priceUsd] [slippagePct]  Buy token\n' +
          '  tokens [chainId]                                               List treasury tokens\n' +
          '  register-token <symbol> <chainId> <decimals> [address] [priceUsd] [issuerAgentId]  Register token',
      );
  }
}

function formatBalances(balances, { output, jsonOutput }) {
  if (jsonOutput) return balances;
  if (balances.length === 0) return { formatted: 'No treasury balances found.' };
  const formatted = output.table(balances, [
    { key: 'chainId', header: 'Chain' },
    { key: 'token', header: 'Token' },
    { key: 'balance', header: 'Balance', align: 'right' },
    { key: 'balanceSmallest', header: 'Smallest Unit', align: 'right' },
  ]);
  return { balances, formatted };
}

function formatLedger(entries, { output, jsonOutput }) {
  if (jsonOutput) return entries;
  if (entries.length === 0) return { formatted: 'No treasury ledger entries found.' };
  const formatted = output.table(entries, [
    { key: 'id', header: 'ID' },
    { key: 'chainId', header: 'Chain' },
    { key: 'tokenSymbol', header: 'Token' },
    { key: 'direction', header: 'Direction' },
    { key: 'amount', header: 'Amount', align: 'right' },
    { key: 'createdAt', header: 'Created' },
  ]);
  return { entries, formatted };
}

function formatTokens(tokens, { output, jsonOutput }) {
  if (jsonOutput) return tokens;
  if (tokens.length === 0) return { formatted: 'No treasury tokens found.' };
  const formatted = output.table(tokens, [
    { key: 'symbol', header: 'Symbol' },
    { key: 'chainId', header: 'Chain' },
    { key: 'decimals', header: 'Decimals', align: 'right' },
    { key: 'address', header: 'Address' },
  ]);
  return { tokens, formatted };
}

export const metadata = {
  name: 'treasury',
  aliases: ['treas', 'cash'],
  description: 'Treasury balance, ledger, and token commands',
  actions: {
    balance: { description: 'Get treasury balance', args: ['[agentId]', '[chainId]', '[token]'] },
    ledger: {
      description: 'List treasury ledger',
      args: ['<agentId>', '[chainId]', '[token]', '[taskId]', '[requestId]', '[limit]'],
    },
    deposit: {
      description: 'Record treasury deposit',
      args: ['<agentId>', '<chainId>', '<token>', '<amount>', '[txId]', '[fromAddress]'],
    },
    buy: {
      description: 'Buy token',
      args: [
        '<agentId>',
        '<chainId>',
        '<toToken>',
        '<amount>',
        '[fromToken]',
        '[priceUsd]',
        '[slippagePct]',
      ],
    },
    tokens: { description: 'List treasury tokens', args: ['[chainId]'] },
    'register-token': {
      description: 'Register treasury token',
      args: ['<symbol>', '<chainId>', '<decimals>', '[address]', '[priceUsd]', '[issuerAgentId]'],
    },
  },
};

export default { execute, metadata };
