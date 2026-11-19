import { CheckoutError } from './checkout-service.js';
import { settledInvoiceIntent } from './stripe-settlement.js';
import { decimalToUnits } from './money.js';
import { readMerchantRecord, saveMerchantRecord } from './merchant-records.js';
import {
  invoiceMembershipLines,
  invoicePriceId,
  linkBillingSubscription,
  recordPaidRenewal,
  recordMembershipStatus,
} from './billing-recovery.js';

const cadence = {
  weekly: ['week', 1],
  biweekly: ['week', 2],
  monthly: ['month', 1],
  bimonthly: ['month', 2],
  quarterly: ['month', 3],
  semiannual: ['month', 6],
  annual: ['year', 1],
  yearly: ['year', 1],
};
const money = (value) => decimalToUnits(value, 2);
const objectId = (value) => (typeof value === 'string' ? value : value?.id);
function mismatch() {
  return new CheckoutError('Membership invoice does not match the reserved purchase', 409);
}

export async function quoteMembership(commerce, policies, quote, rail) {
  const skus = Object.keys(policies).filter((sku) => policies[sku].membershipPlanId);
  if (!skus.length) return null;
  const recurring = skus.filter((sku) => policies[sku].recurring);
  if (recurring.length && (rail !== 'Stripe' || recurring.length !== 1))
    throw new CheckoutError(
      'Check out one renewing membership at a time using Stripe Checkout',
      409,
    );
  let result = null;
  for (const sku of skus) {
    const plan = await commerce.subscriptions.getPlan(policies[sku].membershipPlanId);
    if (!plan || plan.status !== 'active' || plan.currency !== 'USD')
      throw new CheckoutError('Membership plan is unavailable', 409);
    if (!policies[sku].recurring) continue;
    const items = quote.totals.lines.filter((line) => line.sku === sku);
    const line = items[0];
    if (
      items.length !== 1 ||
      line.quantity !== 1 ||
      money(plan.priceExact) !== money(line.unitPrice)
    )
      throw new CheckoutError('The membership price or quantity differs from the plan', 409);
    const [interval, count] =
      cadence[plan.billingInterval.replace(/[-_]/g, '')] ||
      (plan.billingInterval === 'custom' ? ['day', plan.customIntervalDays] : []);
    if (!interval || !Number.isSafeInteger(count) || count < 1)
      throw new CheckoutError('Invalid membership billing period', 409);
    // Preserve the quoted price and inclusive tax on renewals. Shipping, bumps and the
    // initial coupon affect only the first invoice as shown by the hosted checkout.
    const base = money(plan.priceExact),
      rate = decimalToUnits(quote.totals.rate, 6);
    const regular = base + (base * rate + 500000n) / 1000000n;
    const initial = base - money(line.discount || '0') + money(line.tax);
    if (regular <= 0n || regular > 99999999n || initial < 0n || initial > regular)
      throw new CheckoutError('Invalid recurring quote', 409);
    result = {
      planId: plan.id,
      name: plan.name,
      sku,
      interval,
      count,
      regularMinor: Number(regular),
      oneTimeMinor: Number(money(quote.totals.total) - initial),
      firstDiscountMinor: Number(regular - initial),
    };
  }
  return result;
}

export async function subscriptionCheckoutParameters(stripe, terms, order, metadata) {
  const price = {
    currency: 'usd',
    unit_amount: terms.regularMinor,
    product_data: { name: terms.name },
    tax_behavior: 'inclusive',
    recurring: { interval: terms.interval, interval_count: terms.count },
  };
  const lines = [{ quantity: 1, price_data: price }];
  if (terms.oneTimeMinor > 0)
    lines.push({
      quantity: 1,
      price_data: {
        currency: 'usd',
        unit_amount: terms.oneTimeMinor,
        product_data: { name: 'One-time items and shipping for ' + order.orderNumber },
      },
    });
  const params = {
    mode: 'subscription',
    line_items: lines,
    subscription_data: {
      metadata: {
        ...metadata,
        stateset_customer_id: order.customerId,
        stateset_plan_id: terms.planId,
      },
    },
  };
  if (terms.firstDiscountMinor > 0) {
    const coupon = await stripe.coupons.create(
      {
        duration: 'once',
        amount_off: terms.firstDiscountMinor,
        currency: 'usd',
        name: 'First-order discount',
      },
      { idempotencyKey: 'membership-discount-' + order.id },
    );
    params.discounts = [{ coupon: coupon.id }];
  }
  return params;
}

