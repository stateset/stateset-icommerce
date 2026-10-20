import fs from 'node:fs';
import { getCommerceCtor, adaptCommerceApis } from '../commerce.js';
import { createKernelToolExecutor } from '../kernel-tool-execution.js';
import { compareDecimals } from '../utils/exact-decimal.js';
import { refundAction } from './refund-operations.js';
import { readNativeReceipt } from './receipt-engine.js';

export function refundCommitment(kernel, amount, currency) {
  if (typeof currency !== 'string' || !/^[A-Z]{3}$/.test(currency))
    throw new Error('The reviewed refund currency is unavailable.');
  const configured = kernel.commitment ?? {};
  if (!configured || typeof configured !== 'object' || Array.isArray(configured))
    throw new Error('Refund commitment must be operator-owned configuration.');
  if (
    configured.amount !== null &&
    configured.amount !== undefined &&
    (configured.amount.currency !== currency ||
      typeof configured.amount.amount !== 'string' ||
      compareDecimals(configured.amount.amount, amount) !== 0)
  )
    throw new Error('Operator commitment does not match the reviewed refund amount and currency.');
  return { ...configured, amount: { amount, currency } };
}

export async function previewRefund(commerce, kernel, payload) {
  const params = refundAction(payload.params);
  const payment = await commerce.payments.get(params.paymentId);
  if (!payment) throw new Error('Payment is unavailable.');
  const execute = createKernelToolExecutor({ commerce, kernel, allowApply: false });
  const result = await execute('create_refund', params, {
    idempotencyKey: payload.idempotencyKey,
    commitment: refundCommitment(kernel, params.amount, payment.currency),
  });
  return {
    params,
    receipt: result.receipt,
    payment: {
      id: payment.id,
      paymentNumber: payment.paymentNumber,
      orderId: payment.orderId,
      amountExact: payment.amountExact,
      amountRefundedExact: payment.amountRefundedExact,
      currency: payment.currency,
      status: payment.status,
    },
  };
}

export async function applyRefund(database, kernel, payload) {
  const params = refundAction(payload.params);
  if (payload.expectedVersion !== null)
    throw new Error('Refund creation does not accept an aggregate version.');
  if (!fs.statSync(database).isFile()) throw new Error('The payment store must already exist.');
  const Commerce = getCommerceCtor();
  const commerce = adaptCommerceApis(await Commerce.open(database, { maxConnections: 1 }));
  try {
    const execute = createKernelToolExecutor({ commerce, kernel, allowApply: true });
    const result = await execute('create_refund', params, {
      idempotencyKey: payload.idempotencyKey,
      commitment: refundCommitment(kernel, params.amount, payload.context?.currency),
    });
    return { receipt: result.receipt };
  } finally {
    await commerce.close();
  }
}

export function reconcileRefund(database, kernel, payload) {
  return readNativeReceipt(database, kernel, payload.idempotencyKey, 'payments.create_refund');
}
