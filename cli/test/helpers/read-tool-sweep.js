/**
 * Machinery for test/integration/mcp-read-tools-do-not-mutate.test.js: call
 * every `permission: 'read'` MCP tool against a seeded store and report any
 * call after which persisted state differs.
 *
 * Inputs come from the MCP smoke sampler (scripts/mcp-tool-smoke.mjs), which
 * builds the smallest input each tool's own zod schema accepts. Each tool
 * gets that sample plus whichever of these differ from it:
 *
 *   required -- the smoke sampler's output, verbatim
 *   seeded   -- the same, with id-like string fields (customerId, linkId,
 *               identifier, sku, email, ...) pointed at rows that exist in
 *               the seeded store, so lookups HIT instead of missing
 *   flipped  -- seeded (or required) with every boolean flipped from the
 *               sampler's choice, so a `dryRun` default of true becomes false
 *   full     -- every field, optional ones included, seeded and flipped
 *   fresh    -- seeded (or required) with the sampler's constant 'smoke' text
 *               made unique, so a get-or-create keyed on it shows every time
 *   fixture  -- hand-written inputs the sampler cannot produce (FIXTURE_INPUTS)
 *
 * State is fingerprinted by ./state-probe.js, out of process.
 */

import { mkdirSync, mkdtempSync, realpathSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import Database from 'better-sqlite3';

// ---------------------------------------------------------------------------
// Sandbox: every directory a tool could write to, under one temp root
// ---------------------------------------------------------------------------

/**
 * Point HOME, the temp dir and the working directory at a fresh temp tree,
 * so ~/.stateset, ./.stateset (relative config dirs such as the sync key
 * store) and os.tmpdir() all land inside one fingerprinted root.
 *
 * Call it BEFORE importing anything from src/: some modules resolve
 * `os.homedir()` or `path.resolve('.stateset')` once, at import time.
 */
export function createSandbox(prefix = 'mcp-read-sweep-') {
  const root = realpathSync(mkdtempSync(path.join(os.tmpdir(), prefix)));
  const dirs = {
    root,
    db: path.join(root, 'db'),
    home: path.join(root, 'home'),
    cwd: path.join(root, 'cwd'),
    tmp: path.join(root, 'tmp'),
  };
  for (const dir of [dirs.db, dirs.home, dirs.cwd, dirs.tmp]) mkdirSync(dir);
  const saved = {
    cwd: process.cwd(),
    env: Object.fromEntries(
      ['HOME', 'USERPROFILE', 'TMPDIR', 'TMP', 'TEMP'].map((key) => [key, process.env[key]]),
    ),
  };
  process.env.HOME = dirs.home;
  process.env.USERPROFILE = dirs.home;
  process.env.TMPDIR = dirs.tmp;
  process.env.TMP = dirs.tmp;
  process.env.TEMP = dirs.tmp;
  process.chdir(dirs.cwd);
  return {
    ...dirs,
    dbPath: path.join(dirs.db, 'store.db'),
    restore({ keep = false } = {}) {
      process.chdir(saved.cwd);
      for (const [key, value] of Object.entries(saved.env)) {
        if (value === undefined) delete process.env[key];
        else process.env[key] = value;
      }
      if (!keep) rmSync(root, { recursive: true, force: true });
    },
  };
}

// ---------------------------------------------------------------------------
// Presenting schemas to the smoke sampler
// ---------------------------------------------------------------------------

/**
 * The smoke sampler reads zod 3 internals (`_def.typeName`). Under zod 4
 * every schema falls through to its default branch and samples as the string
 * 'smoke', which makes a sweep vacuous: almost every input fails validation
 * before the handler runs. When the installed zod is one the sampler cannot
 * read, present each schema to it in the zod 3 shape it expects. When the
 * sampler reads the schemas natively (zod 3, as master's lockfile pins) this
 * is the identity.
 *
 * @param {(schema: unknown, key?: string) => unknown} sample - the smoke sampler
 * @param {object} z - the zod the tool modules import
 * @returns {{ native: boolean, view: (schema: any) => any }}
 */
export function createSamplerView(sample, z) {
  if (sample(z.number()) === 1) return { native: true, view: (schema) => schema };
  return { native: false, view: zod4AsZod3 };
}

function zod4AsZod3(schema) {
  const def = schema?._zod?.def;
  if (!def) return schema;
  const checks = (def.checks ?? []).map((check) => check?._zod?.def ?? {});
  const minLength = () =>
    checks.find((check) => check.check === 'min_length')?.minimum ??
    checks.find((check) => check.check === 'length_equals')?.length;
  const node = (typeName, extra = {}) => ({
    _def: { typeName, ...extra },
    isOptional: () => {
      try {
        return schema.isOptional?.() === true;
      } catch {
        return false;
      }
    },
  });
  switch (def.type) {
    case 'optional':
      return node('ZodOptional', { innerType: zod4AsZod3(def.innerType) });
    case 'nullable':
      return node('ZodNullable', { innerType: zod4AsZod3(def.innerType) });
    case 'default':
    case 'prefault':
      return node('ZodDefault', {
        defaultValue: () => def.defaultValue,
        innerType: zod4AsZod3(def.innerType),
      });
    case 'nonoptional':
    case 'readonly':
    case 'catch':
    case 'success':
      return node('ZodReadonly', { innerType: zod4AsZod3(def.innerType) });
    case 'pipe':
      // z.preprocess(fn, schema) is pipe(transform, schema): sample the target.
      return node('ZodPipeline', {
        in: zod4AsZod3(def.in?._zod?.def?.type === 'transform' ? def.out : def.in),
      });
    case 'lazy':
      return node('ZodLazy', { getter: () => zod4AsZod3(def.getter()) });
    case 'string': {
      const out = [];
      const min = schema.minLength ?? minLength();
      if (typeof min === 'number') out.push({ kind: 'min', value: min });
      const format = schema.format ?? def.format;
      if (format) out.push({ kind: format === 'guid' ? 'uuid' : format });
      return node('ZodString', { checks: out });
    }
    case 'number': {
      const out = [];
      for (const check of checks) {
        if (check.check === 'greater_than')
          out.push({ kind: 'min', value: Number(check.value), inclusive: check.inclusive });
        if (check.check === 'less_than')
          out.push({ kind: 'max', value: Number(check.value), inclusive: check.inclusive });
      }
      return node('ZodNumber', { checks: out });
    }
    case 'bigint':
      return node('ZodBigInt');
    case 'boolean':
      return node('ZodBoolean');
    case 'date':
      return node('ZodDate');
    case 'enum':
      return node('ZodEnum', { values: Object.values(def.entries) });
    case 'literal':
      return node('ZodLiteral', { value: def.values[0] });
    case 'array': {
      const min = minLength();
      return node('ZodArray', {
        type: zod4AsZod3(def.element),
        minLength: typeof min === 'number' ? { value: min } : null,
      });
    }
    case 'tuple':
      return node('ZodTuple', { items: def.items.map(zod4AsZod3) });
    case 'object':
      return node('ZodObject', {
        shape: () =>
          Object.fromEntries(
            Object.entries(def.shape).map(([key, value]) => [key, zod4AsZod3(value)]),
          ),
      });
    case 'record':
    case 'map':
      return node('ZodRecord');
    case 'union':
      return node('ZodUnion', { options: def.options.map(zod4AsZod3) });
    case 'intersection':
      return node('ZodIntersection', {
        left: zod4AsZod3(def.left),
        right: zod4AsZod3(def.right),
      });
    default:
      return node('ZodAny');
  }
}

const WRAPPERS = new Set([
  'ZodOptional',
  'ZodNullable',
  'ZodDefault',
  'ZodReadonly',
  'ZodCatch',
  'ZodEffects',
  'ZodPipeline',
  'ZodBranded',
  'ZodLazy',
]);

/** The schema under optional/default/effects wrappers (zod 3 shape). */
function baseOf(schema) {
  let current = schema;
  for (let depth = 0; depth < 25; depth += 1) {
    const def = current?._def;
    if (!def || !WRAPPERS.has(def.typeName)) return current;
    if (def.typeName === 'ZodLazy') current = def.getter();
    else if (def.typeName === 'ZodEffects') current = def.schema;
    else if (def.typeName === 'ZodPipeline') current = def.in;
    else if (def.typeName === 'ZodBranded') current = def.type;
    else current = def.innerType;
  }
  return current;
}

const baseType = (schema) => baseOf(schema)?._def?.typeName;

// ---------------------------------------------------------------------------
// Real ids from the seeded store
// ---------------------------------------------------------------------------

const quoteIdent = (name) => `"${String(name).replaceAll('"', '""')}"`;

/**
 * Up to two ids per table, plus a few natural keys, read from the store.
 * Never call this in the process that serves the store (see state-probe.js).
 *
 * @param {string} dbPath
 * @param {object} [options]
 * @param {import('better-sqlite3').Database} [options.db] - an open connection to reuse
 */
export function readSeedPool(dbPath, { db: open = null } = {}) {
  const db = open ?? new Database(dbPath, { readonly: true, fileMustExist: true });
  try {
    const tables = db
      .prepare("SELECT name, sql FROM sqlite_master WHERE type = 'table'")
      .all()
      .filter((table) => !/^\s*CREATE\s+VIRTUAL/i.test(table.sql ?? ''))
      .map((table) => table.name);
    const idsByTable = new Map();
    for (const table of tables) {
      const columns = db.prepare(`PRAGMA table_info(${quoteIdent(table)})`).all();
      const pk = columns.filter((column) => column.pk > 0);
      const idColumn =
        columns.find((column) => column.name === 'id') ?? (pk.length === 1 ? pk[0] : null);
      if (!idColumn) continue;
      const column = quoteIdent(idColumn.name);
      const select = `SELECT ${column} AS v FROM ${quoteIdent(table)} WHERE ${column} IS NOT NULL`;
      let rows;
      try {
        rows = db.prepare(`${select} ORDER BY rowid LIMIT 2`).all();
      } catch {
        rows = db.prepare(`${select} LIMIT 2`).all();
      }
      if (rows.length > 0)
        idsByTable.set(
          table,
          rows.map((row) => String(row.v)),
        );
    }
    const first = (table, column) => {
      if (!tables.includes(table)) return null;
      try {
        return (
          db
            .prepare(
              `SELECT ${quoteIdent(column)} AS v FROM ${quoteIdent(table)} ` +
                `WHERE ${quoteIdent(column)} IS NOT NULL AND ${quoteIdent(column)} <> '' LIMIT 1`,
            )
            .get()?.v ?? null
        );
      } catch {
        return null;
      }
    };
    const sku = first('inventory_items', 'sku') ?? first('product_variants', 'sku');
    let skuPrice = null;
    try {
      skuPrice =
        db.prepare('SELECT price FROM product_variants WHERE sku = ?').get(sku)?.price ?? null;
    } catch {
      // No catalog yet.
    }
    return {
      idsByTable,
      named: {
        sku,
        skuPrice: skuPrice === null ? null : String(skuPrice),
        email: first('customers', 'email'),
        couponCode: first('coupon_codes', 'code') ?? first('promotions', 'code'),
        shortCode: first('payment_links', 'short_code'),
        trackingNumber: first('shipments', 'tracking_number'),
        orderNumber: first('orders', 'order_number'),
      },
    };
  } finally {
    if (!open) db.close();
  }
}

/** Leading tokens that name what a tool does rather than what it reads. */
const VERBS = new Set(
  (
    'get list resolve track check validate preview calculate explain describe find lookup ' +
    'search show view export verify inspect compute estimate summarize analyze count read ' +
    'fetch query generate simulate compare evaluate score diagnose audit trace quote discover ' +
    'recommend forecast predict match reconcile test detect build render format convert plan ' +
    'assess review monitor'
  ).split(' '),
);

const snake = (key) =>
  key
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/([A-Z])([A-Z][a-z])/g, '$1_$2')
    .toLowerCase();

