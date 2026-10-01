#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const manifest = JSON.parse(readFileSync(new URL('../../bindings/node/package.json', import.meta.url), 'utf8'));
const lock = JSON.parse(readFileSync(new URL('../../bindings/node/package-lock.json', import.meta.url), 'utf8'));
const root = lock.packages?.[''];
const changelog = readFileSync(new URL('../../CHANGELOG.md', import.meta.url), 'utf8');

const pendingReleaseLock = process.env.RELEASE_LOCK_SYNC_DEFERRED === 'true';

function parseVersion(version, label) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version ?? '');
  assert.ok(match, `${label} must be a plain SemVer (got ${version ?? '<missing>'})`);
  return match.slice(1).map(Number);
}

function isOlderVersion(candidate, current) {
  const left = parseVersion(candidate, 'Pending lock version');
  const right = parseVersion(current, 'Manifest version');
  return left.some((part, index) => part !== right[index])
    ? left[0] < right[0] ||
        (left[0] === right[0] &&
          (left[1] < right[1] || (left[1] === right[1] && left[2] < right[2])))
    : false;
}

assert.equal(lock.version, manifest.version, 'Node binding lockfile version must match its manifest');
assert.equal(root?.version, manifest.version, 'Node binding lockfile root version must match its manifest');

if (pendingReleaseLock) {
  // release-bump intentionally keeps registry package pins at the last
  // published version. Updating them before npm publish makes npm ci fetch a
  // package that does not exist yet. The release hygiene invocation is the
  // only caller allowed to accept this explicitly named intermediate state.
  const platformNames = Object.keys(manifest.optionalDependencies ?? {}).sort();
  const lockedPlatformNames = Object.keys(root?.optionalDependencies ?? {}).sort();
  assert.deepEqual(
    lockedPlatformNames,
    platformNames,
    'Pending Node binding lock must contain exactly the manifest platform packages',
  );
  const lockedVersions = new Set(platformNames.map((name) => root.optionalDependencies[name]));
  assert.equal(lockedVersions.size, 1, 'Pending Node binding platform pins must agree');
  const [lockedVersion] = lockedVersions;
  assert.ok(
    lockedVersion !== manifest.version && isOlderVersion(lockedVersion, manifest.version),
    `Pending Node binding lock must use one older published version (got ${lockedVersion})`,
  );
  const releasedVersions = [...changelog.matchAll(/^## \[(\d+\.\d+\.\d+)\]/gm)].map(
    ([, version]) => version,
  );
  const previousPublishedVersion = releasedVersions.find((version) => version !== manifest.version);
  assert.equal(
    lockedVersion,
    previousPublishedVersion,
    `Pending Node binding lock must pin the immediately previous release (${previousPublishedVersion ?? '<missing>'})`,
  );
  assert.equal(
    root.peerDependencies?.['@stateset/cli'],
    `^${lockedVersion}`,
    'Pending Node binding lock peer pin must match its platform package version',
  );
  for (const name of platformNames) {
    const entry = lock.packages?.[`node_modules/${name}`];
    assert.ok(entry, `node_modules/${name} must be present in the pending release lockfile`);
    assert.equal(
      entry.version,
      lockedVersion,
      `node_modules/${name} must match the pending platform package version`,
    );
  }
  console.warn(
    `Node binding lockfile is pending npm publication: ${lockedVersion} -> ${manifest.version}. ` +
      'Run release-bump.sh --sync-locks after the platform packages publish.',
  );
} else {
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
}

for (const name of Object.keys(manifest.optionalDependencies ?? {})) {
  const path = `node_modules/${name}`;
  const entry = lock.packages?.[path];
  assert.ok(entry, `${path} must be present in the release lockfile for npm ci`);
  if (!pendingReleaseLock) {
    assert.equal(
      entry.version,
      manifest.version,
      `${path} must not resolve an older platform package in the release lockfile`,
    );
  }
}

console.log(`Node binding lockfile matches ${manifest.version}.`);
