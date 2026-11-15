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
 *   fixture  -- hand-written inputs the sampler cannot produce (FIXTURE_INPUTS),
 *               many built from seeded rows or from values a seed captured
 *
 * A second sweep after LATE_SEED_WRITES labels its variants `late <label>`.
 * State is fingerprinted by ./state-probe.js, out of process; outbound
 * connections and non-Node child processes are refused (installSweepGuard).
 */

import childProcess from 'node:child_process';
import { createHmac } from 'node:crypto';
import { mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { syncBuiltinESMExports } from 'node:module';
import net from 'node:net';
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
  // Credentials and endpoints of external services (vector search, chain
  // RPCs, the agent-receipt demo stack, the sync sequencer) are removed for
  // the sweep, so a developer's shell cannot change which tools run or what
  // they reach. installSweepGuard blocks the connections anyway. Variables
  // that move the CLI's own state elsewhere (STATESET_CONNECTOR_HOME,
  // STATESET_POLICY_DIR, STATESET_CONFIG_DIR, STATESET_DB_PATH, ...) or move
  // XDG files out of HOME are removed too, so every seed write lands in the
  // sandbox and cleanup removes it: a developer's STATESET_CONNECTOR_HOME used
  // to receive the seeded test connector and keep a catalog entry pointing at
  // the deleted temporary WASM.
  const scrubbed = Object.keys(process.env).filter(
    (key) => EXTERNAL_ENV.test(key) || LOCAL_STATE_ENV.test(key),
  );
  const saved = {
    cwd: process.cwd(),
    env: Object.fromEntries(
      ['HOME', 'USERPROFILE', 'TMPDIR', 'TMP', 'TEMP', ...scrubbed].map((key) => [
        key,
        process.env[key],
      ]),
    ),
  };
  for (const key of scrubbed) delete process.env[key];
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

/** Environment variables that move the CLI's own state, or XDG files, outside the sandbox. */
const LOCAL_STATE_ENV = /^STATESET_|^XDG_(?:CONFIG|DATA|STATE|CACHE)_HOME$/;

/** Environment variables that point a tool at an external service or hold its credentials. */
const EXTERNAL_ENV =
  /(?:_API_KEY|_RPC_URL|_RPC_USER|_RPC_PASSWORD|_SEQUENCER_URL|_TOKEN|_SECRET)$|^(?:AGENT_RECEIPT_|ANVIL_URL$|ORDER_ESCROW$|BRIDGE_PAYOUT_URL$|DATABASE_URL$)/;

/**
 * Keep the sweep local: refuse every outbound TCP connection (HTTP, HTTPS,
 * fetch, RPC clients, databases) and every child process that is not this
 * Node binary (the simulation, compliance and state-probe workers are Node
 * forks of src/ or test/ code; nothing in a read tool may run cargo, a shell
 * or any other program). A refused attempt is recorded, not thrown past the
 * caller: a connection fails asynchronously the way an unreachable host
 * would, a spawn throws. `take()` returns and clears the attempts recorded
 * since the last call, so the sweep can attribute each to one tool call.
 *
 * Only this process is guarded. Its Node children inherit no hooks, which is
 * why the sandbox also scrubs the external endpoints from the environment.
 */
export function installSweepGuard() {
  const attempts = [];
  const allowedBinary = (file) => {
    try {
      return realpathSync(String(file)) === realpathSync(process.execPath);
    } catch {
      return String(file) === process.execPath;
    }
  };
  const refuse = (command) => {
    attempts.push({ kind: 'spawn', target: command.slice(0, 200) });
    const error = new Error(`read-tool sweep: refused to run '${command.slice(0, 80)}'`);
    error.code = 'EACCES';
    throw error;
  };
  const saved = {};
  for (const name of [
    'spawn',
    'spawnSync',
    'execFile',
    'execFileSync',
    'fork',
    'exec',
    'execSync',
  ]) {
    const original = childProcess[name];
    saved[name] = original;
    childProcess[name] = function guarded(file, ...rest) {
      // fork always runs a module under this Node binary; exec/execSync take
      // a shell command line, which is never just this binary.
      if (name === 'fork') return original.call(this, file, ...rest);
      if (!name.startsWith('exec') || name.startsWith('execFile')) {
        if (allowedBinary(file)) return original.call(this, file, ...rest);
      }
      return refuse([file, ...(Array.isArray(rest[0]) ? rest[0] : [])].join(' '));
    };
  }

  const connect = net.Socket.prototype.connect;
  net.Socket.prototype.connect = function guardedConnect(...args) {
    // net.connect() passes its normalized [options, callback] pair as one array.
    const list = Array.isArray(args[0]) ? args[0] : args;
    const [first, second] = list;
    const options =
      first && typeof first === 'object'
        ? first
        : typeof first === 'string' && Number.isNaN(Number(first))
          ? { path: first }
          : { port: first, host: typeof second === 'string' ? second : undefined };
    if (options.path) return connect.apply(this, args); // a local IPC socket
    const target = `${options.host ?? 'localhost'}:${options.port ?? ''}`;
    attempts.push({ kind: 'connect', target });
    process.nextTick(() => {
      const error = new Error(`read-tool sweep: refused connection to ${target}`);
      error.code = 'ECONNREFUSED';
      this.destroy(error);
    });
    return this;
  };
  syncBuiltinESMExports();

  return {
    take: () => attempts.splice(0),
    restore() {
      Object.assign(childProcess, saved);
      net.Socket.prototype.connect = connect;
      syncBuiltinESMExports();
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
    /** First row of a query, or null when a table it reads does not exist yet. */
    const row = (sql, ...args) => {
      try {
        return db.prepare(sql).get(...args) ?? null;
      } catch {
        return null;
      }
    };
    const sku = first('inventory_items', 'sku') ?? first('product_variants', 'sku');
    const skuPrice = row('SELECT price FROM product_variants WHERE sku = ?', sku)?.price ?? null;
    const orderItem = row('SELECT id, order_id FROM order_items ORDER BY rowid LIMIT 1');
    const customObject = row(
      'SELECT o.handle AS object, t.handle AS type FROM custom_object_records o ' +
        'JOIN custom_object_types t ON t.id = o.type_id LIMIT 1',
    );
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
        slug: first('products', 'slug'),
        orderItemId: orderItem?.id ?? null,
        orderItemOrderId: orderItem?.order_id ?? null,
        customObjectTypeHandle: customObject?.type ?? first('custom_object_types', 'handle'),
        customObjectHandle: customObject?.object ?? null,
        approvedReviewProductId:
          row("SELECT product_id FROM reviews WHERE status = 'approved' LIMIT 1")?.product_id ??
          null,
        pendingReviewId: row("SELECT id FROM reviews WHERE status = 'pending' LIMIT 1")?.id ?? null,
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
  if (k === 'slug') return named.slug;
  const idMatch = /^(?:(.*)_)?ids?$/.exec(k);
  if (!idMatch && k !== 'identifier') return null;
  const entity = idMatch?.[1];
  // source_warehouse_id -> source_warehouse, then warehouse
  const candidates = entity
    ? entity.split('_').map((_, start, tokens) => tokens.slice(start).join('_'))
    : entitiesFromToolName(toolName);
  const tables = [...idsByTable.keys()];
  const toolTokens = new Set(toolName.split('_'));
  // A table qualified by words of the tool's own name: get_inbound_shipment's
  // shipmentId is an inbound_shipments row, not a shipments row;
  // get_tax_rate_record's rateId a tax_rates row, not an exchange_rates row.
  const qualified = (plural) =>
    tables.find(
      (name) =>
        name.endsWith(`_${plural}`) &&
        name
          .slice(0, -plural.length - 1)
          .split('_')
          .every((token) => toolTokens.has(token)),
    );
  const suffixed = (plural) => tables.find((name) => name.endsWith(`_${plural}`));
  // Candidates run most specific first. One the field named (work_order_id ->
  // work_order) may match any table ending in it (manufacturing_work_orders);
  // one guessed from the tool's name only a qualified or exact table, so
  // check_gift_card_balance's `identifier` finds gift_cards before the
  // `balance` guess can land on inventory_balances. The loose match for guessed
  // candidates comes last.
  const pick = (candidate, loose) => {
    const plurals = pluralsOf(candidate);
    return (
      plurals.map(qualified).find(Boolean) ??
      plurals.find((plural) => idsByTable.has(plural)) ??
      (loose ? plurals.map(suffixed).find(Boolean) : null) ??
      null
    );
  };
  const table =
    candidates.map((candidate) => pick(candidate, Boolean(entity))).find(Boolean) ??
    (entity ? null : candidates.map((candidate) => pick(candidate, true)).find(Boolean));
  return table ? idsByTable.get(table)[0] : null;
}

/**
 * Input variants for one tool (see the module comment).
 *
 * @param {object} deps
 * @param {Function} deps.sample - smoke sampler `sample(schema, key)`
 * @param {Function} deps.sampleShape - smoke sampler `sampleShape(shape)`
 * @param {(schema: any) => any} deps.view - from createSamplerView
 * @param {() => ReturnType<typeof readSeedPool>} deps.pool
 * @param {Record<string, object[] | ((pool: ReturnType<typeof readSeedPool>) => object[])>} [deps.fixtures]
 *   extra inputs per tool, appended: a list, or a function of the seed pool
 *   that returns one (an empty one when the rows it needs were not seeded)
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
    const fixture = fixtures[tool.name] ?? [];
    for (const params of typeof fixture === 'function' ? fixture(pool()) : fixture) {
      variants.push({ label: 'fixture', params });
    }
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

const none = () => ({});
const firstId = (pool, table, fallback = null) => pool.idsByTable.get(table)?.[0] ?? fallback;

/** The connector the seed publishes to the local marketplace. */
const SEED_CONNECTOR = 'read-sweep-connector';

/**
 * Write the smallest valid WebAssembly module (magic number and version) to
 * the temp dir, which the sweep points into its sandbox, and return its path.
 */
function writeSeedWasm() {
  const file = path.join(os.tmpdir(), `${SEED_CONNECTOR}.wasm`);
  writeFileSync(file, Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]));
  return file;
}

/**
 * One fixture input built from seeded values, or none while any of them is
 * missing (its seed refused, or has not run yet).
 */
const withSeeded = (values, build) =>
  values.every((value) => value !== null && value !== undefined) ? [build(...values)] : [];

/** The agent whose sync keys the seed generates (under ./.stateset/keys). */
export const SEED_AGENT_ID = 'seed-agent';
/** The in-process agent runtime the seed creates (agent_create_runtime). */
export const SEED_RUNTIME = 'seed-runtime';

const WALLET_A = `0x${'11'.repeat(20)}`;
const WALLET_B = `0x${'22'.repeat(20)}`;
const WALLET_C = `0x${'33'.repeat(20)}`;
const ADDRESS = {
  name: 'Seed Buyer',
  line1: '1 Market St',
  city: 'San Francisco',
  state: 'CA',
  postalCode: '94105',
  country: 'US',
};
const TAX_ADDRESS = { country: 'US', state: 'CA', city: 'San Francisco', postalCode: '94105' };
const TAX_ITEMS = [{ unitPrice: '10.00', quantity: 1 }];

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
const fixtureEvent2 = { ...fixtureEvent, id: 'evt-2', eventSigningHash: `0x${'cd'.repeat(32)}` };

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

  // Well-formed values the sampler cannot guess: currency codes, calendar
  // dates, a real tool name, a full US address, signed or hashed payloads.
  get_exchange_rate: [{ from: 'USD', to: 'EUR' }],
  convert_currency: [{ from: 'USD', to: 'EUR', amount: 10 }],
  get_trial_balance: [{ asOfDate: '2026-01-31' }],
  get_balance_sheet: [{ asOfDate: '2026-01-31' }],
  get_income_statement: [{ startDate: '2026-01-01', endDate: '2026-01-31' }],
  get_payment_obligation_dashboard: [{ today: '2026-01-15' }],
  agentic_prepare_payment: [{ tool: 'list_customers' }],
  calculate_tax_quote_with_failover: [{ items: TAX_ITEMS, shippingAddress: TAX_ADDRESS }],
  quote_shipping_rates: [
    {
      providerId: 'deterministic-mock',
      destinationAddress: ADDRESS,
      parcels: [{ weightGrams: 500 }],
    },
  ],
  a2a_verify_webhook: [
    {
      rawBody: '{"event":"read-sweep"}',
      // HMAC-SHA256 of rawBody under secret 'read-sweep-secret'.
      signatureHeader: `sha256=${createHmac('sha256', 'read-sweep-secret')
        .update('{"event":"read-sweep"}')
        .digest('hex')}`,
      secret: 'read-sweep-secret',
    },
  ],
  sync_verify_receipt: [
    {
      envelope: {
        eventId: '00000000-0000-4000-8000-000000000001',
        tenantId: '00000000-0000-4000-8000-000000000002',
        storeId: '00000000-0000-4000-8000-000000000003',
        sourceAgent: '00000000-0000-4000-8000-000000000004',
        agentKeyId: 1,
        entityType: 'order',
        entityId: 'read-sweep-order',
        eventType: 'created',
        createdAt: fixtureEvent.timestamp,
        payloadPlainHash: `0x${'ab'.repeat(32)}`,
        payloadCipherHash: `0x${'00'.repeat(32)}`,
        agentSignature: `0x${'00'.repeat(64)}`,
        vesVersion: 1,
      },
      publicKeyHex: `0x${'00'.repeat(32)}`,
    },
  ],
  generate_inclusion_proof: [
    { eventId: fixtureEvent.id, events: JSON.stringify([fixtureEvent, fixtureEvent2]) },
  ],
  inspect_batch: [
    { batchId: 'read-sweep-batch', events: JSON.stringify([fixtureEvent, fixtureEvent2]) },
  ],
  export_compliance_package: [{ events: JSON.stringify([fixtureEvent, fixtureEvent2]) }],
  verify_receipt: (pool) =>
    withSeeded([pool.named.receiptBundle], (receiptBundle) => ({ receiptBundle })),
  verify_inclusion_proof: (pool) =>
    withSeeded([pool.named.inclusionProof], (proof) => ({
      leafHash: proof.leaf,
      proof: JSON.stringify(proof.proof),
      expectedRoot: proof.root,
    })),
  verify_chain_anchor: (pool) =>
    withSeeded([pool.named.inclusionProof], (proof) => ({
      leafHash: proof.leaf,
      proof: JSON.stringify(proof.proof),
      expectedRoot: proof.root,
      anchorTxHash: `0x${'ef'.repeat(32)}`,
    })),
  agent_key_info: [{ agentId: SEED_AGENT_ID, keyType: 'signing', keyId: 1 }],
  agent_receipt_status: [{ order_id_hash: `0x${'ab'.repeat(32)}` }],
  agent_receipt_fx_quote: [{ pair: 'USD/EUR' }],

  // Rows the seed created whose ids live in no table, or in a table the
  // param-name mapping cannot tell from its namesakes.
  get_custom_object: (pool) =>
    withSeeded([firstId(pool, 'custom_object_records')], (id) => ({ id })),
  get_revenue_schedule: (pool) =>
    withSeeded([firstId(pool, 'performance_obligations')], (obligationId) => ({ obligationId })),
  get_custom_object_by_handle: (pool) =>
    withSeeded(
      [pool.named.customObjectTypeHandle, pool.named.customObjectHandle],
      (typeHandle, objectHandle) => ({ typeHandle, objectHandle }),
    ),
  get_coupon: (pool) =>
    withSeeded([firstId(pool, 'coupon_codes'), pool.named.couponCode], (id, code) => [
      { identifier: id },
      { identifier: code },
    ]).flat(),
  get_serial: (pool) =>
    withSeeded([firstId(pool, 'serial_numbers')], (identifier) => ({ identifier })),
  get_ncr: (pool) => withSeeded([firstId(pool, 'non_conformances')], (ncrId) => ({ ncrId })),
  get_agent_card: (pool) => withSeeded([firstId(pool, 'agent_cards')], (agentId) => ({ agentId })),
  get_review_summary: (pool) =>
    withSeeded([pool.named.approvedReviewProductId], (productId) => ({ productId })),
  get_return: (pool) => withSeeded([firstId(pool, 'returns')], (returnId) => ({ returnId })),
  get_loyalty_account: (pool) =>
    withSeeded(
      [firstId(pool, 'loyalty_programs'), pool.named.loyaltyCustomerId],
      (programId, customerId) => ({ programId, customerId }),
    ),
  get_tax_quote: (pool) => withSeeded([pool.named.taxQuoteId], (quoteId) => ({ quoteId })),
  get_tax_transaction: (pool) =>
    withSeeded([pool.named.taxTransactionId], (transactionId) => ({ transactionId })),
  get_payment_intent: (pool) =>
    withSeeded([pool.named.paymentIntentId], (intentId) => ({ intentId })),
  track_shipping_label: (pool) =>
    withSeeded([pool.named.shippingLabelId], (labelId) => ({ labelId })),
  agentic_unsubscribe_events: (pool) =>
    withSeeded([pool.named.eventSubscriptionId], (subscriptionId) => ({ subscriptionId })),
  assess_wasm_connector_safety: [{ connectorId: SEED_CONNECTOR }],
  verify_wasm_connector_attestation: [{ connectorId: SEED_CONNECTOR }],
  get_installed_connector: [{ connectorId: SEED_CONNECTOR }],
  get_agent_wallet: [{}, { chain: 'base' }],
  agent_get_status: [{ name: SEED_RUNTIME }],
  agent_get_budget: [{ name: SEED_RUNTIME }],
  agent_discover_services: [{ name: SEED_RUNTIME }],
  agent_get_event_history: [{ name: SEED_RUNTIME }],
  agent_get_chain_balance: [{ name: SEED_RUNTIME }],
  agent_get_marketplace_metrics: (pool) =>
    withSeeded([pool.named.agentServiceId], (serviceId) => ({ serviceId })),
  agent_check_sla_compliance: (pool) =>
    withSeeded([pool.named.agentServiceId], (serviceId) => ({ serviceId })),
  a2a_get_service: (pool) =>
    withSeeded([pool.named.agentServiceId], (serviceId) => ({ serviceId })),
  agent_get_workflow_status: (pool) =>
    withSeeded([pool.named.agentWorkflowId], (workflowId) => ({ workflowId })),
  a2a_get_escrow: (pool) => withSeeded([pool.named.escrowId], (escrowId) => ({ escrowId })),
  a2a_check_payment_conditions: (pool) =>
    withSeeded([pool.named.escrowId], (escrowId) => ({ escrowId })),
  a2a_get_agent_subscription: (pool) =>
    withSeeded([pool.named.agentSubscriptionId], (subscriptionId) => ({ subscriptionId })),
  a2a_get_split_payment: (pool) =>
    withSeeded([pool.named.splitPaymentId], (splitPaymentId) => ({ splitPaymentId })),
  a2a_coordination_status: (pool) =>
    withSeeded([pool.named.coordinationId], (coordinationId) => ({ coordinationId })),
  a2a_join_results: (pool) =>
    withSeeded([pool.named.coordinationId], (coordinationId) => ({ coordinationId })),
};

