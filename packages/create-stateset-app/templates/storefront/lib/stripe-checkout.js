import { recordRenewalRefund } from './billing-recovery.js';
import {
  quoteMembership,
  subscriptionCheckoutParameters,
  firstMembershipPayment,
  activateMembership,
} from './membership-checkout.js';
import { sellerAllocations } from './seller-payouts.js';
import { getTaxProvider } from './tax.js';
import { getShippingProvider } from './shipping.js';
import { pricingEnvironment } from './store-pricing.js';
import { addDecimals } from './money.js';
import { grantPurchasedAccess, purchaseAccessPolicies } from './digital-access.js';
import { activeOffer, catalogOffer, recordCampaignStep } from './campaigns.js';
import { firstMerchantRecord } from './merchant-records.js';
import { recordStorefrontRefund, recordStorefrontSale } from './merchant-integrations.js';
import { createHash } from 'node:crypto';
import {
  CheckoutError,
  normalizeCheckoutEmail,
  quoteCheckout,
  reserveCheckoutOrder,
} from './checkout-service.js';
import { finishRecordedCheckout } from './checkout-finalization.js';
import { decimalToUnits } from './money.js';
import { postStripeRefund } from './stripe-settlement.js';
import { readMerchantRecord, saveMerchantRecord } from './merchant-records.js';

const sessionKey = (orderId) => `stripe-session-${orderId}`;
const uuid = /^[\da-f]{8}-(?:[\da-f]{4}-){3}[\da-f]{12}$/i;

function cents(value) {
  const units = decimalToUnits(value, 2);
  if (units < 1n || units > 99999999n)
    throw new CheckoutError('Amount outside card checkout limits', 422);
  return Number(units);
}

async function guestCustomer(commerce, cartId, address) {
  // An unverified email entered at guest checkout does not grant account access.
  const email = `guest-${cartId}@checkout.invalid`;
  const existing = await commerce.customers.getByEmail(email);
  if (existing) return existing;
  try {
    return await commerce.customers.create({
      email,
      firstName: address.firstName,
      lastName: address.lastName,
    });
  } catch (error) {
    const winner = await commerce.customers.getByEmail(email);
    if (!winner) throw error;
    return winner;
  }
}

