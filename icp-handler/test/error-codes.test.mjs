// Every error code this handler emits must be in the normative registry.
//
// `icp-spec/schemas/error-codes.md` is what a counterparty branches on: the
// spec says implementations MUST use one of the codes below it, and that
// callers MAY rely on `code` for programmatic branching. It had drifted — nine
// codes the handler returns in production paths (`internal.transaction_failed`,
// `inventory.insufficient`, `replay.intent_seen`, both `settler.key_*`,
// `channel.durable_delivery_unavailable`, three `format.*`) appeared nowhere in
// it, and two whole namespaces were undeclared. A code a caller cannot look up
// is a code a caller cannot handle.
//
// Nothing checked it, so nothing stopped it. This does.
//
// Run: node --test test/error-codes.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const srcDir = join(here, '..', 'src');
const registryPath = join(here, '..', '..', 'icp-spec', 'schemas', 'error-codes.md');

/** Every `<namespace>.<specific>` this handler can put in an `icp.error`. */
function emittedCodes() {
  const codes = new Set();
  for (const file of readdirSync(srcDir).filter((f) => f.endsWith('.mjs'))) {
    const source = readFileSync(join(srcDir, file), 'utf8');
    // Two spellings reach the wire: `err('code', message)` in server.mjs, and
    // the literal `{ type: 'icp.error', code: '…' }` the backend stub returns.
    for (const m of source.matchAll(/\berr\(\s*'([a-z][a-z0-9_]*(?:\.[a-z0-9_]+)+)'/g)) {
      codes.add(m[1]);
    }
    for (const m of source.matchAll(/\bcode:\s*'([a-z][a-z0-9_]*(?:\.[a-z0-9_]+)+)'/g)) {
      codes.add(m[1]);
    }
  }
  return [...codes].sort();
}

const registry = readFileSync(registryPath, 'utf8');

test('the handler emits at least the codes this test knows about', () => {
  // Guards the extraction itself: a refactor that stops matching (say, every
  // `err()` call moves behind a constant) would otherwise make this file pass
  // by finding nothing.
  const codes = emittedCodes();
  assert.ok(
    codes.length >= 50,
    `only found ${codes.length} codes — is the extraction still right?`,
  );
  for (const expected of [
    'delegation.required',
    'internal.transaction_failed',
    'signature.invalid',
  ]) {
    assert.ok(codes.includes(expected), `expected to find ${expected} among the emitted codes`);
  }
});

test('every emitted code is registered in icp-spec/schemas/error-codes.md', () => {
  const missing = emittedCodes().filter((code) => !registry.includes(`\`${code}\``));
  assert.deepEqual(
    missing,
    [],
    `unregistered error codes — add them to icp-spec/schemas/error-codes.md with a one-line meaning:\n  ${missing.join('\n  ')}`,
  );
});

test('every emitted namespace is declared in the Namespaces table', () => {
  const table = registry.slice(registry.indexOf('## Namespaces'), registry.indexOf('## Codes'));
  const namespaces = [...new Set(emittedCodes().map((c) => c.split('.')[0]))].sort();
  const undeclared = namespaces.filter((ns) => !table.includes(`\`${ns}\``));
  assert.deepEqual(
    undeclared,
    [],
    `namespaces emitted but never declared: ${undeclared.join(', ')}`,
  );
});

// ---------------------------------------------------------------------------
// Documented status vs returned status
// ---------------------------------------------------------------------------
//
// A registry row tells a caller what a code *means*; the mapping table tells
// it what to branch on over HTTP. Both had drifted from the handler:
// `replay.intent_seen` is answered 409 while the table said `replay.*` is
// 400/410, and `channel.durable_delivery_unavailable` is answered 503 while
// the `channel.*` row listed neither. A caller that trusted the table would
// have treated a conflict as a malformed request and retried a 503 as fatal.
//
// So derive the truth instead of restating it: every `reply(res, <status>,
// err('<code>', …))` and every `{ status: <status>, body: err('<code>', …) }`
// in the handler is a (code, status) pair, and each pair must be one the
// table allows.

const serverSource = readFileSync(join(srcDir, 'server.mjs'), 'utf8');

/** `{ code: Set<status> }` — the statuses the handler actually replies with. */
function returnedStatuses() {
  const byCode = new Map();
  const add = (code, status) => {
    if (!byCode.has(code)) byCode.set(code, new Set());
    byCode.get(code).add(status);
  };
  const CODE = "'([a-z][a-z0-9_]*(?:\\.[a-z0-9_]+)+)'";
  for (const m of serverSource.matchAll(
    new RegExp(`\\breply\\(\\s*res\\s*,\\s*(\\d{3})\\s*,\\s*err\\(\\s*${CODE}`, 'g'),
  )) {
    add(m[2], m[1]);
  }
  for (const m of serverSource.matchAll(
    new RegExp(`\\bstatus:\\s*(\\d{3})\\s*,\\s*body:\\s*err\\(\\s*${CODE}`, 'g'),
  )) {
    add(m[2], m[1]);
  }
  return byCode;
}

/**
 * The statuses the mapping table allows for `code`.
 *
 * A cell is `;`-separated clauses. A clause naming codes in backticks (a
 * trailing `*` globs) applies only to those codes; a clause naming none is the
 * namespace default. Every three-digit number in an applicable clause counts.
 */
function documentedStatuses(code) {
  const namespace = code.split('.')[0];
  const table = registry.slice(registry.indexOf('## HTTP status mapping'));
  const row = new RegExp(`^\\|\\s*\`${namespace}\\.\\*\`\\s*\\|(.+?)\\|\\s*$`, 'm').exec(table);
  if (!row) return null;
  const allowed = new Set();
  for (const clause of row[1].split(';')) {
    const scoped = [...clause.matchAll(/`([a-z][a-z0-9_.]*\*?)`/g)].map((m) => m[1]);
    const applies =
      scoped.length === 0 ||
      scoped.some((pattern) =>
        new RegExp(`^${pattern.replace(/[.]/g, '\\.').replace(/\*/g, '.*')}$`).test(code),
      );
    if (!applies) continue;
    for (const status of clause.matchAll(/\b(\d{3})\b/g)) allowed.add(status[1]);
  }
  return allowed;
}

test('the status extraction actually sees the handler replying', () => {
  // Same guard as above: a refactor that stops matching would make the
  // mapping assertion below pass by checking nothing.
  const returned = returnedStatuses();
  assert.ok(
    returned.size >= 30,
    `only derived statuses for ${returned.size} codes — has the reply() shape changed?`,
  );
  // The two the registry had wrong, pinned by name so a future edit to the
  // table cannot quietly re-break them.
  assert.deepEqual([...(returned.get('replay.intent_seen') ?? [])], ['409']);
  assert.deepEqual([...(returned.get('channel.durable_delivery_unavailable') ?? [])], ['503']);
});

test('every status the handler returns is one the mapping table documents', () => {
  const wrong = [];
  for (const [code, statuses] of returnedStatuses()) {
    const allowed = documentedStatuses(code);
    if (allowed === null) {
      wrong.push(`${code}: no \`${code.split('.')[0]}.*\` row in the HTTP status mapping table`);
      continue;
    }
    for (const status of statuses) {
      if (!allowed.has(status)) {
        wrong.push(
          `${code}: handler replies ${status}, table allows ${[...allowed].sort().join('/') || '(nothing)'}`,
        );
      }
    }
  }
  assert.deepEqual(
    wrong,
    [],
    `HTTP status mapping in icp-spec/schemas/error-codes.md disagrees with the handler:\n  ${wrong.join('\n  ')}`,
  );
});
