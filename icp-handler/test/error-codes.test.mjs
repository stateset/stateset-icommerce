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
  assert.ok(codes.length >= 50, `only found ${codes.length} codes — is the extraction still right?`);
  for (const expected of ['delegation.required', 'internal.transaction_failed', 'signature.invalid']) {
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
