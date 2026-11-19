import { createHmac, timingSafeEqual } from 'node:crypto';
import {
  firstMerchantRecord,
  merchantRecords,
  readMerchantRecord,
  saveMerchantRecord,
} from './merchant-records.js';

import { recordRenewalPayment, recordRenewalCredit } from './renewal-payments.js';
export { renewalStatement } from './renewal-payments.js';

const day = 86400;

export function retrySchedule(env) {
  const days = JSON.parse(env.STATESET_RECOVERY_RETRY_DAYS || '[1,3,7]');
  if (
    !Array.isArray(days) ||
    days.length > 12 ||
    !days.every((n, i) => Number.isInteger(n) && n >= 0 && n <= 90 && (!i || n > days[i - 1]))
  ) {
    throw new Error('Recovery retry days must be increasing integers from 0 to 90');
  }
  return days;
}

function recoverySecret(env) {
  if (!env.STATESET_RECOVERY_SECRET || Buffer.byteLength(env.STATESET_RECOVERY_SECRET) < 32) {
    throw new Error('Set a recovery signing secret of at least 32 bytes');
  }
  return env.STATESET_RECOVERY_SECRET;
}

function invoiceSubscription(invoice) {
  const sub = invoice.parent?.subscription_details?.subscription ?? invoice.subscription;
  return typeof sub === 'string' ? sub : sub?.id;
}

export async function linkBillingSubscription(commerce, stripe, nativeId, providerId, apply) {
  if (!/^sub_[A-Za-z0-9]+$/.test(providerId || ''))
    throw new Error('Supply a Stripe subscription ID');
  const [local, remote] = await Promise.all([
    commerce.subscriptions.get(nativeId),
    stripe.subscriptions.retrieve(providerId),
  ]);
  if (
    !local ||
    remote.id !== providerId ||
    remote.metadata?.stateset_subscription_id !== local.id ||
    remote.metadata?.stateset_customer_id !== local.customerId ||
    remote.currency !== local.currency.toLowerCase() ||
    typeof remote.customer !== 'string'
  )
    throw new Error(
      'Provider subscription does not match native customer, subscription and currency',
    );
  const binding = {
    nativeId: local.id,
    providerId,
    customer: remote.customer,
    currency: remote.currency,
  };
  if (apply) {
    await saveMerchantRecord(commerce, 'billing-native-' + local.id, binding);
    await saveMerchantRecord(commerce, 'billing-sub-' + providerId, binding);
  }
  return { ...binding, mode: apply ? 'linked' : 'preview' };
}

/** @param {import('stripe').default} [stripe] */
export async function recordRenewalFailure(commerce, event, stripe) {
  const invoice = event.data.object;
  if (event.type !== 'invoice.payment_failed' || invoice.billing_reason !== 'subscription_cycle')
    return;
  if (
    !/^in_[a-zA-Z0-9]+$/.test(invoice.id) ||
    !Number.isSafeInteger(invoice.amount_remaining) ||
    invoice.amount_remaining <= 0 ||
    !Number.isSafeInteger(invoice.amount_paid ?? 0) ||
    (invoice.amount_paid ?? 0) < 0 ||
    typeof invoice.customer !== 'string' ||
    !Number.isSafeInteger(event.created)
  ) {
    throw new Error('Invalid failed renewal');
  }
  const providerId = invoiceSubscription(invoice);
  const nativeId = invoice.parent?.subscription_details?.metadata?.stateset_subscription_id;
  if (
    stripe &&
    nativeId &&
    providerId &&
    !(await readMerchantRecord(commerce, 'billing-sub-' + providerId))
  ) {
    await linkBillingSubscription(commerce, stripe, nativeId, providerId, true);
  }
  await firstMerchantRecord(
    commerce,
    `renewal-${event.id}`,
    {
      invoiceId: invoice.id,
      customer: invoice.customer,
      currency: invoice.currency,
      failedAt: event.created,
      amount: String(invoice.amount_remaining),
      paidAtFailure: String(invoice.amount_paid || 0),
    },
    'renewal-failures',
  );
}