const pluralsOf = (entity) => [`${entity}s`, `${entity}es`, entity.replace(/y$/, 'ies'), entity];

/** Entities a tool most likely reads, most specific first (get_product_variant -> product_variant, variant, product). */
function entitiesFromToolName(toolName) {
  const tokens = toolName.split('_');
  const verbAt = tokens.findIndex((token) => VERBS.has(token));
  const rest = verbAt >= 0 ? tokens.slice(verbAt + 1) : tokens;
  const out = [];
  for (let start = 0; start < rest.length; start += 1) out.push(rest.slice(start).join('_'));
  for (let end = rest.length - 1; end > 0; end -= 1) out.push(rest.slice(0, end).join('_'));
  return out.filter(Boolean);
}

/**
 * A real value for a field, or null to keep the sampler's.
 *
 * @param {ReturnType<typeof readSeedPool>} pool
 * @param {string} key - field name
 * @param {string} toolName
 */
export function seededValueFor(pool, key, toolName) {
  const k = snake(key);
  const { named, idsByTable } = pool;
  if (/(?:^|_)skus?$/.test(k)) return named.sku;
  if (/(?:^|_)emails?$/.test(k)) return named.email;
  if (/short_code$/.test(k)) return named.shortCode;
  if (k === 'code' || /(?:^|_)(?:coupon|promo|promotion|discount)_code$/.test(k))
    return named.couponCode;
  if (/tracking_number$/.test(k)) return named.trackingNumber;
  if (/order_number$/.test(k)) return named.orderNumber;
  const idMatch = /^(?:(.*)_)?ids?$/.exec(k);
  if (!idMatch && k !== 'identifier') return null;
  const entity = idMatch?.[1];
  // source_warehouse_id -> source_warehouse, then warehouse
  const candidates = entity
    ? entity.split('_').map((_, start, tokens) => tokens.slice(start).join('_'))
    : entitiesFromToolName(toolName);
  const tables = [...idsByTable.keys()];
  for (const candidate of candidates) {
    for (const table of pluralsOf(candidate)) {
      if (idsByTable.has(table)) return idsByTable.get(table)[0];
    }
    for (const plural of pluralsOf(candidate)) {
      const table = tables.find((name) => name.endsWith(`_${plural}`));
      if (table) return idsByTable.get(table)[0];
    }
  }
  return null;
}