export async function prepareCardOrder(
  {
    commerce,
    input,
    taxProvider = undefined,
    shippingProvider = undefined,
    campaign = null,
    env = process.env,
    parent = null,
  },
  rail = 'Stripe',
  validateAllocations = undefined,
) {
  if (!uuid.test(input.cartId)) throw new CheckoutError('A valid cart is required');
  const email = normalizeCheckoutEmail(input.email);
  const variant = parent ? null : await activeOffer(commerce, env, campaign);
  const extraSku = parent?.sku || (input.bump === true && variant?.bumpSku);
  if (input.bump === true && !extraSku) throw new CheckoutError('This offer is unavailable', 409);
  const extra = extraSku ? await catalogOffer(commerce, extraSku) : null;
  let terms =
    campaign && (variant || parent)
      ? {
          campaignId: campaign.campaignId,
          ...(campaign.revision > 0 && { revision: campaign.revision }),
          visitor: campaign.visitor,
          variant: campaign.variant,
          source: campaign.source,
          affiliate: campaign.affiliate,
          commissionBps: campaign.commissionBps,
          bump: input.bump === true,
          parentOrderId: parent?.orderId || null,
          upsellSku: variant?.upsellSku || null,
          ...(input.saveCardForOffer === true &&
            variant?.upsellSku &&
            rail === 'Stripe' &&
            /^pk_(test|live)_/.test(env.NEXT_PUBLIC_STRIPE_PUBLISHABLE_KEY || '') && {
              saveCardForOffer: true,
            }),
        }
      : null;
  const pricing = await pricingEnvironment(commerce, env);
  const quote = await quoteCheckout({
    commerce,
    cartId: input.cartId,
    shippingAddress: input.shippingAddress,
    shippingMethodId: input.shippingMethodId,
    taxProvider: taxProvider ?? getTaxProvider(pricing),
    shippingProvider: shippingProvider ?? getShippingProvider(pricing),
    additionalLines: extra ? [extra] : [],
    campaignCoupon: variant?.couponCode ? { sku: variant.sku, code: variant.couponCode } : null,
  });
  if (quote.cart.status !== 'active') throw new CheckoutError('Cart is not active', 409);
  if (variant && !quote.totals.lines.some((line) => line.sku === variant.sku)) {
    if (input.bump) throw new CheckoutError('This bump requires the campaign product', 409);
    terms = null;
  }
  const amount = cents(quote.totals.total);
  if (input.expectedTotal !== undefined && amount !== cents(input.expectedTotal))
    throw new CheckoutError('Offer price changed. Review the new total.', 409);
  const allocations = await sellerAllocations(commerce, env, quote.totals.lines);
  validateAllocations?.(allocations);
  const access = purchaseAccessPolicies(env, quote.totals.lines);
  const recurring = await quoteMembership(commerce, access, quote, rail);
  const fingerprint = createHash('sha256')
    .update(
      JSON.stringify([
        email,
        quote.normalizedAddress,
        quote.shippingQuote,
        quote.totals,
        terms,
        allocations,
        ...(quote.coupon ? [quote.coupon] : []),
        ...(recurring ? [recurring] : []),
      ]),
    )
    .digest('hex');
  const notes = `${rail} checkout: ${fingerprint}; Cart: ${input.cartId}`;
  const customer = await guestCustomer(commerce, input.cartId, quote.normalizedAddress);
  const order = await reserveCheckoutOrder(commerce, input.cartId, customer.id, quote, notes);
  if (
    order.notes !== notes ||
    cents(order.totalAmountExact) !== amount ||
    order.status !== 'pending'
  ) {
    throw new CheckoutError('This cart already has a different or completed checkout', 409);
  }
  if (allocations.length)
    await saveMerchantRecord(commerce, 'seller-order-' + order.id, { allocations });
  await firstMerchantRecord(commerce, 'access-policy-' + order.id, access);
  if (recurring) {
    await saveMerchantRecord(commerce, 'stripe-membership-' + order.id, recurring);
    const line = quote.totals.lines.find((item) => item.sku === recurring.sku);
    const renewalAllocations = await sellerAllocations(commerce, env, [{ ...line, discount: '0' }]);
    await saveMerchantRecord(commerce, 'renewal-sellers-' + order.id, {
      allocations: renewalAllocations,
    });
  }
  await saveMerchantRecord(commerce, 'merchandise-' + order.id, {
    items: quote.totals.lines.map((item) => ({
      sku: item.sku,
      name: item.name,
      quantity: item.quantity,
      unitPrice: item.unitPrice,
      ...(item.discount && { discount: item.discount }),
    })),
  });
  if (terms) {
    await saveMerchantRecord(commerce, 'campaign-order-' + order.id, terms);
    await saveMerchantRecord(commerce, 'checkout-details-' + order.id, {
      email,
      shippingAddress: quote.normalizedAddress,
      shippingMethodId: quote.shippingQuote.id,
    });
    await recordCampaignStep(commerce, terms, 'checkout', order.id);
  }
  return { order, email, fingerprint, amount, terms, recurring };
}

export async function startStripeCheckout(options) {
  const { commerce, stripe, origin, input } = options;
  const { order, email, fingerprint, amount, terms, recurring } = await prepareCardOrder(options);
  const expiresAt = Math.floor(Date.parse(order.createdAt) / 1000) + 3600;
  const metadata = { order_id: order.id, cart_id: input.cartId, storefront_quote: fingerprint };
  const binding = await readMerchantRecord(commerce, sessionKey(order.id));
  let session;
  if (binding) {
    session = await stripe.checkout.sessions.retrieve(binding.sessionId);
  } else {
    // Fixed expiry bounds recovery even after Stripe's idempotency window ends.
    if (Date.now() >= expiresAt * 1000) throw new CheckoutError('Checkout has expired', 409);
    const billing = recurring
      ? await subscriptionCheckoutParameters(stripe, recurring, order, metadata)
      : null;
    session = await stripe.checkout.sessions.create(
      {
        mode: 'payment',
        payment_method_types: ['card'],
        customer_email: email,
        client_reference_id: order.id,
        metadata,
        ...(!recurring && terms?.saveCardForOffer && { customer_creation: 'always' }),
        ...(!recurring && {
          payment_intent_data: {
            metadata,
            ...(terms?.saveCardForOffer && { setup_future_usage: 'on_session' }),
          },
        }),
        expires_at: expiresAt,
        success_url: `${origin}/checkout/card-return?session_id={CHECKOUT_SESSION_ID}`,
        cancel_url: `${origin}/checkout`,
        line_items: [
          {
            quantity: 1,
            price_data: {
              currency: 'usd',
              unit_amount: amount,
              product_data: { name: `Order ${order.orderNumber}` },
            },
          },
        ],
        ...billing,
      },
      { idempotencyKey: `storefront-checkout-${order.id}` },
    );
    await saveMerchantRecord(commerce, sessionKey(order.id), { sessionId: session.id });
  }
  if (session.status !== 'open' || !session.url) throw new CheckoutError('Checkout is closed', 409);
  if (terms) await recordCampaignStep(commerce, terms, 'payment-start', order.id);
  return { url: session.url, sessionId: session.id };
}