/**
 * Tools called in apply mode after the demo data, so read tools in more
 * domains have rows to read. Each runs with its seeded minimal sample (real
 * ids where the schema names one) plus the overrides here, in two passes so a
 * record created late in the first pass can anchor one earlier in the
 * second. A seed that refuses is recorded and skipped: seeding is best effort,
 * the sweep only needs rows to exist.
 *
 * The optional third element picks values out of the tool's result into the
 * seed pool's `named` values, for records whose ids live in no table the pool
 * reads (in-memory payment intents, tax quotes, shipping labels, event
 * subscriptions, fan-out coordinations) or that a fixture needs whole (a
 * receipt bundle, an inclusion proof). A few of these are read tools run for
 * their output; they are swept again like every other read tool.
 *
 * @type {Array<[string, (pool: ReturnType<typeof readSeedPool>) => object, ((result: any) => object)?]>}
 */
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
      // Not the sampler's first category, 'land', which never depreciates.
      category: 'equipment',
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

  // A return against the order NATIVE_SEEDS ships (so it succeeds in pass 2).
  [
    'create_return',
    (pool) => ({
      orderId: pool.named.orderItemOrderId,
      reason: 'defective',
      items: [{ orderItemId: pool.named.orderItemId, quantity: 1 }],
    }),
  ],
  ['create_credit_memo', () => ({ amount: 5, reason: 'service_credit' })],
  ['create_credit_account', () => ({ creditLimit: 1000 })],
  ['set_item_cost', (pool) => ({ sku: pool.named.sku, standardCost: 5 })],

  // Catalog, fulfilment and warehouse records.
  [
    'create_custom_object',
    (pool) => ({ typeHandle: pool.named.customObjectTypeHandle, handle: 'seed-object' }),
  ],
  [
    'approve_review',
    (pool) => ({ reviewId: pool.named.pendingReviewId ?? firstId(pool, 'reviews') }),
  ],
  ['set_active_search_config', (pool) => ({ id: firstId(pool, 'search_configs') })],
  [
    'create_fulfillment_wave',
    (pool) => ({
      warehouseId: Number(firstId(pool, 'warehouses', 1)),
      orderIds: [firstId(pool, 'orders')],
    }),
  ],
  ['release_fulfillment_wave', (pool) => ({ waveId: firstId(pool, 'waves') })],
  [
    'create_cycle_count',
    (pool) => ({
      warehouseId: Number(firstId(pool, 'warehouses', 1)),
      lines: [{ sku: pool.named.sku, expectedQuantity: '1' }],
    }),
  ],
  [
    'create_transfer_order',
    (pool) => ({
      // Transfer orders take warehouse UUIDs; the `warehouses` table's ids
      // are integers, so these name no row (none is required).
      sourceWarehouseId: '00000000-0000-4000-8000-0000000000a1',
      destinationWarehouseId: '00000000-0000-4000-8000-0000000000a2',
      items: [{ productId: firstId(pool, 'products'), quantity: '1' }],
    }),
  ],
  [
    'capture_stock_snapshot',
    (pool) => ({
      lines: [
        {
          productId: firstId(pool, 'products'),
          sku: pool.named.sku,
          quantityOnHand: '5',
          quantityAvailable: '5',
        },
      ],
    }),
  ],
  [
    'capture_topology_snapshot',
    () => ({
      channelsTotal: '1',
      channelsActive: '1',
      warehousesTotal: '2',
      productsTotal: '20',
      openOrders: '2',
    }),
  ],
  ['create_channel', () => ({ name: 'Seed channel', channelType: 'sales_channel' })],
  ['pair_print_station', () => ({ name: 'Seed station' })],
  [
    'ingest_purgatory_order',
    (pool) => ({
      externalOrderId: 'SEED-EXT-1',
      items: [{ externalSku: pool.named.sku, quantity: '1' }],
    }),
  ],
  [
    'create_integration_field_mapping',
    () => ({
      integrationAccount: 'seed-account',
      mappingGroup: 'orders',
      sourceField: 'external_id',
      destinationField: 'order_number',
    }),
  ],
  [
    'create_vendor_return',
    (pool) => ({
      supplierId: firstId(pool, 'suppliers'),
      items: [{ productId: firstId(pool, 'products'), quantity: '1', unitCost: '5.00' }],
    }),
  ],
  [
    'assess_order_fraud',
    (pool) => ({
      orderId: firstId(pool, 'orders'),
      signals: [{ signalType: 'velocity_spike', score: 0.4, details: 'Seed signal' }],
    }),
  ],
  ['create_loyalty_program', () => ({ name: 'Seed loyalty' })],
  [
    'enroll_customer',
    (pool) => ({
      programId: firstId(pool, 'loyalty_programs'),
      customerId: firstId(pool, 'customers'),
    }),
    (result) => ({ loyaltyCustomerId: result?.account?.customerId ?? null }),
  ],

  // Finance: a depreciating asset with a posted schedule, a revenue schedule.
  ['initialize_chart_of_accounts', none],
  [
    'place_asset_in_service',
    (pool) => ({ assetId: firstId(pool, 'fixed_assets'), inServiceDate: '2026-01-15' }),
  ],
  ['generate_depreciation_schedule', (pool) => ({ assetId: firstId(pool, 'fixed_assets') })],
  ['post_depreciation', (pool) => ({ assetId: firstId(pool, 'fixed_assets') })],
  [
    'generate_revenue_schedule',
    (pool) => ({ obligationId: firstId(pool, 'performance_obligations') }),
  ],

  // Provider-backed records the deterministic mock providers keep in memory.
  [
    'calculate_tax_quote',
    () => ({ items: TAX_ITEMS, shippingAddress: TAX_ADDRESS }),
    (result) => ({ taxQuoteId: result?.quote?.id ?? null }),
  ],
  [
    'commit_tax_transaction',
    (pool) => ({ quoteId: pool.named.taxQuoteId }),
    (result) => ({ taxTransactionId: result?.transaction?.id ?? null }),
  ],
  [
    'create_payment_intent',
    () => ({ amount: '5.00', currency: 'USD' }),
    (result) => ({ paymentIntentId: result?.intent?.id ?? null }),
  ],
  [
    'create_shipping_label',
    () => ({
      providerId: 'deterministic-mock',
      destinationAddress: ADDRESS,
      parcels: [{ weightGrams: 500 }],
    }),
    (result) => ({ shippingLabelId: result?.label?.id ?? null }),
  ],
  [
    'agentic_subscribe_events',
    none,
    (result) => ({ eventSubscriptionId: result?.subscription?.id ?? null }),
  ],

  // Agent commerce: a local agent runtime with a service, a workflow, an
  // escrow deal, a split deal; an A2A subscription and a fan-out coordination.
  // The wallet derived for agent 'default' needs no chain; its balance does.
  ['agent_key_generate', () => ({ agentId: 'default', keyType: 'signing' })],
  ['agent_create_runtime', () => ({ name: SEED_RUNTIME })],
  [
    'agent_register_service',
    () => ({ name: SEED_RUNTIME, serviceName: 'Seed service', category: 'data' }),
    (result) => ({ agentServiceId: result?.service?.id ?? null }),
  ],
  [
    'agent_create_workflow',
    () => ({
      name: SEED_RUNTIME,
      workflowName: 'Seed workflow',
      steps: [{ name: 'transform', type: 'transform', params: {} }],
    }),
    (result) => ({ agentWorkflowId: result?.workflow?.id ?? null }),
  ],
  [
    'agent_create_escrow_deal',
    () => ({
      buyerName: SEED_RUNTIME,
      sellerAddress: WALLET_B,
      amount: 1,
      conditions: [{ type: 'buyer_confirmed' }],
    }),
    (result) => ({ escrowId: result?.escrow?.id ?? null }),
  ],
  [
    'agent_create_split_deal',
    () => ({
      payerName: SEED_RUNTIME,
      totalAmount: 2,
      recipients: [
        { address: WALLET_B, percentage: 50 },
        { address: WALLET_C, percentage: 50 },
      ],
    }),
    (result) => ({ splitPaymentId: result?.splitPayment?.id ?? null }),
  ],
  [
    'a2a_create_agent_subscription',
    () => ({
      subscriberAddress: WALLET_A,
      providerAddress: WALLET_B,
      planName: 'Seed plan',
      amount: 1,
    }),
    (result) => ({ agentSubscriptionId: result?.subscription?.id ?? null }),
  ],
  [
    'a2a_scatter',
    () => ({ targets: [WALLET_B], taskType: 'status_check', payload: {}, timeoutMs: 1000 }),
    (result) => ({ coordinationId: typeof result === 'string' ? result : null }),
  ],

  // A WASM connector in the local marketplace (an empty, valid module), installed.
  ['publish_wasm_connector', () => ({ connectorId: SEED_CONNECTOR, wasmPath: writeSeedWasm() })],
  ['install_wasm_connector', () => ({ connectorId: SEED_CONNECTOR })],

  // Read tools run once for output that other read tools take as input.
  [
    'generate_receipt_bundle',
    () => FIXTURE_INPUTS.generate_receipt_bundle[0],
    (result) => ({ receiptBundle: result?.bundle ? JSON.stringify(result.bundle) : null }),
  ],
  [
    'generate_inclusion_proof',
    () => FIXTURE_INPUTS.generate_inclusion_proof[0],
    (result) => ({ inclusionProof: result?.root ? result : null }),
  ],

  ['create_payment_link', (pool) => ({ items: [seedItem(pool, { unitPrice: 10 })] })],
];