async function invoiceBinding(commerce, stripe, invoice) {
  const providerId = invoiceSubscription(invoice);
  if (!providerId) return null;
  let binding = await readMerchantRecord(commerce, 'billing-sub-' + providerId);
  const nativeId = invoice.parent?.subscription_details?.metadata?.stateset_subscription_id;
  if (!binding && nativeId)
    binding = await linkBillingSubscription(commerce, stripe, nativeId, providerId, true);
  if (binding && (binding.customer !== invoice.customer || binding.currency !== invoice.currency))
    throw new Error('Billing invoice differs from its linked membership');
  return binding;
}

export function invoiceMembershipLines(stripe, invoice) {
  return invoice.lines?.has_more
    ? stripe.invoices.listLineItems(invoice.id, { limit: 100 })
    : invoice.lines?.data || [];
}

export function invoicePriceId(line) {
  const price = line.pricing?.price_details?.price ?? line.price;
  return typeof price === 'string' ? price : price?.id;
}

// Stripe supplies the exact paid-through dates, including calendar-month boundaries.
export async function recordPaidRenewal(commerce, stripe, invoiceId) {
  const invoice = await stripe.invoices.retrieve(invoiceId);
  if (invoice.id !== invoiceId) throw new Error('Provider returned a different invoice');
  return recordPaidInvoice(commerce, stripe, invoice);
}

function grantPeriod(commerce, binding, invoiceId, lineId, start, end) {
  return saveMerchantRecord(
    commerce,
    'paid-period-' + invoiceId + '-' + lineId,
    { invoiceId, providerId: binding.providerId, start, end },
    'paid-periods-' + binding.nativeId,
  );
}

async function recordPaidInvoice(commerce, stripe, invoice, grant = true) {
  if (
    !['subscription_create', 'subscription_cycle'].includes(invoice.billing_reason) ||
    invoice.status !== 'paid' ||
    invoice.amount_remaining !== 0 ||
    !Number.isSafeInteger(invoice.amount_paid) ||
    invoice.amount_paid <= 0
  )
    return;
  const binding = await invoiceBinding(commerce, stripe, invoice);
  if (!binding) return;
  const enrollment = await readMerchantRecord(commerce, 'provider-enrollment-' + binding.nativeId);
  const periods = [];
  for await (const line of invoiceMembershipLines(stripe, invoice)) {
    const detail = line.parent?.subscription_item_details;
    const subscription = detail?.subscription ?? line.subscription;
    if (
      subscription !== binding.providerId ||
      (detail?.proration ?? line.proration) !== false ||
      (enrollment && (invoicePriceId(line) !== enrollment.priceId || line.quantity !== 1)) ||
      (line.amount <= 0 &&
        !(enrollment && invoice.id === enrollment.initialInvoice && line.amount === 0)) ||
      line.currency !== binding.currency
    )
      continue;
    const { start, end } = line.period || {};
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || end <= start)
      throw new Error('Invalid provider billing period');
    periods.push({ line, start, end });
  }
  let receipt;
  if (enrollment && invoice.billing_reason === 'subscription_cycle' && periods.length) {
    if (periods.length !== 1) throw new Error('Ambiguous membership renewal period');
    receipt = await recordRenewalPayment(
      commerce,
      stripe,
      binding,
      enrollment,
      invoice,
      periods[0],
    );
  }
  if (grant) {
    for (const { line, start, end } of periods)
      await grantPeriod(commerce, binding, invoice.id, line.id, start * 1000, end * 1000);
  }
  return receipt;
}

