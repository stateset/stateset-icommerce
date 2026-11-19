import { createHash, randomUUID } from 'node:crypto';
import { isDeepStrictEqual } from 'node:util';
import { firstMerchantRecord, readMerchantRecord, saveMerchantRecord } from './merchant-records.js';
import { decimalToUnits, unitsToDecimal } from './money.js';

const hash = (value) => createHash('sha256').update(JSON.stringify(value)).digest('hex');
const key = (...parts) => 'next-' + hash(parts);
const integer = (value) => Number.isSafeInteger(value) && value >= 0 && value <= 2147483647;
const identity = (value) => integer(value) && value > 0;
const nonempty = (value) => typeof value === 'string' && value === value.trim() && value.length > 0;

function requireField(condition, path, message) {
  if (!condition) throw new Error(path + ': ' + message);
}

function unique(seen, value, path) {
  requireField(!seen.has(value), path, 'duplicate ' + value);
  seen.add(value);
}

function price(value, path) {
  requireField(
    typeof value === 'string' && /^(0|[1-9]\d{0,5})(\.\d{1,2}0*)?$/.test(value),
    path,
    'choose a USD amount up to 999999.99 representable in whole cents',
  );
  const canonical = value.includes('.') ? value.replace(/0+$/, '').replace(/\.$/, '') : value;
  return unitsToDecimal(decimalToUnits(canonical, 2), 2);
}

export function mapNextCatalog(page, sourceStore) {
  requireField(
    typeof sourceStore === 'string' && /^[a-z0-9][a-z0-9-]{0,62}$/.test(sourceStore),
    'Source store',
    'supply a stable lowercase store code (up to 63 letters, digits or hyphens)',
  );
  requireField(
    Array.isArray(page?.results) && page.results.length > 0,
    'File',
    'choose a saved Products List JSON page containing results',
  );
  for (const field of ['next', 'previous']) {
    requireField(
      page[field] === null || nonempty(page[field]),
      field,
      'expected a pagination link or null',
    );
  }
  const products = new Set(),
    variants = new Set(),
    stocks = new Set(),
    skus = new Set(),
    slugs = new Set();
  let location;
  const rows = page.results.map((product, index) => {
    const path = 'results[' + index + ']';
    requireField(
      identity(product?.id) &&
        nonempty(product.title) &&
        typeof product.slug === 'string' &&
        /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(product.slug),
      path,
      'expected a positive product ID, title and URL slug',
    );
    unique(products, product.id, path + '.id');
    unique(slugs, product.slug, path + '.slug');
    requireField(
      product.is_public === true &&
        product.enable_subscription === false &&
        product.is_discountable === true &&
        !product.external_tax_code &&
        !product.is_gift_card,
      path,
      'requires a public, one-time, discountable physical product without a gift card or external tax code',
    );
    requireField(
      ['parent', 'standalone'].includes(product.structure) &&
        Array.isArray(product.variants) &&
        product.variants.length > 0 &&
        (product.structure !== 'standalone' || product.variants.length === 1),
      path,
      'expected a parent product with variants or one standalone SKU',
    );
    const attributes = product.variant_attributes ?? [];
    requireField(
      Array.isArray(attributes) &&
        attributes.every(
          (a) =>
            nonempty(a?.name) &&
            Array.isArray(a.values) &&
            a.values.length &&
            a.values.every(nonempty),
        ),
      path,
      'expected named parent attributes with allowed values',
    );
    const attributeMap = new Map(attributes.map((a) => [a.name, a]));
    requireField(
      attributeMap.size === attributes.length &&
        (attributes.length || product.variants.length === 1),
      path,
      'each option needs a unique name and multi-variant products need declared options',
    );
    const combinations = new Set();
    const mapped = product.variants.map((variant, i) => {
      const at = path + '.variants[' + i + ']';
      requireField(
        identity(variant?.id) &&
          variant.product_id === product.id &&
          typeof variant.sku === 'string' &&
          /^[\w-]{1,100}$/.test(variant.sku) &&
          nonempty(variant.title),
        at,
        'expected variant ID, matching product_id, title and an unchanged compatible SKU',
      );
      unique(variants, variant.id, at + '.id');
      unique(skus, variant.sku, at + '.sku');
      requireField(
        variant.requires_shipping === true &&
          variant.track_stock === true &&
          variant.allow_backorders === false,
        at,
        'requires shipping, tracked stock and disabled backorders',
      );
      requireField(
        Array.isArray(variant.prices) &&
          variant.prices.length === 1 &&
          variant.prices[0]?.currency === 'USD' &&
          variant.prices[0]?.subscription == null &&
          variant.prices[0]?.subscription_suggested_downsell == null,
        at,
        'requires exactly one USD price without subscription pricing',
      );
      const quote = variant.prices[0];
      const amount = price(quote.price, at + '.price');
      const retail = quote.retail == null ? null : price(quote.retail, at + '.retail');
      requireField(
        decimalToUnits(amount, 2) > 0n &&
          (retail === null || decimalToUnits(retail, 2) >= decimalToUnits(amount, 2)),
        at,
        'requires a positive selling price and a retail price at or above it',
      );
      requireField(
        Array.isArray(variant.stockrecords) && variant.stockrecords.length === 1,
        at,
        'requires one stockrecord',
      );
      const stock = variant.stockrecords[0];
      requireField(
        identity(stock?.id) &&
          identity(stock.location_id) &&
          integer(stock.num_in_stock) &&
          stock.num_allocated === 0 &&
          (stock.low_stock_threshold == null || integer(stock.low_stock_threshold)),
        at,
        'requires valid stockrecord and location IDs, nonnegative whole stock and zero units allocated to open orders',
      );
      unique(stocks, stock.id, at + '.stockrecord');
      location ??= stock.location_id;
      requireField(
        stock.location_id === location,
        at,
        'all SKUs must use the same source location',
      );
      const available = stock.num_in_stock > 0;
      requireField(
        available
          ? variant.purchase_availability === 'available' &&
              ['in_stock', 'low_stock'].includes(variant.inventory_availability)
          : variant.purchase_availability === 'unavailable' &&
              variant.inventory_availability === 'out_of_stock',
        at,
        'source availability does not match tracked stock',
      );
      const options = variant.variant_attribute_values ?? [];
      requireField(
        Array.isArray(options) && options.every((o) => nonempty(o?.name) && nonempty(o.value)),
        at,
        'expected named variant option values',
      );
      const selections = new Map(options.map((o) => [o.name, o.value]));
      requireField(
        selections.size === options.length &&
          options.length === attributes.length &&
          options.every(
            (o) =>
              attributeMap.get(o.name)?.values.includes(o.value) &&
              (!o.code ||
                !attributeMap.get(o.name).code ||
                o.code === attributeMap.get(o.name).code),
          ),
        at,
        'variant option names and values must match the parent attributes',
      );
      unique(
        combinations,
        JSON.stringify(attributes.map((a) => selections.get(a.name))),
        at + '.variant_attribute_values',
      );
      return {
        id: variant.id,
        sku: variant.sku,
        title: variant.title,
        options: options.map(({ name, value, code }) => ({ name, value, code: code ?? null })),
        price: amount,
        retail,
        stockRecordId: stock.id,
        stock: stock.num_in_stock,
        threshold: stock.low_stock_threshold ?? null,
      };
    });
    return { id: product.id, name: product.title, slug: product.slug, variants: mapped };
  });
  return { sourceStore, location, rows, morePages: Boolean(page.next) };
}