/**
 * Input variants for one tool (see the module comment).
 *
 * @param {object} deps
 * @param {Function} deps.sample - smoke sampler `sample(schema, key)`
 * @param {Function} deps.sampleShape - smoke sampler `sampleShape(shape)`
 * @param {(schema: any) => any} deps.view - from createSamplerView
 * @param {() => ReturnType<typeof readSeedPool>} deps.pool
 * @param {Record<string, object[]>} [deps.fixtures] - extra inputs per tool, appended
 */
export function createVariantBuilder({ sample, sampleShape, view, pool, fixtures = {} }) {
  // The sampler fills free text with the constant 'smoke'. A get-or-create
  // keyed on such a value would show only on the first tool to use it: every
  // later tool finds the row already there. `fresh` swaps each 'smoke' for a
  // token unique to this sweep, of the same length, so length limits that
  // 'smoke' met still hold.
  let counter = 0;
  const freshen = (value) => {
    if (typeof value === 'string' && value.startsWith('smoke')) {
      counter += 1;
      return `sm${counter.toString(36).padStart(3, '0')}${value.slice(5)}`;
    }
    if (Array.isArray(value)) return value.map(freshen);
    if (value && typeof value === 'object')
      return Object.fromEntries(Object.entries(value).map(([key, inner]) => [key, freshen(inner)]));
    return value;
  };

  return (tool) => {
    const variants = sampledVariants(tool);
    const base = (variants.find((v) => v.label === 'seeded') ?? variants[0]).params;
    const fresh = freshen(base);
    if (JSON.stringify(fresh) !== JSON.stringify(base))
      variants.push({ label: 'fresh', params: fresh });
    for (const params of fixtures[tool.name] ?? []) variants.push({ label: 'fixture', params });
    return variants;
  };

  function sampledVariants(tool) {
    const shape = Object.fromEntries(
      Object.entries(tool.inputSchema ?? {}).map(([key, schema]) => [key, view(schema)]),
    );
    const keys = Object.keys(shape);
    const required = sampleShape(shape);
    const variants = [{ label: 'required', params: required }];
    if (keys.length === 0) return variants;

    const seed = (params) => {
      const out = { ...params };
      let changed = false;
      for (const key of Object.keys(out)) {
        const type = baseType(shape[key]);
        const elementType = type === 'ZodArray' ? baseType(baseOf(shape[key])._def.type) : null;
        if (type !== 'ZodString' && elementType !== 'ZodString') continue;
        const value = seededValueFor(pool(), key, tool.name);
        if (value === null || value === undefined) continue;
        out[key] = type === 'ZodArray' ? [String(value)] : String(value);
        changed = true;
      }
      return changed ? out : null;
    };

    const seeded = seed(required);
    if (seeded) variants.push({ label: 'seeded', params: seeded });

    // Booleans flipped from the sampler's choice (false, or the default):
    // `dryRun: true` becomes false, `includeArchived: false` becomes true.
    const booleanKeys = keys.filter((key) => baseType(shape[key]) === 'ZodBoolean');
    const flip = (params) => {
      for (const key of booleanKeys) params[key] = !sample(shape[key], key);
      return params;
    };
    if (booleanKeys.length > 0) {
      variants.push({ label: 'flipped', params: flip({ ...(seeded ?? required) }) });
    }

    if (keys.some((key) => !(key in required))) {
      const full = {};
      for (const key of keys) full[key] = sample(shape[key], key);
      variants.push({ label: 'full', params: flip(seed(full) ?? full) });
    }
    return variants;
  }
}