const exactOr = (api, exact, plain) => (typeof api[exact] === 'function' ? exact : plain);

/**
 * Records created through the native API because the tool that creates them
 * is a governed mutation: in apply mode without kernel configuration -- this
 * sweep's posture -- create_payment, create_refund, ship_order and
 * update_order_status refuse (src/kernel-tool-execution.js). The calls mirror
 * those tools' legacy handlers. They run after the first SEED_WRITES pass, so
 * the second pass can build on them (create_return needs a shipped order).
 *
 * @type {Array<[string, (commerce: any, pool: ReturnType<typeof readSeedPool>) => Promise<void>]>}
 */
export const NATIVE_SEEDS = [
  [
    'payment, completed and partly refunded',
    async (commerce, pool) => {
      const orderId = firstId(pool, 'orders');
      const input = { orderId, amount: '25.00', currency: 'USD', paymentMethod: 'credit_card' };
      const payment = await commerce.payments[exactOr(commerce.payments, 'createExact', 'create')]({
        ...input,
        method: input.paymentMethod,
      });
      await commerce.payments.markCompleted(payment.id);
      await commerce.payments[exactOr(commerce.payments, 'createRefundExact', 'createRefund')]({
        paymentId: payment.id,
        amount: '1.00',
        reason: 'Seed refund',
      });
    },
  ],
  [
    'order shipped',
    async (commerce, pool) => {
      const orderId = pool.named.orderItemOrderId;
      for (const status of ['confirmed', 'processing']) {
        try {
          await commerce.orders.updateStatus(orderId, status);
        } catch {
          // Not every engine version requires (or allows) each step.
        }
      }
      await commerce.orders.ship(orderId, 'SEED-TRACK-1');
    },
  ],
];

