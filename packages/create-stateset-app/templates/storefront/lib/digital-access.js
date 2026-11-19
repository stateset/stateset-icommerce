import { createHash, createHmac, timingSafeEqual } from 'node:crypto';
import {
  firstMerchantRecord,
  merchantRecords,
  readMerchantRecord,
  saveMerchantRecord,
} from './merchant-records.js';
import { membershipExpiry } from './billing-recovery.js';

const digest = (text) => createHash('sha256').update(text).digest('hex');

function secret(env) {
  if (Buffer.byteLength(env.STATESET_LICENSE_SECRET || '') < 32)
    throw new Error('Configure a license signing key of at least 32 bytes');
  return env.STATESET_LICENSE_SECRET;
}

export function digitalProducts(env) {
  const products = JSON.parse(env.STATESET_DIGITAL_PRODUCTS_JSON || '{}');
  for (const product of Object.values(products)) {
    if (
      typeof product.resource !== 'string' ||
      !/^[a-zA-Z0-9_.-]{1,100}$/.test(product.resource) ||
      (product.validDays !== undefined &&
        (!Number.isInteger(product.validDays) ||
          product.validDays < 1 ||
          product.validDays > 36500)) ||
      (product.refundPolicy && !['any', 'full'].includes(product.refundPolicy)) ||
      (product.membershipPlanId !== undefined &&
        (!/^[a-f\d]{8}-(?:[a-f\d]{4}-){3}[a-f\d]{12}$/i.test(product.membershipPlanId) ||
          product.validDays !== undefined)) ||
      (product.recurring !== undefined &&
        (typeof product.recurring !== 'boolean' || !product.membershipPlanId)) ||
      (product.roles &&
        (!Array.isArray(product.roles) ||
          !product.roles.every((r) => typeof r === 'string' && r.length <= 100)))
    ) {
      throw new Error('Invalid digital product access policy');
    }
  }
  if (Object.keys(products).length) secret(env);
  return products;
}

export async function checkCartMembershipRail(commerce, env, cartId) {
  if (!cartId || !env.STATESET_DIGITAL_PRODUCTS_JSON) return;
  const products = digitalProducts(env);
  if (!Object.values(products).some((p) => p.recurring)) return;
  const items = await commerce.carts.getItems(cartId);
  const policies = Object.fromEntries(
    items
      .filter((item) => products[item.sku]?.recurring)
      .map((item) => [item.sku, products[item.sku]]),
  );
  if (!Object.keys(policies).length) return;
  if ((env.STATESET_CHECKOUT_PROVIDER || 'base') !== 'stripe')
    throw new Error(
      'This cart contains a renewing membership. Its merchant must enable Stripe Checkout.',
    );
  return policies;
}

export function purchaseAccessPolicies(env, items) {
  const products = digitalProducts(env);
  return Object.fromEntries(
    items
      .filter((item) => Object.hasOwn(products, item.sku))
      .map((item) => [item.sku, products[item.sku]]),
  );
}

export async function grantPurchasedAccess(commerce, env, order, payment) {
  const key = 'access-policy-' + order.id;
  const policies =
    (await readMerchantRecord(commerce, key)) ??
    (await firstMerchantRecord(commerce, key, purchaseAccessPolicies(env, order.items)));
  for (const [sku, policy] of Object.entries(policies)) {
    const id = digest([order.id, payment.id, sku, policy.resource].join(':'));
    const acquired = Date.parse(payment.createdAt);
    await firstMerchantRecord(
      commerce,
      'grant-' + id,
      {
        id,
        orderId: order.id,
        customerId: order.customerId,
        paymentId: payment.id,
        sku,
        resource: policy.resource,
        roles: policy.roles || [],
        file: policy.file || null,
        expires: policy.validDays ? acquired + policy.validDays * 86400000 : null,
        refundPolicy: policy.refundPolicy || 'any',
        ...(policy.membershipPlanId && { membershipPlanId: policy.membershipPlanId }),
      },
      'grants-' + order.id,
    );
    if (policy.membershipPlanId && !(await readMerchantRecord(commerce, 'membership-' + id))) {
      const matches = policy.recurring
        ? []
        : await availableSubscriptions(commerce, order.customerId, policy.membershipPlanId);
      if (matches.length > 1) continue;
      const choice = await firstMerchantRecord(commerce, 'enroll-choice-' + id, {
        subscriptionId: matches[0]?.id || null,
      });
      let subscriptionId = choice.subscriptionId;
      if (!subscriptionId) {
        const settled = await commerce.payments.get(payment.id);
        if (settled?.status !== 'completed') continue;
        const sub = await commerce.subscriptions.enrollPaidMembership(
          payment.id,
          policy.membershipPlanId,
        );
        subscriptionId = sub.id;
        await saveMerchantRecord(commerce, 'purchase-enrollment-' + sub.id, {
          orderId: order.id,
          paymentId: payment.id,
        });
      }
      await saveMerchantRecord(commerce, 'membership-' + id, { subscriptionId });
    }
  }
}

