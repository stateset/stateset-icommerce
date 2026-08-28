#!/usr/bin/env node

import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const cli = path.join(repoRoot, 'cli', 'bin', 'stateset-omarchy.js');
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-omarchy-acceptance-'));
const home = path.join(root, 'home');
const project = path.join(root, 'storefront');
const xdgConfig = path.join(home, '.config');
const store = path.join(project, 'store.db');
const plugin = path.join(xdgConfig, 'omarchy', 'plugins', 'com.stateset.icommerce');
const menuFile = path.join(xdgConfig, 'omarchy', 'extensions', 'omarchy-menu.jsonc');
const stateSetConfig = path.join(xdgConfig, 'stateset-omarchy', 'config.json');
const env = { ...process.env, HOME: home, XDG_CONFIG_HOME: xdgConfig };

function run(args) {
  const result = spawnSync(process.execPath, [cli, ...args], {
    cwd: project,
    env,
    encoding: 'utf8',
  });
  assert.equal(
    result.status,
    0,
    `stateset-omarchy ${args.join(' ')} failed\nstdout: ${result.stdout}\nstderr: ${result.stderr}`,
  );
  return result.stdout.trim();
}

try {
  fs.mkdirSync(project, { recursive: true });
  fs.mkdirSync(path.dirname(menuFile), { recursive: true });
  fs.writeFileSync(menuFile, `${JSON.stringify({ existing: { label: 'Keep me' } })}\n`);
  fs.writeFileSync(store, '');

  const installed = run(['install', '--db', store, '--no-enable']);
  assert.match(installed, /Installed StateSet Omarchy plugin/);
  assert.equal(fs.existsSync(path.join(plugin, 'manifest.json')), true);
  assert.equal(JSON.parse(fs.readFileSync(stateSetConfig, 'utf8')).apply, false);
  assert.equal(fs.statSync(stateSetConfig).mode & 0o777, 0o600);
  assert.equal(JSON.parse(fs.readFileSync(menuFile, 'utf8')).existing.label, 'Keep me');
  assert.equal(fs.existsSync(path.join(project, '.mcp.json')), true);
  assert.equal(fs.existsSync(path.join(project, '.codex', 'config.toml')), true);
  assert.equal(fs.existsSync(path.join(project, 'opencode.json')), true);

  const status = JSON.parse(run(['status', '--db', store, '--json']));
  assert.equal(status.ok, true);
  assert.equal(status.mode, 'preview');
  assert.deepEqual(status.counts, {
    orders: 0,
    customers: 0,
    products: 0,
    returns: 0,
    payments: 0,
  });

  run(['install', '--db', store, '--force', '--no-enable']);
  assert.equal(fs.existsSync(path.join(plugin, 'Panel.qml')), true);

  const removed = run(['uninstall', '--no-disable']);
  assert.match(removed, /Removed StateSet Omarchy plugin/);
  assert.equal(fs.existsSync(plugin), false);
  assert.deepEqual(JSON.parse(fs.readFileSync(menuFile, 'utf8')), {
    existing: { label: 'Keep me' },
  });
  assert.equal(fs.existsSync(store), true);
  assert.equal(fs.existsSync(stateSetConfig), true);
  assert.equal(fs.existsSync(path.join(project, '.mcp.json')), true);

  const secondRemoval = run(['uninstall', '--no-disable']);
  assert.match(secondRemoval, /is not installed/);
  console.log('Omarchy clean-home lifecycle acceptance passed');
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}
