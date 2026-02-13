/**
 * Agent-to-Agent (A2A) Commerce Tools Module
 *
 * MCP tool definitions for agent-to-agent payments, quotes, and commerce.
 * Makes it dead simple for AI agents to pay each other through natural language.
 */

import { z } from 'zod';

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
      'Pay another AI agent directly. Send USDC or other stablecoins to another agent by their wallet address or agent ID.',
    inputSchema: {
      to: z
        .string()
        .describe('Recipient: wallet address (0x...) or agent ID (UUID)'),
      amount: z
        .number()
        .positive()
        .describe('Amount to pay (e.g., 10.00 for $10 USDC)'),
      asset: z
        .string()
        .optional()
        .describe('Asset to pay with: USDC (default), USDT, ssUSD, DAI'),
      network: z
        .string()
        .optional()
        .describe('Network: set_chain (default), base, ethereum, arbitrum'),
      memo: z
        .string()
        .optional()
        .describe('Payment memo/description (e.g., "API call", "Data processing")'),
      idempotencyKey: z
        .string()
        .optional()
        .describe('Unique key to prevent duplicate payments'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          error: 'Sending payments requires --apply flag.',
          wouldPay: {
            to: params.to,
            amount: params.amount,
            asset: params.asset || 'USDC',
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          error: 'Agent wallet not configured. Set up agent identity first.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const result = await a2a.pay({
        to: params.to,
        amount: params.amount,
        asset: params.asset,
        network: params.network,
        memo: params.memo,
        idempotencyKey: params.idempotencyKey,
      });

      return {
        success: true,
        message: `Paid ${params.amount} ${params.asset || 'USDC'} to ${params.to}`,
        payment: result.payment,
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
        .describe('Amount to request (e.g., 25.00 for $25)'),
      description: z
        .string()
        .describe('What the payment is for (e.g., "Data processing fee", "API access")'),
      from: z
        .string()
        .optional()
        .describe('Specific payer wallet/agent (optional - leave empty for open request)'),
      asset: z
        .string()
        .optional()
        .describe('Asset to request: USDC (default), USDT'),
      expiresInHours: z
        .number()
        .optional()
        .describe('Hours until request expires (default: 24)'),
      allowPartial: z
        .boolean()
        .optional()
        .describe('Allow partial payments'),
      callbackUrl: z
        .string()
        .optional()
        .describe('Webhook URL to notify when paid'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          error: 'Creating payment requests requires --apply flag.',
          wouldRequest: {
            amount: params.amount,
            description: params.description,
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          error: 'Agent wallet not configured. Set up agent identity first.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const result = await a2a.requestPayment({
        from: params.from,
        amount: params.amount,
        description: params.description,
        asset: params.asset,
        expiresInHours: params.expiresInHours,
        allowPartial: params.allowPartial,
        callbackUrl: params.callbackUrl,
      });

      return {
        success: true,
        message: `Payment request created for ${params.amount} ${params.asset || 'USDC'}`,
        request: result.request,
        paymentUrl: result.paymentUrl,
      };
    },
  },

  {
    name: 'a2a_pay_request',
    description: 'Pay an existing payment request from another agent.',
    inputSchema: {
      requestId: z
        .string()
        .describe('Payment request ID (UUID)'),
      amount: z
        .number()
        .optional()
        .describe('Amount to pay (optional - pays full amount if not specified)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          error: 'Paying requests requires --apply flag.',
          wouldPay: { requestId: params.requestId },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          error: 'Agent wallet not configured.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const result = await a2a.payRequest(params.requestId, {
        amount: params.amount,
      });

      return {
        success: true,
        message: result.request.fullyPaid
          ? 'Payment request fully paid'
          : `Partial payment made. Remaining: ${result.request.amountRemaining}`,
        payment: result.payment,
        request: result.request,
      };
    },
  },

  // ==========================================================================
  // Quote Operations
  // ==========================================================================
  {
    name: 'a2a_request_quote',
    description:
      'Request a price quote from another agent for goods or services.',
    inputSchema: {
      seller: z
        .string()
        .describe('Seller agent wallet address or agent ID'),
      items: z
        .array(
          z.object({
            description: z.string().describe('Item description'),
            quantity: z.number().optional().describe('Quantity (default: 1)'),
            sku: z.string().optional().describe('SKU or service code'),
          })
        )
        .describe('Items to get a quote for'),
      asset: z
        .string()
        .optional()
        .describe('Preferred payment asset: USDC (default)'),
      message: z
        .string()
        .optional()
        .describe('Message to seller'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          error: 'Requesting quotes requires --apply flag.',
          wouldRequest: {
            seller: params.seller,
            items: params.items,
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          error: 'Agent wallet not configured.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const result = await a2a.requestQuote({
        seller: params.seller,
        items: params.items,
        asset: params.asset,
        message: params.message,
      });

      return {
        success: true,
        message: 'Quote requested. Waiting for seller response.',
        quote: result.quote,
      };
    },
  },

  {
    name: 'a2a_provide_quote',
    description:
      'Respond to a quote request with pricing (for sellers).',
    inputSchema: {
      quoteId: z
        .string()
        .describe('Quote ID to respond to'),
      total: z
        .number()
        .positive()
        .describe('Total price'),
      fees: z
        .number()
        .optional()
        .describe('Processing/platform fees'),
      tax: z
        .number()
        .optional()
        .describe('Tax amount'),
      expiresInHours: z
        .number()
        .optional()
        .describe('Hours until quote expires (default: 48)'),
      terms: z
        .string()
        .optional()
        .describe('Terms and conditions'),
      estimatedDelivery: z
        .string()
        .optional()
        .describe('Estimated delivery time (e.g., "2 hours", "1 day")'),
      message: z
        .string()
        .optional()
        .describe('Message to buyer'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          error: 'Providing quotes requires --apply flag.',
          wouldQuote: {
            quoteId: params.quoteId,
            total: params.total,
          },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          error: 'Agent wallet not configured.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

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
    description:
      'Accept a quote and pay. Automatically sends payment to the seller.',
    inputSchema: {
      quoteId: z
        .string()
        .describe('Quote ID to accept'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          error: 'Accepting quotes requires --apply flag.',
          wouldAccept: { quoteId: params.quoteId },
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          error: 'Agent wallet not configured.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const result = await a2a.acceptQuote(params.quoteId);

      return {
        success: true,
        message: `Quote accepted and paid: ${result.quote.total} ${result.quote.asset}`,
        payment: result.payment,
        quote: result.quote,
      };
    },
  },

  {
    name: 'a2a_decline_quote',
    description: 'Decline a quote.',
    inputSchema: {
      quoteId: z
        .string()
        .describe('Quote ID to decline'),
      reason: z
        .string()
        .optional()
        .describe('Reason for declining'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          error: 'Declining quotes requires --apply flag.',
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          error: 'Agent wallet not configured.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

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
      quoteId: z
        .string()
        .describe('Quote ID to mark as fulfilled'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply, agentConfig }) => {
      if (!allowApply) {
        return {
          error: 'Fulfilling quotes requires --apply flag.',
        };
      }

      if (!agentConfig?.walletAddress) {
        return {
          error: 'Agent wallet not configured.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

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
    name: 'a2a_list_payments',
    description: 'List A2A payments sent or received by this agent.',
    inputSchema: {
      direction: z
        .enum(['sent', 'received', 'all'])
        .optional()
        .describe('Filter by direction: sent, received, or all (default)'),
      status: z
        .string()
        .optional()
        .describe('Filter by status: pending, submitted, completed, failed'),
      limit: z
        .number()
        .optional()
        .describe('Max results (default: 20)'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!agentConfig?.walletAddress) {
        return {
          error: 'Agent wallet not configured.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const filter = {
        sent: params.direction === 'sent',
        received: params.direction === 'received',
        status: params.status,
        limit: params.limit || 20,
      };

      const payments = await a2a.getPayments(filter);

      return {
        success: true,
        count: payments.length,
        payments,
      };
    },
  },

  {
    name: 'a2a_list_payment_requests',
    description: 'List payment requests created by or sent to this agent.',
    inputSchema: {
      direction: z
        .enum(['created', 'received', 'all'])
        .optional()
        .describe('Filter by direction'),
      status: z
        .string()
        .optional()
        .describe('Filter by status: pending, viewed, paid, declined, expired'),
      limit: z
        .number()
        .optional()
        .describe('Max results (default: 20)'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!agentConfig?.walletAddress) {
        return {
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
      role: z
        .enum(['buyer', 'seller', 'all'])
        .optional()
        .describe('Filter by role'),
      status: z
        .string()
        .optional()
        .describe('Filter by status: requested, quoted, accepted, declined, fulfilled'),
      includeExpired: z
        .boolean()
        .optional()
        .describe('Include expired quotes'),
      limit: z
        .number()
        .optional()
        .describe('Max results (default: 20)'),
    },
    permission: 'read',
    handler: async ({ commerce, params, agentConfig }) => {
      if (!agentConfig?.walletAddress) {
        return {
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
    description: 'Get A2A payment summary for this agent (total sent, received, net flow).',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce, agentConfig }) => {
      if (!agentConfig?.walletAddress) {
        return {
          error: 'Agent wallet not configured.',
        };
      }

      const { createA2AService } = await import('../a2a/index.js');
      const a2a = createA2AService(commerce, agentConfig);

      const balance = await a2a.getBalance();

      return {
        success: true,
        balance,
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
        .describe('Required network support: set_chain, base, ethereum'),
      asset: z
        .string()
        .optional()
        .describe('Required asset support: USDC, USDT, ssUSD'),
      trustLevel: z
        .string()
        .optional()
        .describe('Minimum trust level: sandbox, standard, verified, enterprise'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const agents = await commerce
        .x402()
        .discoverAgents(params.network, params.asset, params.skill, params.trustLevel);

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
          endpointUrl: a.endpoint_url,
          description: a.description,
        })),
      };
    },
  },
];

export default a2aTools;
