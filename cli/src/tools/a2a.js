/**
 * Agent-to-Agent (A2A) Commerce Tools Module
 *
 * MCP tool definitions for agent-to-agent payments, quotes, and commerce.
 * Makes it dead simple for AI agents to pay each other through natural language.
 */

import { z } from 'zod';
import { resolveCommerceApi } from '../commerce.js';

function parseJsonObject(value) {
  if (!value) return null;
  if (typeof value === 'object') return value;
  if (typeof value !== 'string') return null;
  try {
    const parsed = JSON.parse(value);
    return parsed && typeof parsed === 'object' ? parsed : null;
  } catch {
    return null;
  }
}

async function getA2AContext(commerce, agentConfig) {
  const walletAddress = agentConfig?.walletAddress;
  let runtime = null;

  if (walletAddress) {
    const { findRuntimeByWalletAddress } = await import('./agent-runtime.js');
    runtime = findRuntimeByWalletAddress(walletAddress);
  }

  if (runtime) {
    return {
      runtime,
      a2a: runtime.a2a,
    };
  }

  const { createA2AService } = await import('../a2a/index.js');
  return {
    runtime: null,
    a2a: createA2AService(commerce, agentConfig),
  };
}

/**
 * A2A payment tool definitions
 */
export const a2aTools = [
  // ==========================================================================
  // Direct Payments
  // ==========================================================================
  {
    name: 'a2a_pay',
    description:
      'Pay another AI agent directly. Send supported payment assets including USDC, ssUSD, BTC, or shielded ZEC to another agent by identity wallet, native chain address, or agent ID.',
    inputSchema: {
      to: z
        .string()
        .min(1)
        .describe('Recipient: identity wallet, native chain address, or agent ID (UUID)'),
      amount: z
        .number()
        .positive()
        .describe('Amount to pay in the selected asset (e.g., 10.00 USDC or 0.001 BTC)'),
      asset: z.string().optional().describe('Asset to pay with: USDC, USDT, ssUSD, DAI, BTC, ZEC'),
      network: z
        .string()
        .optional()
        .describe('Network: set_chain, base, ethereum, arbitrum, bitcoin, zcash'),
      memo: z
        .string()
        .optional()
        .describe('Payment memo/description (e.g., "API call", "Data processing")'),
      idempotencyKey: z.string().optional().describe('Unique key to prevent duplicate payments'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Sending payments requires --apply flag.',
          wouldPay: {
            to: params.to,
            amount: params.amount,
            asset: params.asset || 'default_for_network',
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured. Set up agent identity first.',
        };
      }

      const { runtime, a2a } = await getA2AContext(commerce, agentConfig);
      const result =
        runtime && typeof runtime.pay === 'function'
          ? await runtime.pay({
              to: params.to,
              amount: params.amount,
              asset: params.asset,
              network: params.network,
              memo: params.memo,
              idempotencyKey: params.idempotencyKey,
            })
          : await a2a.pay({
              to: params.to,
              amount: params.amount,
              asset: params.asset,
              network: params.network,
              memo: params.memo,
              idempotencyKey: params.idempotencyKey,
            });

      return {
        success: true,
        message: `Paid ${result.payment.amount} ${result.payment.asset} to ${result.payment.to}`,
        payment: result.payment,
        viaRuntime: Boolean(runtime),
        settlementChains: runtime?.listSettlementChains?.() || [],
      };
    },
  },

  {
    name: 'a2a_request_payment',
    description:
      'Request payment from another agent. Creates a payment request that the other agent can pay.',
    inputSchema: {
      amount: z
        .number()
        .positive()
        .describe('Amount to request in the selected asset (e.g., 25.00 or 0.001)'),
      description: z
        .string()
        .min(1)
        .describe('What the payment is for (e.g., "Data processing fee", "API access")'),
      from: z
        .string()
        .optional()
        .describe('Specific payer wallet/agent (optional - leave empty for open request)'),
      asset: z
        .string()
        .optional()
        .describe(
          'Asset to request: defaults to the selected network payment asset (e.g., USDC, ssUSD, BTC, ZEC)',
        ),
      network: z
        .string()
        .optional()
        .describe(
          'Preferred settlement network: set_chain, base, ethereum, arbitrum, bitcoin, zcash',
        ),
      expiresInHours: z
        .number()
        .int()
        .min(1)
        .optional()
        .describe('Hours until request expires (default: 24)'),
      allowPartial: z.boolean().optional().describe('Allow partial payments'),
      callbackUrl: z.string().url().optional().describe('Webhook URL to notify when paid'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Creating payment requests requires --apply flag.',
          wouldRequest: {
            amount: params.amount,
            description: params.description,
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured. Set up agent identity first.',
        };
      }

      const { runtime, a2a } = await getA2AContext(commerce, agentConfig);
      const result =
        runtime && typeof runtime.requestPayment === 'function'
          ? await runtime.requestPayment({
              from: params.from,
              amount: params.amount,
              description: params.description,
              asset: params.asset,
              network: params.network,
              expiresInHours: params.expiresInHours,
              allowPartial: params.allowPartial,
              callbackUrl: params.callbackUrl,
            })
          : await a2a.requestPayment({
              from: params.from,
              amount: params.amount,
              description: params.description,
              asset: params.asset,
              network: params.network,
              expiresInHours: params.expiresInHours,
              allowPartial: params.allowPartial,
              callbackUrl: params.callbackUrl,
            });

      return {
        success: true,
        message: `Payment request created for ${result.request.amount} ${result.request.asset}`,
        request: result.request,
        paymentUrl: result.paymentUrl,
        viaRuntime: Boolean(runtime),
        settlementChains: runtime?.listSettlementChains?.() || [],
      };
    },
  },

  {
    name: 'a2a_pay_request',
    description: 'Pay an existing payment request from another agent.',
    inputSchema: {
      requestId: z.string().min(1).describe('Payment request ID (UUID)'),
      amount: z
        .number()
        .optional()
        .describe('Amount to pay (optional - pays full amount if not specified)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Paying requests requires --apply flag.',
          wouldPay: { requestId: params.requestId },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { runtime, a2a } = await getA2AContext(commerce, agentConfig);
      const result =
        runtime && typeof runtime.payRequest === 'function'
          ? await runtime.payRequest(params.requestId, {
              amount: params.amount,
            })
          : await a2a.payRequest(params.requestId, {
              amount: params.amount,
            });

      return {
        success: true,
        message: result.request.fullyPaid
          ? 'Payment request fully paid'
          : `Partial payment made. Remaining: ${result.request.amountRemaining}`,
        payment: result.payment,
        request: result.request,
        viaRuntime: Boolean(runtime),
        settlementChains: runtime?.listSettlementChains?.() || [],
      };
    },
  },

  // ==========================================================================
  // Quote Operations
  // ==========================================================================
  {
    name: 'a2a_request_quote',
    description: 'Request a price quote from another agent for goods or services.',
    inputSchema: {
      seller: z.string().min(1).describe('Seller agent wallet address or agent ID'),
      items: z
        .array(
          z.object({
            description: z.string().describe('Item description'),
            quantity: z.number().int().min(1).optional().describe('Quantity (default: 1)'),
            sku: z.string().optional().describe('SKU or service code'),
          }),
        )
        .min(1)
        .max(100)
        .describe('Items to get a quote for'),
      asset: z
        .string()
        .optional()
        .describe(
          'Preferred payment asset: defaults to the selected network payment asset (e.g., USDC, ssUSD, BTC, ZEC)',
        ),
      network: z
        .string()
        .optional()
        .describe(
          'Preferred settlement network: set_chain, base, ethereum, arbitrum, bitcoin, zcash',
        ),
      message: z.string().optional().describe('Message to seller'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Requesting quotes requires --apply flag.',
          wouldRequest: {
            seller: params.seller,
            items: params.items,
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { runtime, a2a } = await getA2AContext(commerce, agentConfig);
      const result = await a2a.requestQuote({
        seller: params.seller,
        items: params.items,
        asset: params.asset,
        network: params.network,
        message: params.message,
      });

      return {
        success: true,
        message: 'Quote requested. Waiting for seller response.',
        quote: result.quote,
        viaRuntime: Boolean(runtime),
        settlementChains: runtime?.listSettlementChains?.() || [],
      };
    },
  },

  {
    name: 'a2a_provide_quote',
    description: 'Respond to a quote request with pricing (for sellers).',
    inputSchema: {
      quoteId: z.string().min(1).describe('Quote ID to respond to'),
      total: z.number().positive().describe('Total price'),
      fees: z.number().min(0).optional().describe('Processing/platform fees'),
      tax: z.number().min(0).optional().describe('Tax amount'),
      expiresInHours: z
        .number()
        .int()
        .min(1)
        .optional()
        .describe('Hours until quote expires (default: 48)'),
      terms: z.string().optional().describe('Terms and conditions'),
      estimatedDelivery: z
        .string()
        .optional()
        .describe('Estimated delivery time (e.g., "2 hours", "1 day")'),
      message: z.string().optional().describe('Message to buyer'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Providing quotes requires --apply flag.',
          wouldQuote: {
            quoteId: params.quoteId,
            total: params.total,
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { a2a } = await getA2AContext(commerce, agentConfig);

      const result = await a2a.provideQuote(params.quoteId, {
        total: params.total,
        fees: params.fees,
        tax: params.tax,
        expiresInHours: params.expiresInHours,
        terms: params.terms,
        estimatedDelivery: params.estimatedDelivery,
        message: params.message,
      });

      return {
        success: true,
        message: `Quote provided: ${params.total} ${result.quote.asset}`,
        quote: result.quote,
      };
    },
  },

  {
    name: 'a2a_accept_quote',
    description: 'Accept a quote and pay. Automatically sends payment to the seller.',
    inputSchema: {
      quoteId: z.string().min(1).describe('Quote ID to accept'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Accepting quotes requires --apply flag.',
          wouldAccept: { quoteId: params.quoteId },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { runtime, a2a } = await getA2AContext(commerce, agentConfig);
      const result =
        runtime && typeof runtime.acceptQuote === 'function'
          ? await runtime.acceptQuote(params.quoteId)
          : await a2a.acceptQuote(params.quoteId);

      return {
        success: true,
        message: `Quote accepted and paid: ${result.quote.total} ${result.quote.asset}`,
        payment: result.payment,
        quote: result.quote,
        viaRuntime: Boolean(runtime),
        settlementChains: runtime?.listSettlementChains?.() || [],
      };
    },
  },

  {
    name: 'a2a_decline_quote',
    description: 'Decline a quote.',
    inputSchema: {
      quoteId: z.string().min(1).describe('Quote ID to decline'),
      reason: z.string().optional().describe('Reason for declining'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Declining quotes requires --apply flag.',
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { a2a } = await getA2AContext(commerce, agentConfig);

      const result = await a2a.declineQuote(params.quoteId, params.reason);

      return {
        success: true,
        message: 'Quote declined',
        quote: result.quote,
      };
    },
  },

  {
    name: 'a2a_fulfill_quote',
    description: 'Mark a quote as fulfilled after delivering goods/services (for sellers).',
    inputSchema: {
      quoteId: z.string().min(1).describe('Quote ID to mark as fulfilled'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Fulfilling quotes requires --apply flag.',
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { a2a } = await getA2AContext(commerce, agentConfig);

      const result = await a2a.fulfillQuote(params.quoteId);

      return {
        success: true,
        message: 'Quote marked as fulfilled',
        quote: result.quote,
      };
    },
  },

  // ==========================================================================
  // Query Operations
  // ==========================================================================
  {
    name: 'a2a_get_payment',
    description:
      'Get a single A2A payment by ID. Optionally refresh native on-chain confirmation state for supported settlement networks including Bitcoin and shielded Zcash.',
    inputSchema: {
      paymentId: z.string().min(1).describe('Payment ID'),
      refreshOnChain: z
        .boolean()
        .optional()
        .describe(
          'Refresh the stored payment from live on-chain transaction status when a tx hash exists',
        ),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { runtime, a2a } = await getA2AContext(commerce, agentConfig);
      const result = params.refreshOnChain
        ? await a2a.refreshPayment(params.paymentId)
        : {
            success: true,
            refreshed: false,
            payment: await a2a.getPayment(params.paymentId),
            onChain: null,
            finality: (() => {
              try {
                return commerce?._finalityTracker?.getSettlementStatus(params.paymentId) || null;
              } catch {
                return null;
              }
            })(),
          };

      return {
        success: true,
        payment: result.payment,
        refreshed: Boolean(result.refreshed),
        reason: result.reason || null,
        onChain: result.onChain || null,
        finality: result.finality || null,
        viaRuntime: Boolean(runtime),
        settlementChains: runtime?.listSettlementChains?.() || [],
      };
    },
  },

  {
    name: 'a2a_list_payments',
    description:
      'List A2A payments sent or received by this agent. Can optionally refresh pending native-chain settlement state for payments with on-chain transaction hashes.',
    inputSchema: {
      direction: z
        .enum(['sent', 'received', 'all'])
        .optional()
        .describe('Filter by direction: sent, received, or all (default)'),
      asset: z
        .string()
        .optional()
        .describe('Filter by asset symbol, for example USDC, BTC, or ZEC'),
      network: z
        .string()
        .optional()
        .describe('Filter by settlement network, for example set_chain, bitcoin, or zcash'),
      status: z
        .string()
        .optional()
        .describe('Filter by status: pending, submitted, completed, failed'),
      refreshOnChain: z
        .boolean()
        .optional()
        .describe(
          'Refresh pending on-chain BTC/ZEC/EVM/Solana payment statuses before returning results',
        ),
      limit: z.number().int().min(1).max(500).optional().describe('Max results (default: 20)'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { runtime, a2a } = await getA2AContext(commerce, agentConfig);

      const filter = {
        sent: params.direction === 'sent',
        received: params.direction === 'received',
        asset: params.asset,
        network: params.network,
        status: params.status,
        refreshOnChain: params.refreshOnChain || false,
        limit: params.limit || 20,
      };

      const payments = await a2a.getPayments(filter);

      return {
        success: true,
        count: payments.length,
        refreshed: Boolean(params.refreshOnChain),
        payments,
        viaRuntime: Boolean(runtime),
        settlementChains: runtime?.listSettlementChains?.() || [],
      };
    },
  },

  {
    name: 'a2a_list_payment_requests',
    description: 'List payment requests created by or sent to this agent.',
    inputSchema: {
      direction: z.enum(['created', 'received', 'all']).optional().describe('Filter by direction'),
      status: z
        .string()
        .optional()
        .describe('Filter by status: pending, viewed, paid, declined, expired'),
      limit: z.number().int().min(1).max(500).optional().describe('Max results (default: 20)'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const filter = {
        created: params.direction === 'created',
        received: params.direction === 'received',
        status: params.status,
        limit: params.limit || 20,
      };

      const requests = await a2a.getPaymentRequests(filter);

      return {
        success: true,
        count: requests.length,
        requests,
      };
    },
  },

  {
    name: 'a2a_list_quotes',
    description: 'List quotes where this agent is buyer or seller.',
    inputSchema: {
      role: z.enum(['buyer', 'seller', 'all']).optional().describe('Filter by role'),
      status: z
        .string()
        .optional()
        .describe('Filter by status: requested, quoted, accepted, declined, fulfilled'),
      includeExpired: z.boolean().optional().describe('Include expired quotes'),
      limit: z.number().int().min(1).max(500).optional().describe('Max results (default: 20)'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const filter = {
        asBuyer: params.role === 'buyer',
        asSeller: params.role === 'seller',
        status: params.status,
        include_expired: params.includeExpired,
        limit: params.limit || 20,
      };

      const quotes = await a2a.getQuotes(filter);

      return {
        success: true,
        count: quotes.length,
        quotes,
      };
    },
  },

  {
    name: 'a2a_get_balance',
    description:
      'Get an A2A payment summary for this agent, with optional asset/network filters and per-rail breakdowns.',
    inputSchema: {
      asset: z.string().optional().describe('Optional asset filter, for example USDC, BTC, or ZEC'),
      network: z
        .string()
        .optional()
        .describe('Optional settlement network filter, for example set_chain, bitcoin, or zcash'),
      includeBreakdown: z
        .boolean()
        .optional()
        .describe('Include per-asset and per-network payment breakdowns (default: true)'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { runtime, a2a } = await getA2AContext(commerce, agentConfig);

      const balance = await a2a.getBalance({
        asset: params.asset,
        network: params.network,
        includeBreakdown: params.includeBreakdown,
      });

      return {
        success: true,
        balance,
        viaRuntime: Boolean(runtime),
        settlementChains: runtime?.listSettlementChains?.() || [],
      };
    },
  },

  // ==========================================================================
  // Discovery
  // ==========================================================================
  {
    name: 'a2a_discover_agents',
    description:
      'Discover AI agents that can provide goods or services. Find sellers, buyers, or agents with specific capabilities.',
    inputSchema: {
      skill: z
        .string()
        .optional()
        .describe('Required skill: sell, buy, quote, fulfill, ship, support'),
      network: z
        .string()
        .optional()
        .describe('Required network support: set_chain, base, ethereum, arbitrum, bitcoin, zcash'),
      asset: z
        .string()
        .optional()
        .describe('Required asset support: USDC, USDT, ssUSD, DAI, BTC, ZEC'),
      trustLevel: z
        .string()
        .optional()
        .describe('Minimum trust level: sandbox, standard, verified, enterprise'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const x402 = resolveCommerceApi(commerce, 'x402');
      const agents = await x402.discoverAgents(
        params.network,
        params.asset,
        params.skill,
        params.trustLevel,
      );

      return {
        success: true,
        count: agents.length,
        agents: agents.map((a) => ({
          id: a.id,
          name: a.name,
          walletAddress: a.wallet_address,
          trustLevel: a.trust_level,
          skills: a.a2a_skills,
          supportedAssets: a.supported_assets,
          supportedNetworks: a.supported_networks,
          paymentAddresses: parseJsonObject(a.payment_addresses),
          endpointUrl: a.endpoint_url,
          description: a.description,
        })),
      };
    },
  },

  // ==========================================================================
  // Negotiation
  // ==========================================================================
  {
    name: 'a2a_counter_quote',
    description:
      'Counter a quote with a different price (for buyers). Initiates or continues price negotiation with the seller.',
    inputSchema: {
      quoteId: z.string().min(1).describe('Quote ID to counter'),
      total: z.number().positive().describe('Proposed counter-offer total price'),
      message: z.string().optional().describe('Message to seller explaining the counter-offer'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Counter-offering quotes requires --apply flag.',
          wouldCounter: {
            quoteId: params.quoteId,
            total: params.total,
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured. Set up agent identity first.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const result = await a2a.counterQuote(params.quoteId, {
        total: params.total,
        message: params.message,
      });

      return {
        success: true,
        message: `Counter-offer submitted: ${params.total} (round ${result.round})`,
        quote: result.quote,
        round: result.round,
      };
    },
  },

  {
    name: 'a2a_revise_quote',
    description:
      'Revise a quote after a buyer counter-offer (for sellers). Adjusts pricing in response to negotiation.',
    inputSchema: {
      quoteId: z.string().min(1).describe('Quote ID to revise'),
      total: z.number().positive().describe('Revised total price'),
      fees: z.number().min(0).optional().describe('Revised processing/platform fees'),
      tax: z.number().min(0).optional().describe('Revised tax amount'),
      message: z.string().optional().describe('Message to buyer explaining the revision'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Revising quotes requires --apply flag.',
          wouldRevise: {
            quoteId: params.quoteId,
            total: params.total,
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured. Set up agent identity first.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const result = await a2a.reviseQuote(params.quoteId, {
        total: params.total,
        fees: params.fees,
        tax: params.tax,
        message: params.message,
      });

      return {
        success: true,
        message: `Quote revised: ${params.total} (round ${result.round})`,
        quote: result.quote,
        round: result.round,
      };
    },
  },

  // ==========================================================================
  // Escrow
  // ==========================================================================
  {
    name: 'a2a_create_escrow',
    description:
      'Create an escrow to hold funds between buyer and seller agents. Supports conditional release, time-based expiry, and dispute escalation.',
    inputSchema: {
      quoteId: z.string().optional().describe('Associated quote ID (optional)'),
      buyerAddress: z.string().min(1).describe('Buyer wallet address'),
      sellerAddress: z.string().min(1).describe('Seller wallet address'),
      amount: z
        .union([z.string().regex(/^(?:0|[1-9]\d*)(?:\.\d+)?$/), z.number().positive()])
        .describe(
          'Exact decimal string amount to escrow (recommended); JSON numbers are legacy-only',
        ),
      asset: z
        .string()
        .optional()
        .describe(
          'Asset to escrow: defaults to the selected network payment asset (e.g., USDC, ssUSD, BTC, ZEC)',
        ),
      network: z
        .string()
        .optional()
        .describe('Settlement network: set_chain, base, ethereum, arbitrum, bitcoin, zcash'),
      conditions: z
        .array(
          z.object({
            type: z
              .string()
              .describe('Condition type: seller_fulfilled, buyer_confirmed, time_lock, milestone'),
            quoteId: z.string().optional().describe('Quote ID for seller_fulfilled condition'),
            releaseAfter: z.string().optional().describe('ISO timestamp for time_lock condition'),
            description: z.string().optional().describe('Description for milestone condition'),
          }),
        )
        .max(20)
        .optional()
        .describe('Release conditions (optional)'),
      expiresInHours: z
        .number()
        .int()
        .min(1)
        .optional()
        .describe('Hours until escrow expires (default: 72)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Creating escrows requires --apply flag.',
          wouldCreate: {
            buyerAddress: params.buyerAddress,
            sellerAddress: params.sellerAddress,
            amount: params.amount,
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured. Set up agent identity first.',
        };
      }

      const { createEscrowService } = await import('../a2a/escrow.js');
      const escrowSvc = createEscrowService(commerce.a2a());

      const result = await escrowSvc.createEscrow({
        quoteId: params.quoteId,
        buyerAddress: params.buyerAddress,
        sellerAddress: params.sellerAddress,
        amount: params.amount,
        amountDecimal: params.amount,
        asset: params.asset,
        network: params.network,
        conditions: params.conditions,
        expiresInHours: params.expiresInHours,
      });

      return {
        success: true,
        message: `Escrow created for ${params.amount} ${params.asset || 'default asset'}`,
        escrow: result.escrow,
      };
    },
  },

  {
    name: 'a2a_fund_escrow',
    description: 'Fund an escrow, moving it to active status so the seller can begin work.',
    inputSchema: {
      escrowId: z.string().min(1).describe('Escrow ID to fund'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Funding escrows requires --apply flag.',
          wouldFund: { escrowId: params.escrowId },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { createEscrowService } = await import('../a2a/escrow.js');
      const escrowSvc = createEscrowService(commerce.a2a());

      const result = await escrowSvc.fundEscrow(params.escrowId);

      return {
        success: true,
        message: 'Escrow funded and active',
        escrow: result.escrow,
      };
    },
  },

  {
    name: 'a2a_release_escrow',
    description: 'Release escrow funds to the seller. All release conditions must be met.',
    inputSchema: {
      escrowId: z.string().min(1).describe('Escrow ID to release'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Releasing escrows requires --apply flag.',
          wouldRelease: { escrowId: params.escrowId },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { createEscrowService } = await import('../a2a/escrow.js');
      const escrowSvc = createEscrowService(commerce.a2a());

      const result = await escrowSvc.releaseEscrow(params.escrowId);

      if (!result.success) {
        return {
          success: false,
          error: result.error,
          unmetConditions: result.unmetConditions,
          conditions: result.conditions,
        };
      }

      return {
        success: true,
        message: 'Escrow funds released to seller',
        escrow: result.escrow,
      };
    },
  },

  {
    name: 'a2a_refund_escrow',
    description: 'Refund escrow funds back to the buyer.',
    inputSchema: {
      escrowId: z.string().min(1).describe('Escrow ID to refund'),
      reason: z.string().min(1).max(500).optional().describe('Auditable refund reason'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Refunding escrows requires --apply flag.',
          wouldRefund: { escrowId: params.escrowId },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { createEscrowService } = await import('../a2a/escrow.js');
      const escrowSvc = createEscrowService(commerce.a2a());

      const result = await escrowSvc.refundEscrow(params.escrowId);

      return {
        success: true,
        message: 'Escrow funds refunded to buyer',
        escrow: result.escrow,
      };
    },
  },

  {
    name: 'a2a_dispute_escrow',
    description: 'Dispute an escrow, escalating it to the dispute resolution system.',
    inputSchema: {
      escrowId: z.string().min(1).describe('Escrow ID to dispute'),
      reason: z.string().min(1).max(500).describe('Reason for the dispute'),
      category: z
        .enum([
          'non_delivery',
          'poor_quality',
          'not_as_described',
          'overcharged',
          'unauthorized',
          'other',
        ])
        .optional()
        .describe('Dispute category (default: other)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Disputing escrows requires --apply flag.',
          wouldDispute: { escrowId: params.escrowId, reason: params.reason },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { createEscrowService } = await import('../a2a/escrow.js');
      const escrowSvc = createEscrowService(commerce.a2a());

      const result = await escrowSvc.disputeEscrow(params.escrowId, {
        reason: params.reason,
        category: params.category,
      });

      return {
        success: true,
        message: 'Escrow disputed. File a formal dispute to begin resolution.',
        escrow: result.escrow,
        disputeNeeded: result.disputeNeeded,
      };
    },
  },

  {
    name: 'a2a_get_escrow',
    description: 'Get details of an escrow by ID.',
    inputSchema: {
      escrowId: z.string().min(1).describe('Escrow ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { createEscrowService } = await import('../a2a/escrow.js');
      const escrowSvc = createEscrowService(commerce.a2a());

      const escrow = await escrowSvc.getEscrow(params.escrowId);

      if (!escrow) {
        return { success: false, error: 'Escrow not found' };
      }

      return {
        success: true,
        escrow,
      };
    },
  },

  {
    name: 'a2a_list_escrows',
    description: 'List escrows with optional filters.',
    inputSchema: {
      status: z
        .string()
        .optional()
        .describe(
          'Filter by status: created, funded, active, released, refunded, disputed, expired',
        ),
      role: z
        .enum(['buyer', 'seller', 'all'])
        .optional()
        .describe('Filter by role: buyer, seller, or all'),
      limit: z.number().int().min(1).max(500).optional().describe('Max results (default: 20)'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      const { createEscrowService } = await import('../a2a/escrow.js');
      const escrowSvc = createEscrowService(commerce.a2a());

      const filter = {
        status: params.status,
        limit: params.limit || 20,
      };

      if (agentConfig?.walletAddress && params.role === 'buyer') {
        filter.buyer_address = agentConfig.walletAddress;
      } else if (agentConfig?.walletAddress && params.role === 'seller') {
        filter.seller_address = agentConfig.walletAddress;
      }

      const escrows = await escrowSvc.listEscrows(filter);

      return {
        success: true,
        count: escrows.length,
        escrows,
      };
    },
  },

  // ==========================================================================
  // Disputes
  // ==========================================================================
  {
    name: 'a2a_file_dispute',
    description:
      'File a formal dispute against an escrow. Begins the dispute resolution process with evidence collection and review.',
    inputSchema: {
      escrowId: z.string().min(1).describe('Escrow ID being disputed'),
      reason: z.string().min(1).describe('Detailed reason for the dispute'),
      category: z
        .enum([
          'non_delivery',
          'poor_quality',
          'not_as_described',
          'overcharged',
          'unauthorized',
          'other',
        ])
        .optional()
        .describe('Dispute category (default: other)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Filing disputes requires --apply flag.',
          wouldFile: { escrowId: params.escrowId, reason: params.reason },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured. Set up agent identity first.',
        };
      }

      const { createDisputeService } = await import('../a2a/disputes.js');
      const disputeSvc = createDisputeService(commerce.a2a());

      // Determine who is being filed against by looking up the escrow
      const { createEscrowService } = await import('../a2a/escrow.js');
      const escrowSvc = createEscrowService(commerce.a2a());
      const escrow = await escrowSvc.getEscrow(params.escrowId);

      if (!escrow) {
        return { success: false, error: 'Escrow not found' };
      }

      const filedBy = agentConfig.walletAddress;
      const filedAgainst =
        escrow.buyerAddress === filedBy ? escrow.sellerAddress : escrow.buyerAddress;

      const result = await disputeSvc.fileDispute({
        escrowId: params.escrowId,
        filedBy,
        filedAgainst,
        reason: params.reason,
        category: params.category || 'other',
      });

      return {
        success: true,
        message: 'Dispute filed. Evidence period begins now.',
        dispute: result.dispute,
      };
    },
  },

  {
    name: 'a2a_submit_evidence',
    description: 'Submit evidence for an active dispute.',
    inputSchema: {
      disputeId: z.string().min(1).describe('Dispute ID'),
      evidenceType: z
        .string()
        .min(1)
        .describe(
          'Type of evidence: screenshot, transaction_log, communication, delivery_proof, other',
        ),
      title: z.string().min(1).describe('Evidence title'),
      description: z.string().optional().describe('Evidence description'),
      content: z.string().min(1).describe('Evidence content (text, base64 data, or reference)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Submitting evidence requires --apply flag.',
          wouldSubmit: { disputeId: params.disputeId, title: params.title },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { createDisputeService } = await import('../a2a/disputes.js');
      const disputeSvc = createDisputeService(commerce.a2a());

      const result = await disputeSvc.submitEvidence(params.disputeId, {
        submittedBy: agentConfig.walletAddress,
        evidenceType: params.evidenceType,
        title: params.title,
        description: params.description,
        content: params.content,
      });

      return {
        success: true,
        message: 'Evidence submitted',
        evidence: result.evidence,
      };
    },
  },

  {
    name: 'a2a_resolve_dispute',
    description:
      'Resolve a dispute atomically with a full refund, seller release, exact split, or escalation.',
    inputSchema: {
      disputeId: z.string().min(1).describe('Dispute ID to resolve'),
      resolutionType: z
        .enum(['full_refund', 'partial_refund', 'release_to_seller', 'split', 'escalated'])
        .describe('How to resolve the dispute'),
      amount: z
        .number()
        .positive()
        .optional()
        .describe('Legacy amount for partial_refund; unavailable in strict kernel mode'),
      buyerAmount: z
        .string()
        .regex(/^\d+(?:\.\d+)?$/)
        .optional()
        .describe('Exact decimal buyer allocation required for split'),
      sellerAmount: z
        .string()
        .regex(/^\d+(?:\.\d+)?$/)
        .optional()
        .describe('Exact decimal seller allocation required for split'),
      note: z.string().optional().describe('Resolution note'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Resolving disputes requires --apply flag.',
          wouldResolve: { disputeId: params.disputeId, resolutionType: params.resolutionType },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { createDisputeService } = await import('../a2a/disputes.js');
      const disputeSvc = createDisputeService(commerce.a2a());

      const result = await disputeSvc.resolveDispute(params.disputeId, {
        resolutionType: params.resolutionType,
        amount: params.amount ?? (params.buyerAmount ? Number(params.buyerAmount) : undefined),
        note: params.note,
        resolvedBy: agentConfig.walletAddress,
      });

      return {
        success: true,
        message: `Dispute resolved: ${params.resolutionType}`,
        dispute: result.dispute,
        escrowAction: result.escrowAction,
      };
    },
  },

  {
    name: 'a2a_get_dispute',
    description: 'Get details of a dispute by ID, including evidence count.',
    inputSchema: {
      disputeId: z.string().min(1).describe('Dispute ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { createDisputeService } = await import('../a2a/disputes.js');
      const disputeSvc = createDisputeService(commerce.a2a());

      try {
        const result = await disputeSvc.getDispute(params.disputeId);
        return {
          success: true,
          dispute: result.dispute,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'a2a_list_disputes',
    description: 'List disputes with optional filters.',
    inputSchema: {
      status: z
        .string()
        .optional()
        .describe('Filter by status: filed, evidence_period, under_review, resolved, escalated'),
      limit: z.number().int().min(1).max(500).optional().describe('Max results (default: 20)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { createDisputeService } = await import('../a2a/disputes.js');
      const disputeSvc = createDisputeService(commerce.a2a());

      const filter = {
        status: params.status,
        limit: params.limit || 20,
      };

      const disputes = await disputeSvc.listDisputes(filter);

      return {
        success: true,
        count: disputes.length,
        disputes,
      };
    },
  },

  // ==========================================================================
  // Reputation
  // ==========================================================================
  {
    name: 'a2a_rate_agent',
    description:
      'Rate an agent after a transaction. Scores 1-5 with optional dimension ratings (reliability, quality, speed, communication).',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Wallet address of the agent to rate'),
      transactionType: z
        .enum(['quote', 'payment', 'escrow', 'service'])
        .describe('Type of transaction being rated'),
      transactionId: z
        .string()
        .min(1)
        .describe('ID of the transaction (quote, payment, escrow, or service ID)'),
      score: z.number().int().min(1).max(5).describe('Overall score (1-5)'),
      dimensions: z
        .object({
          reliability: z.number().int().min(1).max(5).optional(),
          quality: z.number().int().min(1).max(5).optional(),
          speed: z.number().int().min(1).max(5).optional(),
          communication: z.number().int().min(1).max(5).optional(),
        })
        .optional()
        .describe('Dimension scores (each 1-5)'),
      comment: z.string().max(1000).optional().describe('Optional review comment'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Rating agents requires --apply flag.',
          wouldRate: {
            agentAddress: params.agentAddress,
            score: params.score,
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured. Set up agent identity first.',
        };
      }

      const { createReputationService } = await import('../a2a/reputation.js');
      const repSvc = createReputationService(commerce.a2a());

      const result = await repSvc.rateAgent({
        agentAddress: params.agentAddress,
        reviewerAddress: agentConfig.walletAddress,
        transactionType: params.transactionType,
        transactionId: params.transactionId,
        score: params.score,
        dimensions: params.dimensions,
        comment: params.comment,
      });

      return {
        success: true,
        message: `Rated agent ${params.agentAddress}: ${params.score}/5`,
        feedback: result.feedback,
        reputationUpdated: result.reputationUpdated,
      };
    },
  },

  {
    name: 'a2a_get_reputation',
    description: 'Get reputation and trust score for an agent.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Wallet address of the agent'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { createReputationService } = await import('../a2a/reputation.js');
      const repSvc = createReputationService(commerce.a2a());

      const result = await repSvc.getReputation(params.agentAddress);

      return {
        success: true,
        reputation: result.reputation,
      };
    },
  },

  {
    name: 'a2a_respond_to_feedback',
    description: 'Respond to feedback left on your agent (only the rated agent can respond).',
    inputSchema: {
      feedbackId: z.string().min(1).describe('Feedback ID to respond to'),
      response: z.string().min(1).max(2000).describe('Response text'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Responding to feedback requires --apply flag.',
          wouldRespond: { feedbackId: params.feedbackId },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured.',
        };
      }

      const { createReputationService } = await import('../a2a/reputation.js');
      const repSvc = createReputationService(commerce.a2a());

      const result = await repSvc.respondToFeedback(params.feedbackId, {
        response: params.response,
        responderAddress: agentConfig.walletAddress,
      });

      return {
        success: true,
        message: 'Response posted',
        feedback: result.feedback,
      };
    },
  },

  // ==========================================================================
  // Services
  // ==========================================================================
  {
    name: 'a2a_register_service',
    description:
      'Register a service that this agent provides. Other agents can discover and purchase your services.',
    inputSchema: {
      name: z.string().min(1).describe('Service name'),
      description: z.string().min(1).describe('Service description'),
      category: z
        .enum(['data', 'compute', 'api', 'content', 'analysis', 'goods', 'digital_goods', 'other'])
        .describe('Service category'),
      pricingModel: z
        .enum(['fixed', 'per_unit', 'tiered', 'quote', 'freemium'])
        .describe('Pricing model'),
      pricingDetails: z
        .object({
          basePrice: z.number().positive().optional(),
          currency: z.string().optional(),
          unitName: z.string().optional(),
          tiers: z
            .array(
              z.object({
                upTo: z.number(),
                price: z.number(),
              }),
            )
            .optional(),
        })
        .optional()
        .describe('Pricing details (structure depends on pricing model)'),
      endpointUrl: z.string().url().optional().describe('Service endpoint URL'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Registering services requires --apply flag.',
          wouldRegister: {
            name: params.name,
            category: params.category,
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          success: false,
          error: 'Agent wallet not configured. Set up agent identity first.',
        };
      }

      const store = commerce.a2a();

      const service = store.createService({
        agent_address: agentConfig.walletAddress,
        name: params.name,
        description: params.description,
        category: params.category,
        pricing_model: params.pricingModel,
        pricing_details: params.pricingDetails ? JSON.stringify(params.pricingDetails) : null,
        endpoint_url: params.endpointUrl || null,
      });

      return {
        success: true,
        message: `Service registered: ${params.name}`,
        service: {
          id: service.id,
          name: service.name,
          description: service.description,
          category: service.category,
          pricingModel: service.pricing_model,
          agentAddress: service.agent_address,
          endpointUrl: service.endpoint_url,
          createdAt: service.created_at,
        },
      };
    },
  },

  {
    name: 'a2a_list_services',
    description: 'List available agent services with optional filters and search.',
    inputSchema: {
      category: z
        .string()
        .optional()
        .describe(
          'Filter by category: data, compute, api, content, analysis, goods, digital_goods, other',
        ),
      agentAddress: z.string().optional().describe('Filter by agent wallet address'),
      search: z.string().optional().describe('Search services by name or description'),
      limit: z.number().int().min(1).max(500).optional().describe('Max results (default: 20)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const store = commerce.a2a();

      const services = store.listServices({
        category: params.category,
        agent_address: params.agentAddress,
        search: params.search,
        active: true,
        limit: params.limit || 20,
      });

      return {
        success: true,
        count: services.length,
        services: services.map((s) => ({
          id: s.id,
          name: s.name,
          description: s.description,
          category: s.category,
          pricingModel: s.pricing_model,
          agentAddress: s.agent_address,
          endpointUrl: s.endpoint_url,
          transactionCount: s.transaction_count,
          successRate: s.success_rate,
          createdAt: s.created_at,
        })),
      };
    },
  },

  {
    name: 'a2a_get_service',
    description: 'Get details of a specific agent service.',
    inputSchema: {
      serviceId: z.string().min(1).describe('Service ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const store = commerce.a2a();

      const service = store.getService(params.serviceId);

      if (!service) {
        return { success: false, error: 'Service not found' };
      }

      return {
        success: true,
        service: {
          id: service.id,
          name: service.name,
          description: service.description,
          category: service.category,
          pricingModel: service.pricing_model,
          pricingDetails: service.pricing_details ? JSON.parse(service.pricing_details) : null,
          agentAddress: service.agent_address,
          endpointUrl: service.endpoint_url,
          active: service.active,
          transactionCount: service.transaction_count,
          successRate: service.success_rate,
          avgResponseTime: service.avg_response_time,
          createdAt: service.created_at,
          updatedAt: service.updated_at,
        },
      };
    },
  },
  // ==========================================================================
  // Notifications
  // ==========================================================================
  {
    name: 'a2a_send_notification',
    description:
      'Send a webhook notification to another agent. Delivers a signed payload to their configured endpoint.',
    inputSchema: {
      recipientAddress: z.string().min(1).describe('Recipient agent wallet address'),
      eventType: z
        .string()
        .min(1)
        .describe('Event type (e.g., "payment.completed", "escrow.released")'),
      payload: z.record(z.unknown()).describe('Event payload to deliver'),
      endpointUrl: z
        .string()
        .url()
        .optional()
        .describe('Override endpoint URL (bypasses config lookup)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Sending notifications requires --apply flag.',
          wouldSend: {
            recipientAddress: params.recipientAddress,
            eventType: params.eventType,
          },
        };
      }

      const { createNotificationService } = await import('../a2a/notifications.js');
      const notifSvc = createNotificationService(commerce.a2a());

      const result = await notifSvc.sendNotification({
        recipientAddress: params.recipientAddress,
        eventType: params.eventType,
        payload: params.payload,
        endpointUrl: params.endpointUrl,
      });

      return {
        success: true,
        message: `Notification sent to ${params.recipientAddress}`,
        notification: result,
      };
    },
  },

  {
    name: 'a2a_list_notification_log',
    description: 'View the webhook notification delivery log with optional filters.',
    inputSchema: {
      recipientAddress: z.string().optional().describe('Filter by recipient address'),
      eventType: z.string().optional().describe('Filter by event type'),
      status: z
        .enum(['pending', 'delivered', 'failed'])
        .optional()
        .describe('Filter by status: pending, delivered, failed'),
      limit: z.number().int().min(1).max(500).optional().describe('Max results (default: 20)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { createNotificationService } = await import('../a2a/notifications.js');
      const notifSvc = createNotificationService(commerce.a2a());

      const logs = await notifSvc.getNotificationLog({
        recipient_address: params.recipientAddress,
        event_type: params.eventType,
        status: params.status,
        limit: params.limit || 20,
      });

      return {
        success: true,
        count: logs.length,
        notifications: logs,
      };
    },
  },

  {
    name: 'a2a_configure_webhooks',
    description:
      'Configure webhook settings for an agent. Set the endpoint URL, signing secret, and which event types to receive.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address to configure'),
      endpointUrl: z.string().url().describe('Webhook endpoint URL (https:// recommended)'),
      secret: z.string().optional().describe('HMAC-SHA256 signing secret for payload verification'),
      enabledEvents: z
        .array(z.string())
        .max(50)
        .optional()
        .describe('Event types to receive (default: ["*"] for all)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Configuring webhooks requires --apply flag.',
          wouldConfigure: {
            agentAddress: params.agentAddress,
            endpointUrl: params.endpointUrl,
          },
        };
      }

      const { createNotificationService } = await import('../a2a/notifications.js');
      const notifSvc = createNotificationService(commerce.a2a());

      const result = await notifSvc.configureWebhooks({
        agentAddress: params.agentAddress,
        endpointUrl: params.endpointUrl,
        secret: params.secret,
        enabledEvents: params.enabledEvents,
      });

      return {
        success: true,
        message: `Webhooks configured for ${params.agentAddress}`,
        config: result,
      };
    },
  },

  // ==========================================================================
  // Webhook Dead Letter Queue (DLQ)
  // ==========================================================================
  {
    name: 'a2a_list_webhook_dlq',
    description:
      'List quarantined webhook notifications that permanently failed delivery. Use to inspect and replay failed deliveries.',
    inputSchema: {
      recipientAddress: z.string().optional().describe('Filter by recipient agent address'),
      eventType: z.string().optional().describe('Filter by event type'),
      limit: z.number().int().positive().max(200).default(50).describe('Max entries to return'),
      offset: z.number().int().min(0).default(0).describe('Pagination offset'),
    },
    permission: 'admin',
    handler: async ({ params, commerce }) => {
      const store = commerce.a2a();
      const entries = store.listDLQ({
        recipient_address: params.recipientAddress,
        event_type: params.eventType,
        limit: params.limit,
        offset: params.offset,
      });
      const count = store.countDLQ({
        recipient_address: params.recipientAddress,
        event_type: params.eventType,
      });
      return {
        success: true,
        entries,
        count: entries.length,
        totalCount: count,
      };
    },
  },
  {
    name: 'a2a_quarantine_failed_webhooks',
    description:
      'Move permanently failed webhook notifications to the dead letter queue. Notifications that exhausted all retry attempts are quarantined for inspection.',
    inputSchema: {
      limit: z
        .number()
        .int()
        .positive()
        .max(500)
        .default(100)
        .describe('Max notifications to quarantine'),
    },
    permission: 'admin',
    handler: async ({ params, commerce, applyMode }) => {
      if (!applyMode) {
        return {
          success: false,
          error: 'Quarantining failed webhooks requires --apply flag.',
        };
      }
      const store = commerce.a2a();
      const result = store.quarantineFailedNotifications({ limit: params.limit });
      return {
        success: true,
        message: `Quarantined ${result.quarantined} failed notification(s) to DLQ`,
        ...result,
      };
    },
  },
  {
    name: 'a2a_replay_dlq_entry',
    description:
      'Replay a dead letter queue entry by moving it back to the notification log for retry. Resets the attempt counter.',
    inputSchema: {
      dlqId: z.string().min(1).describe('DLQ entry ID to replay'),
    },
    permission: 'admin',
    handler: async ({ params, commerce, applyMode }) => {
      if (!applyMode) {
        return {
          success: false,
          error: 'Replaying DLQ entries requires --apply flag.',
        };
      }
      const store = commerce.a2a();
      const result = store.replayDLQEntry(params.dlqId);
      if (!result.replayed) {
        return { success: false, error: `DLQ entry ${params.dlqId} not found` };
      }
      return {
        success: true,
        message: `Replayed DLQ entry ${params.dlqId} — notification re-queued for retry`,
        ...result,
      };
    },
  },
  {
    name: 'a2a_purge_dlq',
    description:
      'Purge old dead letter queue entries. Removes entries quarantined more than the specified number of days ago.',
    inputSchema: {
      olderThanDays: z
        .number()
        .int()
        .positive()
        .max(365)
        .default(30)
        .describe('Remove entries older than this many days'),
    },
    permission: 'admin',
    handler: async ({ params, commerce, applyMode }) => {
      if (!applyMode) {
        return {
          success: false,
          error: 'Purging DLQ requires --apply flag.',
        };
      }
      const store = commerce.a2a();
      const result = store.purgeDLQ({ olderThanDays: params.olderThanDays });
      return {
        success: true,
        message: `Purged ${result.purged} DLQ entries older than ${params.olderThanDays} days`,
        ...result,
      };
    },
  },
  {
    name: 'a2a_dlq_count',
    description: 'Get the count of entries in the webhook dead letter queue.',
    inputSchema: {
      recipientAddress: z.string().optional().describe('Filter by recipient agent address'),
      eventType: z.string().optional().describe('Filter by event type'),
    },
    permission: 'read',
    handler: async ({ params, commerce }) => {
      const store = commerce.a2a();
      const count = store.countDLQ({
        recipient_address: params.recipientAddress,
        event_type: params.eventType,
      });
      return { success: true, count };
    },
  },

  // ==========================================================================
  // Agent Subscriptions (Recurring A2A Payments)
  // ==========================================================================
  {
    name: 'a2a_create_agent_subscription',
    description:
      'Create a recurring payment subscription between two agents. Supports trial periods and configurable billing intervals.',
    inputSchema: {
      subscriberAddress: z.string().min(1).describe('Subscriber agent wallet address'),
      providerAddress: z.string().min(1).describe('Provider agent wallet address'),
      serviceId: z.string().optional().describe('Associated service ID'),
      planName: z.string().min(1).describe('Human-readable plan name (e.g., "Pro Plan")'),
      amount: z.number().positive().describe('Amount per billing cycle (e.g., 49.99)'),
      asset: z
        .string()
        .optional()
        .describe(
          'Asset: defaults to the selected network payment asset (e.g., USDC, ssUSD, BTC, ZEC)',
        ),
      network: z
        .string()
        .optional()
        .describe('Network: set_chain, base, ethereum, arbitrum, bitcoin, zcash'),
      billingInterval: z
        .enum(['weekly', 'biweekly', 'monthly', 'quarterly', 'annual'])
        .optional()
        .describe('Billing interval (default: monthly)'),
      trialDays: z.number().int().min(0).optional().describe('Trial period in days (0 = no trial)'),
      maxPastDueCycles: z
        .number()
        .int()
        .min(0)
        .optional()
        .describe('Max past-due billing cycles before cancellation (default: 3)'),
      metadata: z.record(z.unknown()).optional().describe('Additional metadata'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Creating subscriptions requires --apply flag.',
          wouldCreate: {
            subscriberAddress: params.subscriberAddress,
            providerAddress: params.providerAddress,
            planName: params.planName,
            amount: params.amount,
          },
        };
      }

      const { createA2ASubscriptionService } = await import('../a2a/subscriptions.js');
      const subSvc = createA2ASubscriptionService(commerce.a2a());

      const result = await subSvc.createSubscription(params);

      return {
        success: true,
        message: `Subscription created: ${params.planName} at ${params.amount} ${params.asset || 'default asset'}/${params.billingInterval || 'monthly'}`,
        subscription: result.subscription,
      };
    },
  },

  {
    name: 'a2a_pause_agent_subscription',
    description: 'Pause an active agent subscription. Billing is suspended until resumed.',
    inputSchema: {
      subscriptionId: z.string().min(1).describe('Subscription ID to pause'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Pausing subscriptions requires --apply flag.',
          wouldPause: { subscriptionId: params.subscriptionId },
        };
      }

      const { createA2ASubscriptionService } = await import('../a2a/subscriptions.js');
      const subSvc = createA2ASubscriptionService(commerce.a2a());

      const result = await subSvc.pauseSubscription(params.subscriptionId);

      return {
        success: true,
        message: 'Subscription paused',
        subscription: result.subscription,
      };
    },
  },

  {
    name: 'a2a_resume_agent_subscription',
    description: 'Resume a paused agent subscription. Recalculates billing dates from now.',
    inputSchema: {
      subscriptionId: z.string().min(1).describe('Subscription ID to resume'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Resuming subscriptions requires --apply flag.',
          wouldResume: { subscriptionId: params.subscriptionId },
        };
      }

      const { createA2ASubscriptionService } = await import('../a2a/subscriptions.js');
      const subSvc = createA2ASubscriptionService(commerce.a2a());

      const result = await subSvc.resumeSubscription(params.subscriptionId);

      return {
        success: true,
        message: 'Subscription resumed',
        subscription: result.subscription,
      };
    },
  },

  {
    name: 'a2a_cancel_agent_subscription',
    description:
      'Cancel an agent subscription. Can cancel immediately or at the end of the current billing period.',
    inputSchema: {
      subscriptionId: z.string().min(1).describe('Subscription ID to cancel'),
      immediate: z
        .boolean()
        .optional()
        .describe('Cancel immediately (default: true) or at period end'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Cancelling subscriptions requires --apply flag.',
          wouldCancel: { subscriptionId: params.subscriptionId },
        };
      }

      const { createA2ASubscriptionService } = await import('../a2a/subscriptions.js');
      const subSvc = createA2ASubscriptionService(commerce.a2a());

      const result = await subSvc.cancelSubscription(params.subscriptionId, {
        immediate: params.immediate !== false,
      });

      return {
        success: true,
        message:
          params.immediate !== false
            ? 'Subscription cancelled immediately'
            : 'Subscription will cancel at end of billing period',
        subscription: result.subscription,
      };
    },
  },

  {
    name: 'a2a_get_agent_subscription',
    description: 'Get details of an agent-to-agent subscription.',
    inputSchema: {
      subscriptionId: z.string().min(1).describe('Subscription ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { createA2ASubscriptionService } = await import('../a2a/subscriptions.js');
      const subSvc = createA2ASubscriptionService(commerce.a2a());

      try {
        const subscription = await subSvc.getSubscription(params.subscriptionId);
        return { success: true, subscription };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'a2a_list_agent_subscriptions',
    description: 'List agent-to-agent subscriptions with optional filters.',
    inputSchema: {
      subscriberAddress: z.string().optional().describe('Filter by subscriber wallet address'),
      providerAddress: z.string().optional().describe('Filter by provider wallet address'),
      status: z
        .string()
        .optional()
        .describe('Filter by status: active, paused, cancelled, trial, past_due'),
      limit: z.number().int().min(1).max(500).optional().describe('Max results (default: 20)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { createA2ASubscriptionService } = await import('../a2a/subscriptions.js');
      const subSvc = createA2ASubscriptionService(commerce.a2a());

      const subscriptions = await subSvc.listSubscriptions({
        subscriberAddress: params.subscriberAddress,
        providerAddress: params.providerAddress,
        status: params.status,
        limit: params.limit || 20,
      });

      return {
        success: true,
        count: subscriptions.length,
        subscriptions,
      };
    },
  },

  {
    name: 'a2a_process_subscription_billing',
    description:
      'Process all due subscription billing cycles. Bills active subscriptions, handles past-due retries, transitions expired trials, and cancels end-of-period subscriptions.',
    inputSchema: {},
    permission: 'write',
    handler: async ({ commerce, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Processing billing requires --apply flag.',
        };
      }

      const { findRuntimeByWalletAddress } = await import('./agent-runtime.js');
      const runtime = findRuntimeByWalletAddress(agentConfig?.walletAddress);
      if (runtime && typeof runtime.processSubscriptionBilling === 'function') {
        const result = await runtime.processSubscriptionBilling();
        return {
          success: true,
          message: `Processed ${result.processed} subscriptions: ${result.succeeded} succeeded, ${result.failed} failed, ${result.cancelled} cancelled`,
          billing: result,
          viaRuntime: true,
          settlementChains: runtime.listSettlementChains?.() || [],
        };
      }

      const { createA2ASubscriptionService } = await import('../a2a/subscriptions.js');
      const subSvc = createA2ASubscriptionService(commerce.a2a());

      const result = await subSvc.processBilling();

      return {
        success: true,
        message: `Processed ${result.processed} subscriptions: ${result.succeeded} succeeded, ${result.failed} failed, ${result.cancelled} cancelled`,
        billing: result,
        viaRuntime: false,
      };
    },
  },

  // ==========================================================================
  // Split Payments
  // ==========================================================================
  {
    name: 'a2a_create_split_payment',
    description:
      'Create a multi-party split payment. Splits a payment across 2+ recipients by percentage or fixed amounts, with optional platform fee.',
    inputSchema: {
      senderAddress: z.string().min(1).describe('Sender wallet address'),
      totalAmount: z.number().positive().describe('Total amount to split (e.g., 100.00)'),
      asset: z
        .string()
        .optional()
        .describe(
          'Asset: defaults to the selected network payment asset (e.g., USDC, ssUSD, BTC, ZEC)',
        ),
      network: z
        .string()
        .optional()
        .describe('Network: set_chain, base, ethereum, arbitrum, bitcoin, zcash'),
      splitType: z
        .enum(['percentage', 'fixed'])
        .optional()
        .describe('Split type: percentage (default) or fixed amounts'),
      recipients: z
        .array(
          z.object({
            address: z.string().min(1).describe('Recipient wallet address'),
            percent: z
              .number()
              .min(0)
              .max(100)
              .optional()
              .describe('Share percentage (for percentage splits)'),
            amount: z.number().positive().optional().describe('Fixed amount (for fixed splits)'),
          }),
        )
        .min(2)
        .max(20)
        .describe('Recipients (min 2). For percentage: percents must sum to 100'),
      platformFeePercent: z
        .number()
        .min(0)
        .max(100)
        .optional()
        .describe('Platform fee percentage (0-100, deducted before split)'),
      platformFeeAddress: z.string().optional().describe('Platform fee recipient address'),
      memo: z.string().max(500).optional().describe('Payment memo'),
      referenceType: z.string().optional().describe('Reference entity type (e.g., "order")'),
      referenceId: z.string().optional().describe('Reference entity ID'),
      metadata: z.record(z.unknown()).optional().describe('Additional metadata'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Creating split payments requires --apply flag.',
          wouldCreate: {
            senderAddress: params.senderAddress,
            totalAmount: params.totalAmount,
            recipients: params.recipients?.length,
          },
        };
      }

      const { createSplitPaymentService } = await import('../a2a/splits.js');
      const splitSvc = createSplitPaymentService(commerce.a2a());

      const result = await splitSvc.createSplitPayment(params);

      return {
        success: true,
        message: `Split payment created: ${params.totalAmount} ${params.asset || 'default asset'} across ${params.recipients.length} recipients`,
        splitPayment: result.splitPayment,
      };
    },
  },

  {
    name: 'a2a_execute_split_payment',
    description:
      'Execute a pending split payment, sending funds to each recipient. Tracks per-recipient status.',
    inputSchema: {
      splitPaymentId: z.string().min(1).describe('Split payment ID to execute'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Executing split payments requires --apply flag.',
          wouldExecute: { splitPaymentId: params.splitPaymentId },
        };
      }

      if (!agentConfig?.walletAddress) {
        return { success: false, error: 'Agent wallet not configured.' };
      }

      const { findRuntimeByWalletAddress } = await import('./agent-runtime.js');
      const runtime = findRuntimeByWalletAddress(agentConfig.walletAddress);

      let result;
      if (runtime && typeof runtime.executeSplitDeal === 'function') {
        result = await runtime.executeSplitDeal(params.splitPaymentId);
      } else {
        const { createSplitPaymentService } = await import('../a2a/splits.js');
        const splitSvc = createSplitPaymentService(commerce.a2a());

        const { createA2AService } = await import('../a2a/index.js');
        const a2a = createA2AService(commerce, agentConfig);

        result = await splitSvc.executeSplitPayment(
          params.splitPaymentId,
          async (to, amount, asset, network, memo) => {
            const payResult = await a2a.pay({ to, amount, asset, network, memo });
            return payResult.payment;
          },
        );
      }

      return {
        success: result.success,
        message: result.success
          ? 'Split payment completed successfully'
          : 'Split payment partially completed',
        splitPayment: result.splitPayment,
        viaRuntime: Boolean(runtime),
        settlementChains: runtime?.listSettlementChains?.() || [],
      };
    },
  },

  {
    name: 'a2a_get_split_payment',
    description: 'Get details of a split payment including all recipient shares and statuses.',
    inputSchema: {
      splitPaymentId: z.string().min(1).describe('Split payment ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { createSplitPaymentService } = await import('../a2a/splits.js');
      const splitSvc = createSplitPaymentService(commerce.a2a());

      const splitPayment = await splitSvc.getSplitPayment(params.splitPaymentId);

      if (!splitPayment) {
        return { success: false, error: 'Split payment not found' };
      }

      return { success: true, splitPayment };
    },
  },

  {
    name: 'a2a_list_split_payments',
    description: 'List split payments with optional filters.',
    inputSchema: {
      senderAddress: z.string().optional().describe('Filter by sender wallet address'),
      status: z
        .string()
        .optional()
        .describe('Filter by status: pending, processing, completed, partial, failed'),
      limit: z.number().int().min(1).max(500).optional().describe('Max results (default: 20)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { createSplitPaymentService } = await import('../a2a/splits.js');
      const splitSvc = createSplitPaymentService(commerce.a2a());

      const splits = await splitSvc.listSplitPayments({
        senderAddress: params.senderAddress,
        status: params.status,
        limit: params.limit || 20,
      });

      return {
        success: true,
        count: splits.length,
        splitPayments: splits,
      };
    },
  },

  // ==========================================================================
  // Conditional Payments
  // ==========================================================================
  {
    name: 'a2a_create_conditional_payment',
    description:
      'Create a conditional payment that combines escrow with x402 payment intent. Funds are held in escrow until conditions are met, then automatically settled.',
    inputSchema: {
      buyerAddress: z.string().min(1).describe('Buyer wallet address'),
      sellerAddress: z.string().min(1).describe('Seller wallet address'),
      amount: z.number().positive().describe('Payment amount (e.g., 100.00)'),
      asset: z
        .string()
        .optional()
        .describe(
          'Asset: defaults to the selected network payment asset (e.g., USDC, ssUSD, BTC, ZEC)',
        ),
      network: z
        .string()
        .optional()
        .describe('Settlement network: set_chain, base, ethereum, arbitrum, bitcoin, zcash'),
      quoteId: z
        .string()
        .optional()
        .describe('Associated quote ID (adds seller_fulfilled condition)'),
      conditions: z
        .array(
          z.object({
            type: z
              .string()
              .describe('Condition type: seller_fulfilled, buyer_confirmed, time_lock, milestone'),
            quoteId: z.string().optional(),
            releaseAfter: z.string().optional(),
            description: z.string().optional(),
          }),
        )
        .max(20)
        .optional()
        .describe('Release conditions'),
      expiresInHours: z
        .number()
        .int()
        .min(1)
        .optional()
        .describe('Hours until expiry (default: 72)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Creating conditional payments requires --apply flag.',
          wouldCreate: {
            buyerAddress: params.buyerAddress,
            sellerAddress: params.sellerAddress,
            amount: params.amount,
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return { success: false, error: 'Agent wallet not configured.' };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const result = await a2a.createConditionalPayment(params);

      return {
        success: true,
        message: `Conditional payment created: ${params.amount} ${params.asset || 'default asset'} in escrow`,
        escrow: result.escrow,
      };
    },
  },

  {
    name: 'a2a_check_payment_conditions',
    description: 'Check whether all release conditions are met for a conditional payment (escrow).',
    inputSchema: {
      escrowId: z.string().min(1).describe('Escrow ID to check conditions for'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!agentConfig?.walletAddress) {
        return { success: false, error: 'Agent wallet not configured.' };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const result = await a2a.checkPaymentConditions(params.escrowId);

      return {
        success: true,
        escrowId: result.escrowId,
        status: result.status,
        allConditionsMet: result.allMet,
        conditions: result.conditions,
        intentId: result.intentId,
      };
    },
  },

  {
    name: 'a2a_settle_conditional_payment',
    description:
      'Settle a conditional payment. Checks all conditions, releases escrow funds to the seller, and marks the x402 intent as settled.',
    inputSchema: {
      escrowId: z.string().min(1).describe('Escrow ID to settle'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Settling conditional payments requires --apply flag.',
          wouldSettle: { escrowId: params.escrowId },
        };
      }

      if (!agentConfig?.walletAddress) {
        return { success: false, error: 'Agent wallet not configured.' };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const result = await a2a.settleConditionalPayment(params.escrowId);

      return {
        success: true,
        message: `Conditional payment settled: ${result.amount} released to ${result.sellerAddress}`,
        escrowId: result.escrowId,
        status: result.status,
        amount: result.amount,
        sellerAddress: result.sellerAddress,
        intentId: result.intentId,
        intentSettled: result.intentSettled,
      };
    },
  },

  // ==========================================================================
  // Event Streaming
  // ==========================================================================
  {
    name: 'a2a_subscribe_events',
    description:
      'Subscribe an agent to receive real-time events. Supports wildcard and prefix-based event type filtering.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address to subscribe'),
      eventTypes: z
        .array(z.string())
        .max(50)
        .optional()
        .describe(
          'Event types to subscribe to (default: ["*"] for all). Supports prefix wildcards like "a2a_payment.*"',
        ),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Subscribing to events requires --apply flag.',
          wouldSubscribe: {
            agentAddress: params.agentAddress,
            eventTypes: params.eventTypes,
          },
        };
      }

      const { createEventStreamService } = await import('../a2a/event-stream.js');
      const eventSvc = createEventStreamService(commerce.a2a());

      const result = await eventSvc.subscribe({
        agentAddress: params.agentAddress,
        eventTypes: params.eventTypes,
      });

      return {
        success: true,
        message: `Subscribed ${params.agentAddress} to events`,
        subscription: result.subscription,
      };
    },
  },

  {
    name: 'a2a_list_event_subscriptions',
    description: 'List active event subscriptions for an agent.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { createEventStreamService } = await import('../a2a/event-stream.js');
      const eventSvc = createEventStreamService(commerce.a2a());

      const subscriptions = await eventSvc.listSubscriptions({
        agentAddress: params.agentAddress,
      });

      return {
        success: true,
        count: subscriptions.length,
        subscriptions,
      };
    },
  },

  {
    name: 'a2a_get_event_history',
    description: 'Get historical events for an agent with optional filtering.',
    inputSchema: {
      agentAddress: z.string().min(1).describe('Agent wallet address'),
      eventTypes: z.array(z.string()).max(50).optional().describe('Filter by event types'),
      since: z.string().optional().describe('ISO timestamp — only events after this time'),
      limit: z.number().int().min(1).max(500).optional().describe('Max results (default: 50)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { createEventStreamService } = await import('../a2a/event-stream.js');
      const eventSvc = createEventStreamService(commerce.a2a());

      const events = await eventSvc.getEventHistory({
        agentAddress: params.agentAddress,
        eventTypes: params.eventTypes,
        since: params.since,
        limit: params.limit || 50,
      });

      return {
        success: true,
        count: events.length,
        events,
      };
    },
  },
];

export default a2aTools;
