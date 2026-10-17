import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const current = '1.39.0';
const previous = '1.37.0';
const platforms = ['@stateset/embedded-darwin-arm64', '@stateset/embedded-linux-x64-gnu'];

function runCheck(t, { deferred = false, lockedVersion = current, mutate = () => {} } = {}) {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'stateset-binding-lock-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  mkdirSync(path.join(dir, 'scripts/ci'), { recursive: true });
  mkdirSync(path.join(dir, 'bindings/node'), { recursive: true });
  copyFileSync(
    new URL('./check_node_binding_lock.mjs', import.meta.url),
    path.join(dir, 'scripts/ci/check_node_binding_lock.mjs'),
  );
  const pins = (version) => Object.fromEntries(platforms.map((name) => [name, version]));
  const manifest = {
    version: current,
    optionalDependencies: pins(current),
    peerDependencies: { '@stateset/cli': `^${current}` },
  };
  const lock = {
    version: current,
    packages: {
      '': {
        version: current,
        optionalDependencies: pins(lockedVersion),
        peerDependencies: { '@stateset/cli': `^${lockedVersion}` },
      },
      ...Object.fromEntries(
        platforms.map((name) => [`node_modules/${name}`, { version: lockedVersion }]),
      ),
    },
  };
  mutate(lock, manifest);
  writeFileSync(path.join(dir, 'bindings/node/package.json'), JSON.stringify(manifest));
  writeFileSync(path.join(dir, 'bindings/node/package-lock.json'), JSON.stringify(lock));
  writeFileSync(
    path.join(dir, 'CHANGELOG.md'),
    `## [${current}]\n\n## [${previous}]\n\n## [1.36.0]\n`,
  );
  return spawnSync(process.execPath, [path.join(dir, 'scripts/ci/check_node_binding_lock.mjs')], {
    encoding: 'utf8',
    env: { ...process.env, RELEASE_LOCK_SYNC_DEFERRED: String(deferred) },
  });
}

for (const deferred of [false, true]) {
  test(`synchronized locks pass with deferred sync ${deferred}`, (t) => {
    const result = runCheck(t, { deferred });
    assert.equal(result.status, 0, result.stderr);
    assert.doesNotMatch(result.stderr, /pending npm publication/);
  });

  test(`synchronized root cannot conceal stale resolved entries with deferred sync ${deferred}`, (t) => {
    const result = runCheck(t, {
      deferred,
      mutate: (lock) => {
        lock.packages[`node_modules/${platforms[0]}`].version = previous;
      },
    });
    assert.equal(result.status, 1);
    assert.match(result.stderr, /must not resolve an older platform package/);
  });
}

test('only explicitly deferred locks may pin the previous release', (t) => {
  const strict = runCheck(t, { lockedVersion: previous });
  assert.equal(strict.status, 1);
  assert.match(strict.stderr, /platform pins must match/);
  const deferred = runCheck(t, { lockedVersion: previous, deferred: true });
  assert.equal(deferred.status, 0, deferred.stderr);
  assert.match(deferred.stderr, /pending npm publication/);
});

test('mixed platform versions are rejected in deferred mode', (t) => {
  const result = runCheck(t, {
    deferred: true,
    mutate: (lock) => {
      lock.packages[''].optionalDependencies[platforms[0]] = previous;
    },
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /platform pins must agree/);
});

test('deferred pins must use the immediately previous release', (t) => {
  const result = runCheck(t, { deferred: true, lockedVersion: '1.36.0' });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /immediately previous release/);
});

test('a synchronized root still requires matching CLI peer metadata', (t) => {
  const result = runCheck(t, {
    deferred: true,
    mutate: (lock) => {
      lock.packages[''].peerDependencies['@stateset/cli'] = `^${previous}`;
    },
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /peer pins must match/);
});

test('missing platform metadata is rejected in deferred mode', (t) => {
  const result = runCheck(t, {
    deferred: true,
    mutate: (lock) => {
      delete lock.packages[''].optionalDependencies;
    },
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /platform pins must match/);
});

test('missing platform entries are rejected after synchronization', (t) => {
  const result = runCheck(t, {
    deferred: true,
    mutate: (lock) => {
      delete lock.packages[`node_modules/${platforms[0]}`];
    },
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /must be present in the release lockfile/);
});