export function importedOneTimeProduct(commerce, productId) {
  return readMerchantRecord(commerce, key('local-product', productId));
}

function sourceBindings(sourceStore, location, row) {
  const bindings = [
    [key(sourceStore, 'location'), location],
    [key('destination-slug', row.slug), { sourceStore, id: row.id }],
  ];
  for (const v of row.variants) {
    bindings.push(
      [key('destination-sku', v.sku), { sourceStore, id: v.id }],
      [key(sourceStore, 'variant', v.id), { productId: row.id, sku: v.sku }],
      [key(sourceStore, 'stockrecord', v.stockRecordId), { variantId: v.id, location }],
    );
  }
  return bindings;
}

async function inspect(commerce, sourceStore, location, row) {
  const ownershipKey = key(sourceStore, 'product', row.id);
  const claim = await readMerchantRecord(commerce, ownershipKey);
  const fingerprint = hash(row);
  requireField(
    !claim || claim.fingerprint === fingerprint,
    row.slug,
    'source product changed since its initial import',
  );
  const local = await commerce.products.getBySlug(row.slug);
  const receipt = await readMerchantRecord(commerce, key(ownershipKey, 'complete'));
  const bound = local && (await importedOneTimeProduct(commerce, local.id));
  if (receipt)
    requireField(
      local?.id === receipt.productId,
      row.slug,
      'mapped local product is missing or replaced',
    );
  if (local)
    requireField(
      claim &&
        (bound?.ownershipKey === ownershipKey ||
          (local.status === 'draft' && local.description === claim.marker)),
      row.slug,
      'destination product belongs to another catalog',
    );
  for (const [k, value] of sourceBindings(sourceStore, location, row)) {
    const existing = await readMerchantRecord(commerce, k);
    requireField(
      existing === null || isDeepStrictEqual(existing, value),
      row.slug,
      'source location, variant or stockrecord identity conflicts',
    );
  }
  for (const v of row.variants) {
    const variant = await commerce.products.getVariantBySku(v.sku);
    const stock = await commerce.inventory.getStock(v.sku);
    if (local) {
      requireField(
        variant &&
          variant.productId === local.id &&
          (!bound || variant.id === bound.variants[String(v.id)]),
        v.sku,
        'destination variant is missing or replaced',
      );
      if (stock)
        requireField(
          bound && (await readMerchantRecord(commerce, key(ownershipKey, 'inventory', v.id))),
          v.sku,
          'destination already has unrelated inventory',
        );
      if (receipt) requireField(stock, v.sku, 'mapped inventory is missing');
    } else requireField(!stock && !variant, v.sku, 'destination SKU or inventory already exists');
  }
  return { ownershipKey, fingerprint, claim, local, receipt };
}

