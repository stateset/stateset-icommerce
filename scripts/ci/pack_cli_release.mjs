#!/usr/bin/env node
// Bundle the verified Solana dependency tree with the CLI application. npm
// ignores dependency-owned overrides; npm 12 also ignores shrinkwrap files.
// Work in a disposable stage so the repository keeps its local native link.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const embedded = 'node_modules/@stateset/embedded';
const dependencyFields = ['dependencies', 'devDependencies', 'optionalDependencies'];
const readJson = (file) => JSON.parse(readFileSync(file, 'utf8'));
const writeJson = (file, value) => writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);

function run(command, args, cwd) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: 'utf8',
    timeout: 180_000,
    maxBuffer: 16 * 1024 * 1024,
  });
  assert.equal(
    result.status,
    0,
    `${command} ${args.join(' ')} failed:\n${result.error || ''}\n${result.stderr}\n${result.stdout}`,
  );
  return result.stdout;
}

export function registryLockSeed(lock, manifest) {
  assert.equal(lock.lockfileVersion, 3, 'Expected the reviewed npm v3 lockfile');
  assert.equal(lock.version, manifest.version, 'CLI lockfile version is stale');
  const seed = structuredClone(lock);
  const link = seed.packages[embedded];
  assert.equal(link?.link, true, 'Expected the repository native binding link');
  assert.equal(link.resolved, '../bindings/node', 'Unexpected native binding link');
  delete seed.packages[link.resolved];
  delete seed.packages[embedded];
  seed.packages[''].dependencies['@stateset/embedded'] =
    manifest.dependencies['@stateset/embedded'];
  for (const field of dependencyFields) {
    assert.deepEqual(seed.packages[''][field], manifest[field], `Stale CLI lockfile ${field}`);
  }
  return seed;
}

export function validateReleaseLock(source, release, manifest, embeddedVersion) {
  assert.equal(release.version, manifest.version);
  for (const field of dependencyFields) {
    assert.deepEqual(release.packages[''][field], manifest[field]);
  }
  assert.equal(release.packages[embedded]?.version, embeddedVersion, 'Wrong native release');
  for (const [key, entry] of Object.entries(release.packages)) {
    if (!key) continue;
    assert.ok(
      key.startsWith('node_modules/') && !key.split('/').includes('..'),
      `Local lock path: ${key}`,
    );
    assert.ok(
      !entry.link && entry.resolved?.startsWith('https://registry.npmjs.org/'),
      `Non-registry dependency: ${key}`,
    );
    assert.ok(entry.integrity, `Missing integrity: ${key}`);
  }
  // Resolving the published native binding must not silently upgrade any of
  // the reviewed CLI dependencies or weaken an override-derived resolution.
  for (const [key, entry] of Object.entries(source.packages)) {
    if (!key || key === '../bindings/node' || key === embedded) continue;
    for (const field of ['version', 'resolved', 'integrity']) {
      assert.equal(
        release.packages[key]?.[field],
        entry[field],
        `Dependency drift: ${key} ${field}`,
      );
    }
  }
}

function main() {
  const { values } = parseArgs({
    options: {
      'output-dir': { type: 'string' },
      'embedded-version': { type: 'string' },
    },
  });
  assert.ok(
    values['output-dir'],
    'Usage: pack_cli_release.mjs --output-dir DIR [--embedded-version VERSION]',
  );
  // CI can use an already-published binding before the candidate native
  // version exists. Publication omits this option and requires the exact
  // binding version from the release checkout.
  const embeddedVersion =
    values['embedded-version'] ?? readJson(path.join(root, 'bindings/node/package.json')).version;
  assert.match(
    embeddedVersion,
    /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/,
    'Expected an exact embedded version',
  );
  const output = path.resolve(values['output-dir']);
  mkdirSync(output, { recursive: true });
  const temporary = mkdtempSync(path.join(os.tmpdir(), 'stateset-cli-pack-'));
  try {
    const cli = path.join(root, 'cli');
    const [initial] = JSON.parse(
      run('npm', ['pack', '--ignore-scripts', '--json', '--pack-destination', temporary], cli),
    );
    run('tar', ['-xzf', path.join(temporary, initial.filename), '-C', temporary], temporary);
    const stage = path.join(temporary, 'package');
    const manifest = readJson(path.join(stage, 'package.json'));
    manifest.dependencies['@stateset/embedded'] = `^${embeddedVersion}`;
    manifest.bundleDependencies = ['@solana/web3.js'];
    writeJson(path.join(stage, 'package.json'), manifest);
    const source = readJson(path.join(cli, 'package-lock.json'));
    writeJson(path.join(stage, 'package-lock.json'), registryLockSeed(source, manifest));
    run(
      'npm',
      [
        'install',
        '--package-lock-only',
        '--ignore-scripts',
        '--no-audit',
        '--no-fund',
        '--save-prefix=^',
        `@stateset/embedded@${embeddedVersion}`,
      ],
      stage,
    );
    assert.deepEqual(
      readJson(path.join(stage, 'package.json')),
      manifest,
      'npm changed the release manifest',
    );
    validateReleaseLock(
      source,
      readJson(path.join(stage, 'package-lock.json')),
      manifest,
      embeddedVersion,
    );
    // Install from the validated registry lock without running native build
    // scripts. Bundled native accelerators retain their registry-provided
    // cross-platform prebuilds; no build-host binaries may enter the tarball.
    run('npm', ['ci', '--ignore-scripts', '--include=optional', '--no-audit', '--no-fund'], stage);
    const [packed] = JSON.parse(
      run('npm', ['pack', '--ignore-scripts', '--json', '--pack-destination', output], stage),
    );
    assert.ok(
      packed.files.some(({ path: name }) => name === 'node_modules/@solana/web3.js/package.json'),
      'Tarball omitted the Solana bundle',
    );
    for (const { path: name } of packed.files) {
      assert.ok(
        !name.includes('node_modules/@stateset/embedded'),
        'The native engine must resolve on the consumer platform',
      );
      assert.ok(
        !name.includes('node_modules/better-sqlite3/'),
        'SQLite must resolve on the consumer platform',
      );
      assert.ok(
        !name.endsWith('.node') || name.includes('/prebuilds/'),
        `Build-host native binary: ${name}`,
      );
    }
    console.log(
      JSON.stringify({
        tarball: path.join(output, packed.filename),
        embeddedVersion,
        integrity: packed.integrity,
      }),
    );
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
