import { createHash, createHmac, randomUUID, timingSafeEqual } from 'node:crypto';
import {
  firstMerchantRecord,
  merchantRecords,
  readMerchantRecord,
  saveMerchantRecord,
} from './merchant-records.js';
import { campaignAccounting } from './campaign-accounting.js';
export { campaignAccounting };

export const campaignCookie = 'stateset_campaign';
const slug = /^[a-z0-9][a-z0-9_-]{0,47}$/;
const lifetime = 30 * 86400;
const hash = (value) => createHash('sha256').update(value).digest('hex');

function signingKey(env) {
  if (Buffer.byteLength(env.STATESET_CAMPAIGN_SECRET || '') < 32)
    throw new Error('Configure a campaign signing secret of at least 32 bytes');
  return env.STATESET_CAMPAIGN_SECRET;
}

function validateCampaign(campaign) {
  if (
    !campaign ||
    !Array.isArray(campaign.variants) ||
    !campaign.variants.length ||
    campaign.variants.length > 32 ||
    !campaign.variants.every((v) => v && typeof v === 'object') ||
    new Set(campaign.variants.map((v) => v.id)).size !== campaign.variants.length ||
    !campaign.variants.every(
      (v) =>
        typeof v.id === 'string' &&
        slug.test(v.id) &&
        Number.isSafeInteger(v.weight) &&
        v.weight > 0 &&
        v.weight <= 10000 &&
        typeof v.title === 'string' &&
        v.title.trim() &&
        v.title.length <= 240 &&
        (!v.description || (typeof v.description === 'string' && v.description.length <= 3000)) &&
        typeof v.sku === 'string' &&
        v.sku.trim() &&
        [v.sku, v.bumpSku, v.upsellSku].every(
          (s) => s == null || (typeof s === 'string' && s.trim() && s.length <= 128),
        ) &&
        (v.couponCode === undefined ||
          (typeof v.couponCode === 'string' && /^[\w-]{1,100}$/.test(v.couponCode))),
    )
  ) {
    throw new Error('Each variant requires a unique ID, positive weight, title and catalog SKU');
  }
  return campaign;
}

export async function campaignConfig(commerce, env, id, revision = undefined) {
  if (!slug.test(id)) return null;
  if (Number.isSafeInteger(revision) && revision > 0)
    return readMerchantRecord(commerce, 'campaign-' + id + '-' + revision);
  if (revision === 0) {
    const original = await readMerchantRecord(commerce, 'campaign-' + id + '-0');
    if (original) return original;
  }
  if (revision !== 0) {
    let latest;
    for await (const config of merchantRecords(commerce, 'campaign-revisions-' + id)) {
      if (!latest || config.revision > latest.revision) latest = config;
    }
    if (latest) return latest;
  }
  const campaigns = JSON.parse(env.STATESET_CAMPAIGNS_JSON || '{}');
  if (!Object.hasOwn(campaigns, id)) return null;
  return { ...validateCampaign(campaigns[id]), revision: 0 };
}

export async function campaignCatalog(commerce, env) {
  const ids = new Set(Object.keys(JSON.parse(env.STATESET_CAMPAIGNS_JSON || '{}')));
  for await (const row of merchantRecords(commerce, 'campaign-catalog')) ids.add(row.id);
  const result = [];
  for (const id of [...ids].sort()) {
    const config = await campaignConfig(commerce, env, id);
    if (config) result.push({ id, revision: config.revision, variants: config.variants });
  }
  return result;
}

export async function publishCampaign(commerce, env, input, apply = false) {
  signingKey(env);
  if (
    typeof input?.id !== 'string' ||
    !slug.test(input.id) ||
    !Number.isSafeInteger(input.revision) ||
    input.revision < 0
  )
    throw new Error('Invalid campaign ID or revision');
  const variants = validateCampaign(input).variants.map((v) =>
    Object.fromEntries(
      ['id', 'weight', 'title', 'description', 'sku', 'bumpSku', 'upsellSku', 'couponCode']
        .filter((k) => v[k] !== undefined && v[k] !== '')
        .map((k) => [k, v[k]]),
    ),
  );
  const live = await campaignConfig(commerce, env, input.id);
  const revision = input.revision + 1;
  if (
    (live?.revision ?? 0) !== input.revision &&
    !(live?.revision === revision && JSON.stringify(live.variants) === JSON.stringify(variants))
  ) {
    throw new Error('This campaign changed. Reload its latest revision before publishing');
  }
  const products = [];
  for (const sku of new Set(
    variants.flatMap((v) => [v.sku, v.bumpSku, v.upsellSku]).filter(Boolean),
  )) {
    const product = await catalogOffer(commerce, sku);
    products.push({ sku, name: product.name, price: product.unitPrice });
  }
  for (const code of new Set(variants.map((v) => v.couponCode).filter(Boolean))) {
    if (!(await commerce.promotions.validateCoupon(code)))
      throw new Error('Coupon unavailable: ' + code);
  }
  if (apply) {
    if (live?.revision === 0)
      await firstMerchantRecord(commerce, 'campaign-' + input.id + '-0', {
        revision: 0,
        variants: live.variants,
      });
    await firstMerchantRecord(
      commerce,
      'campaign-list-' + input.id,
      { id: input.id },
      'campaign-catalog',
    );
    await saveMerchantRecord(
      commerce,
      'campaign-' + input.id + '-' + revision,
      { revision, variants },
      'campaign-revisions-' + input.id,
    );
  }
  return { id: input.id, revision, variants, products, mode: apply ? 'published' : 'preview' };
}