export async function reconcileStripeCheckout(commerce, stripe, sessionId) {
  const { receipt } = await settleStripeCheckout(commerce, stripe, sessionId);
  return receipt;
}

async function checkoutOrder(commerce, session) {
  const {
    order_id: orderId,
    cart_id: cartId,
    storefront_quote: fingerprint,
  } = session.metadata || {};
  if (!uuid.test(orderId) || !uuid.test(cartId) || !/^[a-f0-9]{64}$/.test(fingerprint)) {
    throw new CheckoutError('Unrecognized storefront checkout', 404);
  }
  const order = await commerce.orders.get(orderId);
  const recurring = await readMerchantRecord(commerce, 'stripe-membership-' + orderId);
  if (
    !order ||
    order.notes !== `Stripe checkout: ${fingerprint}; Cart: ${cartId}` ||
    session.client_reference_id !== orderId ||
    session.mode !== (recurring ? 'subscription' : 'payment') ||
    session.currency !== 'usd' ||
    session.amount_total !== cents(order.totalAmountExact)
  ) {
    throw new CheckoutError('Provider session does not match the reserved order', 409);
  }
  return { order, recurring };
}

async function checkoutSettlement(stripe, session, recurring, order) {
  const proof = recurring ? await firstMembershipPayment(stripe, session, recurring, order) : null;
  const intent = proof?.intent ?? session.payment_intent;
  if (
    !intent ||
    intent.status !== 'succeeded' ||
    intent.currency !== 'usd' ||
    intent.amount_received !== session.amount_total ||
    (!proof &&
      (intent.metadata?.order_id !== order.id ||
        intent.metadata?.storefront_quote !== session.metadata.storefront_quote ||
        intent.metadata?.cart_id !== session.metadata.cart_id))
  ) {
    throw new CheckoutError('Provider settlement does not match the reserved order', 409);
  }
  return { proof, intent };
}

export async function verifyStripeTestWallet(commerce, stripe, session, expected) {
  const { order, recurring } = await checkoutOrder(commerce, session);
  const binding = await readMerchantRecord(commerce, sessionKey(order.id));
  if (
    session.livemode !== false ||
    session.status !== 'complete' ||
    session.payment_status !== 'paid' ||
    binding?.sessionId !== session.id
  )
    throw new CheckoutError('Complete the bound test checkout before verifying the wallet', 409);
  const { intent } = await checkoutSettlement(stripe, session, recurring, order);
  const chargeId =
    typeof intent.latest_charge === 'string' ? intent.latest_charge : intent.latest_charge?.id;
  if (intent.livemode !== false || !/^ch_[a-zA-Z0-9]+$/.test(chargeId || ''))
    throw new CheckoutError('Stripe has not identified a captured test charge', 409);
  const charge = await stripe.charges.retrieve(chargeId);
  if (
    charge.id !== chargeId ||
    charge.payment_intent !== intent.id ||
    charge.livemode !== false ||
    charge.status !== 'succeeded' ||
    charge.paid !== true ||
    charge.captured !== true ||
    charge.currency !== session.currency ||
    charge.amount_captured !== session.amount_total
  )
    throw new CheckoutError('Stripe charge does not match the captured test payment', 409);
  const details = charge.payment_method_details;
  const wallet = details?.card?.wallet;
  let observed = details?.type || 'unknown';
  if (observed === 'card' && (!details.card || wallet)) observed = wallet?.type || 'unknown';
  if (!wallet || observed !== expected || details?.type !== 'card')
    throw new CheckoutError(
      `Stripe charge ${chargeId} used ${observed}. This drill requires ${expected}.`,
      409,
    );
  return {
    method: observed,
    chargeId,
    paymentIntentId: intent.id,
    amountMinor: charge.amount_captured,
    currency: charge.currency,
  };
}