function licenseKey(env, id) {
  return (
    'sslic_' +
    id +
    '.' +
    createHmac('sha256', secret(env))
      .update('license:' + id)
      .digest('base64url')
  );
}

// Only an unambiguous native customer and plan can supply a purchase's membership.
async function availableSubscriptions(commerce, customerId, planId) {
  const matches = [];
  for (let offset = 0; ; offset += 50) {
    const rows = await commerce.subscriptions.list({ customerId, planId, offset, limit: 50 });
    for (const row of rows) {
      if (
        !row.purchaseOrderId &&
        !(await readMerchantRecord(commerce, 'purchase-enrollment-' + row.id))
      )
        matches.push(row);
      if (matches.length === 2) return matches;
    }
    if (rows.length < 50) return matches;
  }
}

async function matchingSubscription(commerce, customerId, planId) {
  const matches = await availableSubscriptions(commerce, customerId, planId);
  return matches.length === 1 ? matches[0] : null;
}

// Call only after the host has authenticated the owner of this purchase.
export async function licensesForOrder(commerce, env, orderId) {
  const licenses = [];
  for await (const grant of merchantRecords(commerce, 'grants-' + orderId)) {
    const token = licenseKey(env, grant.id);
    const access = await checkDigitalAccess(commerce, env, token, grant.resource);
    licenses.push({
      resource: grant.resource,
      licenseKey: token,
      active: access.allowed,
      hasDownload: Boolean(grant.file) && access.allowed,
      expires: access.expires ?? grant.expires,
    });
  }
  return licenses;
}

export async function checkDigitalAccess(commerce, env, token, resource, now = Date.now()) {
  if (typeof token !== 'string' || typeof resource !== 'string') return { allowed: false };
  const match = /^sslic_([a-f0-9]{64})\.[\w-]{43}$/.exec(token || '');
  if (!match || !timingSafeEqual(Buffer.from(token), Buffer.from(licenseKey(env, match[1]))))
    return { allowed: false };
  const grant = await readMerchantRecord(commerce, 'grant-' + match[1]);
  if (!grant || grant.resource !== resource || (grant.expires && grant.expires <= now))
    return { allowed: false };
  const [payment, order] = await Promise.all([
    commerce.payments.get(grant.paymentId),
    commerce.orders.get(grant.orderId),
  ]);
  const allowPartial = grant.refundPolicy === 'full' && payment?.status === 'partially_refunded';
  if (
    !order ||
    ['cancelled', 'refunded', 'returned'].includes(order.status) ||
    payment?.orderId !== order.id ||
    (payment.status !== 'completed' && !allowPartial)
  )
    return { allowed: false };
  let expires = grant.expires;
  if (grant.membershipPlanId) {
    const membership = await readMerchantRecord(commerce, 'membership-' + grant.id);
    if (!membership && (await readMerchantRecord(commerce, 'enroll-choice-' + grant.id)))
      return { allowed: false };
    const sub = membership
      ? await commerce.subscriptions.get(membership.subscriptionId)
      : await matchingSubscription(commerce, order.customerId, grant.membershipPlanId);
    if (
      !sub ||
      sub.customerId !== order.customerId ||
      sub.planId !== grant.membershipPlanId ||
      !['active', 'trial', 'cancelled'].includes(sub.status)
    )
      return { allowed: false };
    expires = await membershipExpiry(
      commerce,
      sub,
      now,
      Date.parse(sub.currentPeriodEnd),
      grant.refundPolicy,
    );
    if (sub.endsAt) expires = Math.min(expires, Date.parse(sub.endsAt));
    if (sub.status === 'trial' && sub.trialEndsAt)
      expires = Math.min(expires, Date.parse(sub.trialEndsAt));
    const started = Date.parse(sub.startedAt);
    if (!Number.isFinite(started) || !Number.isFinite(expires) || expires <= now || started > now)
      return { allowed: false };
  }
  return { allowed: true, resource: grant.resource, roles: grant.roles, expires, file: grant.file };
}

export async function linkMembership(commerce, orderId, resource, subscriptionId, apply) {
  const subscription = await commerce.subscriptions.get(subscriptionId);
  const order = await commerce.orders.get(orderId);
  if (!subscription || !order || subscription.customerId !== order.customerId)
    throw new Error('The subscription must belong to the order customer');
  let matched = 0;
  for await (const grant of merchantRecords(commerce, 'grants-' + orderId)) {
    if (grant.resource !== resource || grant.membershipPlanId !== subscription.planId) continue;
    if (apply) await saveMerchantRecord(commerce, 'membership-' + grant.id, { subscriptionId });
    matched++;
  }
  if (!matched) throw new Error('The purchased resource does not grant this membership plan');
  return { orderId, resource, subscriptionId, mode: apply ? 'linked' : 'preview' };
}