export async function reconcileRenewals(commerce, stripe, subscriptionId, apply = false) {
  const enrollment = await readMerchantRecord(commerce, 'provider-enrollment-' + subscriptionId);
  const binding = await readMerchantRecord(commerce, 'billing-native-' + subscriptionId);
  if (!enrollment || !binding || enrollment.providerId !== binding.providerId)
    throw new Error('Choose a membership purchased through Stripe Checkout');
  const invoices = [];
  for await (const invoice of stripe.invoices.list({
    subscription: binding.providerId,
    status: 'paid',
    limit: 100,
  })) {
    if (invoice.billing_reason !== 'subscription_cycle') continue;
    if (
      invoiceSubscription(invoice) !== binding.providerId ||
      invoice.customer !== binding.customer ||
      invoice.currency !== binding.currency
    )
      throw new Error('Provider invoice differs from its linked membership');
    if (!apply) {
      invoices.push({ invoice: invoice.id, state: 'preview' });
      continue;
    }
    const receipt = await recordPaidRenewal(commerce, stripe, invoice.id);
    if (!receipt) {
      invoices.push({ invoice: invoice.id, state: 'unmatched' });
      continue;
    }
    for await (const refund of stripe.refunds.list({
      payment_intent: receipt.intentId,
      limit: 100,
    }))
      await recordRenewalRefund(commerce, stripe, invoice, refund);
    invoices.push({ invoice: invoice.id, state: 'reconciled', payment: receipt.paymentId });
  }
  return { subscriptionId, mode: apply ? 'apply' : 'preview', invoices };
}

// Refunds of later invoices reverse only the affected paid period. Fetch the
// captured charge for the cumulative refunded amount after multiple partials.
export async function recordRenewalRefund(commerce, stripe, invoice, refund) {
  if (refund.currency !== invoice.currency || invoice.billing_reason !== 'subscription_cycle')
    return;
  const binding = await invoiceBinding(commerce, stripe, invoice);
  if (!binding) return;
  const enrollment = await readMerchantRecord(commerce, 'provider-enrollment-' + binding.nativeId);
  if (!enrollment) return;
  const policies = await readMerchantRecord(commerce, 'access-policy-' + enrollment.orderId);
  const policy = Object.values(policies || {}).find(
    (p) => p.membershipPlanId === enrollment.planId && p.recurring,
  );
  const charge = await stripe.charges.retrieve(
    typeof refund.charge === 'string' ? refund.charge : refund.charge?.id,
  );
  if (
    !Number.isSafeInteger(refund.amount) ||
    refund.amount <= 0 ||
    charge.id !== (typeof refund.charge === 'string' ? refund.charge : refund.charge?.id) ||
    charge.payment_intent !== refund.payment_intent ||
    charge.currency !== invoice.currency ||
    charge.customer !== binding.customer ||
    charge.amount_captured !== invoice.amount_paid ||
    !Number.isSafeInteger(charge.amount_refunded) ||
    charge.amount_refunded > charge.amount_captured ||
    (refund.status === 'succeeded' && charge.amount_refunded < refund.amount)
  )
    throw new Error('Refund differs from the paid membership invoice');
  const receipt = await recordPaidInvoice(commerce, stripe, invoice, false);
  if (!receipt || receipt.intentId !== refund.payment_intent)
    throw new Error('Refund does not match the membership charge');
  const posted = await recordRenewalCredit(commerce, receipt, refund);
  if (
    policy &&
    charge.amount_refunded > 0 &&
    (policy.refundPolicy !== 'full' || charge.amount_refunded === invoice.amount_paid)
  )
    await saveMerchantRecord(commerce, 'period-revoked-' + invoice.id, {
      invoiceId: invoice.id,
      providerId: binding.providerId,
    });
  await grantPeriod(
    commerce,
    binding,
    invoice.id,
    receipt.lineId,
    Date.parse(receipt.periodStart),
    Date.parse(receipt.periodEnd),
  );
  return posted;
}

export async function recordMembershipStatus(commerce, stripe, event, now = Date.now()) {
  const id = event.data.object.id;
  let binding = await readMerchantRecord(commerce, 'billing-sub-' + id);
  const nativeId = event.data.object.metadata?.stateset_subscription_id;
  if (!binding && nativeId)
    binding = await linkBillingSubscription(commerce, stripe, nativeId, id, true);
  if (!binding) return;
  const remote = await stripe.subscriptions.retrieve(id);
  if (remote.customer !== binding.customer || remote.currency !== binding.currency)
    throw new Error('Membership identity changed');
  const endsAt = remote.ended_at || remote.cancel_at || null;
  await firstMerchantRecord(
    commerce,
    'membership-event-' + event.id,
    {
      providerId: id,
      status: remote.status,
      observedAt: now,
      endsAt: endsAt ? endsAt * 1000 : null,
    },
    'billing-status-' + binding.nativeId,
  );
}