async function applyRow(commerce, sourceStore, location, row) {
  const state = await inspect(commerce, sourceStore, location, row);
  if (state.receipt) return false;
  const { ownershipKey, fingerprint } = state;
  const claim = await firstMerchantRecord(commerce, ownershipKey, {
    fingerprint,
    marker: 'Next catalog import ' + randomUUID(),
  });
  requireField(claim.fingerprint === fingerprint, row.slug, 'source product changed during import');
  for (const [k, value] of sourceBindings(sourceStore, location, row))
    await saveMerchantRecord(commerce, k, value);
  // A private draft marker binds an interrupted native create to this source. A matching SKU or name alone never proves ownership.
  let local = state.local;
  let inserted = false;
  if (!local) {
    try {
      local = await commerce.products.create({
        name: row.name,
        slug: row.slug,
        description: claim.marker,
        variants: row.variants.map((v, i) => ({
          sku: v.sku,
          name: v.title,
          priceExact: v.price,
          compareAtPriceExact: v.retail ?? undefined,
          isDefault: i === 0,
        })),
      });
      inserted = true;
    } catch (error) {
      local = (await inspect(commerce, sourceStore, location, row)).local;
      if (!local) throw error;
    }
  }
  const localVariants = await commerce.products.getVariants(local.id);
  const ids = Object.fromEntries(
    row.variants.map((v) => {
      const variant = localVariants.find((item) => item.sku === v.sku);
      requireField(variant, v.sku, 'destination variant is missing');
      return [v.id, variant.id];
    }),
  );
  await saveMerchantRecord(commerce, key('local-product', local.id), {
    ownershipKey,
    sourceStore,
    sourceProductId: row.id,
    location,
    variants: ids,
    sourceVariants: row.variants,
  });
  for (const v of row.variants) {
    await saveMerchantRecord(commerce, key(ownershipKey, 'inventory', v.id), {
      variantId: ids[v.id],
    });
    if (!(await commerce.inventory.getStock(v.sku))) {
      try {
        await commerce.inventory.createItem({
          sku: v.sku,
          name: v.title,
          initialQuantity: v.stock,
          reorderPoint: v.threshold ?? undefined,
        });
      } catch (error) {
        if (!(await commerce.inventory.getStock(v.sku))) throw error;
      }
    }
  }
  if (local.description === claim.marker)
    await commerce.products.update(local.id, { description: '' });
  if ((await commerce.products.get(local.id))?.status === 'draft')
    await commerce.products.activate(local.id);
  await saveMerchantRecord(commerce, key(ownershipKey, 'complete'), { productId: local.id });
  return inserted;
}

/**
 * @param {any} commerce
 * @param {any} page
 * @param {{sourceStore?: string, apply?: boolean, expectedFingerprint?: string}} options
 */
export async function importNextCatalog(
  commerce,
  page,
  { sourceStore, apply = false, expectedFingerprint } = {},
) {
  const { rows, location, morePages } = mapNextCatalog(page, sourceStore);
  const fingerprint = hash([sourceStore, location, rows]);
  requireField(
    !expectedFingerprint || expectedFingerprint === fingerprint,
    'Preview',
    'review the current file and source store before applying',
  );
  const items = [];
  for (const row of rows) {
    const state = commerce && (await inspect(commerce, sourceStore, location, row));
    items.push({
      id: row.id,
      slug: row.slug,
      action: state?.receipt ? 'preserve' : 'import',
      variants: row.variants.map(({ id, sku, title, price, retail, stock }) => ({
        id,
        sku,
        title,
        currency: 'USD',
        price,
        retail,
        openingStock: stock,
      })),
    });
  }
  let inserted = 0;
  if (apply) {
    requireField(commerce, 'Destination', 'open a persistent store to apply');
    for (const [i, row] of rows.entries()) {
      if (items[i].action === 'import') {
        if (await applyRow(commerce, sourceStore, location, row)) inserted++;
      }
    }
  }
  return {
    mode: apply ? 'apply' : 'preview',
    sourceStore,
    location,
    fingerprint,
    total: rows.length,
    variants: rows.reduce((count, row) => count + row.variants.length, 0),
    inserted,
    items,
    warning: morePages
      ? 'This export contains one page. Save and import the remaining pages separately.'
      : '',
  };
}
