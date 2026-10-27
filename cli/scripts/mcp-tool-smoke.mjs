#!/usr/bin/env node
/**
 * Call every MCP tool through the real server against a fresh store, with
 * the minimal input its own schema requires, and classify what happens.
 *
 * A tool may legitimately refuse a synthetic input (not found, validation,
 * not configured). What it must never do is crash: a TypeError, "is not a
 * function", a read of undefined, or a service that was never attached. Those
 * are defects in the tool, not in the input -- the class of bug that unit
 * tests with mocks cannot see.
 *
 *   node scripts/mcp-tool-smoke.mjs [--json out.json] [--only name,name]
 */

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { randomUUID } from 'node:crypto';

import { createStatesetMcpServer } from '../src/mcp-server.js';
import { ALL_DOMAIN_TOOLS } from '../src/tools/domain-registry.js';
import { toToolResultContract } from '../src/mcp/tool-result-contract.js';

/** Messages that mean the tool itself is broken, whatever its input. */
export const DEFECT_PATTERNS = [
  /is not a function/i,
  /Cannot read (?:property|properties) of (?:undefined|null)/i,
  /Cannot destructure/i,
  /is not defined/i,
  /is not iterable/i,
  /not initialized/i,
  /CreateListFromArrayLike/i,
  /Missing field `?\w+`?/i,
  /Failed to convert (?:JavaScript|napi) value/i,
  /\bexpect(?:ed)? .* but got\b/i,
  // A fresh store's own schema disagreeing with the code: never an input problem.
  /no such (?:column|table)/i,
];

/**
 * Classify one call from its result contract (src/mcp/tool-result-contract.js):
 * `ok` -> 'ok' (a preview is ok too); a refusal with a crash-shaped message ->
 * 'defect'; any other `ok: false` -> 'refused'.
 */
export function classifyOutcome(outcome) {
  if (outcome.timedOut) return 'timeout';
  if (outcome.threw)
    return DEFECT_PATTERNS.some((p) => p.test(outcome.message)) ? 'defect' : 'refused';
  if (outcome.contract?.ok ?? outcome.success) return 'ok';
  return DEFECT_PATTERNS.some((p) => p.test(outcome.message ?? '')) ? 'defect' : 'refused';
}

// ---------------------------------------------------------------------------
// Minimal input from a zod schema
// ---------------------------------------------------------------------------

/** Where tools that write files (backups, exports, snapshots) may write. */
const OUTPUT_DIR = path.join(os.tmpdir(), 'mcp-smoke-output');

/** A Zod 4 schema's checks, as their definitions (`{ check: 'min_length', minimum: 2 }`). */
const checksOf = (def) => (def.checks ?? []).map((c) => c._zod.def);

/**
 * Values tried, in order, against a string's own regex checks when the
 * key-based guess does not match them: decimals, dates, hashes, codes.
 */
const PATTERN_CANDIDATES = [
  'smoke',
  '10',
  '10.00',
  '2026-01-15',
  'USD',
  'USD/EUR',
  '1234',
  `0x${'a'.repeat(64)}`,
  `0x${'a'.repeat(40)}`,
];

function sampleString(key, def) {
  const checks = checksOf(def);
  const formats = new Set([def.format, ...checks.map((c) => c.format)].filter(Boolean));
  const has = (format) => formats.has(format);
  const min = Math.max(0, ...checks.filter((c) => c.check === 'min_length').map((c) => c.minimum));
  const k = key.toLowerCase();
  let value;
  if (/path|file|dir|destination|output|location/.test(k) && !/id$/.test(k))
    value = path.join(OUTPUT_DIR, `${randomUUID()}.out`);
  else if (has('uuid') || /(^|_)id$|id$|ids?$/.test(k)) value = randomUUID();
  else if (has('email') || k.includes('email')) value = 'smoke@example.com';
  else if (has('url') || /url|endpoint|webhook/.test(k)) value = 'https://example.com/hook';
  else if (has('datetime') || /date|_at$|at$|since|until|from|to$/.test(k))
    value = new Date('2026-01-15T00:00:00Z').toISOString();
  else if (/currency/.test(k)) value = 'USD';
  else if (/countr/.test(k)) value = 'US';
  else if (/state|region/.test(k)) value = 'CA';
  else if (/sku/.test(k)) value = 'SMOKE-SKU';
  else if (/amount|price|total|value/.test(k)) value = '10.00';
  else value = 'smoke';
  while (value.length < min) value += 'x';
  const max = Math.min(...checks.filter((c) => c.check === 'max_length').map((c) => c.maximum));
  value = value.slice(0, max);
  const patterns = checks.filter((c) => c.format === 'regex').map((c) => c.pattern);
  const fits = (candidate) => patterns.every((pattern) => pattern.test(candidate));
  return fits(value) ? value : (PATTERN_CANDIDATES.find(fits) ?? value);
}

function sampleNumber(def) {
  let v = 1;
  for (const c of checksOf(def)) {
    if (c.check === 'greater_than' && (v < c.value || (v === c.value && !c.inclusive)))
      v = c.value + (c.inclusive ? 0 : 1);
    if (c.check === 'less_than' && v > c.value) v = c.value;
  }
  return v;
}

