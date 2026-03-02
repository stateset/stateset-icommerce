/**
 * Express Checkout & Payment Links Tools Module
 *
 * MCP tool definitions for creating shareable payment links, express checkout,
 * and agent-to-agent instant checkout flows.
 */

import { z } from 'zod';

// ---------------------------------------------------------------------------
// Lazy singleton — initialised on first tool call
// ---------------------------------------------------------------------------

let _checkoutSvc = null;

/**
 * Get or create the express checkout service singleton.
 * Uses the same A2AStore pattern for database access.
 * @returns {Promise<ReturnType<import('../checkout/express.js').createExpressCheckout>>}
 */
async function getCheckoutSvc() {
  if (_checkoutSvc) return _checkoutSvc;
  const { A2AStore } = await import('../a2a/store.js');
  const { createExpressCheckout } = await import('../checkout/express.js');
  const store = new A2AStore();
  store.init();
  _checkoutSvc = createExpressCheckout(store);
  return _checkoutSvc;
}

// ---------------------------------------------------------------------------
// Shared Zod shapes
// ---------------------------------------------------------------------------

const lineItemSchema = z.object({
  name: z.string().min(1).describe('Item name'),
  sku: z.string().optional().describe('SKU'),
  quantity: z.number().int().positive().describe('Quantity'),
  unitPrice: z.number().nonnegative().describe('Unit price'),
});

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

export const checkoutTools = [
  // ========================================================================
  // Create Payment Link
  // ========================================================================
  {
    name: 'create_payment_link',
    description:
      'Create a shareable payment link for instant checkout. Returns a short URL that buyers or agents can use.',
    inputSchema: {
      items: z.array(lineItemSchema).min(1).describe('Line items for the payment link'),
      currency: z.string().min(1).max(6).default('USD').describe('Currency code (e.g. USD, USDC)'),
      expiresIn: z
        .number()
        .int()
        .positive()
        .optional()
        .describe('Expiry in seconds (default: 86400 = 24h). Omit for no expiry.'),
      customerId: z
        .string()
        .min(1)
        .optional()
        .describe('Customer ID to pre-associate with the link'),
      metadata: z.record(z.string()).optional().describe('Custom key-value metadata'),
    },
    permission: 'write',
    handler: async ({ params }) => {
      try {
        const svc = await getCheckoutSvc();
        const result = svc.createPaymentLink(params);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ========================================================================
  // Resolve Payment Link
  // ========================================================================
  {
    name: 'resolve_payment_link',
    description:
      'Resolve a payment link by ID or short code. Returns the link details, items, total, and expiry status.',
    inputSchema: {
      linkId: z.string().min(1).describe('Payment link ID (UUID) or short code'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const svc = await getCheckoutSvc();
        const result = svc.resolvePaymentLink(params.linkId);
        if (!result) {
          return { success: false, error: `Payment link not found: ${params.linkId}` };
        }
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ========================================================================
  // Express Checkout
  // ========================================================================
  {
    name: 'express_checkout',
    description:
      'One-call checkout from a payment link. Converts the link into an order and payment.',
    inputSchema: {
      linkId: z.string().min(1).describe('Payment link ID or short code to checkout'),
      customerId: z.string().min(1).optional().describe('Customer ID for the order'),
      paymentMethod: z.string().optional().describe('Payment method (e.g. card, wallet, crypto)'),
    },
    permission: 'write',
    handler: async ({ params }) => {
      try {
        const svc = await getCheckoutSvc();
        const result = svc.expressCheckout(params);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ========================================================================
  // Agent Instant Checkout
  // ========================================================================
  {
    name: 'agent_instant_checkout',
    description:
      'Agent-to-agent instant checkout. Creates a payment link and converts it in one step. Returns order and escrow IDs for A2A settlement.',
    inputSchema: {
      buyerAgent: z.string().min(1).describe('Buyer agent ID or wallet address'),
      sellerAgent: z.string().min(1).describe('Seller agent ID or wallet address'),
      items: z.array(lineItemSchema).min(1).describe('Line items for the agent checkout'),
      paymentMethod: z.string().optional().describe('Payment method (default: a2a)'),
      currency: z.string().min(1).max(6).default('USD').describe('Currency code'),
    },
    permission: 'write',
    handler: async ({ params }) => {
      try {
        const svc = await getCheckoutSvc();
        const result = svc.agentCheckout(params);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ========================================================================
  // Get Payment Link Status
  // ========================================================================
  {
    name: 'get_payment_link_status',
    description: 'Get the status and metrics (views, conversions) for a payment link.',
    inputSchema: {
      linkId: z.string().min(1).describe('Payment link ID or short code'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const svc = await getCheckoutSvc();
        const result = svc.getPaymentLinkStatus(params.linkId);
        if (!result) {
          return { success: false, error: `Payment link not found: ${params.linkId}` };
        }
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ========================================================================
  // List Payment Links
  // ========================================================================
  {
    name: 'list_payment_links',
    description: 'List payment links with optional filters by status and customer.',
    inputSchema: {
      status: z
        .enum(['active', 'converted', 'revoked', 'expired'])
        .optional()
        .describe('Filter by status'),
      customerId: z.string().min(1).optional().describe('Filter by customer ID'),
      limit: z.number().int().min(1).max(500).optional().default(50).describe('Maximum results'),
      offset: z.number().int().min(0).optional().default(0).describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const svc = await getCheckoutSvc();
        const links = svc.listPaymentLinks(params);
        return { success: true, count: links.length, links };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ========================================================================
  // Revoke Payment Link
  // ========================================================================
  {
    name: 'revoke_payment_link',
    description: 'Revoke (cancel) an active payment link. Prevents further checkouts from it.',
    inputSchema: {
      linkId: z.string().min(1).describe('Payment link ID or short code to revoke'),
    },
    permission: 'write',
    handler: async ({ params }) => {
      try {
        const svc = await getCheckoutSvc();
        const result = svc.revokePaymentLink(params.linkId);
        return { success: true, ...result };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ========================================================================
  // Checkout with Crypto
  // ========================================================================
  {
    name: 'checkout_with_crypto',
    description:
      'Express checkout with a crypto wallet. Similar to express_checkout but takes a wallet address and network for on-chain payment.',
    inputSchema: {
      linkId: z.string().min(1).describe('Payment link ID or short code'),
      walletAddress: z.string().min(1).describe('Buyer wallet address (0x... or base58)'),
      network: z
        .string()
        .min(1)
        .default('set_chain')
        .describe('Blockchain network (set_chain, base, solana, ethereum, arbitrum)'),
      customerId: z.string().min(1).optional().describe('Customer ID'),
    },
    permission: 'write',
    handler: async ({ params }) => {
      try {
        const svc = await getCheckoutSvc();
        const result = svc.expressCheckout({
          linkId: params.linkId,
          customerId: params.customerId,
          walletAddress: params.walletAddress,
          paymentMethod: `crypto:${params.network}`,
        });
        return {
          success: true,
          ...result,
          walletAddress: params.walletAddress,
          network: params.network,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },
];

export default checkoutTools;
