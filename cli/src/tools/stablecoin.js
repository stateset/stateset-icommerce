/**
 * Stablecoin Payment Tools Module
 *
 * MCP tool definitions for native crypto/stablecoin payment operations.
 * Supports stablecoins plus native BTC/ZEC payment flows where configured.
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
        .min(1)
        .optional()
        .describe(
          'Blockchain: set_chain, base, ethereum, arbitrum, solana, solana_devnet, bitcoin, bitcoin_testnet, zcash, zcash_testnet (default: set_chain)',
        ),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const chain = params.chain || 'set_chain';
      const { getWalletAddress, getChain, getDefaultPaymentToken, getExplorerAddressUrl } =
        await import('../chains/index.js');
      const address = await getWalletAddress('default', chain, { configDir: '.stateset' });
      const chainConfig = getChain(chain);
      const defaultToken = getDefaultPaymentToken(chain);
      return {
        success: true,
        chain: chainConfig?.name || chain,
        network: chainConfig?.network,
        address,
        stablecoin: defaultToken?.symbol,
        defaultToken: defaultToken?.symbol,
        explorerUrl: getExplorerAddressUrl(chain, address),
      };
    },
  },

  {
    name: 'get_wallet_balance',
    description: 'Check the balance of the agent wallet on a blockchain.',
    inputSchema: {
      chain: z
        .string()
        .min(1)
        .optional()
        .describe(
          'Blockchain: set_chain, base, ethereum, arbitrum, solana, bitcoin, bitcoin_testnet, zcash, zcash_testnet (default: set_chain)',
        ),
      token: z
        .string()
        .min(1)
        .optional()
        .describe(
          'Token symbol: USDC, ssUSD, USDT, BTC, ZEC (default: chain default payment token)',
        ),
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
      'Create and execute a blockchain payment to a wallet address. Supports stablecoins plus native BTC and shielded ZEC flows.',
    inputSchema: {
      toAddress: z.string().min(1).describe('Recipient wallet address'),
      amount: z.number().positive().describe('Amount to send (e.g., 50.00)'),
      chain: z
        .string()
        .min(1)
        .optional()
        .describe(
          'Blockchain: set_chain, base, ethereum, arbitrum, solana, bitcoin, bitcoin_testnet, zcash, zcash_testnet (default: set_chain)',
        ),
      token: z
        .string()
        .min(1)
        .optional()
        .describe('Token: USDC, ssUSD, BTC, ZEC (default: chain default payment token)'),
      orderId: z.string().min(1).optional().describe('Order ID for audit trail'),
      customerId: z.string().min(1).optional().describe('Customer ID for audit trail'),
      memo: z.string().max(500).optional().describe('Payment memo'),
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
          success: false,
          error: 'Stablecoin payment requires --apply flag.',
          wouldSend: {
            to: params.toAddress,
            amount: params.amount,
            chain: params.chain || 'set_chain',
            token: params.token || 'default_payment_token_for_chain',
          },
          instruction: 'Run with --apply to execute this payment',
        };
      }

      const effectiveAgentId = resolveTreasuryAgentId ? await resolveTreasuryAgentId() : 'default';
      const { executePayment, getDefaultPaymentToken } = await import('../chains/index.js');
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
          const defaultToken = getDefaultPaymentToken(chainId);
          const identityMeta = buildTreasuryIdentityMetadata
            ? await buildTreasuryIdentityMetadata()
            : {};
          await recordWithdrawal(
            {
              agentId: effectiveAgentId,
              chainId,
              tokenSymbol: params.token || defaultToken?.symbol,
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
    description: 'List all supported blockchain networks for agent payment execution.',
    inputSchema: {},
    permission: 'read',
    handler: async () => {
      const { listChains, getChain, getDefaultPaymentToken } = await import('../chains/index.js');
      const chains = listChains().map((id) => {
        const chain = getChain(id);
        const defaultToken = getDefaultPaymentToken(id);
        return {
          id,
          name: chain?.name,
          network: chain?.network,
          stablecoin: defaultToken?.symbol,
          defaultToken: defaultToken?.symbol,
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

export default stablecoinTools;
