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

for (const [path, entry] of Object.entries(lock.packages)) {
  if (path.startsWith('node_modules/@stateset/embedded-')) {
    assert.equal(
      entry.version,
      manifest.version,
      `${path} must not resolve an older platform package in the release lockfile`,
    );
  }
}

console.log(`Node binding lockfile matches ${manifest.version}.`);
