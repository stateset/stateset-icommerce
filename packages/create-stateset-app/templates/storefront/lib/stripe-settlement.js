import { CheckoutError } from './checkout-service.js';
import { decimalToUnits, unitsToDecimal } from './money.js';
import { saveMerchantRecord } from './merchant-records.js';

const id = (value) => (typeof value === 'string' ? value : value?.id);

// Hosted membership Checkout collects the complete invoice on one PaymentIntent.
export async function settledInvoiceIntent(stripe, invoice) {
  const transactions = await stripe.invoicePayments.list({
    invoice: invoice.id,
    status: 'paid',
    limit: 2,
    expand: ['data.payment.payment_intent'],
  });
  const tx = transactions.data?.[0];
  if (
    !tx ||
    transactions.has_more ||
    transactions.data.length !== 1 ||
    tx.status !== 'paid' ||
    id(tx.invoice) !== invoice.id ||
    tx.currency !== invoice.currency ||
    tx.amount_paid !== invoice.amount_paid ||
    tx.payment?.type !== 'payment_intent'
  )
    return null;
  const ref = tx.payment.payment_intent;
  const intent = typeof ref === 'string' ? await stripe.paymentIntents.retrieve(ref) : ref;
  if (
    !intent ||
    id(intent) !== id(ref) ||
    !/^pi_[A-Za-z0-9]+$/.test(intent.id) ||
    intent.status !== 'succeeded' ||
    id(intent.customer) !== id(invoice.customer) ||
    intent.currency !== invoice.currency ||
    intent.amount_received !== invoice.amount_paid
  )
    return null;
  return intent;
}

// Reuse a merchant-issued refund when its webhook crosses the originating request.
export async function postStripeRefund(commerce, paymentId, remote) {
  if (remote.currency !== 'usd' || !Number.isSafeInteger(remote.amount) || remote.amount <= 0)
    throw new CheckoutError('Invalid refund amount or currency', 409);
  const localId = remote.metadata?.stateset_refund_id;
  const refund = localId
    ? await commerce.payments.getRefund(localId)
    : await commerce.payments.createRefundExact({
        paymentId,
        amount: unitsToDecimal(BigInt(remote.amount), 2),
        reason: 'Stripe refund',
        idempotencyKey: 'stripe-refund:' + remote.id,
      });
  if (
    !refund ||
    refund.paymentId !== paymentId ||
    decimalToUnits(refund.amountExact, 2) !== BigInt(remote.amount)
  )
    throw new CheckoutError('Provider refund differs from local refund', 409);
  await saveMerchantRecord(commerce, 'stripe-refund-' + refund.id, { refundId: remote.id });
  if (['pending', 'processing'].includes(refund.status)) {
    if (remote.status === 'succeeded') return commerce.payments.completeRefund(refund.id);
    if (['failed', 'canceled'].includes(remote.status))
      return commerce.payments.failRefund(refund.id, remote.failure_reason || remote.status);
  }
  return refund;
}
