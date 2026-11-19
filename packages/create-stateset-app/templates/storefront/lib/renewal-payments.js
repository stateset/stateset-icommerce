import { settledInvoiceIntent, postStripeRefund } from './stripe-settlement.js';
import { decimalToUnits, unitsToDecimal } from './money.js';
import { merchantRecords, readMerchantRecord, saveMerchantRecord } from './merchant-records.js';

export async function recordRenewalPayment(
  commerce,
  stripe,
  binding,
  enrollment,
  invoice,
  { line, start, end },
) {
  const [intent, terms, merchandise, sub] = await Promise.all([
    settledInvoiceIntent(stripe, invoice),
    readMerchantRecord(commerce, 'stripe-membership-' + enrollment.orderId),
    readMerchantRecord(commerce, 'merchandise-' + enrollment.orderId),
    commerce.subscriptions.get(binding.nativeId),
  ]);
  const item = merchandise?.items.find((item) => item.sku === terms?.sku);
  if (
    !intent ||
    !sub ||
    !item ||
    enrollment.providerId !== binding.providerId ||
    sub.purchaseOrderId !== enrollment.orderId ||
    terms?.planId !== enrollment.planId ||
    binding.currency !== 'usd' ||
    line.amount !== terms.regularMinor ||
    invoice.amount_paid !== terms.regularMinor
  )
    throw new Error('Renewal payment does not match the purchased membership');
  const gross = BigInt(invoice.amount_paid),
    base = decimalToUnits(item.unitPrice, 2);
  if (base > gross) throw new Error('Renewal price is below its purchased base price');
  const tax = unitsToDecimal(gross - base, 2);
  const references = { invoiceId: invoice.id, intentId: intent.id, subscriptionId: sub.id };
  await saveMerchantRecord(commerce, 'renewal-intent-' + intent.id, references);
  await saveMerchantRecord(commerce, 'renewal-invoice-' + invoice.id, references);
  const payment = await commerce.payments.createExact({
    customerId: sub.customerId,
    idempotencyKey: 'stripe:' + intent.id,
    amount: unitsToDecimal(gross, 2),
    currency: 'USD',
    paymentMethod: 'credit_card',
  });
  if (
    payment.orderId ||
    payment.invoiceId ||
    payment.customerId !== sub.customerId ||
    payment.currency !== 'USD' ||
    decimalToUnits(payment.amountExact, 2) !== gross ||
    payment.idempotencyKey !== 'stripe:' + intent.id
  )
    throw new Error('Renewal charge already belongs to another payment');
  if (!['completed', 'partially_refunded', 'refunded'].includes(payment.status))
    await commerce.payments.markCompleted(payment.id);
  const periodStart = new Date(start * 1000).toISOString(),
    periodEnd = new Date(end * 1000).toISOString();
  const cycle = await commerce.subscriptions.recordPaidRenewal(
    sub.id,
    payment.id,
    'stripe:' + invoice.id,
    periodStart,
    periodEnd,
    tax,
  );
  return saveMerchantRecord(
    commerce,
    'renewal-receipt-' + invoice.id,
    {
      ...references,
      paymentId: payment.id,
      cycleId: cycle.id,
      providerId: binding.providerId,
      lineId: line.id,
      periodStart,
      periodEnd,
      tax,
      paidAt: invoice.status_transitions?.paid_at
        ? new Date(invoice.status_transitions.paid_at * 1000).toISOString()
        : null,
    },
    'renewal-receipts',
  );
}

export async function recordRenewalCredit(commerce, receipt, refund) {
  // A failed attempt can first appear during backfill after the balance was
  // fully refunded. Only an existing reservation needs releasing.
  if (['canceled', 'failed'].includes(refund.status) && !refund.metadata?.stateset_refund_id) {
    const history = await commerce.payments.getRefunds(receipt.paymentId);
    if (!history.some((row) => row.idempotencyKey === 'stripe-refund:' + refund.id)) return;
  }
  const posted = await postStripeRefund(commerce, receipt.paymentId, refund);
  if (posted.status === 'completed')
    await commerce.subscriptions.recordPaidRenewal(
      receipt.subscriptionId,
      receipt.paymentId,
      'stripe:' + receipt.invoiceId,
      receipt.periodStart,
      receipt.periodEnd,
      receipt.tax,
    );
  return posted;
}

export async function renewalStatement(commerce) {
  const receipts = [];
  for await (const row of merchantRecords(commerce, 'renewal-receipts')) {
    const payment = await commerce.payments.get(row.paymentId);
    if (!payment) throw new Error('Renewal receipt is missing its native payment');
    const gross = decimalToUnits(payment.amountExact, 2),
      refunded = decimalToUnits(payment.amountRefundedExact, 2);
    receipts.push({
      invoice: row.invoiceId,
      subscription: row.subscriptionId,
      cycle: row.cycleId,
      payment: row.paymentId,
      currency: payment.currency,
      status: payment.status,
      paidAt: row.paidAt,
      periodStart: row.periodStart,
      periodEnd: row.periodEnd,
      tax: row.tax,
      collected: unitsToDecimal(gross, 2),
      refunded: unitsToDecimal(refunded, 2),
      netCollected: unitsToDecimal(gross - refunded, 2),
      sellerAllocation: (await renewalSellerTerms(commerce, row.subscriptionId)).status,
    });
  }
  return receipts.sort(
    (a, b) => b.periodStart.localeCompare(a.periodStart) || a.invoice.localeCompare(b.invoice),
  );
}

export async function renewalSellerTerms(commerce, subscriptionId) {
  const enrollment = await readMerchantRecord(commerce, 'provider-enrollment-' + subscriptionId);
  if (!enrollment) throw new Error('Unknown purchased membership');
  const orderId = enrollment.orderId;
  const snapshot = await readMerchantRecord(commerce, 'renewal-sellers-' + orderId);
  if (snapshot)
    return { orderId, status: snapshot.allocations.length ? 'assigned' : 'merchant', ...snapshot };
  const original = await readMerchantRecord(commerce, 'seller-order-' + orderId);
  if (!original?.allocations.length) return { orderId, status: 'merchant', allocations: [] };
  const terms = await readMerchantRecord(commerce, 'stripe-membership-' + orderId);
  const merchandise = await readMerchantRecord(commerce, 'merchandise-' + orderId);
  const items = merchandise?.items || [];
  const item = items.find((row) => row.sku === terms?.sku);
  // Older checkouts froze seller amounts per order. Reuse them only when
  // the renewal has exactly the same merchandise base as that single item.
  if (
    item &&
    items.length === 1 &&
    original.allocations.length === 1 &&
    item.quantity === 1 &&
    decimalToUnits(item.discount || '0', 2) === 0n &&
    decimalToUnits(item.unitPrice, 2) === decimalToUnits(original.allocations[0].gross, 2)
  )
    return { orderId, status: 'assigned', allocations: original.allocations };
  return { orderId, status: 'needs review', allocations: [], item, original: original.allocations };
}
