import { GovernedOperations } from './governed-operations.js';
import { compareDecimals } from '../utils/exact-decimal.js';

const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const currency = (value) => typeof value === 'string' && /^[A-Z]{3}$/.test(value);

export function refundAction(value) {
  if (
    !value ||
    typeof value !== 'object' ||
    Array.isArray(value) ||
    Object.keys(value).some((key) => !['paymentId', 'amount', 'reason'].includes(key)) ||
    typeof value.paymentId !== 'string' ||
    !uuid.test(value.paymentId) ||
    typeof value.amount !== 'string' ||
    !/^(0|[1-9]\d{0,27})(?:\.\d{1,28})?$/.test(value.amount) ||
    value.amount.replace('.', '').replace(/^0+/, '').length > 28 ||
    compareDecimals(value.amount, '0') <= 0 ||
    typeof value.reason !== 'string' ||
    !value.reason.trim() ||
    value.reason.length > 240 ||
    /[\p{Cc}\p{Bidi_Control}]/u.test(value.reason)
  )
    throw new Error(
      'Choose a valid payment, a positive exact decimal amount (up to 28 digits) and a reason of 1–240 characters.',
    );
  return { paymentId: value.paymentId, amount: value.amount, reason: value.reason.trim() };
}

export function refundReceiptIssues(receipt, review) {
  if (receipt?.status !== 'succeeded') return [];
  const issues = [],
    result = receipt.result;
  if (
    !result ||
    typeof result.id !== 'string' ||
    !uuid.test(result.id) ||
    receipt.aggregate_id !== result.id ||
    result.payment_id !== review.params.paymentId
  )
    issues.push('Native refund identifies a different or unavailable payment or refund.');
  let exact = false;
  try {
    exact =
      typeof result?.amount === 'string' &&
      result.amount.length <= 64 &&
      compareDecimals(result.amount, review.params.amount) === 0;
  } catch {
    /* Keep the actual receipt visible even when its money is malformed. */
  }
  if (!exact || result?.currency !== review.context.currency)
    issues.push('Native refund amount or currency does not match the reviewed request.');
  if (result?.status !== 'pending' || result?.reason !== review.params.reason)
    issues.push('Native refund status or reason does not match the reviewed pending request.');
  if (receipt.version_before !== null || receipt.version_after !== 1)
    issues.push('Native refund receipt does not describe creation of one new refund.');
  if (
    receipt.economic_context?.principal?.id !== review.principalId ||
    receipt.economic_context?.store_id !== review.storeId
  )
    issues.push('Native receipt scope does not match the reviewed operator and store.');
  return issues;
}

export class RefundOperations extends GovernedOperations {
  constructor(options) {
    super(options, {
      name: 'refund',
      title: 'Refund',
      resource: 'payment',
      commandType: 'payments.create_refund',
      contextInApply: true,
      validate: refundAction,
      receiptIssues: refundReceiptIssues,
      validVersion: (version) => version === null,
      context: (data) => {
        if (!currency(data.payment?.currency))
          throw new Error('The payment currency is unavailable.');
        return { currency: data.payment.currency };
      },
      validContext: (context) => currency(context?.currency),
      // A refund is a new aggregate. A succeeded receipt is checked against the
      // reviewed payment/amount; its aggregate ID is the new refund, not payment.
      matchesAggregate: () => true,
      historySummary: (params, context) => ({
        paymentId: params.paymentId,
        amount: params.amount,
        currency: context.currency,
      }),
    });
  }
}
