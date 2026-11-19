import { randomBytes, createHash } from 'node:crypto';
import { affiliateBalances, campaignAccounting, campaignCatalog } from './campaigns.js';
import { firstMerchantRecord, merchantRecords, readMerchantRecord } from './merchant-records.js';
import { sellerBalances } from './seller-payouts.js';
import { integrationStatus, processIntegrations } from './merchant-integrations.js';
import { localLaunchChecks } from './merchant-onboarding.js';
import { pricingEnvironment, storePricing } from './store-pricing.js';
import { renewalStatement } from './billing-recovery.js';

const digest = (value) => createHash('sha256').update(value).digest('hex');
const roles = new Set(['affiliate', 'seller', 'merchant']);

// Store only a digest. The operator delivers the resulting expiring link directly to the recipient.
export async function issuePortal(
  commerce,
  origin,
  role,
  subject,
  days,
  apply = false,
  now = Date.now(),
) {
  if (
    !roles.has(role) ||
    !/^[a-z0-9][a-z0-9_-]{0,47}$/.test(subject || '') ||
    !Number.isInteger(days) ||
    days < 1 ||
    days > 90
  )
    throw new Error('Supply a role, account code and 1 to 90 days');
  if (role !== 'merchant' && !(await readMerchantRecord(commerce, role + '-' + subject)))
    throw new Error('Unknown ' + role);
  const expires = now + days * 86400000;
  if (!apply) return { role, subject, expires, mode: 'preview' };
  const url = new URL('/portal', origin);
  if (
    url.username ||
    url.password ||
    (url.protocol !== 'https:' &&
      !(url.protocol === 'http:' && ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname)))
  )
    throw new Error('Configure an HTTPS portal origin');
  const token = randomBytes(32).toString('base64url');
  const id = digest(token);
  await firstMerchantRecord(commerce, 'portal-' + id, { id, role, subject, expires });
  url.hash = token;
  return { role, subject, expires, id, url: url.href };
}
export async function revokePortal(commerce, id, apply) {
  if (!/^[a-f0-9]{64}$/.test(id || '') || !(await readMerchantRecord(commerce, 'portal-' + id)))
    throw new Error('Unknown portal link');
  if (apply) await firstMerchantRecord(commerce, 'portal-revoked-' + id, { id });
  return { id, mode: apply ? 'revoked' : 'preview' };
}
export async function portalPrincipal(commerce, token, now = Date.now()) {
  if (typeof token !== 'string' || !/^[\w-]{43}$/.test(token)) return null;
  const id = digest(token);
  const grant = await readMerchantRecord(commerce, 'portal-' + id);
  if (
    !grant ||
    grant.expires <= now ||
    (await readMerchantRecord(commerce, 'portal-revoked-' + id))
  )
    return null;
  return grant;
}

export async function portalReport(commerce, env, principal, filters = {}) {
  const { role, subject } = principal;
  if (role === 'seller')
    return { role, subject, statements: await sellerBalances(commerce, subject) };
  if (role === 'merchant') {
    const [
      campaigns,
      affiliates,
      sellers,
      deliveries,
      catalog,
      integrations,
      pricing,
      renewalReceipts,
    ] = await Promise.all([
      campaignAccounting(commerce, { ...filters, bySource: true, includeOrders: true }),
      affiliateBalances(commerce),
      sellerBalances(commerce),
      processIntegrations(commerce, env),
      campaignCatalog(commerce, env),
      integrationStatus(commerce, env),
      storePricing(commerce, env),
      renewalStatement(commerce),
    ]);
    const drills = [];
    for await (const row of merchantRecords(commerce, 'launch-drills'))
      drills.push({
        drill: row.drill,
        orderId: row.orderId,
        shipmentId: row.shipmentId,
        refundId: row.refundId,
        wallet: row.wallet?.method,
        chargeId: row.wallet?.chargeId,
      });
    return {
      role,
      subject,
      campaigns: campaigns.variants,
      campaignOrders: campaigns.orders,
      catalog,
      integrations,
      pricing,
      renewalReceipts,
      drills,
      routingConfigured: Boolean(env.STATESET_CARD_ACCOUNTS_JSON),
      stripeConfigured: Boolean(env.STRIPE_SECRET_KEY && env.STRIPE_WEBHOOK_SECRET),
      testMode: /^sk_test_/.test(env.STRIPE_SECRET_KEY || ''),
      launch: localLaunchChecks(await pricingEnvironment(commerce, env)),
      recoveryEnabled: env.STATESET_RENEWAL_RECOVERY === 'true',
      affiliates: affiliates.balances,
      payouts: affiliates.payouts,
      sellers,
      deliveries,
    };
  }
  if (role !== 'affiliate') throw new Error('Unknown account role');
  const { balances, payouts, commissions } = await affiliateBalances(commerce);
  const referrals = (await campaignCatalog(commerce, env)).map(({ id: campaign }) => {
    const url = new URL('/c/' + encodeURIComponent(campaign), env.NEXT_PUBLIC_BASE_URL);
    url.searchParams.set('ref', subject);
    return { campaign, url: url.href };
  });
  return {
    role,
    subject,
    balance: balances.find((row) => row.affiliate === subject) || {
      currency: 'USD',
      earned: '0',
      paid: '0',
      reserved: '0',
      available: '0',
    },
    orders: commissions.filter((row) => row.affiliate === subject),
    referrals,
    payoutAccountConfigured: Boolean(
      await readMerchantRecord(commerce, 'affiliate-connect-' + subject),
    ),
    payouts: payouts.filter((row) => row.affiliate === subject),
  };
}