// ---------------------------------------------------------------------------
// Seeding
// ---------------------------------------------------------------------------

const seedItem = (pool, extra = {}) => ({
  sku: pool.named.sku ?? 'SEED-SKU',
  name: 'Seed item',
  quantity: 1,
  unitPrice: 25,
  ...extra,
});

/**
 * Write tools called in apply mode after the demo data, so read tools in more
 * domains have rows to read. Each runs with its seeded minimal sample (real
 * ids where the schema names one) plus the overrides here, in two passes so a
 * record created late in the first pass can anchor one earlier in the
 * second. A seed that refuses is recorded and skipped: seeding is best effort,
 * the sweep only needs rows to exist.
 *
 * @type {Array<[string, (pool: ReturnType<typeof readSeedPool>) => object]>}
 */
const none = () => ({});
const firstId = (pool, table, fallback) => pool.idsByTable.get(table)?.[0] ?? fallback;

/** The agent whose sync keys the seed generates (under ./.stateset/keys). */
export const SEED_AGENT_ID = 'seed-agent';

const fixtureScenario = {
  name: 'read-sweep-scenario',
  inventory: [{ sku: 'SIM', unitPrice: '0.10', quantity: 3 }],
  steps: [
    { type: 'place_order', id: 'order', items: [{ sku: 'SIM', quantity: 2 }] },
    { type: 'capture_payment', id: 'payment', order: 'order' },
    { type: 'refund_payment', id: 'refund', payment: 'payment', amount: '0.10' },
  ],
};
const fixturePlan = {
  name: 'read-sweep-plan',
  days: 2,
  seed: 42,
  startingCash: '20',
  products: [
    {
      sku: 'PLAN',
      initialStock: 2,
      unitPrice: '2.50',
      unitCost: '1.25',
      demand: { min: 2, max: 2 },
      reorderPoint: 2,
      targetStock: 6,
      leadTimeDays: 1,
    },
  ],
};
const { seed: _fixtureSeed, ...fixturePolicy } = fixturePlan;
const fixtureEvent = {
  id: 'evt-1',
  payload: { kind: 'read-sweep' },
  eventSigningHash: `0x${'ab'.repeat(32)}`,
  timestamp: '2026-01-15T00:00:00.000Z',
};

