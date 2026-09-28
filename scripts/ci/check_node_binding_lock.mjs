#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const manifest = JSON.parse(readFileSync(new URL('../../bindings/node/package.json', import.meta.url), 'utf8'));
const lock = JSON.parse(readFileSync(new URL('../../bindings/node/package-lock.json', import.meta.url), 'utf8'));
const root = lock.packages?.[''];

assert.equal(lock.version, manifest.version, 'Node binding lockfile version must match its manifest');
assert.equal(root?.version, manifest.version, 'Node binding lockfile root version must match its manifest');
assert.deepEqual(
  root.optionalDependencies,
  manifest.optionalDependencies,
  'Node binding lockfile platform pins must match its manifest',
);
assert.deepEqual(
  root.peerDependencies,
  manifest.peerDependencies,
  'Node binding lockfile peer pins must match its manifest',
);

// A release PR bumps the manifest before its platform packages exist on npm,
// so their lock entries cannot be resolved yet: they may be absent until
// `scripts/release-bump.sh --sync-locks` records the published packages. What
// must never happen is a lockfile that resolves an OLDER platform package.
const pending = [];
for (const name of Object.keys(manifest.optionalDependencies ?? {})) {
  const path = `node_modules/${name}`;
  const entry = lock.packages?.[path];
  if (!entry) {
    pending.push(name);
    continue;
  }
  assert.equal(
    entry.version,
    manifest.version,
    `${path} must not resolve an older platform package in the release lockfile`,
  );
}

if (pending.length > 0) {
  console.log(
    `Node binding lockfile pins ${manifest.version}; ${pending.length} platform package(s) are ` +
      'not locked yet (unpublished). After the npm publish lands, run ' +
      '`bash scripts/release-bump.sh --sync-locks` and merge it before any other PR.',
  );
} else {
  console.log(`Node binding lockfile matches ${manifest.version}.`);
}
