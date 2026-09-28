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

/** Messages that mean the tool itself is broken, whatever its input. */
export const DEFECT_PATTERNS = [
  /is not a function/i,
  /Cannot read propert(?:y|ies) of (?:undefined|null)/i,
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

export function classifyOutcome(outcome) {
  if (outcome.timedOut) return 'timeout';
  if (outcome.threw)
    return DEFECT_PATTERNS.some((p) => p.test(outcome.message)) ? 'defect' : 'refused';
  if (outcome.success) return 'ok';
  return DEFECT_PATTERNS.some((p) => p.test(outcome.message ?? '')) ? 'defect' : 'refused';
}

// ---------------------------------------------------------------------------
// Minimal input from a zod schema
// ---------------------------------------------------------------------------

/** Where tools that write files (backups, exports, snapshots) may write. */
const OUTPUT_DIR = path.join(os.tmpdir(), 'mcp-smoke-output');

function sampleString(key, def) {
  const checks = def.checks ?? [];
  const has = (kind) => checks.some((c) => c.kind === kind);
  const min = checks.find((c) => c.kind === 'min')?.value ?? 0;
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
  else if (/country/.test(k)) value = 'US';
  else if (/state|region/.test(k)) value = 'CA';
  else if (/sku/.test(k)) value = 'SMOKE-SKU';
  else if (/amount|price|total|value/.test(k)) value = '10.00';
  else value = 'smoke';
  while (value.length < min) value += 'x';
  return value;
}

export function sample(schema, key = '') {
  const def = schema?._def;
  switch (def?.typeName) {
    case 'ZodOptional':
    case 'ZodNullable':
      return sample(def.innerType, key);
    case 'ZodDefault':
      return def.defaultValue();
    case 'ZodEffects':
      return sample(def.schema, key);
    case 'ZodPipeline':
      return sample(def.in, key);
    case 'ZodBranded':
    case 'ZodReadonly':
    case 'ZodCatch':
      return sample(def.innerType ?? def.type, key);
    case 'ZodLazy':
      return sample(def.getter(), key);
    case 'ZodString':
      return sampleString(key, def);
    case 'ZodNumber': {
      const min = def.checks?.find((c) => c.kind === 'min');
      const max = def.checks?.find((c) => c.kind === 'max');
      let v = 1;
      if (min && v < min.value) v = min.value + (min.inclusive ? 0 : 1);
      if (max && v > max.value) v = max.value;
      return v;
    }
    case 'ZodBigInt':
      return 1n;
    case 'ZodBoolean':
      return false;
    case 'ZodDate':
      return new Date('2026-01-15T00:00:00Z');
    case 'ZodEnum':
      return def.values[0];
    case 'ZodNativeEnum':
      return Object.values(def.values)[0];
    case 'ZodLiteral':
      return def.value;
    case 'ZodArray': {
      const min = def.minLength?.value ?? 0;
      return Array.from({ length: Math.max(min, 1) }, () => sample(def.type, key));
    }
    case 'ZodTuple':
      return def.items.map((item) => sample(item, key));
    case 'ZodObject':
      return sampleShape(def.shape());
    case 'ZodRecord':
    case 'ZodMap':
      return {};
    case 'ZodUnion':
    case 'ZodDiscriminatedUnion':
      return sample(
        def.options instanceof Map ? [...def.options.values()][0] : def.options[0],
        key,
      );
    case 'ZodIntersection':
      return { ...sample(def.left, key), ...sample(def.right, key) };
    default:
      return 'smoke';
  }
}

function isOptional(schema) {
  const t = schema?._def?.typeName;
  return t === 'ZodOptional' || t === 'ZodDefault' || schema?.isOptional?.() === true;
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
      // A tool can succeed at the dispatch layer while its own result says it
      // failed ({ success: false, error } -- including a kernel receipt that
      // was rejected). Judge the tool by its own result.
      const inner = result?.result;
      const innerFailed = inner && typeof inner === 'object' && inner.success === false;
      const message =
        result?.error?.message ??
        (typeof result?.error === 'string' ? result.error : undefined) ??
        (typeof inner?.error === 'string' ? inner.error : inner?.error?.message) ??
        inner?.receipt?.error_message ??
        (result?.success && !innerFailed ? undefined : JSON.stringify(result).slice(0, 300));
      return { success: result?.success === true && !innerFailed, message };
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
 */
export async function runSmoke({ only = null, tools = ALL_DOMAIN_TOOLS, timeoutMs = 20_000 } = {}) {
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
  // Every tier, so any tool can be swept regardless of the default profile.
  const server = createStatesetMcpServer({ dbPath, allowApply: true, toolProfile: 'all' });
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
        message: outcome.message?.split('\n')[0]?.slice(0, 240),
      });
    }
  } finally {
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
  if (jsonAt >= 0) writeFileSync(args[jsonAt + 1], JSON.stringify(rows, null, 2));
  process.exit(0);
}
