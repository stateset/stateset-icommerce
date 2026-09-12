#!/usr/bin/env node
// Tests for scripts/ci/set_embedded_registry_range.mjs.
//
// The repository manifest keeps `file:../bindings/node` so `npm ci` survives a
// version bump; the PUBLISHED tarball must still carry `^<version>` or every
// consumer installs a path that does not exist on their machine. These tests
// assert both halves, and the registry half is asserted on the tarball npm
// actually produces rather than on the file in the tree.

import test from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..', '..');
const script = path.join(scriptDir, 'set_embedded_registry_range.mjs');

function makeFixture(specifier = 'file:../bindings/node') {
  const root = mkdtempSync(path.join(tmpdir(), 'embedded-range-'));
  const packageDir = path.join(root, 'cli');
  mkdirSync(packageDir);
  writeFileSync(
    path.join(packageDir, 'package.json'),
    `${JSON.stringify(
      {
        name: '@stateset/cli-fixture',
        version: '1.35.1',
        private: false,
        dependencies: {
          '@stateset/embedded': specifier,
          chalk: '^5.3.0',
        },
      },
      null,
      2,
    )}\n`,
  );
  return { root, manifest: path.join(packageDir, 'package.json') };
}

function run(args, options = {}) {
  return spawnSync(process.execPath, [script, ...args], {
    encoding: 'utf8',
    cwd: options.cwd ?? repoRoot,
  });
}

test('rewrites the workspace link to the registry range', () => {
  const { root, manifest } = makeFixture();
  try {
    const result = run(['--manifest', manifest, '--version', '1.35.1']);
    assert.equal(result.status, 0, result.stderr);
    const written = JSON.parse(readFileSync(manifest, 'utf8'));
    assert.equal(written.dependencies['@stateset/embedded'], '^1.35.1');
    assert.equal(written.dependencies.chalk, '^5.3.0', 'other dependencies must be untouched');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('accepts a v-prefixed version', () => {
  const { root, manifest } = makeFixture();
  try {
    const result = run(['--manifest', manifest, '--version', 'v1.35.1']);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(
      JSON.parse(readFileSync(manifest, 'utf8')).dependencies['@stateset/embedded'],
      '^1.35.1',
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('refuses a version that is not SemVer', () => {
  const { root, manifest } = makeFixture();
  try {
    const result = run(['--manifest', manifest, '--version', 'latest']);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /non-SemVer/);
    assert.equal(
      JSON.parse(readFileSync(manifest, 'utf8')).dependencies['@stateset/embedded'],
      'file:../bindings/node',
      'a rejected version must leave the manifest alone',
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('--to-link restores the workspace link for local development', () => {
  const { root, manifest } = makeFixture('^1.35.1');
  try {
    const result = run(['--manifest', manifest, '--to-link']);
    assert.equal(result.status, 0, result.stderr);
    const specifier = JSON.parse(readFileSync(manifest, 'utf8')).dependencies['@stateset/embedded'];
    assert.ok(specifier.startsWith('file:'), `expected a file: specifier, got ${specifier}`);
    assert.ok(
      specifier.endsWith('bindings/node'),
      `expected the link to point at bindings/node, got ${specifier}`,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('is idempotent', () => {
  const { root, manifest } = makeFixture();
  try {
    run(['--manifest', manifest, '--version', '1.35.1']);
    const first = readFileSync(manifest, 'utf8');
    const result = run(['--manifest', manifest, '--version', '1.35.1']);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(readFileSync(manifest, 'utf8'), first);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('--check fails when the manifest still holds the workspace link', () => {
  const { root, manifest } = makeFixture();
  try {
    const result = run(['--manifest', manifest, '--version', '1.35.1', '--check']);
    assert.notEqual(result.status, 0);
    assert.equal(
      JSON.parse(readFileSync(manifest, 'utf8')).dependencies['@stateset/embedded'],
      'file:../bindings/node',
      '--check must not rewrite anything',
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('the published tarball carries the registry range, not the workspace link', () => {
  const { root, manifest } = makeFixture();
  const packageDir = path.dirname(manifest);
  try {
    const rewrite = run(['--manifest', manifest, '--version', '1.35.1']);
    assert.equal(rewrite.status, 0, rewrite.stderr);

    const packed = spawnSync('npm', ['pack', '--json', '--pack-destination', root], {
      cwd: packageDir,
      encoding: 'utf8',
    });
    assert.equal(packed.status, 0, packed.stderr || packed.stdout);
    const [{ filename }] = JSON.parse(packed.stdout);

    const extractDir = path.join(root, 'extracted');
    mkdirSync(extractDir, { recursive: true });
    const extracted = spawnSync('tar', ['-xzf', path.join(root, filename), '-C', extractDir], {
      encoding: 'utf8',
    });
    assert.equal(extracted.status, 0, extracted.stderr);

    const publishedManifest = JSON.parse(
      readFileSync(path.join(extractDir, 'package', 'package.json'), 'utf8'),
    );
    assert.equal(
      publishedManifest.dependencies['@stateset/embedded'],
      '^1.35.1',
      'a consumer installing the tarball must resolve @stateset/embedded from the registry',
    );
    assert.ok(
      !JSON.stringify(publishedManifest).includes('file:'),
      'no file: specifier may reach the published tarball',
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('the repository manifests keep the workspace link', () => {
  for (const relative of ['cli/package.json', 'admin/package.json', 'examples/node/package.json']) {
    const manifest = JSON.parse(readFileSync(path.join(repoRoot, relative), 'utf8'));
    const specifier = manifest.dependencies?.['@stateset/embedded'];
    assert.ok(
      typeof specifier === 'string' && specifier.startsWith('file:'),
      `${relative} must link the workspace copy so a version bump cannot break npm ci (got ${specifier})`,
    );
  }
});
