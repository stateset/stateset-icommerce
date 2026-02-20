/**
 * Treasury Tools Module
 *
 * MCP tool definitions for agent treasury balance, deposit, buy, and token registry operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

/**
 * Treasury tool definitions
 */
export const treasuryTools = [
  {
    name: 'treasury_balance',
    description: 'Get treasury balances for an agent.',
    inputSchema: {
      agentId: z.string().optional().describe('Agent ID (default: default)'),
      chainId: z.string().optional().describe('Chain ID'),
      token: z.string().optional().describe('Token symbol (requires chainId)'),
    },
    permission: 'read',
    handler: async ({ params, treasuryContextOptions }) => {
      const { agentId = 'default', chainId, token } = params;
      const { loadTreasuryContext, resolveToken, computeBalanceDisplay } =
        await import('../treasury/index.js');
      const ctx = await loadTreasuryContext(treasuryContextOptions || {});

      if (token && !chainId) {
        return { success: false, error: 'token requires chainId' };
      }

      if (token) {
        const resolved = resolveToken(chainId, token, ctx.registry);
        if (!resolved) {
          return { success: false, error: `Unknown token ${token} on ${chainId}` };
        }
        const balance = ctx.store.getBalance({
          agentId,
          chainId,
          tokenSymbol: resolved.symbol,
          tokenDecimals: resolved.decimals,
        });
        return {
          success: true,
          agentId,
          chainId,
          token: resolved.symbol,
          balance: computeBalanceDisplay(balance.balanceSmallest, resolved.decimals),
          balanceSmallest: balance.balanceSmallest.toString(),
        };
      }

      const balances = ctx.store.getBalances({ agentId, chainId: chainId || null });
      return {
        success: true,
        agentId,
        chainId: chainId || null,
        balances: balances.map((b) => ({
          chainId: b.chainId,
          token: b.tokenSymbol,
          balance: computeBalanceDisplay(b.balanceSmallest, b.tokenDecimals || 0),
          balanceSmallest: b.balanceSmallest.toString(),
        })),
      };
    },
  },

  {
    name: 'treasury_ledger',
    description: 'List recent treasury transactions for an agent.',
    inputSchema: {
      agentId: z.string().describe('Agent ID'),
      chainId: z.string().optional().describe('Chain ID'),
      token: z.string().optional().describe('Token symbol'),
      taskId: z.string().optional().describe('Task id filter'),
      requestId: z.string().optional().describe('Request id filter'),
      limit: z.number().optional().default(25).describe('Max entries'),
    },
    permission: 'read',
    handler: async ({ params, treasuryContextOptions }) => {
      const { agentId, chainId, token, taskId, requestId, limit } = params;
      const { loadTreasuryContext } = await import('../treasury/index.js');
      const ctx = await loadTreasuryContext(treasuryContextOptions || {});
      const entries = ctx.store.list({
        agentId,
        chainId: chainId || null,
        tokenSymbol: token ? token.toUpperCase() : null,
        taskId: taskId || null,
        requestId: requestId || null,
        limit,
      });
      return { success: true, count: entries.length, entries };
    },
  },

  {
    name: 'treasury_deposit',
    description: 'Record a treasury deposit for an agent (funds received).',
    inputSchema: {
      agentId: z.string().describe('Agent ID'),
      chainId: z.string().describe('Chain ID'),
      token: z.string().describe('Token symbol'),
      amount: z.number().describe('Amount to deposit'),
      txId: z.string().optional().describe('Transaction hash'),
      fromAddress: z.string().optional().describe('Sender wallet address'),
    },
    permission: 'write',
    handler: async ({ params, treasuryContextOptions, buildAuditContext, extra }) => {
      const { agentId, chainId, token, amount, txId, fromAddress } = params;
      const { loadTreasuryContext, recordDeposit } = await import('../treasury/index.js');
      const ctx = await loadTreasuryContext(treasuryContextOptions || {});
      const audit = buildAuditContext ? buildAuditContext(extra, 'treasury_deposit') : {};
      const entry = await recordDeposit(
        {
          agentId,
          chainId,
          tokenSymbol: token,
          amount,
          txId: txId || null,
          fromAddress: fromAddress || null,
          source: 'mcp',
          ...audit,
        },
        ctx,
      );
      return { success: true, entry };
    },
  },

  {
    name: 'treasury_buy',
    description: 'Purchase tokens using treasury stablecoin balances.',
    inputSchema: {
      agentId: z.string().describe('Agent ID'),
      chainId: z.string().describe('Chain ID'),
      toToken: z.string().describe('Target token symbol'),
      amount: z.number().describe('Stablecoin amount to spend'),
      fromToken: z.string().optional().describe('Funding token symbol (default: chain stablecoin)'),
      priceUsd: z.number().optional().describe('Override token price in USD'),
      slippagePct: z.number().optional().default(1).describe('Slippage percentage'),
    },
    permission: 'write',
    handler: async ({ params, treasuryContextOptions, buildAuditContext, extra }) => {
      const { agentId, chainId, toToken, amount, fromToken, priceUsd, slippagePct } = params;
      const { loadTreasuryContext, buyTokens } = await import('../treasury/index.js');
      const ctx = await loadTreasuryContext(treasuryContextOptions || {});
      const audit = buildAuditContext ? buildAuditContext(extra, 'treasury_buy') : {};
      const result = await buyTokens(
        {
          agentId,
          chainId,
          fromSymbol: fromToken || null,
          toSymbol: toToken,
          amount,
          priceUsd,
          slippagePct,
          ...audit,
        },
        ctx,
      );
      return { success: true, result };
    },
  },

  {
    name: 'treasury_list_tokens',
    description: 'List available tokens from chain config and custom registry.',
    inputSchema: {
      chainId: z.string().optional().describe('Chain ID'),
    },
    permission: 'read',
    handler: async ({ params, treasuryContextOptions }) => {
      const { chainId } = params;
      const { loadTreasuryContext, listTokens } = await import('../treasury/index.js');
      const ctx = await loadTreasuryContext(treasuryContextOptions || {});
      const tokens = listTokens(chainId || null, ctx.registry);
      return { success: true, count: tokens.length, tokens };
    },
  },

  {
    name: 'treasury_register_token',
    description: 'Add or update a token in the treasury registry.',
    inputSchema: {
      symbol: z.string().describe('Token symbol'),
      chainId: z.string().describe('Chain ID'),
      decimals: z.number().describe('Token decimals'),
      address: z.string().optional().describe('Token contract address'),
      priceUsd: z.number().optional().describe('Token price in USD'),
      issuerAgentId: z.string().optional().describe('Issuing agent ID'),
    },
    permission: 'write',
    handler: async ({ params, treasuryContextOptions }) => {
      const { symbol, chainId, decimals, address, priceUsd, issuerAgentId } = params;
      const { loadTreasuryContext, addRegistryToken } = await import('../treasury/index.js');
      const ctx = await loadTreasuryContext(treasuryContextOptions || {});
      const updated = await addRegistryToken(ctx.registryPath, ctx.registry, {
        symbol,
        chainId,
        decimals,
        address: address || null,
        priceUsd: priceUsd ?? null,
        issuerAgentId: issuerAgentId || null,
      });
      return { success: true, tokens: updated.tokens };
    },
  },
];

export default treasuryTools;
