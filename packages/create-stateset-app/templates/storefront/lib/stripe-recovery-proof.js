import { invoiceMembershipLines } from './billing-recovery.js';
import { readMerchantRecord } from './merchant-records.js';
import { decimalToUnits } from './money.js';
import { settledInvoiceIntent } from './stripe-settlement.js';

const id = (value) => (typeof value === 'string' ? value : value?.id);
const subscriptionId = (invoice) =>
  id(invoice.parent?.subscription_details?.subscription ?? invoice.subscription);

// Read the same failure and paid-invoice records used by the webhook and worker.
// This check must never repair missing evidence: a drill should expose a broken delivery.
export async function verifyStripeRecovery(commerce, stripe, eventId) {
  if (!/^evt_[a-zA-Z0-9]+$/.test(eventId || '')) throw new Error('Supply a Stripe event ID');
  const event = await stripe.events.retrieve(eventId);
  const failed = event.data?.object;
  if (
    event.id !== eventId ||
    event.livemode !== false ||
    event.type !== 'invoice.payment_failed' ||
    failed?.object !== 'invoice' ||
    failed.livemode !== false ||
    failed.billing_reason !== 'subscription_cycle' ||
    failed.status !== 'open' ||
    failed.collection_method !== 'charge_automatically' ||
    !/^in_[a-zA-Z0-9]+$/.test(failed.id || '') ||
    !/^cus_[a-zA-Z0-9]+$/.test(failed.customer || '') ||
    !/^sub_[a-zA-Z0-9]+$/.test(subscriptionId(failed) || '') ||
    failed.currency !== 'usd' ||
    !Number.isSafeInteger(event.created) ||
    !Number.isSafeInteger(failed.amount_remaining) ||
    failed.amount_remaining <= 0 ||
    failed.amount_paid !== 0
  )
    throw new Error('Choose a sandbox payment failure on an unpaid USD subscription renewal');

  const invoice = await stripe.invoices.retrieve(failed.id);
  const sub = subscriptionId(failed);
  if (
    invoice.id !== failed.id ||
    invoice.livemode !== false ||
    invoice.customer !== failed.customer ||
    invoice.currency !== failed.currency ||
    subscriptionId(invoice) !== sub ||
    invoice.billing_reason !== failed.billing_reason
  )
    throw new Error('Current invoice differs from the failed renewal');
  const report = {
    mode: 'sandbox',
    eventId,
    invoiceId: invoice.id,
    providerSubscriptionId: sub,
    testClock: id(invoice.test_clock) ?? null,
    currency: invoice.currency,
    failedMinor: String(failed.amount_remaining),
    invoiceStatus: invoice.status,
    providerConfirmed: false,
  };
  const failure = await readMerchantRecord(commerce, 'renewal-' + eventId);
  if (!failure) return { ...report, state: 'missing_failure_webhook' };
  if (
    failure.invoiceId !== failed.id ||
    failure.customer !== failed.customer ||
    failure.currency !== failed.currency ||
    failure.amount !== report.failedMinor ||
    failure.paidAtFailure !== '0' ||
    failure.failedAt !== event.created
  )
    throw new Error('Recorded failure differs from the Stripe event');

  if (invoice.status !== 'paid')
    return { ...report, state: invoice.status === 'open' ? 'awaiting_payment' : 'closed_unpaid' };
  if (invoice.amount_remaining !== 0 || invoice.amount_paid !== failed.amount_remaining)
    throw new Error('Paid invoice does not match the amount at risk');
  const intent = await settledInvoiceIntent(stripe, invoice);
  const chargeId = id(intent?.latest_charge);
  if (intent?.livemode !== false || !/^ch_[a-zA-Z0-9]+$/.test(chargeId || ''))
    throw new Error('Invoice needs one successful sandbox PaymentIntent with a charge');
  const charge = await stripe.charges.retrieve(chargeId);
  if (
    charge.id !== chargeId ||
    charge.livemode !== false ||
    charge.status !== 'succeeded' ||
    !charge.paid ||
    !charge.captured ||
    id(charge.payment_intent) !== intent.id ||
    id(charge.customer) !== invoice.customer ||
    charge.currency !== invoice.currency ||
    charge.amount_captured !== invoice.amount_paid ||
    !Number.isSafeInteger(charge.amount_refunded) ||
    charge.amount_refunded < 0 ||
    charge.amount_refunded > charge.amount_captured ||
    charge.disputed !== false
  )
    throw new Error('Renewal needs a matching, undisputed captured charge');
  const settled = {
    ...report,
    intentId: intent.id,
    chargeId,
    capturedMinor: String(charge.amount_captured),
  };

  const receipt = await readMerchantRecord(commerce, 'renewal-receipt-' + invoice.id);
  if (!receipt) return { ...settled, state: 'missing_native_renewal' };
  const binding = await readMerchantRecord(commerce, 'billing-sub-' + sub);
  if (
    receipt.invoiceId !== invoice.id ||
    receipt.intentId !== intent.id ||
    receipt.providerId !== sub ||
    !binding ||
    binding.nativeId !== receipt.subscriptionId ||
    binding.providerId !== sub ||
    binding.customer !== invoice.customer ||
    binding.currency !== invoice.currency
  )
    throw new Error('Native renewal receipt or membership binding differs from Stripe');
  const [membership, payment, cycle] = await Promise.all([
    commerce.subscriptions.get(binding.nativeId),
    commerce.payments.get(receipt.paymentId),
    commerce.subscriptions.getBillingCycle(receipt.cycleId),
  ]);
  const amount = BigInt(invoice.amount_paid);
  const refunded = BigInt(charge.amount_refunded);
  const paymentStatus =
    refunded === amount ? 'refunded' : refunded > 0n ? 'partially_refunded' : 'completed';
  let period;
  for await (const line of invoiceMembershipLines(stripe, invoice)) {
    if (
      line.id === receipt.lineId &&
      line.currency === invoice.currency &&
      line.amount === invoice.amount_paid &&
      id(line.parent?.subscription_item_details?.subscription ?? line.subscription) === sub &&
      (line.parent?.subscription_item_details?.proration ?? line.proration) === false
    )
      period = line.period;
  }
  if (
    !membership ||
    membership.id !== binding.nativeId ||
    !membership.purchaseOrderId ||
    !payment ||
    payment.id !== receipt.paymentId ||
    payment.customerId !== membership.customerId ||
    payment.status !== paymentStatus ||
    payment.currency !== 'USD' ||
    payment.idempotencyKey !== 'stripe:' + intent.id ||
    decimalToUnits(payment.amountExact, 2) !== amount ||
    decimalToUnits(payment.amountRefundedExact, 2) !== refunded ||
    !cycle ||
    cycle.id !== receipt.cycleId ||
    cycle.status !== (refunded === amount ? 'refunded' : 'paid') ||
    !Number.isInteger(cycle.cycleNumber) ||
    cycle.cycleNumber < 2 ||
    cycle.subscriptionId !== membership.id ||
    cycle.paymentId !== payment.id ||
    cycle.currency !== 'USD' ||
    decimalToUnits(cycle.totalExact, 2) !== amount ||
    !period ||
    !Number.isSafeInteger(period.start) ||
    !Number.isSafeInteger(period.end) ||
    period.end <= period.start ||
    Date.parse(cycle.periodStart) !== period.start * 1000 ||
    Date.parse(cycle.periodEnd) !== period.end * 1000
  )
    throw new Error('Native payment or paid membership cycle differs from the captured charge');
  return {
    ...settled,
    state: 'recovered',
    providerConfirmed: true,
    recoveredMinor: String(amount),
    refundedMinor: String(refunded),
    netRetainedMinor: String(amount - refunded),
    membershipId: membership.id,
    paymentId: payment.id,
    cycleId: cycle.id,
  };
}
