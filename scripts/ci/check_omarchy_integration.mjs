#!/usr/bin/env node

import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const pluginDir = path.join(repoRoot, 'cli', 'omarchy');
const manifestFile = path.join(pluginDir, 'manifest.json');
const manifest = JSON.parse(fs.readFileSync(manifestFile, 'utf8'));
const cliPackage = JSON.parse(fs.readFileSync(path.join(repoRoot, 'cli', 'package.json'), 'utf8'));
const errors = [];

function check(condition, message) {
  if (!condition) errors.push(message);
}

check(manifest.schemaVersion === 1, 'manifest schemaVersion must be 1');
check(manifest.id === 'com.stateset.icommerce', 'manifest id must be com.stateset.icommerce');
check(manifest.version === cliPackage.version, 'manifest version must match cli/package.json');
check(
  Array.isArray(manifest.kinds) && manifest.kinds.includes('service'),
  'service kind is required',
);
check(
  Array.isArray(manifest.kinds) && manifest.kinds.includes('bar-widget'),
  'bar-widget kind is required',
);

for (const [kind, entryPoint] of Object.entries(manifest.entryPoints || {})) {
  check(typeof entryPoint === 'string' && entryPoint.length > 0, `${kind} entry point is invalid`);
  if (typeof entryPoint === 'string') {
    const resolved = path.resolve(pluginDir, entryPoint);
    check(
      resolved.startsWith(`${pluginDir}${path.sep}`),
      `${kind} entry point escapes plugin directory`,
    );
    check(fs.existsSync(resolved), `${kind} entry point does not exist: ${entryPoint}`);
  }
}

function inspectTree(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    const stat = fs.lstatSync(file);
    check(
      !stat.isSymbolicLink(),
      `plugin must not contain symlinks: ${path.relative(pluginDir, file)}`,
    );
    if (entry.isDirectory() && !stat.isSymbolicLink()) inspectTree(file);
  }
}
inspectTree(pluginDir);

const panel = fs.readFileSync(path.join(pluginDir, 'Panel.qml'), 'utf8');
const service = fs.readFileSync(path.join(pluginDir, 'Service.qml'), 'utf8');
check(/^Panel\s*\{/m.test(panel), 'Panel.qml must expose an Omarchy Panel root');
check(/moduleName:\s*"com\.stateset\.icommerce"/.test(panel), 'Panel.qml module id is missing');
check(/stateset-omarchy/.test(panel), 'Panel.qml must use the fixed StateSet controller');
check(
  /allowedCommands\.indexOf\(command\)/.test(panel),
  'Panel.qml must allowlist controller actions',
);
check(/command -v stateset-omarchy/.test(panel), 'Panel.qml must prefer the offline controller');
check(/root\.launch\("attention"\)/.test(panel), 'Panel.qml must expose an attention workflow');
check(/root\.launch\("remediate"\)/.test(panel), 'Panel.qml must expose safe remediation');
check(/Model\.normalizeAlerts/.test(service), 'Service.qml must normalize status alerts');
check(/notify-send/.test(service), 'Service.qml must provide desktop alert delivery');
check(
  /command -v stateset-omarchy/.test(service),
  'Service.qml must prefer the installed offline controller',
);

if (errors.length > 0) {
  for (const error of errors) console.error(`ERROR: ${error}`);
  process.exit(1);
}
console.log(`Omarchy integration check passed (${manifest.id} v${manifest.version})`);
