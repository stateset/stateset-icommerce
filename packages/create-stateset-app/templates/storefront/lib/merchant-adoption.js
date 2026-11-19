import { randomUUID } from 'node:crypto';
import {
  firstMerchantRecord,
  merchantRecords,
  readMerchantRecord,
  saveMerchantRecord,
} from './merchant-records.js';
import { decimalToUnits } from './money.js';
import { settledInvoiceIntent } from './stripe-settlement.js';
import { routedAccounts, routedSession } from './routed-checkout.js';

const enrollmentKey = 'adoption-enrollment-v1';
const day = 86400000;
const captured = new Set(['completed', 'partially_refunded', 'refunded', 'disputed']);

export async function enrollStore(commerce, source, apply, now = Date.now()) {
  if (!/^[a-z0-9][a-z0-9_-]{0,47}$/.test(source || ''))
    throw new Error('Supply an acquisition channel such as partner or direct with --source');
  const existing = await readMerchantRecord(commerce, enrollmentKey);
  if (!existing && !apply) return { mode: 'preview', source };
  const enrollment =
    existing ||
    (await firstMerchantRecord(commerce, enrollmentKey, {
      installationId: randomUUID(),
      source,
      enrolledAt: new Date(now).toISOString(),
    }));
  if (enrollment.source !== source)
    throw new Error('This store already has a different enrollment source');
  return enrollment;
}

function stripeCapture(intent, amount) {
  return (
    /^pi_[A-Za-z0-9]+$/.test(intent?.id || '') &&
    intent.status === 'succeeded' &&
    intent.currency === 'usd' &&
    intent.amount_received === amount &&
    typeof intent.livemode === 'boolean'
  );
}

// All remote calls in this report are lookups. Checkout, refund and recovery commands never run.
async function hostedEvidence(order, payments, sessionId, stripe, amount) {
  const session = await stripe.checkout.sessions.retrieve(sessionId, {
    expand: ['payment_intent'],
  });
  if (
    session.id !== sessionId ||
    session.metadata?.order_id !== order.id ||
    session.client_reference_id !== order.id ||
    session.status !== 'complete' ||
    session.payment_status !== 'paid' ||
    session.currency !== 'usd' ||
    session.amount_total !== amount
  )
    return 'mismatch';
  let intent = session.payment_intent;
  if (session.mode === 'subscription') {
    const invoiceId = session.invoice?.id ?? session.invoice;
    if (!invoiceId) return 'mismatch';
    const invoice = await stripe.invoices.retrieve(invoiceId);
    if (
      invoice.id !== invoiceId ||
      invoice.status !== 'paid' ||
      invoice.billing_reason !== 'subscription_create' ||
      invoice.livemode !== session.livemode ||
      invoice.amount_paid !== amount
    )
      return 'mismatch';
    intent = await settledInvoiceIntent(stripe, invoice);
  } else if (session.mode !== 'payment' || intent?.metadata?.order_id !== order.id)
    return 'mismatch';
  if (
    !stripeCapture(intent, amount) ||
    session.livemode !== intent.livemode ||
    !payments.some((p) => p.idempotencyKey === 'stripe:' + intent.id)
  )
    return 'mismatch';
  return session.livemode ? 'live' : 'sandbox';
}

async function routedEvidence(commerce, env, order, payments, binding, amount, stripeFor, fetcher) {
  const access = await readMerchantRecord(commerce, 'routed-access-' + order.id);
  const session = await routedSession(commerce, access?.token);
  const payment = payments.find(
    (p) => p.id === session.paymentId && p.idempotencyKey === 'routed:' + order.id,
  );
  if (!payment || session.orderId !== order.id) return 'mismatch';
  const accounts = routedAccounts(env, session);
  const selected = await commerce.payments.routeCard({
    paymentId: payment.id,
    action: 'selected',
    accounts,
  });
  const account = accounts.find(
    (a) => a.account === binding.account && a.provider === binding.provider,
  );
  if (
    !account ||
    selected.state !== 'selected' ||
    selected.account !== binding.account ||
    selected.provider !== binding.provider ||
    selected.reference !== binding.reference
  )
    return 'mismatch';
  if (account.provider === 'stripe') {
    const intent = await stripeFor(account.secret).paymentIntents.retrieve(binding.reference);
    if (intent.id !== binding.reference || !stripeCapture(intent, amount)) return 'mismatch';
    return intent.livemode ? 'live' : 'sandbox';
  }
  const origin = account.sandbox
    ? 'https://connect.squareupsandbox.com'
    : 'https://connect.squareup.com';
  const response = await fetcher(origin + '/v2/payments/' + encodeURIComponent(binding.reference), {
    headers: { Authorization: 'Bearer ' + account.secret, 'Square-Version': '2025-10-16' },
    redirect: 'error',
    cache: 'no-store',
    signal: AbortSignal.timeout(12000),
  });
  if (!response.ok) return 'unavailable';
  const remote = (await response.json()).payment;
  if (
    remote?.id !== binding.reference ||
    remote.location_id !== account.location ||
    remote.status !== 'COMPLETED' ||
    remote.amount_money?.currency !== 'USD' ||
    remote.amount_money.amount !== amount
  )
    return 'mismatch';
  return account.sandbox ? 'sandbox' : 'live';
}