/**
 * Inputs the sampler cannot produce, added to (never instead of) a tool's
 * sampled variants: nested, refined simulation inputs (shapes from
 * test/unit/embedded-simulation.test.js), JSON-in-a-string arguments, a hit
 * on the seeded agent key, and a dry run of a real mutation.
 */
export const FIXTURE_INPUTS = {
  agent_key_export: [{ agentId: SEED_AGENT_ID }, { agentId: SEED_AGENT_ID, keyType: 'encryption' }],
  agentic_simulate_mutation: [
    {
      tool: 'create_payment_link',
      params: { items: [{ name: 'Fixture item', quantity: 1, unitPrice: 5 }] },
    },
  ],
  generate_receipt_bundle: [
    { event: JSON.stringify(fixtureEvent), batchEvents: JSON.stringify([fixtureEvent]) },
  ],
  simulate_commerce_scenario: [{ scenario: fixtureScenario }],
  compare_commerce_scenarios: [
    { scenarios: [fixtureScenario, { ...fixtureScenario, name: 'read-sweep-scenario-b' }] },
  ],
  simulate_inventory_plan: [{ plan: fixturePlan }],
  compare_inventory_plans: [
    {
      plans: [
        { ...fixturePlan, name: 'read-sweep-plan-a', days: 1 },
        { ...fixturePlan, name: 'read-sweep-plan-b', days: 1, startingCash: '21.00' },
      ],
    },
  ],
  simulate_inventory_experiment: [
    {
      experiment: {
        name: 'read-sweep-experiment',
        seeds: [7, 42],
        plans: [
          {
            ...fixturePolicy,
            days: 1,
            products: [{ ...fixturePolicy.products[0], demand: { dailyUnits: [2] } }],
          },
        ],
      },
    },
  ],
};