export async function membershipExpiry(commerce, sub, now, nativeEnd, refundPolicy = 'any') {
  const binding = await readMerchantRecord(commerce, 'billing-native-' + sub.id);
  if (!binding) return nativeEnd;
  const enrolled = await readMerchantRecord(commerce, 'provider-enrollment-' + sub.id);
  let end = enrolled ? 0 : nativeEnd,
    latest;
  for await (const period of merchantRecords(commerce, 'paid-periods-' + sub.id)) {
    if (period.providerId !== binding.providerId || period.start > now || now >= period.end)
      continue;
    if (await readMerchantRecord(commerce, 'period-revoked-' + period.invoiceId)) continue;
    if (enrolled && period.invoiceId !== enrolled.initialInvoice) {
      const receipt = await readMerchantRecord(commerce, 'renewal-receipt-' + period.invoiceId);
      // Historical provider-only grants have no corresponding native renewal payment.
      if (receipt) {
        const payment = await commerce.payments.get(receipt.paymentId);
        if (
          receipt.subscriptionId !== sub.id ||
          receipt.providerId !== binding.providerId ||
          payment?.customerId !== sub.customerId ||
          !(
            payment?.status === 'completed' ||
            (refundPolicy === 'full' && payment?.status === 'partially_refunded')
          )
        )
          continue;
      }
    }
    end = Math.max(end, period.end);
  }
  for await (const state of merchantRecords(commerce, 'billing-status-' + sub.id)) {
    if (state.providerId !== binding.providerId) continue;
    if (state.status === 'canceled' || state.status === 'incomplete_expired')
      return Math.min(end, state.endsAt ?? state.observedAt);
    if (!latest || latest.observedAt < state.observedAt) latest = state;
  }
  if (latest?.status === 'paused' || latest?.status === 'unpaid' || latest?.status === 'incomplete')
    return 0;
  return latest?.endsAt ? Math.min(end, latest.endsAt) : end;
}

function recoveryToken(env, invoice, expires) {
  const payload = `${invoice}.${expires}`;
  const signature = createHmac('sha256', recoverySecret(env)).update(payload).digest('base64url');
  return `${payload}.${signature}`;
}

export async function recoveryPortal(stripe, env, token, now = Date.now()) {
  const [invoiceId, expires, signature, extra] = token.split('.');
  if (
    extra !== undefined ||
    !/^in_[a-zA-Z0-9]+$/.test(invoiceId) ||
    !/^\d{10}$/.test(expires) ||
    !/^[\w-]{43}$/.test(signature || '') ||
    Number(expires) * 1000 < now ||
    !timingSafeEqual(Buffer.from(recoveryToken(env, invoiceId, expires)), Buffer.from(token))
  ) {
    throw new Error('Payment update link is invalid or expired');
  }
  const invoice = await stripe.invoices.retrieve(invoiceId);
  if (
    invoice.status !== 'open' ||
    invoice.billing_reason !== 'subscription_cycle' ||
    !invoice.customer
  ) {
    throw new Error('This invoice is no longer awaiting payment');
  }
  const session = await stripe.billingPortal.sessions.create({
    customer: invoice.customer,
    flow_data: { type: 'payment_method_update' },
    return_url: env.NEXT_PUBLIC_BASE_URL,
  });
  return session.url;
}

export async function renewalFailures(commerce) {
  const first = new Map();
  for await (const failure of merchantRecords(commerce, 'renewal-failures')) {
    const prior = first.get(failure.invoiceId);
    if (!prior || failure.failedAt < prior.failedAt) first.set(failure.invoiceId, failure);
  }
  return [...first.values()];
}

function recoveredAmount(invoice, failure) {
  if (!Number.isSafeInteger(invoice.amount_paid) || invoice.amount_paid < 0)
    throw new Error('Invalid paid invoice amount');
  const paid = BigInt(invoice.amount_paid) - BigInt(failure.paidAtFailure);
  const risk = BigInt(failure.amount);
  return paid < 0n ? 0n : paid > risk ? risk : paid;
}

