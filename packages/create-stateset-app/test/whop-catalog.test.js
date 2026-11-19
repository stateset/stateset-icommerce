import assert from 'node:assert/strict';
import test from 'node:test';
import { importWhopCatalog, mapWhopCatalog } from '../templates/storefront/lib/whop-catalog.js';

const account = { id: 'biz_WhopShop' };
const single = (data) => ({
  data,
  page_info: {
    end_cursor: null,
    start_cursor: null,
    has_next_page: false,
    has_previous_page: false,
  },
});
function source() {
  const product = {
    id: 'prod_Course',
    title: 'The course',
    visibility: 'visible',
    account,
    description: null,
    experiences: [{ id: 'exp_private' }],
    metadata: { internal: 'private' },
    member_count: 32,
  };
  const variant = {
    id: 'plan_Plus',
    account,
    product: { id: product.id, title: product.title },
    title: 'Plus',
    visibility: 'visible',
    billing_type: 'one_time',
    release_method: 'buy_now',
    invoice: null,
    currency: 'usd',
    initial_price: 12.1,
    renewal_price: 0,
    initial_price_due: { amount: '12.10', currency: 'usd', decimals: 2, display_decimals: 2 },
    billing_period: null,
    trial_period_days: null,
    expiration_days: null,
    split_pay_required_payments: null,
    custom_fields: [],
    attributes: null,
    adaptive_pricing_enabled: false,
    unlimited_stock: true,
    stock: null,
    member_count: 3,
    internal_notes: 'private@merchant.example',
    purchase_url: 'https://example.com/private',
  };
  return { accountId: account.id, products: single([product]), variants: single([variant]) };
}

test('Whop maps public fixed-price offers to stable draft catalog rows with exact Money', () => {
  const data = source(),
    base = data.variants.data[0];
  for (const [id, fields] of [
    ['Recurring', { billing_type: 'renewal' }],
    ['Hidden', { visibility: 'hidden' }],
    ['Waitlist', { release_method: 'waitlist' }],
    ['Terms', { expiration_days: 5 }],
    ['Installment', { split_pay_required_payments: 3 }],
    ['Trial', { trial_period_days: 7 }],
    ['Price', { currency: 'eur' }],
    ['Adaptive', { adaptive_pricing_enabled: true }],
    ['Fraction', { initial_price_due: { currency: 'usd', amount: '12.001' } }],
    ['Float', { initial_price_due: { currency: 'usd', amount: 12.1 } }],
    ['Form', { custom_fields: [{ prompt: 'Email' }] }],
    ['Attributes', { attributes: { size: 'L' } }],
    ['NoProduct', { product: null }],
    ['Gone', { product: { id: 'prod_Missing' } }],
    ['Invoice', { invoice: { id: 'inv_Test' } }],
    ['HiddenParent', { product: { id: 'prod_Hidden' } }],
  ])
    data.variants.data.push({ ...base, id: 'plan_' + id, ...fields });
  data.products.data.push(
    { ...data.products.data[0], id: 'prod_Hidden', visibility: 'hidden' },
    { ...data.products.data[0], id: 'prod_Empty' },
  );
  const mapped = mapWhopCatalog(data);
  assert.equal(mapped.rows.length, 1);
  assert.equal(mapped.skipped.length, 17);
  const [row] = mapped.rows;
  assert.deepEqual(row, {
    name: 'The course — Plus',
    price: '12.10',
    stock: 0,
    slug: 'whop-' + row.sku.slice(5),
    sku: row.sku,
  });
  assert.match(row.sku, /^WHOP-[0-9a-f]{64}$/);
  assert.deepEqual(mapped.mappings, [
    { productId: 'prod_Course', variantId: 'plan_Plus', sku: row.sku },
  ]);
  assert.doesNotMatch(JSON.stringify(mapped), /private|member_count|internal_notes/);
  const shuffled = { ...data, variants: single([...data.variants.data].reverse()) };
  assert.deepEqual(mapWhopCatalog(shuffled), mapped);
});

test('Whop rejects truncated listings, duplicate IDs and cross-account rows', () => {
  const data = source();
  const first = {
    ...data.variants,
    page_info: { ...data.variants.page_info, has_next_page: true, end_cursor: 'cursor1' },
  };
  assert.throws(() => mapWhopCatalog({ ...data, variants: first }), /first through the final/);
  assert.equal(mapWhopCatalog({ ...data, variants: [first, single([])] }).rows.length, 1);
  assert.throws(() => mapWhopCatalog({ ...data, variants: [first, data.variants] }), /duplicate/);
  assert.throws(() => mapWhopCatalog({ ...data, accountId: 'biz_Other' }), /account mismatch/);
  data.products.data[0].account = { id: 'biz_Other' };
  assert.throws(() => mapWhopCatalog(data), /account mismatch/);
});

function database() {
  const products = new Map(),
    variants = new Map(),
    stocks = new Map();
  return {
    products: {
      getBySlug: async (slug) => products.get(slug),
      getVariantBySku: async (sku) => variants.get(sku),
      create: async (row) => {
        const product = { id: 'local-' + products.size, status: 'draft', ...row };
        products.set(row.slug, product);
        const [variant] = row.variants;
        variants.set(variant.sku, { ...variant, productId: product.id });
      },
      activate: async () => assert.fail('Whop items must remain in draft'),
    },
    inventory: {
      getStock: async (sku) => stocks.get(sku),
      createItem: async (row) => stocks.set(row.sku, { available: row.initialQuantity }),
    },
  };
}

test('Whop applies only reviewed eligible catalog entries and preserves later inventory and activation', async () => {
  const payload = source();
  const db = database();
  const plan = await importWhopCatalog(null, payload);
  const row = plan.rows[0];
  assert.equal(await db.products.getBySlug(row.slug), undefined);
  assert.equal((await importWhopCatalog(db, payload)).fingerprint, plan.fingerprint);
  await assert.rejects(
    importWhopCatalog(db, payload, { apply: true, expectedFingerprint: 'stale' }),
    /preview changed/,
  );
  const done = await importWhopCatalog(db, payload, {
    apply: true,
    expectedFingerprint: plan.fingerprint,
  });
  assert.equal(done.inserted, 1);
  const product = await db.products.getBySlug(row.slug),
    stock = await db.inventory.getStock(row.sku);
  assert.equal(product.status, 'draft');
  assert.equal(stock.available, 0);
  stock.available = 8;
  product.status = 'active';
  const replay = await importWhopCatalog(db, payload, { apply: true });
  assert.equal(replay.existing, 1);
  assert.equal(replay.inserted, 0);
  assert.equal(stock.available, 8);
  assert.equal(product.status, 'active');
  // The new row sorts before the existing row. A conflict on the latter must stop the whole import before writes.
  payload.variants.data.unshift({ ...payload.variants.data[0], id: 'plan_A', title: 'Another' });
  payload.variants.data[1].initial_price_due.amount = '20.10';
  await assert.rejects(
    importWhopCatalog(db, payload, { apply: true }),
    /Existing catalog conflicts/,
  );
  assert.equal(await db.products.getBySlug(mapWhopCatalog(payload).rows[0].slug), undefined);
});