async function checkSale(commerce, event, order, options) {
  if (
    !order ||
    order.id !== event.orderId ||
    order.createdAt !== event.placedAt ||
    order.currency !== 'USD' ||
    event.currency !== 'USD'
  )
    return 'mismatch';
  const amount = Number(decimalToUnits(order.totalAmountExact, 2));
  if (
    !Number.isSafeInteger(amount) ||
    amount <= 0 ||
    BigInt(amount) !== decimalToUnits(event.total, 2)
  )
    return 'mismatch';
  const payments = (await commerce.payments.list({ orderId: order.id })).filter(
    (p) =>
      p.orderId === order.id &&
      p.customerId === order.customerId &&
      captured.has(p.status) &&
      p.currency === 'USD' &&
      decimalToUnits(p.amountExact, 2) === BigInt(amount),
  );
  if (!payments.length) return 'mismatch';
  const hosted = await readMerchantRecord(commerce, 'stripe-session-' + order.id);
  if (hosted?.sessionId?.startsWith('cs_test_')) return 'sandbox';
  if (hosted)
    return hostedEvidence(
      order,
      payments,
      hosted.sessionId,
      options.stripeFor(options.env.STRIPE_SECRET_KEY),
      amount,
    );
  const routed = await readMerchantRecord(commerce, 'routed-authorization-' + order.id);
  if (routed)
    return routedEvidence(
      commerce,
      options.env,
      order,
      payments,
      routed,
      amount,
      options.stripeFor,
      options.fetcher ?? fetch,
    );
  return 'mismatch';
}

export async function adoptionReport(commerce, options = {}) {
  if (options.apply && !options.check)
    throw new Error('Use --check with --apply to retain verified captures');
  const enrollment = await readMerchantRecord(commerce, enrollmentKey);
  if (!enrollment)
    return { status: 'unenrolled', next: 'merchant adoption-enroll --source CHANNEL --apply' };
  const now = options.now ?? Date.now();
  const started = Date.parse(enrollment.enrolledAt);
  const counts = {
    live: 0,
    sandbox: 0,
    unchecked: 0,
    unavailable: 0,
    mismatch: 0,
    otherRails: 0,
    supplemental: 0,
  };
  const live = [];
  for await (const event of merchantRecords(commerce, 'merchant-events')) {
    if (event.type !== 'sale') continue;
    const placed = Date.parse(event.placedAt),
      posted = Date.parse(event.at);
    if (
      event.key !== 'sale-' + event.orderId ||
      !Number.isFinite(placed) ||
      !Number.isFinite(posted) ||
      posted < placed
    ) {
      counts.mismatch++;
      continue;
    }
    if (placed < started || posted > now) continue;
    const campaign = await readMerchantRecord(commerce, 'campaign-order-' + event.orderId);
    if (campaign?.parentOrderId) {
      counts.supplemental++;
      continue;
    }
    const order = await commerce.orders.get(event.orderId);
    if (order?.notes?.startsWith('Verified Base settlement:')) {
      counts.otherRails++;
      continue;
    }
    const key = 'adoption-card-capture-' + event.orderId;
    const receipt = await readMerchantRecord(commerce, key);
    let result = 'unchecked';
    if (receipt) {
      result =
        receipt.orderId === event.orderId &&
        receipt.saleAt === event.at &&
        receipt.amount === event.total &&
        receipt.currency === event.currency &&
        ['live', 'sandbox'].includes(receipt.mode)
          ? receipt.mode
          : 'mismatch';
    } else if (options.check) {
      try {
        result = await checkSale(commerce, event, order, options);
      } catch {
        // Account rotation, a missing key or provider downtime leaves this enrollment partially observed.
        result = 'unavailable';
      }
      if (options.apply && ['live', 'sandbox'].includes(result)) {
        await saveMerchantRecord(commerce, key, {
          orderId: event.orderId,
          saleAt: event.at,
          currency: event.currency,
          amount: event.total,
          mode: result,
        });
      }
    }
    counts[result]++;
    if (result === 'live') live.push({ placed, posted });
  }
  live.sort((a, b) => a.posted - b.posted);
  const first = live[0]?.posted;
  const repeat = live.find((sale) => first !== undefined && sale.placed >= first + 7 * day);
  return {
    schema: 'stateset.storefront-adoption.v1',
    ...enrollment,
    observedAt: new Date(now).toISOString(),
    coverage: counts.unavailable || counts.mismatch || counts.unchecked ? 'partial' : 'complete',
    scope: 'primary card checkouts placed since enrollment',
    counts,
    firstVerifiedSaleAt: first === undefined ? null : new Date(first).toISOString(),
    hoursToFirstVerifiedSale: first === undefined ? null : (first - started) / 3600000,
    repeatVerifiedSaleAt: repeat ? new Date(repeat.posted).toISOString() : null,
  };
}