/** The smallest value a Zod 4 schema accepts, guessed from its definition. */
export function sample(schema, key = '') {
  const def = schema?._zod?.def;
  switch (def?.type) {
    case 'optional':
    case 'nullable':
    case 'nonoptional':
    case 'readonly':
    case 'catch':
      return sample(def.innerType, key);
    case 'default':
    case 'prefault':
      return def.defaultValue;
    case 'pipe':
      // `z.preprocess(fn, schema)` pipes a transform into `schema`: sample the schema.
      return sample(def.in._zod.def.type === 'transform' ? def.out : def.in, key);
    case 'lazy':
      return sample(def.getter(), key);
    case 'string':
      return sampleString(key, def);
    case 'number':
      return sampleNumber(def);
    case 'bigint':
      return 1n;
    case 'boolean':
      return false;
    case 'date':
      return new Date('2026-01-15T00:00:00Z');
    case 'enum':
      return Object.values(def.entries)[0];
    case 'literal':
      return def.values[0];
    case 'array': {
      const min = Math.max(
        1,
        ...checksOf(def)
          .filter((c) => c.check === 'min_length')
          .map((c) => c.minimum),
      );
      return Array.from({ length: min }, () => sample(def.element, key));
    }
    case 'tuple':
      return def.items.map((item) => sample(item, key));
    case 'object':
      return sampleShape(def.shape);
    case 'record':
    case 'map':
      return {};
    case 'union':
      return sample(def.options[0], key);
    case 'intersection':
      return { ...sample(def.left, key), ...sample(def.right, key) };
    default:
      return 'smoke';
  }
}

function isOptional(schema) {
  return schema?.isOptional?.() === true;
}

/** Required fields only: the smallest input the schema accepts. */
export function sampleShape(shape) {
  const out = {};
  for (const [key, schema] of Object.entries(shape ?? {})) {
    if (isOptional(schema)) continue;
    out[key] = sample(schema, key);
  }
  return out;
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

async function callWithTimeout(server, name, params, ms) {
  let timer;
  const timeout = new Promise((resolve) => {
    timer = setTimeout(() => resolve({ timedOut: true }), ms);
    timer.unref?.();
  });
  const call = server
    .executeTool(name, params)
    .then((result) => {
      // The contract judges the tool by its own result too: `{ success: false }`
      // and a rejected kernel receipt are `ok: false`.
      const contract = toToolResultContract(result);
      return {
        success: contract.ok,
        contract,
        code: contract.error?.code,
        message: contract.error?.message,
      };
    })
    .catch((error) => ({ threw: true, message: String(error?.stack ?? error) }));
  const outcome = await Promise.race([call, timeout]);
  clearTimeout(timer);
  return outcome;
}

/**
 * @param {object} [options]
 * @param {Set<string>|null} [options.only] - restrict the sweep to these tool names
 * @param {Array<object>} [options.tools] - tool definitions to sweep (default: every domain tool)
 * @param {number} [options.timeoutMs]
 * @param {boolean} [options.allowApply] - `false` runs the sweep in preview mode (writes are previews)
 */
export async function runSmoke({
  only = null,
  tools = ALL_DOMAIN_TOOLS,
  timeoutMs = 20_000,
  allowApply = true,
} = {}) {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'mcp-smoke-'));
  const dbPath = path.join(dir, 'store.db');
  // Some tools keep state under ~/.stateset (a2a.db, audit.db, wallets). A
  // fresh store means a fresh home too: otherwise the sweep writes into the
  // developer's real home and its verdict depends on what is already there.
  const home = path.join(dir, 'home');
  mkdirSync(home);
  const savedHome = { HOME: process.env.HOME, USERPROFILE: process.env.USERPROFILE };
  process.env.HOME = home;
  process.env.USERPROFILE = home;
  // Others keep project-local state under ./.stateset (agent keys): run from
  // the sweep's own directory so it never lands in the checkout running it.
  const savedCwd = process.cwd();
  process.chdir(dir);
  // Every tier, so any tool can be swept regardless of the default profile.
  const server = createStatesetMcpServer({ dbPath, allowApply, toolProfile: 'all' });
  const rows = [];
  try {
    for (const tool of tools) {
      if (only && !only.has(tool.name)) continue;
      let params;
      try {
        params = sampleShape(tool.inputSchema);
      } catch (error) {
        rows.push({ tool: tool.name, kind: 'defect', message: `sampler: ${error.message}` });
        continue;
      }
      const outcome = await callWithTimeout(server, tool.name, params, timeoutMs);
      rows.push({
        tool: tool.name,
        kind: classifyOutcome(outcome),
        code: outcome.code,
        preview: outcome.contract?.preview,
        contract: outcome.contract,
        message: outcome.message?.split('\n')[0]?.slice(0, 240),
      });
    }
  } finally {
    process.chdir(savedCwd);
    for (const [key, value] of Object.entries(savedHome)) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
    rmSync(dir, { recursive: true, force: true });
  }
  return rows;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const args = process.argv.slice(2);
  const jsonAt = args.indexOf('--json');
  const onlyAt = args.indexOf('--only');
  const only = onlyAt >= 0 ? new Set(args[onlyAt + 1].split(',')) : null;
  const rows = await runSmoke({ only });
  const counts = rows.reduce((acc, r) => ({ ...acc, [r.kind]: (acc[r.kind] ?? 0) + 1 }), {});
  console.log(JSON.stringify(counts));
  for (const r of rows.filter((r) => r.kind === 'defect' || r.kind === 'timeout')) {
    console.log(`${r.kind.padEnd(8)} ${r.tool.padEnd(40)} ${r.message ?? ''}`);
  }
  if (jsonAt >= 0) {
    const slim = rows.map(({ contract: _contract, ...row }) => row);
    writeFileSync(args[jsonAt + 1], JSON.stringify(slim, null, 2));
  }
  process.exit(0);
}