async function settleStripeCheckout(commerce, stripe, sessionId) {
  if (!/^cs_(test_|live_)?[a-zA-Z0-9]+$/.test(sessionId))
    throw new CheckoutError('Invalid checkout session');
  const session = await stripe.checkout.sessions.retrieve(sessionId, {
    expand: ['payment_intent'],
  });
  const { order, recurring } = await checkoutOrder(commerce, session);
  const orderId = order.id,
    cartId = session.metadata.cart_id;
  await saveMerchantRecord(commerce, sessionKey(orderId), { sessionId });
  if (session.status === 'expired') {
    if (order.status === 'pending') await commerce.orders.cancel(orderId);
    const cart = await commerce.carts.get(cartId);
    if (cart?.status === 'active') await commerce.carts.cancel(cartId);
    return { receipt: { state: 'expired', cartId } };
  }
  if (session.payment_status !== 'paid' || session.status !== 'complete')
    return { receipt: { state: 'pending' } };
  const { proof, intent } = await checkoutSettlement(stripe, session, recurring, order);
  const key = `stripe:${intent.id}`;
  const payment = await commerce.payments.createExact({
    orderId,
    customerId: order.customerId,
    idempotencyKey: key,
    amount: order.totalAmountExact,
    currency: 'USD',
    paymentMethod: 'credit_card',
  });
  if (
    payment.orderId !== orderId ||
    payment.idempotencyKey !== key ||
    cents(payment.amountExact) !== intent.amount_received ||
    payment.currency.toUpperCase() !== 'USD'
  ) {
    throw new CheckoutError('Provider settlement is already bound to another payment', 409);
  }
  const confirmed = await finishRecordedCheckout(commerce, payment, cartId);
  if (proof && recurring) {
    await activateMembership(commerce, stripe, proof, recurring, confirmed, payment, sessionId);
    await saveMerchantRecord(commerce, 'membership-settled-' + orderId, {
      providerId: proof.sub.id,
      customer: proof.sub.customer,
    });
  }
  await grantPurchasedAccess(commerce, process.env, confirmed, payment);
  await recordStorefrontSale(
    commerce,
    confirmed,
    normalizeCheckoutEmail(session.customer_email || session.customer_details?.email),
  );
  const campaign = await readMerchantRecord(commerce, 'campaign-order-' + orderId);
  const upsell =
    campaign?.upsellSku && confirmed.status !== 'cancelled' && payment.status !== 'refunded'
      ? await catalogOffer(commerce, campaign.upsellSku).catch(() => null)
      : null;
  let offer, savedCard;
  if (upsell) {
    offer = { name: upsell.name, unitPrice: upsell.unitPrice };
    if (
      campaign.saveCardForOffer &&
      typeof session.customer === 'string' &&
      intent.customer === session.customer &&
      typeof intent.payment_method === 'string' &&
      !purchaseAccessPolicies(process.env, [upsell])[upsell.sku]?.recurring
    ) {
      const details = await readMerchantRecord(commerce, 'checkout-details-' + orderId);
      const pricing = await pricingEnvironment(commerce);
      const tax = getTaxProvider(pricing).calculateCart([upsell], details.shippingAddress.state, 2);
      const shipping = getShippingProvider(pricing).quote(
        details.shippingAddress,
        details.shippingMethodId,
      );
      offer.chargeTotal = addDecimals([tax.total, shipping.amount], 2);
      savedCard = { customer: session.customer, method: intent.payment_method };
    }
  }
  return {
    receipt: {
      state: 'paid',
      ...(recurring && { renewingMembership: true }),
      ...(offer && { upsell: offer }),
      orderId,
      orderNumber: confirmed.orderNumber,
      cartId,
      paymentId: payment.id,
    },
    savedCard,
  };
}