export async function firstMembershipPayment(stripe, session, terms, order) {
  const providerId = objectId(session.subscription),
    invoiceId = objectId(session.invoice),
    customer = objectId(session.customer);
  if (!providerId || !invoiceId || !customer) throw mismatch();
  const [sub, invoice] = await Promise.all([
    stripe.subscriptions.retrieve(providerId),
    stripe.invoices.retrieve(invoiceId),
  ]);
  const meta = sub.metadata || {},
    ref = invoice.parent?.subscription_details?.subscription ?? invoice.subscription;
  if (
    sub.id !== providerId ||
    objectId(sub.customer) !== customer ||
    sub.currency !== 'usd' ||
    meta.order_id !== order.id ||
    meta.stateset_customer_id !== order.customerId ||
    meta.stateset_plan_id !== terms.planId ||
    meta.storefront_quote !== session.metadata.storefront_quote ||
    meta.cart_id !== session.metadata.cart_id ||
    invoice.id !== invoiceId ||
    objectId(ref) !== providerId ||
    objectId(invoice.customer) !== customer ||
    invoice.billing_reason !== 'subscription_create' ||
    invoice.status !== 'paid' ||
    invoice.currency !== 'usd' ||
    invoice.amount_paid !== session.amount_total ||
    invoice.amount_remaining !== 0
  )
    throw mismatch();
  const intent = await settledInvoiceIntent(stripe, invoice);
  if (!intent) throw mismatch();
  let priceId;
  for await (const line of invoiceMembershipLines(stripe, invoice)) {
    const detail = line.parent?.subscription_item_details;
    if (
      objectId(detail?.subscription ?? line.subscription) !== providerId ||
      (detail?.proration ?? line.proration) !== false
    )
      continue;
    const linePrice = invoicePriceId(line),
      { start, end } = line.period || {};
    if (
      priceId ||
      !linePrice ||
      line.quantity !== 1 ||
      line.currency !== 'usd' ||
      !Number.isSafeInteger(start) ||
      !Number.isSafeInteger(end) ||
      end <= start ||
      !Number.isSafeInteger(line.amount) ||
      line.amount < 0
    )
      throw mismatch();
    priceId = linePrice;
  }
  if (!priceId) throw mismatch();
  const price = await stripe.prices.retrieve(priceId);
  if (
    price.id !== priceId ||
    price.currency !== 'usd' ||
    price.unit_amount !== terms.regularMinor ||
    price.recurring?.interval !== terms.interval ||
    price.recurring?.interval_count !== terms.count
  )
    throw mismatch();
  return { intent, sub, invoice, priceId };
}

export async function activateMembership(
  commerce,
  stripe,
  proof,
  terms,
  order,
  payment,
  sessionId,
) {
  const { sub, invoice, intent } = proof;
  const native = await commerce.subscriptions.enrollPaidMembership(payment.id, terms.planId);
  const metadata = sub.metadata || {};
  if (metadata.stateset_subscription_id && metadata.stateset_subscription_id !== native.id)
    throw mismatch();
  if (!metadata.stateset_subscription_id)
    await stripe.subscriptions.update(
      sub.id,
      { metadata: { stateset_subscription_id: native.id } },
      { idempotencyKey: 'membership-bind-' + order.id },
    );
  await saveMerchantRecord(commerce, 'provider-enrollment-' + native.id, {
    sessionId,
    orderId: order.id,
    planId: terms.planId,
    providerId: sub.id,
    initialInvoice: invoice.id,
    priceId: proof.priceId,
  });
  await linkBillingSubscription(commerce, stripe, native.id, sub.id, true);
  await recordPaidRenewal(commerce, stripe, invoice.id);
  await recordMembershipStatus(commerce, stripe, {
    id: 'checkout-' + sessionId,
    data: { object: { id: sub.id } },
  });
  await saveMerchantRecord(commerce, 'membership-intent-' + intent.id, { sessionId });
}

// The completed Checkout Session is the guest's receipt capability. Restrict the
// hosted portal flow to the bound subscription and its owning Stripe customer.
export async function membershipManagement(commerce, stripe, sessionId, origin) {
  if (!/^cs_(test_|live_)?[a-zA-Z0-9]+$/.test(sessionId || '')) throw mismatch();
  const session = await stripe.checkout.sessions.retrieve(sessionId);
  const orderId = session.metadata?.order_id;
  const registered = await readMerchantRecord(commerce, 'stripe-session-' + orderId);
  const settled = await readMerchantRecord(commerce, 'membership-settled-' + orderId);
  if (
    registered?.sessionId !== sessionId ||
    !settled ||
    session.mode !== 'subscription' ||
    session.status !== 'complete' ||
    session.payment_status !== 'paid' ||
    settled.providerId !== objectId(session.subscription) ||
    settled.customer !== objectId(session.customer)
  )
    throw mismatch();
  const back = new URL('/checkout/card-return', origin);
  back.searchParams.set('session_id', sessionId);
  return stripe.billingPortal.sessions.create({
    customer: settled.customer,
    return_url: back.href,
    flow_data: {
      type: 'subscription_cancel',
      subscription_cancel: { subscription: settled.providerId },
    },
  });
}
