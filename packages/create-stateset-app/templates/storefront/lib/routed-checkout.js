import { createHash, randomBytes } from 'node:crypto';
import { prepareCardOrder, supplementalCheckout } from './stripe-checkout.js';
import { catalogOffer, recordCampaignStep } from './campaigns.js';
import { CheckoutError } from './checkout-service.js';
import { finishRecordedCheckout } from './checkout-finalization.js';
import { grantPurchasedAccess, licensesForOrder } from './digital-access.js';
import {
  firstMerchantRecord,
  merchantRecords,
  readMerchantRecord,
  saveMerchantRecord,
} from './merchant-records.js';
import { decimalToUnits } from './money.js';
import { recordStorefrontRefund, recordStorefrontSale } from './merchant-integrations.js';
import { cardAccounts, publicCardAccount } from './card-accounts.js';

const idFor = (token) => createHash('sha256').update(token).digest('hex');

function sessionAccounts(env, saved) {
  if (!saved) return cardAccounts(env);
  if (
    !/^pk_(test|live)_/.test(env.NEXT_PUBLIC_STRIPE_PUBLISHABLE_KEY || '') ||
    !env.STRIPE_SECRET_KEY
  ) {
    throw new Error('Configure the Stripe account used to save the card');
  }
  return [
    {
      account: 'hosted-stripe',
      provider: 'stripe',
      currencies: ['USD'],
      methods: ['credit_card'],
      available: true,
      publicKey: env.NEXT_PUBLIC_STRIPE_PUBLISHABLE_KEY,
      secret: env.STRIPE_SECRET_KEY,
      customer: saved.customer,
    },
  ];
}

export async function startRoutedCheckout(options, saved = null) {
  const { commerce, input } = options;
  const env = options.env || process.env;
  const accounts = sessionAccounts(env, saved);
  if (typeof commerce.payments.routeCard !== 'function')
    throw new CheckoutError('Install a native engine with card routing', 503);
  if (
    !accounts.some(
      (a) => a.available && a.methods.includes('credit_card') && a.currencies.includes('USD'),
    )
  )
    throw new CheckoutError('No eligible card account', 503);
  let accountIds = accounts.map((a) => a.account);
  const { order, email, terms } = await prepareCardOrder(options, 'Routed', (allocations) => {
    if (!allocations.length) return;
    const eligible = accounts.filter(
      (a) => a.provider === 'stripe' && a.secret === env.STRIPE_SECRET_KEY,
    );
    if (
      !eligible.some(
        (a) => a.available && a.methods.includes('credit_card') && a.currencies.includes('USD'),
      )
    ) {
      throw new CheckoutError('Seller orders require an available Connect platform account', 503);
    }
    accountIds = eligible.map((a) => a.account);
  });
  const payment = await commerce.payments.createExact({
    orderId: order.id,
    customerId: order.customerId,
    idempotencyKey: 'routed:' + order.id,
    amount: order.totalAmountExact,
    currency: 'USD',
    paymentMethod: 'credit_card',
  });
  const access = await firstMerchantRecord(commerce, 'routed-access-' + order.id, {
    token: randomBytes(32).toString('base64url'),
  });
  await saveMerchantRecord(
    commerce,
    'routed-session-' + idFor(access.token),
    {
      orderId: order.id,
      cartId: input.cartId,
      email,
      accountIds,
      paymentId: payment.id,
      created: order.createdAt,
      ...(saved && { saved }),
    },
    'routed-checkouts',
  );
  if (terms) await recordCampaignStep(commerce, terms, 'payment-start', order.id);
  return { url: '/checkout/card-payment#' + access.token };
}

export async function routedSession(commerce, token) {
  if (typeof token !== 'string' || !/^[\w-]{43}$/.test(token))
    throw new CheckoutError('Unknown card checkout', 404);
  const session = await readMerchantRecord(commerce, 'routed-session-' + idFor(token));
  if (!session) throw new CheckoutError('Unknown card checkout', 404);
  return session;
}

export function routedAccounts(env, session) {
  return sessionAccounts(env, session.saved).filter((account) =>
    session.accountIds.includes(account.account),
  );
}

