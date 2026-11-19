import { createHash } from 'node:crypto';
import { importCatalog, validateCatalog } from './merchant-onboarding.js';
import { decimalToUnits } from './money.js';

const digest = (value) => createHash('sha256').update(value).digest('hex');
const ids = { products: /^prod_[A-Za-z0-9]{1,96}$/, variants: /^plan_[A-Za-z0-9]{1,96}$/ };
const warning =
  'New items are drafts with zero stock. Configure digital delivery, inventory and activation before selling. Whop experiences, memberships, billing, stock and sales history stay in Whop.';

function entries(payload, resource) {
  const pages = Array.isArray(payload[resource]) ? payload[resource] : [payload[resource]];
  if (!pages.length) throw new Error('Missing Whop ' + resource + ' pages');
  const rows = new Map();
  for (const [index, page] of pages.entries()) {
    if (
      !Array.isArray(page?.data) ||
      !page.page_info ||
      typeof page.page_info.has_previous_page !== 'boolean' ||
      (index === 0 && page.page_info.has_previous_page) ||
      page.page_info.has_next_page !== index < pages.length - 1 ||
      (page.page_info.has_next_page && !page.page_info.end_cursor)
    ) {
      throw new Error(
        'Supply forward Whop ' + resource + ' pages from the first through the final page',
      );
    }
    for (const entry of page.data) {
      if (typeof entry?.id !== 'string' || !ids[resource].test(entry.id) || rows.has(entry.id))
        throw new Error('Invalid or duplicate ID in Whop ' + resource);
      if (entry.account !== null && entry.account?.id !== payload.accountId)
        throw new Error('Whop account mismatch in ' + resource + ' for ' + entry.id);
      rows.set(entry.id, entry);
    }
  }
  return rows;
}

function exclusion(variant, product) {
  if (!variant.account || !variant.product || variant.invoice !== null)
    return 'standalone or invoice variant';
  if (!product || !product.account) return 'missing account product';
  if (product.visibility !== 'visible' || variant.visibility !== 'visible')
    return 'non-public product or variant';
  if (
    variant.billing_type !== 'one_time' ||
    variant.billing_period !== null ||
    variant.trial_period_days !== null ||
    variant.expiration_days !== null ||
    variant.split_pay_required_payments !== null
  )
    return 'recurring, time-limited or installment offer';
  if (variant.release_method !== 'buy_now') return 'unavailable for immediate purchase';
  if (
    variant.adaptive_pricing_enabled !== false ||
    variant.currency !== 'usd' ||
    variant.initial_price_due?.currency !== 'usd'
  )
    return 'variable pricing or non-USD currency';
  if (!Array.isArray(variant.custom_fields) || variant.custom_fields.length)
    return 'custom checkout fields';
  if (
    variant.attributes !== null &&
    (!variant.attributes ||
      typeof variant.attributes !== 'object' ||
      Object.keys(variant.attributes).length)
  )
    return 'variant attributes';
  if (
    typeof product.title !== 'string' ||
    !product.title.trim() ||
    (variant.title !== null && typeof variant.title !== 'string')
  )
    return 'missing public title';
  const amount = variant.initial_price_due.amount;
  if (
    typeof amount !== 'string' ||
    !/^(0|[1-9]\d{0,13})(\.\d{1,2})?$/.test(amount) ||
    decimalToUnits(amount, 2) === 0n ||
    decimalToUnits(amount, 2) > BigInt(Number.MAX_SAFE_INTEGER)
  )
    return 'unsupported exact USD amount';
  return null;
}

export function mapWhopCatalog(payload) {
  if (typeof payload?.accountId !== 'string' || !/^biz_[A-Za-z0-9]{1,96}$/.test(payload.accountId))
    throw new Error(
      'Supply the Whop accountId (biz_...) with its product and variant list responses',
    );
  const products = entries(payload, 'products'),
    variants = entries(payload, 'variants');
  const rows = [],
    skipped = [],
    mappings = [],
    referenced = new Set();
  for (const id of [...variants.keys()].sort()) {
    const variant = variants.get(id),
      product = products.get(variant.product?.id);
    if (product) referenced.add(product.id);
    const reason = exclusion(variant, product);
    if (reason) {
      skipped.push({ id, reason });
      continue;
    }
    // Whop IDs are case-sensitive. Only this stable, namespaced digest enters StateSet identifiers.
    const identity = digest(JSON.stringify([payload.accountId, product.id, id]));
    const sku = 'WHOP-' + identity;
    const title = product.title.trim(),
      option = variant.title?.trim();
    rows.push({
      sku,
      slug: 'whop-' + identity,
      name: option && option !== title ? title + ' — ' + option : title,
      price: variant.initial_price_due.amount,
      stock: 0,
    });
    mappings.push({ productId: product.id, variantId: id, sku });
  }
  for (const id of [...products.keys()].sort()) {
    if (!referenced.has(id))
      skipped.push({ id, reason: 'no variants in supplied account listing' });
  }
  if (rows.length) validateCatalog(rows);
  return { accountId: payload.accountId, rows, mappings, skipped, warning };
}

// Imports reserve a stable 256-bit namespace for each account/product/variant tuple.
// Matching the parent slug and the variant SKU also recognizes previously imported drafts.
export function isWhopCatalogVariant(product, variant) {
  return Boolean(
    product &&
    variant &&
    variant.productId === product.id &&
    /^WHOP-[a-f0-9]{64}$/.test(variant.sku) &&
    product.slug === variant.sku.toLowerCase(),
  );
}

/**
 * @param {any} commerce
 * @param {any} payload
 * @param {{apply?: boolean, expectedFingerprint?: string}} options
 */
export async function importWhopCatalog(
  commerce,
  payload,
  { apply = false, expectedFingerprint } = {},
) {
  if (apply && !commerce) throw new Error('A store is required to apply a Whop catalog');
  const mapped = mapWhopCatalog(payload);
  const existingSkus = [];
  for (const row of mapped.rows) {
    if (!commerce) break;
    const [product, variant, stock] = await Promise.all([
      commerce.products.getBySlug(row.slug),
      commerce.products.getVariantBySku(row.sku),
      commerce.inventory.getStock(row.sku),
    ]);
    if (!product && !variant && !stock) continue;
    if (
      !product ||
      !variant ||
      variant.productId !== product.id ||
      product.name !== row.name ||
      decimalToUnits(variant.priceExact, 2) !== decimalToUnits(row.price, 2)
    )
      throw new Error('Existing catalog conflicts with ' + row.sku);
    existingSkus.push(row.sku);
  }
  const fingerprint = digest(
    JSON.stringify([mapped.accountId, mapped.rows, mapped.skipped, existingSkus]),
  );
  if (expectedFingerprint !== undefined && expectedFingerprint !== fingerprint)
    throw new Error('Whop import preview changed; review a fresh preview');
  const result =
    apply && mapped.rows.length
      ? await importCatalog(commerce, mapped.rows, { activate: false })
      : { inserted: 0 };
  return {
    ...mapped,
    mode: apply ? 'apply' : 'preview',
    total: mapped.rows.length,
    inserted: result.inserted,
    existing: existingSkus.length,
    wouldInsert: mapped.rows.length - existingSkus.length,
    fingerprint,
  };
}
