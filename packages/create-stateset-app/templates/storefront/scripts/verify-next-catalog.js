// Run the generated storefront's public onboarding CLI and exercise real native storage.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, rm, writeFile, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Commerce } from '@stateset/embedded';
import { importNextCatalog, importedOneTimeProduct } from '../lib/next-catalog.js';

function product(id) {
  return {
    id,
    structure: 'parent',
    slug: 'mug-' + id,
    title: 'Workshop mug ' + id,
    is_public: true,
    is_discountable: true,
    enable_subscription: false,
    variant_attributes: [{ name: 'Color', code: 'color', values: ['Red', 'Blue'] }],
    variants: ['Red', 'Blue'].map((color, i) => ({
      id: id * 10 + i,
      product_id: id,
      title: color,
      sku: 'MUG-' + id + '-' + color,
      variant_attribute_values: [{ name: 'Color', code: 'color', value: color }],
      requires_shipping: true,
      track_stock: true,
      allow_backorders: false,
      purchase_availability: 'available',
      inventory_availability: 'in_stock',
      prices: [
        { currency: 'USD', price: i ? '24.9100' : '19.99', retail: '30.00', subscription: null },
      ],
      stockrecords: [
        {
          id: id * 100 + i,
          location_id: 7,
          num_in_stock: 3,
          num_allocated: 0,
          low_stock_threshold: 1,
        },
      ],
    })),
  };
}

const page = (rows) => ({ next: null, previous: null, results: rows });
const sourceStore = 'demo-store',
  exportPage = page([product(10)]);
const directory = await mkdtemp(join(tmpdir(), 'stateset-next-catalog-'));
const filename = join(directory, 'catalog.json'),
  db = join(directory, 'catalog.db');
let commerce;
try {
  const cli = (...args) =>
    spawnSync(
      process.execPath,
      [
        fileURLToPath(new URL('./onboard.js', import.meta.url)),
        '--next-catalog',
        filename,
        '--source-store',
        sourceStore,
        ...args,
      ],
      { env: { ...process.env, STATESET_DB_PATH: db }, encoding: 'utf8' },
    );
  await writeFile(filename, JSON.stringify(exportPage));
  const preview = cli();
  assert.equal(preview.status, 0, preview.stderr);
  const { fingerprint, items } = JSON.parse(preview.stdout);
  assert.equal(items[0].variants[1].price, '24.91');
  assert.equal(cli('--apply').status, 1);
  assert.equal(cli('--apply', '--reviewed', 'bad').status, 1);
  await assert.rejects(access(db), { code: 'ENOENT' });

  const applied = cli('--apply', '--reviewed', fingerprint);
  assert.equal(applied.status, 0, applied.stderr);
  assert.equal(JSON.parse(applied.stdout).inserted, 1);
  commerce = new Commerce(db);
  const mug = await commerce.products.getBySlug('mug-10');
  const blue = await commerce.products.getVariantBySku('MUG-10-Blue');
  const mapping = await importedOneTimeProduct(commerce, mug.id);
  assert.equal(mug.status, 'active');
  assert.equal(mug.description, '');
  assert.equal(blue.priceExact, '24.91');
  assert.equal(blue.compareAtPriceExact, '30');
  assert.equal(mapping.variants[101], blue.id);
  assert.equal(mapping.sourceVariants[1].stockRecordId, 1001);
  assert.equal(mapping.sourceVariants[1].options[0].value, 'Blue');
  assert.equal((await commerce.inventory.getStock(blue.sku)).totalAvailable, '3');
  await commerce.inventory.adjust(blue.sku, -2, 'Fixture sale');
  const replay = await importNextCatalog(commerce, exportPage, { sourceStore, apply: true });
  assert.equal(replay.inserted, 0);
  assert.equal((await commerce.inventory.getStock(blue.sku)).totalAvailable, '1');

  for (const corrupt of [
    (r) => {
      r.variants[1].variant_attribute_values[0].value = 'Green';
    },
    (r) => {
      r.variants[1].variant_attribute_values[0].value = 'Red';
    },
    (r) => {
      r.variants[1].prices[0].currency = 'CAD';
    },
    (r) => {
      r.variants[1].prices[0].price = '13.123';
    },
    (r) => {
      r.variants[1].stockrecords[0].location_id++;
    },
    (r) => {
      r.variants[1].stockrecords[0].num_allocated++;
    },
    (r) => {
      r.variants[1].sku = r.variants[0].sku;
    },
    (r) => {
      r.variants[1].product_id++;
    },
    (r) => {
      r.variants[1].id = r.variants[0].id;
    },
    (r) => {
      r.variants[1].purchase_availability = 'unavailable';
    },
    (r) => {
      r.variants[1].prices[0].retail = '0';
    },
    (r) => {
      r.variants[1].allow_backorders = true;
    },
    (r) => {
      r.is_public = false;
    },
    (r) => {
      r.enable_subscription = true;
    },
    (r) => {
      r.variants[0].sku = blue.sku;
    },
    (r) => {
      r.variants[0].id = 101;
    },
  ]) {
    const row = product(30);
    corrupt(row);
    await assert.rejects(
      importNextCatalog(commerce, page([product(20), row]), { sourceStore, apply: true }),
    );
    assert.equal(await commerce.products.getBySlug('mug-20'), null);
    assert.equal(await commerce.products.count(), 1);
  }
  const changed = product(10);
  changed.variants[0].prices[0].price = '18.12';
  await assert.rejects(
    importNextCatalog(commerce, page([changed]), { sourceStore, apply: true }),
    /changed/,
  );
  await assert.rejects(
    importNextCatalog(commerce, exportPage, { sourceStore: 'another-store', apply: true }),
    /another catalog/,
  );
  const fresh = product(40);
  const failingInventory = {
    getStock: (...args) => commerce.inventory.getStock(...args),
    createItem: async (input) => {
      if (input.sku.endsWith('Blue')) throw Error('fixture interruption');
      return commerce.inventory.createItem(input);
    },
  };
  const interrupted = {
    products: commerce.products,
    customObjects: commerce.customObjects,
    inventory: failingInventory,
  };
  await assert.rejects(
    importNextCatalog(interrupted, page([fresh]), { sourceStore, apply: true }),
    /fixture interruption/,
  );
  const draft = await commerce.products.getBySlug(fresh.slug);
  assert.equal(draft.status, 'draft');
  await importNextCatalog(commerce, page([fresh]), { sourceStore, apply: true });
  assert.equal((await commerce.products.getBySlug(fresh.slug)).id, draft.id);
  assert.equal((await commerce.inventory.getStock(fresh.variants[0].sku)).totalAvailable, '3');
  await commerce.products.archive(mug.id);
  await importNextCatalog(commerce, exportPage, { sourceStore, apply: true });
  assert.equal((await commerce.products.get(mug.id)).status, 'archived');
  console.log(
    'Next Commerce import verified against native SQLite: preview, exact prices, variants, stock, source ownership, conflicts, interruption and replay.',
  );
} finally {
  if (commerce) await commerce.close();
  await rm(directory, { recursive: true, force: true });
}
