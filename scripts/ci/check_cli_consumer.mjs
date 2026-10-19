#!/usr/bin/env node
// Install the actual release tarball as a dependency of an unrelated project.
// No workspace links, root overrides, injected binaries or skipped scripts.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

assert.equal(process.argv.length, 3, 'Usage: check_cli_consumer.mjs CLI_TARBALL');
const tarball = path.resolve(process.argv[2]);
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const consumer = mkdtempSync(path.join(os.tmpdir(), 'stateset-cli-consumer-'));
const env = { ...process.env, NODE_PATH: '' };
function run(command, args, timeout = 300_000) {
  const result = spawnSync(command, args, {
    cwd: consumer,
    env,
    encoding: 'utf8',
    timeout,
    maxBuffer: 16 * 1024 * 1024,
  });
  assert.equal(
    result.status,
    0,
    `${command} ${args.join(' ')} failed:\n${result.error || ''}\n${result.stderr}\n${result.stdout}`,
  );
  return result.stdout;
}

try {
  writeFileSync(
    path.join(consumer, 'package.json'),
    JSON.stringify({ name: 'cli-consumer-acceptance', version: '1.0.0', private: true }),
  );
  console.log(
    run('npm', [
      'install',
      '--include=optional',
      '--ignore-scripts=false',
      '--no-audit',
      '--no-fund',
      tarball,
    ]),
  );
  const cliDir = path.join(consumer, 'node_modules/@stateset/cli');
  const manifestPath = path.join(cliDir, 'package.json');
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  assert.deepEqual(
    manifest.bundleDependencies,
    ['@solana/web3.js'],
    'Installed CLI must carry the verified Solana bundle',
  );
  const require = createRequire(manifestPath);
  for (const name of Object.keys(manifest.optionalDependencies)) {
    assert.ok(require.resolve(name), `Optional integration missing: ${name}`);
  }
  console.log(
    run(process.execPath, [
      path.join(root, 'cli/scripts/check-optional-dependency-security.mjs'),
      manifestPath,
    ]),
  );
  // Load both native modules from the installed tree. Optional install failures
  // must not turn this acceptance gate into a misleading successful install.
  const Database = require('better-sqlite3');
  const database = new Database(':memory:');
  try {
    assert.equal(database.prepare('SELECT 42 AS answer').get().answer, 42);
  } finally {
    database.close();
  }
  const { Commerce } = require('@stateset/embedded');
  const commerce = new Commerce(':memory:');
  const customer = await commerce.customers.create({
    email: 'consumer@example.com',
    firstName: 'Consumer',
    lastName: 'Fixture',
  });
  assert.equal((await commerce.customers.get(customer.id)).email, 'consumer@example.com');
  const version = run(
    process.execPath,
    [path.join(cliDir, 'bin/stateset.js'), '--version'],
    30_000,
  );
  assert.ok(version.includes(manifest.version), 'Installed CLI reports the wrong version');
  assert.match(
    run(process.execPath, [path.join(cliDir, 'bin/stateset-simulate.js'), '--help'], 30_000),
    /usage/i,
  );
  // README links and runnable examples must survive both release pack stages.
  const readme = readFileSync(path.join(cliDir, 'README.md'), 'utf8');
  for (const [, example] of readme.matchAll(/\]\((examples\/[^\s)#]+\.json)\)/g)) {
    JSON.parse(readFileSync(path.join(cliDir, example), 'utf8'));
  }
  for (const example of ['commerce', 'inventory', 'experiment']) {
    const report = JSON.parse(
      run(
        process.execPath,
        [path.join(cliDir, 'bin/stateset-simulate.js'), '--example', example, '--json'],
        45_000,
      ),
    );
    assert.equal(report.success, true, `Installed ${example} example failed`);
    assert.equal(report.liveStoreModified, false);
    assert.ok(report.results?.every((result) => result.expectationsMet !== false) ?? true);
  }
  console.log(run('npm', ['audit', '--audit-level=high']));
  console.log(
    'Packed CLI consumer install, native engines, optional integrations and security audit: passed',
  );
} finally {
  rmSync(consumer, { recursive: true, force: true });
}