export const SEED_WRITES = [
  ['create_order', (pool) => ({ items: [seedItem(pool)] })],
  ['create_order', (pool) => ({ items: [seedItem(pool, { quantity: 2 })] })],
  ['create_cart', none],
  ['add_cart_item', (pool) => ({ unitPrice: pool.named.skuPrice ?? '10.00' })],
  ['create_shipment', none],
  [
    'create_invoice',
    () => ({
      items: JSON.stringify([{ description: 'Seed service', quantity: 1, unitPrice: 10 }]),
    }),
  ],
  ['create_gift_card', none],
  ['create_store_credit', none],
  ['create_subscription_plan', none],
  ['create_subscription', none],
  ['create_coupon', none],
  ['create_tax_exemption', () => ({ exemptionType: 'resale' })],
  ['create_supplier', none],
  [
    'create_purchase_order',
    (pool) => ({
      items: JSON.stringify([
        { sku: pool.named.sku, name: 'Seed item', quantity: 10, unitCostExact: '5.00' },
      ]),
    }),
  ],
  ['create_warehouse', none],
  [
    'create_location',
    (pool) => ({ locationType: 'pick', warehouseId: Number(firstId(pool, 'warehouses', 1)) }),
  ],
  ['create_lot', none],
  ['create_serial', none],
  ['create_backorder', none],
  [
    'create_receipt',
    (pool) => ({
      receiptType: 'purchase_order',
      warehouseId: Number(firstId(pool, 'warehouses', 1)),
    }),
  ],
  [
    'create_inbound_shipment',
    (pool) => ({
      items: [{ productId: firstId(pool, 'products'), sku: pool.named.sku, quantityExpected: '5' }],
    }),
  ],
  ['create_supplier_sku', none],
  ['create_bom', none],
  ['create_work_order', none],
  ['create_production_batch', none],
  ['create_inspection', () => ({ inspectionType: 'incoming', referenceType: 'purchase_order' })],
  ['create_ncr', () => ({ source: 'inspection', severity: 'minor', description: 'Seed NCR' })],
  ['create_quality_hold', () => ({ holdType: 'quarantine', reason: 'Seed hold' })],
  ['create_warranty', none],
  ['create_warranty_claim', () => ({ issueDescription: 'Seed claim' })],
  ['create_review', none],
  ['create_wishlist', none],
  ['add_to_wishlist', none],
  ['create_segment', none],
  ['create_shipping_zone', () => ({ name: 'Seed zone', countries: ['US'] })],
  ['create_unit_class', none],
  ['create_unit_of_measure', () => ({ factor: '1' })],
  ['create_custom_object_type', none],
  ['create_company', none],
  ['create_price_level', none],
  ['create_price_schedule', none],
  ['create_payment_obligation', () => ({ dueDate: '2026-12-31' })],
  ['create_integration_mapping', none],
  [
    'create_gl_account',
    () => ({ accountNumber: '1999', name: 'Seed account', accountType: 'asset' }),
  ],
  [
    'create_gl_period',
    () => ({
      periodName: 'January 2026',
      fiscalYear: 2026,
      startDate: '2026-01-01',
      endDate: '2026-01-31',
    }),
  ],
  ['create_bill', none],
  ['create_prepayment', none],
  ['create_vendor_credit', none],
  [
    'create_fixed_asset',
    () => ({
      acquisitionCost: '1000.00',
      acquisitionDate: '2026-01-15',
      usefulLifeMonths: 12,
    }),
  ],
  ['create_revenue_contract', () => ({ effectiveDate: '2026-01-15' })],
  ['create_edi_document', none],
  ['record_activity', () => ({ subjectType: 'order', actorKind: 'system' })],
  ['create_search_config', none],
  ['register_agent_card', none],
  ['x402_create_payment_intent', none],
  ['agent_key_generate', () => ({ agentId: SEED_AGENT_ID })],
  ['create_payment_link', (pool) => ({ items: [seedItem(pool, { unitPrice: 10 })] })],
];