export async function recoveryReport(commerce, stripe, env = process.env, now = Date.now()) {
  const currencies = {};
  const invoices = [];
  for (const failure of await renewalFailures(commerce)) {
    const invoice = await stripe.invoices.retrieve(failure.invoiceId);
    if (invoice.currency !== failure.currency || invoice.customer !== failure.customer)
      throw new Error('Renewal invoice identity changed');
    const providerId = invoiceSubscription(invoice);
    const binding = providerId && (await readMerchantRecord(commerce, 'billing-sub-' + providerId));
    if (binding && (binding.customer !== invoice.customer || binding.currency !== invoice.currency))
      throw new Error('Renewal invoice differs from subscription binding');
    const recovered = recoveredAmount(invoice, failure);
    const remaining = invoice.status === 'open' ? BigInt(invoice.amount_remaining) : 0n;
    const totals = (currencies[invoice.currency] ||= {
      failedMinor: '0',
      recoveredMinor: '0',
      outstandingMinor: '0',
    });
    totals.failedMinor = String(BigInt(totals.failedMinor) + BigInt(failure.amount));
    totals.recoveredMinor = String(BigInt(totals.recoveredMinor) + recovered);
    totals.outstandingMinor = String(BigInt(totals.outstandingMinor) + remaining);
    const future = retrySchedule(env)
      .map((offset) => failure.failedAt + offset * day)
      .find((at) => at * 1000 > now);
    const next =
      invoice.status === 'open' &&
      (invoice.next_payment_attempt || (invoice.auto_advance && future));
    invoices.push({
      id: invoice.id,
      status: invoice.status,
      currency: invoice.currency,
      subscriptionId: binding?.nativeId ?? null,
      providerSubscriptionId: providerId ?? null,
      failedAt: new Date(failure.failedAt * 1000).toISOString(),
      nextRetry: next ? new Date(next * 1000).toISOString() : null,
      failedMinor: failure.amount,
      recoveredMinor: String(recovered),
      outstandingMinor: String(remaining),
    });
  }
  const recoveryPercent = Object.fromEntries(
    Object.entries(currencies).map(([currency, sums]) => {
      const hundredths = (BigInt(sums.recoveredMinor) * 10000n) / BigInt(sums.failedMinor);
      return [
        currency,
        String(hundredths / 100n) + '.' + String(hundredths % 100n).padStart(2, '0'),
      ];
    }),
  );
  return { currencies, invoices, recoveryPercent };
}

export class RenewalRecoveryError extends Error {
  constructor(report) {
    const first = report.failures[0];
    super(`Renewal recovery failed for ${first.invoice}: ${first.error}`);
    this.report = report;
  }
}

