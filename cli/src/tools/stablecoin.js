/**
 * Stablecoin Payment Tools Module
 *
 * MCP tool definitions for native crypto/stablecoin payment operations.
 * Supports USDC on Solana, ssUSD on SET Chain, and other chain/token combinations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

/**
 * Stablecoin tool definitions
 */
export const stablecoinTools = [
  {
    name: 'get_agent_wallet',
    description:
      'Get the agent wallet address for a specific blockchain. Returns the wallet address derived from VES keys.',
    inputSchema: {
      chain: z
        .string()
        .optional()
        .describe(
          'Blockchain: set_chain, base, ethereum, arbitrum, solana, solana_devnet (default: set_chain)',
        ),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const chain = params.chain || 'set_chain';
      const { getWalletAddress, getChain, getDefaultStablecoin, getExplorerAddressUrl } =
        await import('../chains/index.js');
      const address = await getWalletAddress('default', chain, { configDir: '.stateset' });
      const chainConfig = getChain(chain);
      const stablecoin = getDefaultStablecoin(chain);
      return {
        success: true,
        chain: chainConfig?.name || chain,
        network: chainConfig?.network,
        address,
        stablecoin: stablecoin?.symbol,
        explorerUrl: getExplorerAddressUrl(chain, address),
      };
    },
  },

  {
    name: 'get_wallet_balance',
    description: 'Check the stablecoin balance of the agent wallet on a blockchain.',
    inputSchema: {
      chain: z
        .string()
        .optional()
        .describe('Blockchain: set_chain, base, ethereum, arbitrum, solana (default: set_chain)'),
      token: z
        .string()
        .optional()
        .describe('Token symbol: USDC, ssUSD, USDT (default: chain stablecoin)'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const chain = params.chain || 'set_chain';
      const { getWalletAddress, getBalance } = await import('../chains/index.js');
      const address = await getWalletAddress('default', chain, { configDir: '.stateset' });
      const balance = await getBalance(address, chain, params.token);
      return {
        success: true,
        chain,
        address,
        balance: balance.balance,
        symbol: balance.symbol,
        humanReadable: `${balance.balance} ${balance.symbol}`,
      };
    },
  },

  {
    name: 'create_stablecoin_payment',
    description:
      'Create and execute a stablecoin payment to a wallet address. Supports USDC on Solana, ssUSD on SET Chain, etc.',
    inputSchema: {
      toAddress: z.string().describe('Recipient wallet address'),
      amount: z.number().describe('Amount to send (e.g., 50.00)'),
      chain: z
        .string()
        .optional()
        .describe('Blockchain: set_chain, base, ethereum, arbitrum, solana (default: set_chain)'),
      token: z.string().optional().describe('Token: USDC, ssUSD (default: chain stablecoin)'),
      orderId: z.string().optional().describe('Order ID for audit trail'),
      customerId: z.string().optional().describe('Customer ID for audit trail'),
      memo: z.string().optional().describe('Payment memo'),
    },
    permission: 'write',
    handler: async ({
      commerce: _commerce,
      params,
      allowApply,
      resolveTreasuryAgentId,
      treasuryContextOptions,
      buildAuditContext,
      buildTreasuryIdentityMetadata,
      extra,
    }) => {
      if (!allowApply) {
        return {
          error: 'Stablecoin payment requires --apply flag.',
          wouldSend: {
            to: params.toAddress,
            amount: params.amount,
            chain: params.chain || 'set_chain',
            token: params.token || 'default_stablecoin_for_chain',
          },
          instruction: 'Run with --apply to execute this payment',
        };
      }

      const effectiveAgentId = resolveTreasuryAgentId ? await resolveTreasuryAgentId() : 'default';
      const { executePayment, getDefaultStablecoin } = await import('../chains/index.js');
      const result = await executePayment(
        {
          agentId: effectiveAgentId,
          chainId: params.chain || 'set_chain',
          toAddress: params.toAddress,
          amount: params.amount,
          tokenSymbol: params.token,
          metadata: {
            order_id: params.orderId,
            customer_id: params.customerId,
            memo: params.memo,
          },
        },
        {
          configDir: '.stateset',
          simulate: false,
        },
      );

      if (result.success) {
        try {
          const chainId = params.chain || 'set_chain';
          const { loadTreasuryContext, recordWithdrawal } = await import('../treasury/index.js');
          const ctx = await loadTreasuryContext(treasuryContextOptions || {});
          const audit = buildAuditContext
            ? buildAuditContext(extra, 'create_stablecoin_payment')
            : {};
          const defaultStablecoin = getDefaultStablecoin(chainId);
          const identityMeta = buildTreasuryIdentityMetadata
            ? await buildTreasuryIdentityMetadata()
            : {};
          await recordWithdrawal(
            {
              agentId: effectiveAgentId,
              chainId,
              tokenSymbol: params.token || defaultStablecoin?.symbol,
              amount: params.amount,
              txId: result.txHash || null,
              toAddress: params.toAddress,
              source: 'stablecoin_payment',
              metadata: {
                orderId: params.orderId || null,
                customerId: params.customerId || null,
                memo: params.memo || null,
                ...identityMeta,
              },
              ...audit,
            },
            ctx,
          );
        } catch (auditError) {
          console.warn(`[Treasury] Failed to record stablecoin payment: ${auditError.message}`);
        }
        return {
          success: true,
          message: 'Stablecoin payment completed',
          intentId: result.intentId,
          txHash: result.txHash,
          explorerUrl: result.explorerUrl,
          blockNumber: result.blockNumber,
          confirmations: result.confirmations,
        };
      } else {
        return {
          success: false,
          error: result.error,
          intentId: result.intentId,
        };
      }
    },
  },

  {
    name: 'list_supported_chains',
    description: 'List all supported blockchain networks for stablecoin payments.',
    inputSchema: {},
    permission: 'read',
    handler: async () => {
      const { listChains, getChain, getDefaultStablecoin } = await import('../chains/index.js');
      const chains = listChains().map((id) => {
        const chain = getChain(id);
        const stablecoin = getDefaultStablecoin(id);
        return {
          id,
          name: chain?.name,
          network: chain?.network,
          stablecoin: stablecoin?.symbol,
          blockTime: chain?.blockTimeMs ? `${chain.blockTimeMs}ms` : null,
        };
      });
      return {
        success: true,
        count: chains.length,
        chains,
        recommended: 'set_chain (ssUSD) for default live settlement, base/ethereum for USDC',
      };
    },
  },
];

/**
 * Get all stablecoin tools
 */
export function getStablecoinTools() {
  return stablecoinTools;
}

/**
 * Get stablecoin tool by name
 */
export function getStablecoinTool(name) {
  return stablecoinTools.find((t) => t.name === name);
}

export default stablecoinTools;