/**
 * Run after every other seed, so it is the newest replayable write in the
 * MCP replay log -- the call `agentic_replay_mutation` re-runs when given no
 * filter -- and one that writes a new row every time it runs.
 */
const LAST_SEED_WRITE = SEED_WRITES.find(([name]) => name === 'create_payment_link');

/**
 * Seed the store: demo data through the native API (the same seeder
 * `stateset-mcp-http` boots with), products activated so orders can be
 * placed, then SEED_WRITES through the server's own dispatch.
 *
 * @param {object} deps
 * @param {object} deps.commerce - native Commerce on the store
 * @param {Function} deps.seedDemoData - src/seeds/demo.js
 * @param {(name: string, params: object) => Promise<object>} deps.invoke
 * @param {(tool: object) => Array<{label: string, params: object}>} deps.variantsFor
 * @param {Map<string, object>} deps.toolsByName
 * @param {() => Promise<void>} deps.refreshPool - re-read seed ids after a write
 * @param {() => ReturnType<typeof readSeedPool>} deps.pool
 */
export async function seedStore({
  commerce,
  seedDemoData,
  invoke,
  variantsFor,
  toolsByName,
  refreshPool,
  pool,
}) {
  const report = { demo: null, activated: 0, writes: [] };
  const debug = console.debug;
  console.debug = () => {}; // the demo seeder logs each skipped record
  try {
    report.demo = await seedDemoData(commerce, { quiet: true });
  } finally {
    console.debug = debug;
  }
  for (const product of await commerce.products.list()) {
    if (product.status === 'active') continue;
    try {
      await commerce.products.activate(product.id);
      report.activated += 1;
    } catch {
      // A product the engine will not activate stays a draft.
    }
  }
  await refreshPool();

  const write = async ([name, overrides]) => {
    const tool = toolsByName.get(name);
    if (!tool) return { ok: false, error: 'no such tool' };
    const variants = variantsFor(tool);
    const base = (variants.find((v) => v.label === 'seeded') ?? variants[0]).params;
    const result = await invoke(name, { ...base, ...overrides(pool()) });
    if (result?.ok) await refreshPool();
    return result?.ok
      ? { ok: true }
      : { ok: false, error: String(result?.error ?? '').slice(0, 160) };
  };

  const done = new Set();
  for (const pass of [1, 2]) {
    for (const [index, entry] of SEED_WRITES.entries()) {
      if (done.has(index)) continue;
      const outcome = await write(entry);
      if (outcome.ok) done.add(index);
      if (outcome.ok || pass === 2) report.writes.push({ tool: entry[0], ...outcome });
    }
  }
  report.last = { tool: LAST_SEED_WRITE[0], ...(await write(LAST_SEED_WRITE)) };
  return report;
}