// Called by the authenticated buyer route and the operator recovery worker.
/** @param {{ account?: string, token?: string, stripeSecret?: (key: string, reference: string) => Promise<string|null>, showOffer?: boolean }} [options] */
export async function advanceRoutedCheckout(commerce, env, session, options = {}) {
  const { account, token, stripeSecret, showOffer = false } = options;
  const payment = await commerce.payments.get(session.paymentId);
  const order = await commerce.orders.get(session.orderId);
  if (
    !payment ||
    !order ||
    payment.orderId !== order.id ||
    payment.amountExact !== order.totalAmountExact ||
    payment.currency !== 'USD' ||
    payment.idempotencyKey !== 'routed:' + order.id
  )
    throw new CheckoutError('Card checkout ledger mismatch', 409);
  const display = {
    amount: payment.amountExact,
    currency: payment.currency,
    orderNumber: order.orderNumber,
  };
  if (['failed', 'cancelled', 'voided'].includes(payment.status)) {
    if (order.status === 'pending') await commerce.orders.cancel(order.id);
    await saveMerchantRecord(commerce, 'routed-finished-' + order.id, { paymentId: payment.id });
    return { ...display, state: payment.status === 'cancelled' ? 'expired' : 'declined' };
  }
  if (order.status === 'cancelled') return { ...display, state: 'expired' };
  const accounts = routedAccounts(env, session);
  if (
    token !== undefined &&
    (typeof token !== 'string' ||
      token.length > 4096 ||
      !accounts.some((a) => a.account === account))
  ) {
    throw new CheckoutError('Invalid processor token');
  }
  let settled = ['completed', 'refunded', 'partially_refunded', 'disputed'].includes(
    payment.status,
  );
  if (!settled) {
    const routes = accounts.map((a) => ({
      ...a,
      ...(session.saved
        ? { token: session.saved.method }
        : a.account === account && token
          ? { token }
          : {}),
    }));
    if (Date.now() - Date.parse(session.created) >= 3600000) {
      const expiry = await commerce.payments.routeCard({
        action: 'expire',
        accounts: routes,
        paymentId: payment.id,
      });
      if (expiry.state === 'expired') {
        if (order.status === 'pending') await commerce.orders.cancel(order.id);
        await saveMerchantRecord(commerce, 'routed-finished-' + order.id, {
          paymentId: payment.id,
        });
        return { ...display, state: 'expired' };
      }
    }
    let result = await commerce.payments.routeCard({
      action: 'authorize',
      accounts: routes,
      paymentId: payment.id,
    });
    const selected = accounts.find((a) => a.account === result.account);
    if (result.state === 'collect_token' && selected) {
      const buyer = await commerce.customers.get(order.customerId);
      return {
        ...display,
        state: result.state,
        account: publicCardAccount(selected),
        contact: {
          email: session.email,
          givenName: buyer.firstName,
          familyName: buyer.lastName,
          countryCode: order.shippingAddress.country,
        },
      };
    }
    if (result.state === 'requires_action' && selected?.provider === 'stripe') {
      const clientSecret = await stripeSecret?.(selected.secret, result.reference);
      return {
        ...display,
        state: result.state,
        account: publicCardAccount(selected),
        clientSecret,
      };
    }
    if (result.state === 'authorized' || result.state === 'captured') {
      await saveMerchantRecord(commerce, 'routed-authorization-' + order.id, {
        account: selected.account,
        provider: selected.provider,
        reference: result.reference,
      });
      if (result.state === 'authorized')
        result = await commerce.payments.routeCard({
          action: 'capture',
          accounts: routes,
          paymentId: payment.id,
        });
    }
    settled = result.state === 'captured';
    if (!settled) return { ...display, state: result.state };
  }
  const confirmed = await finishRecordedCheckout(commerce, payment, session.cartId);
  if (!(await readMerchantRecord(commerce, 'routed-authorization-' + order.id))) {
    const selected = await commerce.payments.routeCard({
      action: 'selected',
      accounts,
      paymentId: payment.id,
    });
    if (selected.state !== 'selected')
      throw new CheckoutError('Awaiting the original settlement reference', 409);
    await saveMerchantRecord(commerce, 'routed-authorization-' + order.id, {
      account: selected.account,
      provider: selected.provider,
      reference: selected.reference,
    });
  }
  await grantPurchasedAccess(commerce, env, confirmed, payment);
  await recordStorefrontSale(commerce, confirmed, session.email);
  await saveMerchantRecord(commerce, 'routed-finished-' + order.id, { paymentId: payment.id });
  const receipt = {
    ...display,
    state: 'paid',
    cartId: session.cartId,
    licenses: await licensesForOrder(commerce, env, order.id),
  };
  if (!showOffer) return receipt;
  const campaign = await readMerchantRecord(commerce, 'campaign-order-' + order.id);
  if (
    !campaign?.upsellSku ||
    campaign.parentOrderId ||
    ['cancelled', 'returned', 'refunded'].includes(confirmed.status) ||
    ['refunded', 'partially_refunded', 'disputed'].includes(payment.status)
  )
    return receipt;
  const offer = await catalogOffer(commerce, campaign.upsellSku).catch(() => null);
  if (!offer || (await readMerchantRecord(commerce, 'upsell-skipped-' + order.id))) return receipt;
  await recordCampaignStep(commerce, campaign, 'upsell-view', order.id);
  return { ...receipt, offer: { name: offer.name, unitPrice: offer.unitPrice } };
}