async function recoverRenewalInvoice(
  failure,
  { commerce, stripe, env, apply, notify, days, timestamp },
  actions,
) {
  const invoice = await stripe.invoices.retrieve(failure.invoiceId);
  if (
    invoice.id !== failure.invoiceId ||
    invoice.customer !== failure.customer ||
    invoice.currency !== failure.currency
  )
    throw new Error('Renewal invoice identity changed');
  if (apply && invoice.status === 'paid' && invoiceSubscription(invoice))
    await recordPaidRenewal(commerce, stripe, invoice.id);
  if (
    invoice.status !== 'open' ||
    invoice.collection_method !== 'charge_automatically' ||
    invoice.amount_remaining <= 0
  )
    return;
  const age = timestamp - failure.failedAt;
  // A restarted worker runs the current step once, without catching up a burst of charges.
  const step = days.findLast((d) => age >= d * day);
  const stage = step === undefined ? 'initial' : String(step);
  const messageKey = `recovery-email-${invoice.id}-${stage}`;
  if (invoice.customer_email && !(await readMerchantRecord(commerce, messageKey))) {
    actions.push({ invoice: invoice.id, action: 'email', stage });
    if (apply) {
      const issued = await firstMerchantRecord(commerce, messageKey + '-start', { timestamp });
      if (timestamp - issued.timestamp >= day && env.RESEND_API_KEY) {
        actions.push({ invoice: invoice.id, action: 'review-email-delivery', stage });
        return;
      }
      const link = new URL('/recover', env.NEXT_PUBLIC_BASE_URL);
      link.searchParams.set('token', recoveryToken(env, invoice.id, issued.timestamp + 14 * day));
      await notify({
        key: messageKey,
        email: invoice.customer_email,
        invoiceId: invoice.id,
        updateUrl: link.href,
        paymentUrl: invoice.hosted_invoice_url,
      });
      await saveMerchantRecord(commerce, messageKey, { invoiceId: invoice.id });
    }
  }
  if (step === undefined || invoice.next_payment_attempt || !invoice.auto_advance) return;
  const key = `recovery-retry-${invoice.id}-${step}`;
  if (await readMerchantRecord(commerce, key)) return;
  const attempts = 'renewal-attempts-' + invoice.id;
  let unresolved = false;
  for await (const attempt of merchantRecords(commerce, attempts)) {
    if (attempt.stage === stage || (await readMerchantRecord(commerce, attempt.operation)))
      continue;
    if (invoice.attempt_count <= attempt.count) {
      actions.push({
        invoice: invoice.id,
        action: 'review-uncertain-retry',
        stage: attempt.stage,
      });
      unresolved = true;
    } else if (apply)
      await saveMerchantRecord(commerce, attempt.operation, { invoiceId: invoice.id });
  }
  if (unresolved) return;
  actions.push({ invoice: invoice.id, action: 'retry', stage });
  if (apply) {
    // Snapshot the attempt counter before dispatch. After a timeout, inspect
    // Stripe's authoritative invoice before reissuing a provider operation.
    const attempt = await firstMerchantRecord(
      commerce,
      key + '-start',
      {
        count: invoice.attempt_count,
        at: timestamp,
        stage,
        operation: key,
      },
      attempts,
    );
    if (invoice.attempt_count === attempt.count) {
      if (timestamp - attempt.at >= day) {
        actions.push({ invoice: invoice.id, action: 'review-uncertain-retry', stage });
        return;
      }
      try {
        await stripe.invoices.pay(invoice.id, {}, { idempotencyKey: key });
      } catch (error) {
        if (error?.type !== 'StripeCardError') throw error;
      }
    }
    await saveMerchantRecord(commerce, key, { invoiceId: invoice.id });
  }
}

export async function runRenewalRecovery({
  commerce,
  stripe,
  env,
  apply = false,
  now = Date.now(),
  notify,
}) {
  const context = {
    commerce,
    stripe,
    env,
    apply,
    notify,
    days: retrySchedule(env),
    timestamp: Math.floor(now / 1000),
  };
  const actions = [];
  const failures = [];
  for (const failure of await renewalFailures(commerce)) {
    try {
      await recoverRenewalInvoice(failure, context, actions);
    } catch (error) {
      failures.push({
        invoice: failure.invoiceId,
        error: error instanceof Error ? error.message : 'Invoice recovery failed',
      });
    }
  }
  const report = { mode: apply ? 'apply' : 'preview', actions };
  if (failures.length) throw new RenewalRecoveryError({ ...report, failures });
  return report;
}

export async function sendRecoveryEmail(env, notice, fetchImpl = fetch) {
  if (!env.RESEND_API_KEY || !env.STATESET_RECOVERY_FROM)
    throw new Error('Configure RESEND_API_KEY and STATESET_RECOVERY_FROM');
  const text = [
    'We could not collect your renewal payment.',
    'Update your payment method: ' + notice.updateUrl,
    ...(notice.paymentUrl ? ['Review or pay your invoice: ' + notice.paymentUrl] : []),
  ].join('\n\n');
  const response = await fetchImpl('https://api.resend.com/emails', {
    method: 'POST',
    redirect: 'error',
    signal: AbortSignal.timeout(12000),
    headers: {
      Authorization: 'Bearer ' + env.RESEND_API_KEY,
      'Content-Type': 'application/json',
      'Idempotency-Key': notice.key,
    },
    body: JSON.stringify({
      from: env.STATESET_RECOVERY_FROM,
      to: [notice.email],
      subject: 'Update your renewal payment',
      text,
    }),
  });
  if (!response.ok) throw new Error('Recovery email provider returned HTTP ' + response.status);
}
