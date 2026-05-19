// Drift guard: every `export` in `src/index.mjs` must have a matching
// declaration in `src/index.d.ts`. If a new SDK helper is added without
// updating the types, this test fails — keeping the TypeScript surface
// honest for partner consumers.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PKG = join(__dirname, '..');

function exportedNames(source) {
  // Match: `export class X`, `export function x`, `export const X = `, etc.
  const names = new Set();
  const patterns = [
    /^export\s+class\s+([A-Za-z_$][\w$]*)\b/gm,
    /^export\s+(?:async\s+)?function\s+\*?\s*([A-Za-z_$][\w$]*)\b/gm,
    /^export\s+const\s+([A-Za-z_$][\w$]*)\b/gm,
    /^export\s+let\s+([A-Za-z_$][\w$]*)\b/gm,
  ];
  for (const re of patterns) {
    let m;
    while ((m = re.exec(source)) !== null) names.add(m[1]);
  }
  return names;
}

function declaredNames(source) {
  // Match `.d.ts` declarations: `export class X`, `export function x`,
  // `export interface X`, `export type X`, `export const X`.
  const names = new Set();
  const patterns = [
    /^export\s+(?:declare\s+)?class\s+([A-Za-z_$][\w$]*)\b/gm,
    /^export\s+(?:declare\s+)?function\s+([A-Za-z_$][\w$]*)\b/gm,
    /^export\s+interface\s+([A-Za-z_$][\w$]*)\b/gm,
    /^export\s+type\s+([A-Za-z_$][\w$]*)\b/gm,
    /^export\s+(?:declare\s+)?const\s+([A-Za-z_$][\w$]*)\b/gm,
  ];
  for (const re of patterns) {
    let m;
    while ((m = re.exec(source)) !== null) names.add(m[1]);
  }
  return names;
}

test('every JS export has a matching .d.ts declaration', () => {
  const js = readFileSync(join(PKG, 'src/index.mjs'), 'utf8');
  const dts = readFileSync(join(PKG, 'src/index.d.ts'), 'utf8');
  const exported = exportedNames(js);
  const declared = declaredNames(dts);
  const missing = [...exported].filter((n) => !declared.has(n));
  assert.deepEqual(
    missing,
    [],
    `JS exports lacking .d.ts declarations: ${missing.join(', ')}`,
  );
});

test('package.json points at the .d.ts via `types` and `exports.types`', () => {
  const pkg = JSON.parse(readFileSync(join(PKG, 'package.json'), 'utf8'));
  assert.equal(pkg.types, './src/index.d.ts', 'package.json `types` field');
  const dot = pkg.exports?.['.'];
  assert.ok(dot, 'package.json must have `exports["."]`');
  assert.equal(dot.types, './src/index.d.ts', 'exports["."].types');
  // exports["."].types must precede import in object insertion order so
  // TypeScript's conditional-export resolver picks the .d.ts first.
  const keys = Object.keys(dot);
  assert.equal(keys[0], 'types', 'exports["."].types must be the first conditional key');
});

test('.d.ts declares every key runtime artifact (ICPClient, verifyWebhook, ICPError, …)', () => {
  const dts = readFileSync(join(PKG, 'src/index.d.ts'), 'utf8');
  for (const name of [
    'ICPClient',
    'ICPError',
    'Identity',
    'Money',
    'Signature',
    'EventEnvelope',
    'EventType',
    'VerifyWebhookOptions',
    'RegisterWebhookOpts',
    'FetchChannelEventsOpts',
    'generateIdentity',
    'identityFromSeeds',
    'canonicalJson',
    'signEd25519',
    'verifyEd25519',
    'verifyWebhook',
  ]) {
    assert.match(
      dts,
      new RegExp(`export\\s+(class|function|interface|type|const)\\s+${name}\\b`),
      `.d.ts missing declaration for: ${name}`,
    );
  }
});