/**
 * Seeds that would absorb a read tool's first-use initialisation if they ran
 * before the sweep: publishing a catalog entry creates the agent_catalog
 * table that query_agent_catalog otherwise creates (an `init` baseline
 * entry). They run after the sweep instead, and the read tools that found
 * nothing to read the first time are swept again (see the test).
 */
export const LATE_SEED_WRITES = [
  [
    'publish_product_catalog',
    (pool) => ({
      productId: firstId(pool, 'products'),
      name: 'Seed catalog entry',
      capabilities: ['read-sweep'],
    }),
  ],
];

/**
 * Run once more after both seeding passes, as a canary: it writes a new row
 * every time, and the test requires this last run to succeed, so a seeding
 * pass that broke the server's dispatch part-way fails loudly instead of
 * leaving the sweep to run against a half-seeded store.
 */
const LAST_SEED_WRITE = SEED_WRITES.find(([name]) => name === 'create_payment_link');

/**
 * Seed the store: demo data through the native API (the same seeder
 * `stateset-mcp-http` boots with), products activated so orders can be
 * placed, then SEED_WRITES through the server's own dispatch, with
 * NATIVE_SEEDS between its two passes.
 *
 * @param {object} deps
 * @param {object} deps.commerce - native Commerce on the store
 * @param {Function} deps.seedDemoData - src/seeds/demo.js
 * @param {(name: string, params: object) => Promise<object>} deps.invoke
 * @param {(tool: object) => Array<{label: string, params: object}>} deps.variantsFor
 * @param {Map<string, object>} deps.toolsByName
 * @param {() => Promise<void>} deps.refreshPool - re-read seed ids after a write
 * @param {() => ReturnType<typeof readSeedPool>} deps.pool
 * @param {(values: object) => void} deps.remember - keep values a seed captured
 *   from its result; refreshPool must merge them into the pool's `named`
 */
