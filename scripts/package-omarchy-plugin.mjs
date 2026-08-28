#!/usr/bin/env node

import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const sourceDir = path.join(repoRoot, 'cli', 'omarchy');
const repositoryTemplateDir = path.join(
  repoRoot,
  'scripts',
  'templates',
  'omarchy-plugin-repository',
);
const args = process.argv.slice(2);

function option(name) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : null;
}

if (args.includes('--help') || args.includes('-h')) {
  console.log('Usage: npm run package:omarchy-plugin -- [--output PATH] [--force]');
  process.exit(0);
}

const outputDir = path.resolve(
  option('--output') || path.join(repoRoot, 'dist', 'stateset-omarchy-plugin'),
);
if (outputDir === sourceDir || sourceDir.startsWith(`${outputDir}${path.sep}`)) {
  throw new Error(`Output must not contain the bundled plugin source: ${outputDir}`);
}
if (fs.existsSync(outputDir)) {
  if (!args.includes('--force'))
    throw new Error(`Output already exists: ${outputDir} (use --force)`);
  fs.rmSync(outputDir, { recursive: true, force: true });
}

const cliPackage = JSON.parse(fs.readFileSync(path.join(repoRoot, 'cli', 'package.json'), 'utf8'));
const manifest = JSON.parse(fs.readFileSync(path.join(sourceDir, 'manifest.json'), 'utf8'));
if (manifest.version !== cliPackage.version) {
  throw new Error(
    `Plugin version ${manifest.version} does not match CLI version ${cliPackage.version}`,
  );
}

fs.mkdirSync(path.dirname(outputDir), { recursive: true });
fs.cpSync(sourceDir, outputDir, { recursive: true, errorOnExist: true });
fs.cpSync(repositoryTemplateDir, outputDir, { recursive: true });
for (const license of ['LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE']) {
  fs.copyFileSync(path.join(repoRoot, license), path.join(outputDir, license));
}
console.log(outputDir);
