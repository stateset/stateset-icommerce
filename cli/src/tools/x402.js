/**
 * x402 Protocol Tools Module
 *
 * MCP tool definitions for x402 AI agent commerce protocol operations.
 * Includes payment intents, signing, settlement, and credit ledger (metered billing).
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';

/**
 * x402 tool definitions
 */
export const x402Tools = [
  {
    name: 'x402_create_payment_intent',
    description:
      'Create an x402 payment intent for AI agent commerce. Returns a signing hash that the payer agent must sign with Ed25519.',
    inputSchema: {
      payerAddress: z.string().describe('Payer wallet address (sender)'),
      payeeAddress: z.string().describe('Payee wallet address (recipient)'),
      amount: z.number().describe('Amount in smallest unit (e.g., 1000000 = 1 USDC)'),
      asset: z.string().optional().describe('Asset: usdc, ssusd, usdt, dai (default: usdc)'),
      network: z
        .string()
        .optional()
        .describe('Network: set_chain, base, ethereum, arbitrum (default: set_chain)'),
      cartId: z.string().optional().describe('Cart ID to link this payment to'),
      orderId: z.string().optional().describe('Order ID for reference'),
      description: z.string().optional().describe('Description of what this payment is for'),
      validitySeconds: z
        .number()
        .optional()
        .describe('How long the intent is valid (default: 3600)'),
    },
    permission: 'write',
    handler: async ({ commerce, params }) => {
      const intent = await commerce.x402().createIntent({
        payer_address: params.payerAddress,
        payee_address: params.payeeAddress,
        amount: params.amount,
        asset: params.asset || 'usdc',
        network: params.network || 'set_chain',
        cart_id: params.cartId,
        order_id: params.orderId,
        description: params.description,
        validity_seconds: params.validitySeconds,
      });
      return {
        success: true,
        message: 'x402 payment intent created. Payer must sign the signing_hash.',
        intent: {
          id: intent.id,
          status: intent.status,
          payerAddress: intent.payer_address,
          payeeAddress: intent.payee_address,
          amount: intent.amount,
          amountDecimal: intent.amount_decimal,
          asset: intent.asset,
          network: intent.network,
          chainId: intent.chain_id,
          signingHash: intent.signing_hash,
          validUntil: intent.valid_until,
          nonce: intent.nonce,
        },
      };
    },
  },

  {
    name: 'x402_sign_intent',
    description:
      'Sign an x402 payment intent with an Ed25519 signature. This authorizes the payment.',
    inputSchema: {
      intentId: z.string().describe('Payment intent ID to sign'),
      signature: z
        .string()
        .describe('Ed25519 signature over the signing_hash (hex or base64 encoded)'),
      publicKey: z.string().describe('Payer Ed25519 public key (hex or base64 encoded)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { intentId, signature, publicKey } = params;
      if (!allowApply) {
        return {
          error: 'Signing x402 intent requires --apply flag.',
          wouldSign: { intentId, hasSignature: !!signature, hasPublicKey: !!publicKey },
          instruction: 'Run with --apply to sign this payment intent',
        };
      }

      const signed = await commerce.x402().signIntent(intentId, {
        intent_id: intentId,
        signature,
        public_key: publicKey,
      });
      return {
        success: true,
        message: 'Payment intent signed. Ready for settlement.',
        intent: {
          id: signed.id,
          status: signed.status,
          payerSignature: signed.payer_signature,
          payerPublicKey: signed.payer_public_key,
        },
      };
    },
  },

  {
    name: 'x402_get_intent',
    description: 'Get details of an x402 payment intent.',
    inputSchema: {
      intentId: z.string().describe('Payment intent ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { intentId } = params;
      const intent = await commerce.x402().getIntent(intentId);
      if (!intent) {
        return { error: 'Payment intent not found' };
      }
      return {
        success: true,
        intent: {
          id: intent.id,
          status: intent.status,
          payerAddress: intent.payer_address,
          payeeAddress: intent.payee_address,
          amount: intent.amount,
          amountDecimal: intent.amount_decimal,
          asset: intent.asset,
          network: intent.network,
          chainId: intent.chain_id,
          signingHash: intent.signing_hash,
          payerSignature: intent.payer_signature,
          validUntil: intent.valid_until,
          nonce: intent.nonce,
          txHash: intent.tx_hash,
          blockNumber: intent.block_number,
          createdAt: intent.created_at,
        },
      };
    },
  },

  {
    name: 'x402_list_intents',
    description: 'List x402 payment intents with optional filtering.',
    inputSchema: {
      payerAddress: z.string().optional().describe('Filter by payer address'),
      payeeAddress: z.string().optional().describe('Filter by payee address'),
      status: z
        .string()
        .optional()
        .describe('Filter by status: created, signed, sequenced, settled, expired, failed'),
      network: z.string().optional().describe('Filter by network'),
      limit: z.number().optional().describe('Maximum results (default: 50)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const intents = await commerce.x402().listIntents({
        payer_address: params.payerAddress,
        payee_address: params.payeeAddress,
        status: params.status,
        network: params.network,
        limit: params.limit || 50,
      });
      return {
        success: true,
        count: intents.length,
        intents: intents.map((i) => ({
          id: i.id,
          status: i.status,
          payerAddress: i.payer_address,
          payeeAddress: i.payee_address,
          amount: i.amount,
          asset: i.asset,
          network: i.network,
          createdAt: i.created_at,
        })),
      };
    },
  },

  {
    name: 'x402_mark_settled',
    description:
      'Mark an x402 payment intent as settled on-chain. Called after blockchain confirmation.',
    inputSchema: {
      intentId: z.string().describe('Payment intent ID'),
      txHash: z.string().describe('On-chain transaction hash'),
      blockNumber: z.number().describe('Block number where settled'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { intentId, txHash, blockNumber } = params;
      if (!allowApply) {
        return {
          error: 'Marking settled requires --apply flag.',
          wouldSettle: { intentId, txHash, blockNumber },
        };
      }

      const settled = await commerce.x402().markSettled(intentId, txHash, blockNumber);
      return {
        success: true,
        message: 'Payment intent marked as settled.',
        intent: {
          id: settled.id,
          status: settled.status,
          txHash: settled.tx_hash,
          blockNumber: settled.block_number,
          settledAt: settled.settled_at,
        },
      };
    },
  },

  {
    name: 'x402_get_next_nonce',
    description: 'Get the next nonce for a payer address. Used for replay protection.',
    inputSchema: {
      payerAddress: z.string().describe('Payer wallet address'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { payerAddress } = params;
      const nonce = await commerce.x402().getNextNonce(payerAddress);
      return {
        success: true,
        payerAddress,
        nextNonce: nonce,
      };
    },
  },

  // x402 Credit Ledger Tools (Metered Billing)
  {
    name: 'x402_credit_balance',
    description: 'Get x402 credit balance for a payer (prepaid meter for streaming usage).',
    inputSchema: {
      payerAddress: z.string().describe('Payer wallet address'),
      asset: z.string().optional().describe('Asset: usdc, ssusd, usdt, dai (default: usdc)'),
      network: z
        .string()
        .optional()
        .describe('Network: set_chain, base, ethereum, arbitrum (default: set_chain)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { payerAddress, asset, network } = params;
      const balance = await commerce.x402().getCreditBalance({
        payer_address: payerAddress,
        asset,
        network,
      });
      return {
        success: true,
        payerAddress,
        asset: asset || 'usdc',
        network: network || 'set_chain',
        balance,
      };
    },
  },

  {
    name: 'x402_credit_deposit',
    description: 'Credit (deposit) x402 balance for metered usage. Requires --apply.',
    inputSchema: {
      payerAddress: z.string().describe('Payer wallet address'),
      amount: z.number().describe('Amount in smallest unit (e.g., 1000000 = 1 USDC)'),
      asset: z.string().optional().describe('Asset: usdc, ssusd, usdt, dai (default: usdc)'),
      network: z
        .string()
        .optional()
        .describe('Network: set_chain, base, ethereum, arbitrum (default: set_chain)'),
      reason: z.string().optional().describe('Reason for deposit'),
      referenceId: z.string().optional().describe('Reference ID for audit'),
      metadata: z.string().optional().describe('Metadata (JSON string)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { payerAddress, amount, asset, network, reason, referenceId, metadata } = params;
      if (!allowApply) {
        return {
          error: 'Depositing credit requires --apply flag.',
          wouldDeposit: { payerAddress, amount, asset, network },
        };
      }

      const txn = await commerce.x402().creditAccount({
        payer_address: payerAddress,
        asset,
        network,
        amount,
        reason,
        reference_id: referenceId,
        metadata,
      });
      return {
        success: true,
        message: 'Credit deposited.',
        transaction: {
          id: txn.id,
          accountId: txn.account_id,
          direction: txn.direction,
          amount: txn.amount,
          balanceAfter: txn.balance_after,
          createdAt: txn.created_at,
        },
      };
    },
  },

  {
    name: 'x402_credit_debit',
    description: 'Debit x402 balance for metered usage. Requires --apply.',
    inputSchema: {
      payerAddress: z.string().describe('Payer wallet address'),
      amount: z.number().describe('Amount in smallest unit (e.g., 1000000 = 1 USDC)'),
      asset: z.string().optional().describe('Asset: usdc, ssusd, usdt, dai (default: usdc)'),
      network: z
        .string()
        .optional()
        .describe('Network: set_chain, base, ethereum, arbitrum (default: set_chain)'),
      reason: z.string().optional().describe('Reason for debit'),
      referenceId: z.string().optional().describe('Reference ID for audit'),
      metadata: z.string().optional().describe('Metadata (JSON string)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { payerAddress, amount, asset, network, reason, referenceId, metadata } = params;
      if (!allowApply) {
        return {
          error: 'Debiting credit requires --apply flag.',
          wouldDebit: { payerAddress, amount, asset, network },
        };
      }

      const txn = await commerce.x402().debitAccount({
        payer_address: payerAddress,
        asset,
        network,
        amount,
        reason,
        reference_id: referenceId,
        metadata,
      });
      return {
        success: true,
        message: 'Credit debited.',
        transaction: {
          id: txn.id,
          accountId: txn.account_id,
          direction: txn.direction,
          amount: txn.amount,
          balanceAfter: txn.balance_after,
          createdAt: txn.created_at,
        },
      };
    },
  },

  {
    name: 'x402_credit_transactions',
    description: 'List x402 credit ledger transactions.',
    inputSchema: {
      payerAddress: z.string().optional().describe('Filter by payer address'),
      asset: z.string().optional().describe('Filter by asset'),
      network: z.string().optional().describe('Filter by network'),
      direction: z.string().optional().describe('Filter by direction: credit, debit'),
      limit: z.number().optional().describe('Maximum results (default: 50)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const txns = await commerce.x402().listCreditTransactions({
        payer_address: params.payerAddress,
        asset: params.asset,
        network: params.network,
        direction: params.direction,
        limit: params.limit || 50,
      });
      return {
        success: true,
        count: txns.length,
        transactions: txns.map((txn) => ({
          id: txn.id,
          accountId: txn.account_id,
          payerAddress: txn.payer_address,
          direction: txn.direction,
          amount: txn.amount,
          balanceAfter: txn.balance_after,
          createdAt: txn.created_at,
        })),
      };
    },
  },
];

/**
 * Get all x402 tools
 */
export function getX402Tools() {
  return x402Tools;
}

/**
 * Get x402 tool by name
 */
export function getX402Tool(name) {
  return x402Tools.find((t) => t.name === name);
}

export default x402Tools;