// ---------------------------------------------------------------------------
// The sweep
// ---------------------------------------------------------------------------

async function invokeWithTimeout(invoke, name, params, timeoutMs) {
  const startedAt = Date.now();
  let timer;
  const timeout = new Promise((resolve) => {
    timer = setTimeout(() => resolve({ outcome: 'timeout' }), timeoutMs);
    timer.unref?.();
  });
  const call = Promise.resolve()
    .then(() => invoke(name, params))
    .then((result) => ({
      outcome: result?.ok ? 'ok' : 'refused',
      status: result?.status ?? null,
      message: result?.ok ? null : String(result?.error ?? '').slice(0, 200),
    }))
    .catch((error) => ({
      outcome: 'threw',
      message: String(error?.message ?? error).slice(0, 200),
    }));
  const result = await Promise.race([call, timeout]);
  clearTimeout(timer);
  return { ...result, ms: Date.now() - startedAt };
}

/**
 * Call each tool with each of its input variants, measuring persisted state
 * after every call. Each measurement is against the state left by the call
 * before it, so a change is attributed to exactly one call.
 *
 * @param {object} options
 * @param {object[]} options.tools - tool definitions to call
 * @param {(tool: object) => Array<{label: string, params: object}>} options.variantsFor
 * @param {(name: string, params: object) => Promise<object>} options.invoke - the server's dispatch
 * @param {(toolName: string) => Promise<object[]>} options.changesSince - changes
 *   since the previous measurement (see state-probe.js `diff`); the caller
 *   takes the first baseline before sweeping
 * @param {number} [options.timeoutMs]
 */
export async function sweepTools({ tools, variantsFor, invoke, changesSince, timeoutMs = 30_000 }) {
  const calls = [];
  const violations = [];
  for (const tool of tools) {
    let variants;
    try {
      variants = variantsFor(tool);
    } catch (error) {
      calls.push({
        tool: tool.name,
        variant: 'sampler',
        outcome: 'sampler-error',
        message: error.message,
      });
      continue;
    }
    for (const { label, params } of variants) {
      const result = await invokeWithTimeout(invoke, tool.name, params, timeoutMs);
      const changes = await changesSince(tool.name);
      calls.push({ tool: tool.name, variant: label, ...result, changed: changes.length > 0 });
      if (changes.length > 0) {
        violations.push({ tool: tool.name, variant: label, params, ...result, changes });
      }
    }
  }
  return { calls, violations };
}
