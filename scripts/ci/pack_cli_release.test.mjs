import assert from 'node:assert/strict';
import { test } from 'node:test';
import { registryLockSeed, validateReleaseLock } from './pack_cli_release.mjs';

function fixture() {
  const manifest = {
    version: '1.2.3',
    dependencies: { '@stateset/embedded': '^1.2.3', sdk: '^4.0.0' },
  };
  const sdk = {
    version: '4.0.1',
    resolved: 'https://registry.npmjs.org/sdk/-/sdk-4.0.1.tgz',
    integrity: 'sha512-fixture',
  };
  const source = {
    version: '1.2.3',
    lockfileVersion: 3,
    packages: {
      '': {
        dependencies: { ...manifest.dependencies, '@stateset/embedded': 'file:../bindings/node' },
      },
      '../bindings/node': { version: '1.2.3' },
      'node_modules/@stateset/embedded': { link: true, resolved: '../bindings/node' },
      'node_modules/sdk': sdk,
    },
  };
  const release = {
    version: '1.2.3',
    packages: {
      '': { dependencies: manifest.dependencies },
      'node_modules/@stateset/embedded': {
        version: '1.2.3',
        resolved: 'https://registry.npmjs.org/@stateset/embedded/-/embedded-1.2.3.tgz',
        integrity: 'sha512-native',
      },
      'node_modules/sdk': { ...sdk },
    },
  };
  return { manifest, source, release };
}

test('registry staging replaces only the native link and preserves the source lock', () => {
  const { source, manifest } = fixture();
  const original = structuredClone(source);
  const seed = registryLockSeed(source, manifest);
  assert.deepEqual(source, original);
  assert.deepEqual(seed.packages['node_modules/sdk'], source.packages['node_modules/sdk']);
  assert.equal(seed.packages['../bindings/node'], undefined);
  assert.equal(seed.packages['node_modules/@stateset/embedded'], undefined);
  assert.equal(seed.packages[''].dependencies['@stateset/embedded'], '^1.2.3');
});

test('stale source dependency declarations cannot be silently repaired during publication', () => {
  const { source, manifest } = fixture();
  manifest.dependencies.sdk = '^5.0.0';
  assert.throws(() => registryLockSeed(source, manifest), /Stale CLI lockfile/);
});

test('a registry release preserves the verified dependency graph', () => {
  const { source, release, manifest } = fixture();
  validateReleaseLock(source, release, manifest, '1.2.3');
});

for (const [name, mutate, message] of [
  [
    'version drift',
    (r) => {
      r.packages['node_modules/sdk'].version = '4.0.2';
    },
    /Dependency drift/,
  ],
  [
    'integrity drift',
    (r) => {
      r.packages['node_modules/sdk'].integrity = 'sha512-other';
    },
    /Dependency drift/,
  ],
  [
    'missing dependency',
    (r) => {
      delete r.packages['node_modules/sdk'];
    },
    /Dependency drift/,
  ],
  [
    'local dependency',
    (r) => {
      r.packages['node_modules/sdk'].resolved = 'file:../sdk';
    },
    /Non-registry/,
  ],
  [
    'linked dependency',
    (r) => {
      r.packages['node_modules/sdk'].link = true;
    },
    /Non-registry/,
  ],
  [
    'local lock path',
    (r) => {
      r.packages['../sdk'] = r.packages['node_modules/sdk'];
    },
    /Local lock path/,
  ],
  [
    'missing integrity',
    (r) => {
      delete r.packages['node_modules/sdk'].integrity;
    },
    /Missing integrity/,
  ],
  [
    'wrong native version',
    (r) => {
      r.packages['node_modules/@stateset/embedded'].version = '1.2.4';
    },
    /Wrong native release/,
  ],
]) {
  test(`publication refuses ${name}`, () => {
    const { source, release, manifest } = fixture();
    mutate(release);
    assert.throws(() => validateReleaseLock(source, release, manifest, '1.2.3'), message);
  });
}