export async function routedOffer(commerce, env, session, action) {
  const payment = await commerce.payments.get(session.paymentId);
  const order = await commerce.orders.get(session.orderId);
  const terms = await readMerchantRecord(commerce, 'campaign-order-' + session.orderId);
  if (
    payment?.status !== 'completed' ||
    payment.orderId !== session.orderId ||
    !order ||
    ['pending', 'cancelled', 'returned', 'refunded'].includes(order.status) ||
    !terms?.upsellSku ||
    terms.parentOrderId ||
    !(await readMerchantRecord(commerce, 'routed-finished-' + session.orderId))
  ) {
    throw new CheckoutError('No offer is available for this purchase', 409);
  }
  const resultKey = 'upsell-result-' + order.id;
  const prior = await readMerchantRecord(commerce, resultKey);
  if (prior) return prior;
  if (action === 'skip') {
    await firstMerchantRecord(commerce, 'upsell-skipped-' + order.id, { orderId: order.id });
    await recordCampaignStep(commerce, terms, 'upsell-skip', order.id);
    return { skipped: true };
  }
  if (action !== 'offer' || (await readMerchantRecord(commerce, 'upsell-skipped-' + order.id)))
    throw new CheckoutError('Offer is closed', 409);
  const options = await supplementalCheckout(commerce, order.id, terms);
  return firstMerchantRecord(commerce, resultKey, await startRoutedCheckout({ ...options, env }));
}

export async function refundRoutedCheckout(
  commerce,
  env,
  { paymentId, amount, requestKey, reason },
) {
  const payment = await commerce.payments.get(paymentId);
  if (payment?.idempotencyKey !== 'routed:' + payment?.orderId)
    throw new CheckoutError('Unknown routed payment', 404);
  const access = await readMerchantRecord(commerce, 'routed-access-' + payment.orderId);
  const session = await routedSession(commerce, access?.token);
  if (
    typeof requestKey !== 'string' ||
    !requestKey.trim() ||
    typeof reason !== 'string' ||
    !reason.trim()
  )
    throw new CheckoutError('Supply a refund reason and request key');
  const units = decimalToUnits(amount, 2);
  if (units <= 0n) throw new CheckoutError('Refund must be positive');
  const local = await commerce.payments.createRefundExact({
    paymentId,
    amount,
    reason,
    idempotencyKey: requestKey,
  });
  if (
    local.paymentId !== paymentId ||
    local.currency !== payment.currency ||
    local.reason !== reason ||
    decimalToUnits(local.amountExact, 2) !== units
  )
    throw new CheckoutError('Refund request differs from its original payment', 409);
  const intent = { paymentId, amount: local.amountExact, reason, requestKey, refundId: local.id };
  await saveMerchantRecord(commerce, 'routed-refund-' + local.id, intent, 'routed-refunds');
  const result = await commerce.payments.routeCard({
    action: 'refund',
    accounts: routedAccounts(env, session),
    paymentId,
    amount,
    requestKey,
    reason,
  });
  const refund = await commerce.payments.getRefund(result.refundId);
  if (refund?.id !== local.id)
    throw new CheckoutError('Processor refund differs from its reserved refund', 409);
  await recordStorefrontRefund(commerce, payment.orderId, refund);
  if (['completed', 'failed', 'cancelled'].includes(refund.status))
    await saveMerchantRecord(commerce, 'routed-refund-finished-' + local.id, {
      status: refund.status,
    });
  return refund;
}

export async function recoverRoutedRefunds(commerce, env, apply = false) {
  const rows = [];
  for await (const request of merchantRecords(commerce, 'routed-refunds')) {
    if (await readMerchantRecord(commerce, 'routed-refund-finished-' + request.refundId)) continue;
    try {
      const state = apply ? (await refundRoutedCheckout(commerce, env, request)).status : 'preview';
      rows.push({ refundId: request.refundId, state });
    } catch (error) {
      rows.push({ refundId: request.refundId, state: 'review', message: error.message });
    }
  }
  return rows;
}
