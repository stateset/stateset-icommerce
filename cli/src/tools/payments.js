/**
 * Payment Tools Module
 *
 * MCP tool definitions for payment processing and refund operations.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';
import {
  capturePaymentIntent,
  cancelPaymentIntent,
  createPaymentIntent,
  createPaymentSettlementBatch,
  getPaymentIntent,
  ingestPaymentProviderWebhook,
  listPaymentIntents,
  listPaymentProviders,
  listPaymentSettlementBatches,
  listPaymentSettlements,
  reconcilePaymentProvider,
  refundPaymentIntent,
} from './providers/payments.js';

/**
 * Payment tool definitions
 */
export const paymentTools = [
  {
    name: 'list_payments',
    description: 'List all payments in the system.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const payments = await commerce.payments.list();
      const count = await commerce.payments.count();
      return { success: true, count, payments };
    },
  },

  {
    name: 'get_payment',
    description: 'Get a payment by ID.',
    inputSchema: {
      paymentId: z.string().min(1).describe('Payment ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { paymentId } = params;
      const payment = await commerce.payments.get(paymentId);
      return { success: true, payment };
    },
  },

  {
    name: 'create_payment',
    description: 'Create a payment for an order.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID'),
      amount: z.number().positive().describe('Payment amount'),
      currency: z.string().max(10).optional().describe('Currency (default: USD)'),
      method: z
        .string()
        .optional()
        .describe('Payment method: credit_card, paypal, bank_transfer, crypto'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create payment', params);
      }

      const payment = await commerce.payments.create({
        orderId: params.orderId,
        amount: String(params.amount),
        currency: params.currency || 'USD',
        method: params.method || 'credit_card',
      });
      return { success: true, message: 'Payment created', payment };
    },
  },

  {
    name: 'complete_payment',
    description: 'Mark a payment as completed.',
    inputSchema: {
      paymentId: z.string().min(1).describe('Payment ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { paymentId } = params;
      if (!allowApply) {
        return applyRequired('Complete payment', params);
      }

      const payment = await commerce.payments.markCompleted(paymentId);
      return { success: true, message: 'Payment completed', payment };
    },
  },

  {
    name: 'create_refund',
    description: 'Create a refund for a payment.',
    inputSchema: {
      paymentId: z.string().min(1).describe('Payment ID to refund'),
      amount: z.number().positive().describe('Refund amount'),
      reason: z.string().max(500).optional().describe('Refund reason'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create refund', params);
      }

      const refund = await commerce.payments.createRefund({
        paymentId: params.paymentId,
        amount: String(params.amount),
        reason: params.reason,
      });
      return { success: true, message: 'Refund created', refund };
    },
  },

  {
    name: 'list_payment_providers',
    description: 'List available payment providers and capabilities for agentic payment flows.',
    inputSchema: {
      capability: z
        .string()
        .optional()
        .describe('Optional capability filter (e.g., intents, capture, refund, webhooks)'),
      mode: z
        .enum(['sandbox', 'shadow', 'production'])
        .optional()
        .describe('Optional provider mode filter'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const providers = listPaymentProviders({
        capability: params.capability,
        mode: params.mode,
      });
      return {
        success: true,
        count: providers.length,
        providers,
      };
    },
  },

  {
    name: 'create_payment_intent',
    description:
      'Create a provider-backed payment intent with idempotency support for governed checkout flows.',
    inputSchema: {
      providerId: z.string().optional().describe('Provider ID (default: deterministic-mock)'),
      amount: z.number().positive().describe('Payment amount'),
      currency: z.string().max(10).optional().describe('Currency code (default: USD)'),
      captureMethod: z
        .enum(['manual', 'automatic'])
        .optional()
        .describe('Capture mode (manual or automatic)'),
      orderId: z.string().optional().describe('Order ID'),
      customerId: z.string().optional().describe('Customer ID'),
      paymentMethodId: z.string().optional().describe('Payment method identifier'),
      metadata: z.record(z.string(), z.any()).optional().describe('Additional metadata'),
      idempotencyKey: z.string().max(255).optional().describe('Idempotency key for safe retries'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create payment intent', params);
      }

      const result = createPaymentIntent({
        providerId: params.providerId,
        amount: params.amount,
        currency: params.currency || 'USD',
        captureMethod: params.captureMethod || 'manual',
        orderId: params.orderId,
        customerId: params.customerId,
        paymentMethodId: params.paymentMethodId,
        metadata: params.metadata || {},
        idempotencyKey: params.idempotencyKey,
      });

      return {
        success: true,
        message: result.idempotent
          ? 'Payment intent reused via idempotency key'
          : 'Payment intent created',
        provider: result.provider,
        intent: result.intent,
        idempotent: result.idempotent,
      };
    },
  },

  {
    name: 'get_payment_intent',
    description: 'Get a provider-backed payment intent by ID.',
    inputSchema: {
      intentId: z.string().min(1).describe('Payment intent ID'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const intent = getPaymentIntent(params.intentId);
      if (!intent) {
        return { success: false, error: 'Payment intent not found' };
      }
      return { success: true, intent };
    },
  },

  {
    name: 'list_payment_intents',
    description: 'List provider-backed payment intents with optional filtering.',
    inputSchema: {
      providerId: z.string().optional().describe('Filter by provider ID'),
      status: z
        .enum(['pending', 'requires_action', 'succeeded', 'failed', 'cancelled'])
        .optional()
        .describe('Filter by intent status'),
      orderId: z.string().optional().describe('Filter by order ID'),
      customerId: z.string().optional().describe('Filter by customer ID'),
      limit: z.number().int().min(1).max(500).optional().describe('Maximum intents to return'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const intents = listPaymentIntents({
        providerId: params.providerId,
        status: params.status,
        orderId: params.orderId,
        customerId: params.customerId,
        limit: params.limit,
      });
      return {
        success: true,
        count: intents.length,
        intents,
      };
    },
  },

  {
    name: 'list_payment_settlements',
    description: 'List settlement records produced by provider payout reconciliation.',
    inputSchema: {
      providerId: z.string().optional().describe('Filter by provider ID'),
      status: z
        .enum(['pending', 'completed', 'failed'])
        .optional()
        .describe('Filter by settlement status'),
      batchId: z.string().optional().describe('Filter by settlement batch ID'),
      payoutReference: z.string().optional().describe('Filter by payout reference ID'),
      intentId: z.string().optional().describe('Filter by payment intent ID'),
      orderId: z.string().optional().describe('Filter by order ID'),
      customerId: z.string().optional().describe('Filter by customer ID'),
      limit: z.number().int().min(1).max(500).optional().describe('Maximum settlements to return'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const settlements = listPaymentSettlements({
        providerId: params.providerId,
        status: params.status,
        batchId: params.batchId,
        payoutReference: params.payoutReference,
        intentId: params.intentId,
        orderId: params.orderId,
        customerId: params.customerId,
        limit: params.limit,
      });
      return {
        success: true,
        count: settlements.length,
        settlements,
      };
    },
  },

  {
    name: 'list_payment_settlement_batches',
    description: 'List provider payout batches generated from settlement runs.',
    inputSchema: {
      providerId: z.string().optional().describe('Filter by provider ID'),
      status: z
        .enum(['pending', 'processing', 'completed', 'failed'])
        .optional()
        .describe('Filter by batch status'),
      payoutReference: z.string().optional().describe('Filter by payout reference ID'),
      limit: z.number().int().min(1).max(500).optional().describe('Maximum batches to return'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const batches = listPaymentSettlementBatches({
        providerId: params.providerId,
        status: params.status,
        payoutReference: params.payoutReference,
        limit: params.limit,
      });
      return {
        success: true,
        count: batches.length,
        batches,
      };
    },
  },

  {
    name: 'create_payment_settlement_batch',
    description:
      'Create a settlement batch for captured/refunded payment intents to simulate provider payout reconciliation.',
    inputSchema: {
      providerId: z.string().optional().describe('Provider ID (default: deterministic-mock)'),
      intentIds: z
        .array(z.string().min(1))
        .optional()
        .describe('Optional explicit payment intent IDs to include in settlement'),
      payoutReference: z.string().optional().describe('Provider payout reference'),
      settledAt: z.string().optional().describe('Settlement timestamp (ISO-8601)'),
      includeZeroBalances: z
        .boolean()
        .optional()
        .describe('Include intents with zero remaining settleable balance'),
      idempotencyKey: z.string().max(255).optional().describe('Idempotency key for safe retries'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create payment settlement batch', params);
      }

      const result = createPaymentSettlementBatch({
        providerId: params.providerId,
        intentIds: params.intentIds,
        payoutReference: params.payoutReference,
        settledAt: params.settledAt,
        includeZeroBalances: params.includeZeroBalances || false,
        idempotencyKey: params.idempotencyKey,
      });

      return {
        success: true,
        message:
          result.count > 0
            ? result.idempotent
              ? 'Settlement batch reused via idempotency key'
              : 'Settlement batch created'
            : 'No settleable balance for selected intents',
        provider: result.provider,
        batch: result.batch,
        count: result.count,
        settlements: result.settlements,
        idempotent: result.idempotent,
      };
    },
  },

  {
    name: 'reconcile_payment_provider',
    description:
      'Reconcile payment intents against settlement records to find pending settlement or over-settlement drift.',
    inputSchema: {
      providerId: z.string().optional().describe('Filter by provider ID'),
      status: z
        .enum(['pending', 'requires_action', 'succeeded', 'failed', 'cancelled'])
        .optional()
        .describe('Filter by payment intent status'),
      orderId: z.string().optional().describe('Filter by order ID'),
      customerId: z.string().optional().describe('Filter by customer ID'),
      intentId: z.string().optional().describe('Filter by intent ID'),
      includeBalanced: z
        .boolean()
        .optional()
        .describe('Include already balanced intents in output'),
      limit: z.number().int().min(1).max(500).optional().describe('Maximum intents to reconcile'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const reconciliation = reconcilePaymentProvider({
        providerId: params.providerId,
        status: params.status,
        orderId: params.orderId,
        customerId: params.customerId,
        intentId: params.intentId,
        includeBalanced: params.includeBalanced ?? true,
        limit: params.limit,
      });

      return {
        success: true,
        ...reconciliation,
      };
    },
  },

  {
    name: 'capture_payment_intent',
    description: 'Capture all or part of a provider-backed payment intent.',
    inputSchema: {
      intentId: z.string().min(1).describe('Payment intent ID'),
      amount: z.number().positive().optional().describe('Optional partial capture amount'),
      idempotencyKey: z.string().max(255).optional().describe('Idempotency key for safe retries'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Capture payment intent', params);
      }

      const result = capturePaymentIntent({
        intentId: params.intentId,
        amount: params.amount,
        idempotencyKey: params.idempotencyKey,
      });

      return {
        success: true,
        message: result.idempotent ? 'Capture request was idempotent' : 'Payment intent captured',
        intent: result.intent,
        capture: result.capture,
        idempotent: result.idempotent,
      };
    },
  },

  {
    name: 'cancel_payment_intent',
    description: 'Cancel an uncaptured provider-backed payment intent.',
    inputSchema: {
      intentId: z.string().min(1).describe('Payment intent ID'),
      reason: z.string().max(500).optional().describe('Cancellation reason'),
    },
    permission: 'delete',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Cancel payment intent', params);
      }

      const result = cancelPaymentIntent({
        intentId: params.intentId,
        reason: params.reason,
      });

      return {
        success: true,
        message: result.idempotent
          ? 'Payment intent was already canceled'
          : 'Payment intent canceled',
        intent: result.intent,
        idempotent: result.idempotent,
      };
    },
  },

  {
    name: 'refund_payment_intent',
    description: 'Refund all or part of a captured provider-backed payment intent.',
    inputSchema: {
      intentId: z.string().min(1).describe('Payment intent ID'),
      amount: z.number().positive().optional().describe('Optional partial refund amount'),
      reason: z.string().max(500).optional().describe('Refund reason'),
      idempotencyKey: z.string().max(255).optional().describe('Idempotency key for safe retries'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Refund payment intent', params);
      }

      const result = refundPaymentIntent({
        intentId: params.intentId,
        amount: params.amount,
        reason: params.reason,
        idempotencyKey: params.idempotencyKey,
      });

      return {
        success: true,
        message: result.idempotent ? 'Refund request was idempotent' : 'Payment intent refunded',
        intent: result.intent,
        refund: result.refund,
        idempotent: result.idempotent,
      };
    },
  },

  {
    name: 'ingest_payment_provider_webhook',
    description:
      'Ingest a payment provider webhook event and reconcile payment intent state in shadow or production mode.',
    inputSchema: {
      providerId: z.string().optional().describe('Provider ID (default: deterministic-mock)'),
      eventType: z.string().min(1).describe('Webhook event type'),
      eventId: z
        .string()
        .optional()
        .describe('Optional provider event ID for idempotent ingestion'),
      payload: z
        .record(z.string(), z.any())
        .optional()
        .describe('Webhook payload object from provider'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Ingest payment provider webhook', params);
      }

      const result = ingestPaymentProviderWebhook({
        providerId: params.providerId,
        eventType: params.eventType,
        eventId: params.eventId,
        payload: params.payload || {},
      });

      return {
        success: true,
        message: result.applied
          ? 'Payment webhook ingested'
          : 'Payment webhook processed with no mutation',
        webhook: result,
      };
    },
  },
];

export default paymentTools;
