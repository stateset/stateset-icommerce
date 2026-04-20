/**
 * Payments Commands Module
 */

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
} from '../tools/providers/payments.js';

function parseAmount(value, usage) {
  const amount = Number.parseFloat(value);
  if (!Number.isFinite(amount) || amount <= 0) {
    throw new Error(usage);
  }
  return amount;
}

function parseLimit(value, fallback = 100) {
  const limit = Number.parseInt(value || String(fallback), 10);
  return Number.isInteger(limit) && limit > 0 ? limit : fallback;
}

function parseBoolean(value, fallback = true) {
  if (value === undefined) return fallback;
  return ['true', '1', 'yes', 'y'].includes(String(value).toLowerCase());
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const [orderId, status] = args;
      const payments = await commerce.payments.list();
      const filtered = payments.filter(
        (payment) =>
          (!orderId || payment.orderId === orderId) && (!status || payment.status === status),
      );
      return formatPaymentList(filtered, { output, jsonOutput });
    }

    case 'get': {
      const paymentId = args[0];
      if (!paymentId) throw new Error('Usage: payments get <paymentId>');
      const payment = await commerce.payments.get(paymentId);
      if (!payment) throw new Error(`Payment not found: ${paymentId}`);
      return formatPaymentDetail(payment, { jsonOutput });
    }

    case 'create': {
      const [orderId, amountRaw, currency = 'USD', method = 'credit_card'] = args;
      if (!orderId || !amountRaw) {
        throw new Error('Usage: payments create <orderId> <amount> [currency] [method]');
      }
      const amount = parseAmount(
        amountRaw,
        'Usage: payments create <orderId> <amount> [currency] [method]',
      );
      const payment = await commerce.payments.create({
        orderId,
        amount: String(amount),
        currency: currency.toUpperCase(),
        method,
      });
      return {
        payment,
        formatted: `Created payment ${payment.id} for order ${payment.orderId}`,
      };
    }

    case 'complete': {
      const paymentId = args[0];
      if (!paymentId) throw new Error('Usage: payments complete <paymentId>');
      const payment = await commerce.payments.markCompleted(paymentId);
      return {
        payment,
        formatted: `Completed payment ${payment.id}`,
      };
    }

    case 'fail': {
      const [paymentId, reason, code] = args;
      if (!paymentId || !reason) {
        throw new Error('Usage: payments fail <paymentId> <reason> [code]');
      }
      const payment = await commerce.payments.markFailed(paymentId, reason, code);
      return {
        payment,
        formatted: `Marked payment ${payment.id} as failed`,
      };
    }

    case 'cancel': {
      const paymentId = args[0];
      if (!paymentId) throw new Error('Usage: payments cancel <paymentId>');
      const payment = await commerce.payments.cancel(paymentId);
      return {
        payment,
        formatted: `Cancelled payment ${payment.id}`,
      };
    }

    case 'refund': {
      const [paymentId, amountRaw, ...reasonParts] = args;
      if (!paymentId || !amountRaw) {
        throw new Error('Usage: payments refund <paymentId> <amount> [reason]');
      }
      const amount = parseAmount(amountRaw, 'Usage: payments refund <paymentId> <amount> [reason]');
      const refund = await commerce.payments.createRefund({
        paymentId,
        amount: String(amount),
        reason: reasonParts.join(' ') || undefined,
      });
      return {
        refund,
        formatted: `Created refund ${refund.id || ''}`.trim(),
      };
    }

    case 'providers': {
      const [capability, mode] = args;
      const providers = listPaymentProviders({ capability, mode });
      return formatProviders(providers, { output, jsonOutput });
    }

    case 'intents': {
      const [providerId, status, orderId, customerId, limitRaw] = args;
      const intents = listPaymentIntents({
        providerId,
        status,
        orderId,
        customerId,
        limit: parseLimit(limitRaw),
      });
      return formatIntentList(intents, { output, jsonOutput });
    }

    case 'intent': {
      const intentId = args[0];
      if (!intentId) throw new Error('Usage: payments intent <intentId>');
      const intent = getPaymentIntent(intentId);
      if (!intent) throw new Error(`Payment intent not found: ${intentId}`);
      return formatIntentDetail(intent, { jsonOutput });
    }

    case 'create-intent': {
      const [
        amountRaw,
        currency = 'USD',
        captureMethod = 'manual',
        orderId,
        customerId,
        providerId,
      ] = args;
      if (!amountRaw) {
        throw new Error(
          'Usage: payments create-intent <amount> [currency] [captureMethod] [orderId] [customerId] [providerId]',
        );
      }
      const amount = parseAmount(
        amountRaw,
        'Usage: payments create-intent <amount> [currency] [captureMethod] [orderId] [customerId] [providerId]',
      );
      const result = createPaymentIntent({
        providerId,
        amount,
        currency: currency.toUpperCase(),
        captureMethod,
        orderId,
        customerId,
      });
      return formatIntentMutation('Payment intent created', result, { jsonOutput });
    }

    case 'capture-intent': {
      const [intentId, amountRaw] = args;
      if (!intentId) throw new Error('Usage: payments capture-intent <intentId> [amount]');
      const result = capturePaymentIntent({
        intentId,
        amount: amountRaw
          ? parseAmount(amountRaw, 'Usage: payments capture-intent <intentId> [amount]')
          : undefined,
      });
      return formatCaptureResult(result, { jsonOutput });
    }

    case 'cancel-intent': {
      const [intentId, ...reasonParts] = args;
      if (!intentId) throw new Error('Usage: payments cancel-intent <intentId> [reason]');
      const result = cancelPaymentIntent({
        intentId,
        reason: reasonParts.join(' ') || undefined,
      });
      return formatIntentMutation('Payment intent canceled', result, { jsonOutput });
    }

    case 'refund-intent': {
      const [intentId, amountRaw, ...reasonParts] = args;
      if (!intentId) {
        throw new Error('Usage: payments refund-intent <intentId> [amount] [reason]');
      }
      const result = refundPaymentIntent({
        intentId,
        amount: amountRaw
          ? parseAmount(amountRaw, 'Usage: payments refund-intent <intentId> [amount] [reason]')
          : undefined,
        reason: reasonParts.join(' ') || undefined,
      });
      return formatRefundResult(result, { jsonOutput });
    }

    case 'settlements': {
      const [providerId, status, batchId, limitRaw] = args;
      const settlements = listPaymentSettlements({
        providerId,
        status,
        batchId,
        limit: parseLimit(limitRaw),
      });
      return formatSettlementList(settlements, { output, jsonOutput });
    }

    case 'batches': {
      const [providerId, status, payoutReference, limitRaw] = args;
      const batches = listPaymentSettlementBatches({
        providerId,
        status,
        payoutReference,
        limit: parseLimit(limitRaw),
      });
      return formatBatchList(batches, { output, jsonOutput });
    }

    case 'settle': {
      const [providerId, intentIdsCsv, payoutReference] = args;
      const intentIds = intentIdsCsv
        ? intentIdsCsv
            .split(',')
            .map((id) => id.trim())
            .filter(Boolean)
        : undefined;
      const result = createPaymentSettlementBatch({
        providerId,
        intentIds,
        payoutReference,
      });
      return formatSettlementBatchResult(result, { jsonOutput });
    }

    case 'reconcile': {
      const [providerId, status, orderId, includeBalancedRaw, limitRaw] = args;
      const result = reconcilePaymentProvider({
        providerId,
        status,
        orderId,
        includeBalanced: parseBoolean(includeBalancedRaw, true),
        limit: parseLimit(limitRaw),
      });
      return formatReconciliation(result, { output, jsonOutput });
    }

    case 'webhook': {
      const [eventType, intentId, providerId] = args;
      if (!eventType)
        throw new Error('Usage: payments webhook <eventType> [intentId] [providerId]');
      const payload = intentId ? { intentId } : {};
      const result = ingestPaymentProviderWebhook({
        providerId,
        eventType,
        payload,
      });
      return formatWebhookResult(result, { jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: payments ${action}\n\n` +
          'Available actions:\n' +
          '  list [orderId] [status]                        List payments\n' +
          '  get <paymentId>                               Get payment details\n' +
          '  create <orderId> <amount> [currency] [method] Create payment\n' +
          '  complete <paymentId>                          Complete payment\n' +
          '  fail <paymentId> <reason> [code]              Mark payment failed\n' +
          '  cancel <paymentId>                            Cancel payment\n' +
          '  refund <paymentId> <amount> [reason]          Create refund\n' +
          '  providers [capability] [mode]                 List payment providers\n' +
          '  intents [providerId] [status] [orderId] [customerId] [limit]  List payment intents\n' +
          '  intent <intentId>                             Get payment intent details\n' +
          '  create-intent <amount> [currency] [captureMethod] [orderId] [customerId] [providerId]\n' +
          '  capture-intent <intentId> [amount]            Capture payment intent\n' +
          '  cancel-intent <intentId> [reason]             Cancel payment intent\n' +
          '  refund-intent <intentId> [amount] [reason]    Refund payment intent\n' +
          '  settlements [providerId] [status] [batchId] [limit]  List settlements\n' +
          '  batches [providerId] [status] [payoutReference] [limit]  List settlement batches\n' +
          '  settle [providerId] [intentIdsCsv] [payoutReference]   Create settlement batch\n' +
          '  reconcile [providerId] [status] [orderId] [includeBalanced] [limit]  Reconcile provider\n' +
          '  webhook <eventType> [intentId] [providerId]   Ingest payment webhook',
      );
  }
}

function formatPaymentList(payments, { output, jsonOutput }) {
  if (jsonOutput) return payments;
  if (payments.length === 0) return { formatted: 'No payments found.' };
  const formatted = output.table(payments, [
    { key: 'id', header: 'ID' },
    { key: 'orderId', header: 'Order' },
    { key: 'status', header: 'Status' },
    { key: 'amount', header: 'Amount', align: 'right' },
    { key: 'currency', header: 'Currency' },
    { key: 'method', header: 'Method' },
  ]);
  return { payments, formatted };
}

function formatPaymentDetail(payment, { jsonOutput }) {
  if (jsonOutput) return payment;
  return {
    payment,
    formatted:
      `Payment: ${payment.id}\n` +
      `${'-'.repeat(36)}\n` +
      `Order:       ${payment.orderId}\n` +
      `Status:      ${payment.status}\n` +
      `Amount:      ${payment.amount} ${payment.currency}\n` +
      `Method:      ${payment.method || 'N/A'}\n` +
      `Created:     ${payment.createdAt || 'N/A'}`,
  };
}

function formatProviders(providers, { output, jsonOutput }) {
  if (jsonOutput) return providers;
  if (providers.length === 0) return { formatted: 'No payment providers found.' };
  const formatted = output.table(
    providers.map((provider) => ({
      id: provider.id,
      mode: provider.mode,
      status: provider.status,
      capabilities: (provider.capabilities || []).join(','),
    })),
    [
      { key: 'id', header: 'Provider' },
      { key: 'mode', header: 'Mode' },
      { key: 'status', header: 'Status' },
      { key: 'capabilities', header: 'Capabilities' },
    ],
  );
  return { providers, formatted };
}

function formatIntentList(intents, { output, jsonOutput }) {
  if (jsonOutput) return intents;
  if (intents.length === 0) return { formatted: 'No payment intents found.' };
  const formatted = output.table(intents, [
    { key: 'id', header: 'Intent' },
    { key: 'providerId', header: 'Provider' },
    { key: 'status', header: 'Status' },
    { key: 'amount', header: 'Amount', align: 'right' },
    { key: 'capturedAmount', header: 'Captured', align: 'right' },
    { key: 'currency', header: 'Currency' },
  ]);
  return { intents, formatted };
}

function formatIntentDetail(intent, { jsonOutput }) {
  if (jsonOutput) return intent;
  return {
    intent,
    formatted:
      `Payment intent: ${intent.id}\n` +
      `${'-'.repeat(40)}\n` +
      `Provider:      ${intent.providerId}\n` +
      `Status:        ${intent.status}\n` +
      `Amount:        ${intent.amount} ${intent.currency}\n` +
      `Captured:      ${intent.capturedAmount}\n` +
      `Refunded:      ${intent.refundedAmount}\n` +
      `Order:         ${intent.orderId || 'N/A'}\n` +
      `Customer:      ${intent.customerId || 'N/A'}`,
  };
}

function formatIntentMutation(message, result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    ...result,
    formatted: `${message}: ${result.intent.id}${result.idempotent ? ' (idempotent)' : ''}`,
  };
}

function formatCaptureResult(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    ...result,
    formatted: result.capture
      ? `Captured ${result.capture.amount} on intent ${result.intent.id}${result.idempotent ? ' (idempotent)' : ''}`
      : `Intent ${result.intent.id} has no remaining capturable balance`,
  };
}

function formatRefundResult(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    ...result,
    formatted: `Refunded ${result.refund.amount} on intent ${result.intent.id}${result.idempotent ? ' (idempotent)' : ''}`,
  };
}

function formatSettlementList(settlements, { output, jsonOutput }) {
  if (jsonOutput) return settlements;
  if (settlements.length === 0) return { formatted: 'No payment settlements found.' };
  const formatted = output.table(settlements, [
    { key: 'id', header: 'Settlement' },
    { key: 'intentId', header: 'Intent' },
    { key: 'status', header: 'Status' },
    { key: 'amount', header: 'Amount', align: 'right' },
    { key: 'currency', header: 'Currency' },
    { key: 'batchId', header: 'Batch' },
  ]);
  return { settlements, formatted };
}

function formatBatchList(batches, { output, jsonOutput }) {
  if (jsonOutput) return batches;
  if (batches.length === 0) return { formatted: 'No payment settlement batches found.' };
  const formatted = output.table(batches, [
    { key: 'id', header: 'Batch' },
    { key: 'providerId', header: 'Provider' },
    { key: 'status', header: 'Status' },
    { key: 'currency', header: 'Currency' },
    { key: 'grossAmount', header: 'Gross', align: 'right' },
    { key: 'settlementCount', header: 'Items', align: 'right' },
  ]);
  return { batches, formatted };
}

function formatSettlementBatchResult(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    ...result,
    formatted:
      `Settlement batch ${result.batch.id}\n` +
      `${'-'.repeat(30)}\n` +
      `Provider:     ${result.provider.id}\n` +
      `Status:       ${result.batch.status}\n` +
      `Settlements:  ${result.count}\n` +
      `Idempotent:   ${result.idempotent ? 'yes' : 'no'}`,
  };
}

function formatReconciliation(result, { output, jsonOutput }) {
  if (jsonOutput) return result;
  if (result.reconciliation.length === 0) {
    return { formatted: 'No payment reconciliation rows found.' };
  }
  const table = output.table(result.reconciliation, [
    { key: 'intentId', header: 'Intent' },
    { key: 'providerId', header: 'Provider' },
    { key: 'intentStatus', header: 'Intent Status' },
    { key: 'outstandingAmount', header: 'Outstanding', align: 'right' },
    { key: 'reconciliationStatus', header: 'Reconciliation' },
  ]);
  return {
    result,
    formatted:
      `Payment reconciliation\n` +
      `${'-'.repeat(32)}\n` +
      `Authorized:   ${result.summary.authorizedAmount}\n` +
      `Captured:     ${result.summary.capturedAmount}\n` +
      `Refunded:     ${result.summary.refundedAmount}\n` +
      `Settled:      ${result.summary.settledAmount}\n` +
      `Outstanding:  ${result.summary.outstandingAmount}\n\n` +
      table,
  };
}

function formatWebhookResult(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    result,
    formatted:
      `Payment webhook\n` +
      `${'-'.repeat(24)}\n` +
      `Provider:    ${result.provider.id}\n` +
      `Event type:  ${result.eventType}\n` +
      `Action:      ${result.action}\n` +
      `Applied:     ${result.applied ? 'yes' : 'no'}\n` +
      `Intent:      ${result.intent?.id || 'N/A'}`,
  };
}

export const metadata = {
  name: 'payments',
  aliases: ['pay', 'pmt'],
  description: 'Payments, intents, settlements, and provider webhooks',
  actions: {
    list: { description: 'List payments', args: ['[orderId]', '[status]'] },
    get: { description: 'Get payment', args: ['<paymentId>'] },
    create: {
      description: 'Create payment',
      args: ['<orderId>', '<amount>', '[currency]', '[method]'],
    },
    complete: { description: 'Complete payment', args: ['<paymentId>'] },
    fail: { description: 'Mark payment failed', args: ['<paymentId>', '<reason>', '[code]'] },
    cancel: { description: 'Cancel payment', args: ['<paymentId>'] },
    refund: { description: 'Create refund', args: ['<paymentId>', '<amount>', '[reason]'] },
    providers: { description: 'List payment providers', args: ['[capability]', '[mode]'] },
    intents: {
      description: 'List payment intents',
      args: ['[providerId]', '[status]', '[orderId]', '[customerId]', '[limit]'],
    },
    intent: { description: 'Get payment intent', args: ['<intentId>'] },
    'create-intent': {
      description: 'Create payment intent',
      args: [
        '<amount>',
        '[currency]',
        '[captureMethod]',
        '[orderId]',
        '[customerId]',
        '[providerId]',
      ],
    },
    'capture-intent': { description: 'Capture payment intent', args: ['<intentId>', '[amount]'] },
    'cancel-intent': { description: 'Cancel payment intent', args: ['<intentId>', '[reason]'] },
    'refund-intent': {
      description: 'Refund payment intent',
      args: ['<intentId>', '[amount]', '[reason]'],
    },
    settlements: {
      description: 'List settlements',
      args: ['[providerId]', '[status]', '[batchId]', '[limit]'],
    },
    batches: {
      description: 'List settlement batches',
      args: ['[providerId]', '[status]', '[payoutReference]', '[limit]'],
    },
    settle: {
      description: 'Create settlement batch',
      args: ['[providerId]', '[intentIdsCsv]', '[payoutReference]'],
    },
    reconcile: {
      description: 'Reconcile provider state',
      args: ['[providerId]', '[status]', '[orderId]', '[includeBalanced]', '[limit]'],
    },
    webhook: {
      description: 'Ingest payment webhook',
      args: ['<eventType>', '[intentId]', '[providerId]'],
    },
  },
};

export default { execute, metadata };