export function decodeCampaign(env, token, now = Date.now()) {
  if (!token || token.length > 1600) return null;
  const [encoded, signature, extra] = token.split('.');
  if (extra !== undefined || !/^[\w-]+$/.test(encoded) || !/^[\w-]{43}$/.test(signature || ''))
    return null;
  const expected = createHmac('sha256', signingKey(env)).update(encoded).digest('base64url');
  if (!timingSafeEqual(Buffer.from(signature), Buffer.from(expected))) return null;
  const data = JSON.parse(Buffer.from(encoded, 'base64url').toString());
  return data.expires * 1000 >= now ? data : null;
}

function encodeCampaign(env, data) {
  const encoded = Buffer.from(JSON.stringify(data)).toString('base64url');
  return encoded + '.' + createHmac('sha256', signingKey(env)).update(encoded).digest('base64url');
}

export async function enterCampaign(
  commerce,
  env,
  { campaignId, token, source = '', affiliate = '' },
  now = Date.now(),
) {
  const campaign = await campaignConfig(commerce, env, campaignId);
  if (!campaign) return null;
  const previous = decodeCampaign(env, token, now);
  const visitor = previous?.visitor || randomUUID();
  let selection =
    parseInt(hash(visitor + ':' + campaignId).slice(0, 8), 16) %
    campaign.variants.reduce((sum, v) => sum + v.weight, 0);
  const variant = campaign.variants.find((v) => (selection -= v.weight) < 0);
  const partner =
    slug.test(affiliate) && (await readMerchantRecord(commerce, 'affiliate-' + affiliate));
  const assignment = await firstMerchantRecord(
    commerce,
    'visit-' + hash(campaignId + visitor),
    {
      visitor,
      campaignId,
      revision: campaign.revision,
      variant: variant.id,
      source: String(source).slice(0, 80),
      affiliate: partner ? affiliate : null,
      commissionBps: partner ? partner.commissionBps : 0,
      at: new Date(now).toISOString(),
    },
    'campaign-visits',
  );
  const expires = Math.floor(now / 1000) + lifetime;
  return { token: encodeCampaign(env, { ...assignment, expires }), assignment, maxAge: lifetime };
}

export async function catalogOffer(commerce, sku) {
  if (!sku) return null;
  const variant = await commerce.products.getVariantBySku(sku);
  const product = variant && (await commerce.products.get(variant.productId));
  if (!variant || product?.status !== 'active') throw new Error('Offer is no longer available');
  return {
    productId: product.id,
    variantId: variant.id,
    sku,
    name: product.name,
    quantity: 1,
    unitPrice: variant.priceExact,
  };
}

export async function activeOffer(commerce, env, attribution) {
  if (!attribution) return null;
  const campaign = await campaignConfig(
    commerce,
    env,
    attribution.campaignId,
    attribution.revision ?? 0,
  );
  return campaign?.variants.find((v) => v.id === attribution.variant) || null;
}

export async function recordCampaignStep(commerce, attribution, step, id = '') {
  if (!attribution) return;
  const { campaignId, variant, visitor, affiliate, commissionBps, source } = attribution;
  await firstMerchantRecord(
    commerce,
    'step-' + hash([campaignId, visitor, step, id].join(':')),
    {
      campaignId,
      variant,
      revision: attribution.revision ?? 0,
      visitor,
      affiliate,
      commissionBps,
      source,
      step,
      id,
    },
    'campaign-steps',
  );
}

export async function registerAffiliate(commerce, code, commissionBps, apply) {
  if (
    typeof code !== 'string' ||
    !slug.test(code) ||
    !Number.isSafeInteger(commissionBps) ||
    commissionBps < 0 ||
    commissionBps > 10000
  )
    throw new Error('Invalid affiliate code or commission in basis points');
  const terms = { code, commissionBps };
  if (apply) await saveMerchantRecord(commerce, 'affiliate-' + code, terms, 'affiliates');
  return { ...terms, mode: apply ? 'apply' : 'preview' };
}

// Exports are cumulative, per-order commission amounts so rerunning an export
// can be reconciled by order ID even when refunds arrive after a previous run.
export async function affiliateExport(commerce) {
  const { commissions } = await campaignAccounting(commerce);
  const rows = [
    [
      'affiliate',
      'order_id',
      'currency',
      'original_commission',
      'refund_reversal',
      'earned_commission',
    ],
    ...commissions
      .sort((a, b) => a.orderId.localeCompare(b.orderId))
      .map((row) => [
        row.affiliate,
        row.orderId,
        row.currency,
        row.original,
        row.reversal,
        row.earned,
      ]),
  ];
  return rows
    .map((row) => row.map((cell) => '"' + String(cell).replaceAll('"', '""') + '"').join(','))
    .join('\r\n');
}

export { affiliateBalances, payoutExport, confirmPayoutExport } from './affiliate-payouts.js';