export async function refundStripePayment(
  commerce,
  stripe,
  { paymentId, amount, reason, idempotencyKey },
) {
  if (!idempotencyKey) throw new CheckoutError('A refund idempotency key is required');
  const payment = await commerce.payments.get(paymentId);
  if (!/^stripe:pi_[a-zA-Z0-9]+$/.test(payment?.idempotencyKey || '')) {
    throw new CheckoutError('Payment does not belong to Stripe checkout', 409);
  }
  const units = cents(amount);
  const intentId = payment.idempotencyKey.slice('stripe:'.length);
  const refund = await commerce.payments.createRefundExact({
    paymentId,
    amount,
    reason,
    idempotencyKey,
  });
  if (refund.paymentId !== paymentId || cents(refund.amountExact) !== units) {
    throw new CheckoutError('Refund key is already in use', 409);
  }
  if (['completed', 'failed', 'cancelled'].includes(refund.status)) {
    if (payment.orderId) await recordStorefrontRefund(commerce, payment.orderId, refund);
    else {
      const saved = await readMerchantRecord(commerce, 'stripe-refund-' + refund.id);
      if (saved) await reconcileStripeRefund(commerce, stripe, saved.refundId);
    }
    return refund;
  }
  const binding = await readMerchantRecord(commerce, `stripe-refund-${refund.id}`);
  let remote;
  if (binding) {
    remote = await stripe.refunds.retrieve(binding.refundId);
  } else {
    // Listing by intent also recovers a provider success after a process crash
    // when Stripe has already pruned the original idempotency key.
    for await (const candidate of stripe.refunds.list({ payment_intent: intentId, limit: 100 })) {
      if (candidate.metadata?.stateset_refund_id === refund.id) {
        remote = candidate;
        break;
      }
    }
    if (!remote)
      remote = await stripe.refunds.create(
        {
          payment_intent: intentId,
          amount: units,
          metadata: { stateset_refund_id: refund.id },
        },
        { idempotencyKey: `storefront-refund-${refund.id}` },
      );
    await saveMerchantRecord(commerce, `stripe-refund-${refund.id}`, { refundId: remote.id });
  }
  if (
    remote.amount !== units ||
    remote.currency !== 'usd' ||
    remote.payment_intent !== intentId ||
    remote.metadata?.stateset_refund_id !== refund.id
  )
    throw new CheckoutError('Provider refund differs from local refund', 409);
  if (!payment.orderId) return reconcileStripeRefund(commerce, stripe, remote.id);
  if (remote.status === 'succeeded') {
    const completed = await commerce.payments.completeRefund(refund.id);
    await recordStorefrontRefund(commerce, payment.orderId, completed);
    return completed;
  }
  if (['failed', 'canceled'].includes(remote.status))
    return commerce.payments.failRefund(refund.id, remote.failure_reason || remote.status);
  return refund;
}

export async function reconcileStripeRefund(commerce, stripe, refundId) {
  const remote = await stripe.refunds.retrieve(refundId);
  if (typeof remote.payment_intent !== 'string') return;
  const saved = await readMerchantRecord(commerce, 'membership-intent-' + remote.payment_intent);
  let session = saved
    ? await stripe.checkout.sessions.retrieve(saved.sessionId)
    : (await stripe.checkout.sessions.list({ payment_intent: remote.payment_intent, limit: 1 }))
        .data[0];
  if (!session && stripe.invoicePayments) {
    const transactions = await stripe.invoicePayments.list({
      payment: { type: 'payment_intent', payment_intent: remote.payment_intent },
      status: 'paid',
      limit: 2,
    });
    if (transactions.data.length !== 1 || transactions.has_more) return;
    const invoice = await stripe.invoices.retrieve(transactions.data[0].invoice);
    const sub = invoice.parent?.subscription_details?.subscription ?? invoice.subscription;
    if (!sub) return;
    if (invoice.billing_reason === 'subscription_cycle') {
      return recordRenewalRefund(commerce, stripe, invoice, remote);
    }
    if (invoice.billing_reason !== 'subscription_create') return;
    session = (await stripe.checkout.sessions.list({ subscription: sub, limit: 1 })).data[0];
    if (session?.invoice !== invoice.id) return;
  }
  if (!session?.metadata?.storefront_quote) return;
  const checkout = await reconcileStripeCheckout(commerce, stripe, session.id);
  if (checkout.state !== 'paid') throw new CheckoutError('Waiting for payment settlement', 409);
  const refund = await postStripeRefund(commerce, checkout.paymentId, remote);
  await recordStorefrontRefund(commerce, checkout.orderId, refund);
  return refund;
}