export async function seedStore({
  commerce,
  seedDemoData,
  invoke,
  variantsFor,
  toolsByName,
  refreshPool,
  pool,
  remember,
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

  const writer = createSeedWriter({
    invoke,
    variantsFor,
    toolsByName,
    refreshPool,
    pool,
    remember,
  });
  report.native = [];
  report.writes = await writer.writeAll(SEED_WRITES, async () => {
    for (const [label, seed] of NATIVE_SEEDS) {
      try {
        await seed(commerce, pool());
        report.native.push({ seed: label, ok: true });
      } catch (error) {
        report.native.push({ seed: label, ok: false, error: String(error?.message).slice(0, 160) });
      }
      await refreshPool();
    }
  });
  report.last = { tool: LAST_SEED_WRITE[0], ...(await writer.write(LAST_SEED_WRITE)) };
  return report;
}

/**
 * Runs seed entries (SEED_WRITES shape) through the server's dispatch.
 * `writeAll` makes two passes, so an entry that needs a record created later
 * in the first pass succeeds in the second; `between` runs between them.
 *
 * @param {object} deps - as seedStore's, without commerce and seedDemoData
 */
export function createSeedWriter({
  invoke,
  variantsFor,
  toolsByName,
  refreshPool,
  pool,
  remember,
}) {
  const write = async ([name, overrides, capture]) => {
    const tool = toolsByName.get(name);
    if (!tool) return { ok: false, error: 'no such tool' };
    const variants = variantsFor(tool);
    const base = (variants.find((v) => v.label === 'seeded') ?? variants[0]).params;
    const result = await invoke(name, { ...base, ...overrides(pool()) });
    if (result?.ok && capture) {
      const values = Object.entries(capture(result.result) ?? {}).filter(
        ([, value]) => value !== null && value !== undefined,
      );
      if (values.length === 0) return { ok: false, error: 'succeeded, but captured nothing' };
      remember(Object.fromEntries(values));
    }
    if (result?.ok) await refreshPool();
    return result?.ok
      ? { ok: true }
      : { ok: false, error: String(result?.error ?? '').slice(0, 160) };
  };

  const writeAll = async (entries, between = async () => {}) => {
    const outcomes = [];
    const done = new Set();
    for (const pass of [1, 2]) {
      if (pass === 2) await between();
      for (const [index, entry] of entries.entries()) {
        if (done.has(index)) continue;
        const outcome = await write(entry);
        if (outcome.ok) done.add(index);
        if (outcome.ok || pass === 2) outcomes.push({ tool: entry[0], ...outcome });
      }
    }
    return outcomes;
  };

  return { write, writeAll };
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
 * @param {() => Array<{kind: string, target: string}>} [options.takeRefused] -
 *   connections and processes refused since the last call (installSweepGuard)
 * @param {number} [options.timeoutMs]
 */
export async function sweepTools({
  tools,
  variantsFor,
  invoke,
  changesSince,
  takeRefused = () => [],
  timeoutMs = 30_000,
}) {
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
      const refused = takeRefused();
      calls.push({
        tool: tool.name,
        variant: label,
        ...result,
        changed: changes.length > 0,
        ...(refused.length > 0 ? { refused } : {}),
      });
      if (changes.length > 0) {
        violations.push({ tool: tool.name, variant: label, params, ...result, changes });
      }
    }
  }
  return { calls, violations };
}