// Recover an interrupted session creation, including the interval between the
// upstream response and persisting its ID. Call from a scheduled operator job.
export async function recoverStripeOrder(commerce, stripe, orderId, apply = false) {
  const order = await commerce.orders.get(orderId);
  const match = /^Stripe checkout: ([\da-f]{64}); Cart: ([\da-f-]+)$/.exec(order?.notes || '');
  if (!match || !uuid.test(match[2])) throw new CheckoutError('Unknown Stripe checkout order', 404);
  const existing = await readMerchantRecord(commerce, sessionKey(orderId));
  let sessionId = existing?.sessionId;
  const created = Math.floor(Date.parse(order.createdAt) / 1000);
  if (!sessionId) {
    for await (const candidate of stripe.checkout.sessions.list({
      created: { gte: created - 60, lte: created + 3600 },
      limit: 100,
    })) {
      if (
        candidate.client_reference_id === orderId &&
        candidate.metadata?.storefront_quote === match[1]
      ) {
        sessionId = candidate.id;
        break;
      }
    }
  }
  if (sessionId) {
    if (!apply) {
      const session = await stripe.checkout.sessions.retrieve(sessionId);
      return {
        action: 'reconcile',
        sessionId,
        providerStatus: session.status,
        paymentStatus: session.payment_status,
      };
    }
    return reconcileStripeCheckout(commerce, stripe, sessionId);
  }
  if (order.status === 'pending' && Date.now() > (created + 3660) * 1000) {
    if (apply) {
      await commerce.orders.cancel(orderId);
      const cart = await commerce.carts.get(match[2]);
      if (cart?.status === 'active') await commerce.carts.cancel(match[2]);
    }
    return { action: 'release-expired-reservation', applied: apply };
  }
  return { action: 'wait', orderStatus: order.status };
}

export async function supplementalCheckout(commerce, orderId, terms) {
  const key = 'upsell-cart-' + orderId;
  let target = await readMerchantRecord(commerce, key);
  if (!target) {
    const cart = await commerce.carts.create({ currency: 'USD' });
    target = await firstMerchantRecord(commerce, key, { cartId: cart.id });
    if (target.cartId !== cart.id) await commerce.carts.cancel(cart.id);
  }
  const details = await readMerchantRecord(commerce, 'checkout-details-' + orderId);
  return {
    commerce,
    input: { ...details, cartId: target.cartId },
    campaign: terms,
    parent: { sku: terms.upsellSku, orderId },
  };
}

export async function startCheckoutUpsell(
  commerce,
  stripe,
  origin,
  sessionId,
  visitor,
  action = 'accept',
  expectedTotal,
  startSaved,
) {
  const { receipt: paid, savedCard } = await settleStripeCheckout(commerce, stripe, sessionId);
  const terms =
    paid.orderId && (await readMerchantRecord(commerce, 'campaign-order-' + paid.orderId));
  if (!paid.upsell || !terms || terms.visitor !== visitor || terms.parentOrderId) {
    throw new CheckoutError('This purchase is ineligible for the offer', 409);
  }
  if (action === 'skip') {
    await recordCampaignStep(commerce, terms, 'upsell-skip', paid.orderId);
    return { skipped: true };
  }
  if (action !== 'accept') throw new CheckoutError('Unknown offer action');
  const resultKey = 'upsell-result-' + paid.orderId;
  const prior = await readMerchantRecord(commerce, resultKey);
  if (prior) {
    if (prior.sessionId) {
      const original = await stripe.checkout.sessions.retrieve(prior.sessionId);
      if (original.status === 'complete')
        return {
          ...prior,
          url: '/checkout/card-return?session_id=' + encodeURIComponent(prior.sessionId),
        };
    }
    return prior;
  }
  const payment = await commerce.payments.get(paid.paymentId);
  if (payment.status !== 'completed')
    throw new CheckoutError('The original payment has been adjusted', 409);
  const options = {
    ...(await supplementalCheckout(commerce, paid.orderId, terms)),
    stripe,
    origin,
  };
  let result;
  if (expectedTotal !== undefined) {
    if (
      !startSaved ||
      !savedCard ||
      !paid.upsell.chargeTotal ||
      cents(expectedTotal) !== cents(paid.upsell.chargeTotal)
    )
      throw new CheckoutError('Offer total changed. Review the offer again.', 409);
    result = await startSaved(
      { ...options, input: { ...options.input, expectedTotal } },
      savedCard,
    );
  } else result = await startStripeCheckout(options);
  return firstMerchantRecord(commerce, resultKey, result);
}
