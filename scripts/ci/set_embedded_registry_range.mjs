#!/usr/bin/env node
// Swap the @stateset/embedded dependency between the local workspace link and
// the published registry range.
//
//   node scripts/ci/set_embedded_registry_range.mjs [--manifest PATH] [--version V]
//   node scripts/ci/set_embedded_registry_range.mjs --to-link
//
// In the repository, cli/package.json depends on `file:../bindings/node` (as
// admin/package.json already did). That is the only form that survives a
// release: a `^1.35.1` range plus a lockfile that links the workspace copy
// makes `npm ci` fail on every branch from the moment the version is bumped
// until a lockfile-sync PR lands, which is what broke master after 1.31, 1.32
// and 1.33.
//
// The published tarball must still declare the registry range, because a
// consumer has no ../bindings/node. publish-cli.yml runs this script after
// `npm ci` and before `npm publish` to rewrite the manifest in the checkout
// only; nothing is committed, and package-lock.json is never published.

import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const DEPENDENCY = '@stateset/embedded';
const DEPENDENCY_FIELDS = ['dependencies', 'devDependencies', 'optionalDependencies'];

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..', '..');
const bindingDir = path.join(repoRoot, 'bindings', 'node');

function usage() {
  console.log(
    [
      'Usage: set_embedded_registry_range.mjs [options]',
      '',
      `Rewrites the ${DEPENDENCY} dependency of a package manifest.`,
      '',
      'Options:',
      '  --manifest PATH   Manifest to rewrite (default: cli/package.json).',
      '  --version V       Registry version to pin (default: the version in',
      '                    bindings/node/package.json).',
      '  --to-link         Inverse: point the dependency back at the workspace',
      '                    copy (file:<relative path to bindings/node>).',
      '  --check           Report the current value and exit non-zero if the',
      '                    requested rewrite would change anything.',
      '  -h, --help        Show this help message.',
    ].join('\n'),
  );
}

function parseArgs(argv) {
  const options = {
    manifest: path.join(repoRoot, 'cli', 'package.json'),
    version: null,
    toLink: false,
    check: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--manifest') {
      const value = argv[index + 1];
      if (!value) throw new Error('missing value for --manifest');
      options.manifest = path.resolve(process.cwd(), value);
      index += 1;
    } else if (arg === '--version') {
      const value = argv[index + 1];
      if (!value) throw new Error('missing value for --version');
      options.version = value.replace(/^v/, '');
      index += 1;
    } else if (arg === '--to-link') {
      options.toLink = true;
    } else if (arg === '--check') {
      options.check = true;
    } else if (arg === '-h' || arg === '--help') {
      usage();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  return options;
}

function readJson(file) {
  return JSON.parse(readFileSync(file, 'utf8'));
}

function linkSpecifier(manifestPath) {
  const relative = path.relative(path.dirname(manifestPath), bindingDir);
  return `file:${relative.split(path.sep).join('/')}`;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const manifest = readJson(options.manifest);

  const field = DEPENDENCY_FIELDS.find(
    (candidate) => manifest[candidate] && DEPENDENCY in manifest[candidate],
  );

  if (!field) {
    console.error(
      `::error file=${options.manifest}::${DEPENDENCY} is not a dependency of this manifest`,
    );
    process.exit(1);
  }

  const current = manifest[field][DEPENDENCY];

  let next;
  if (options.toLink) {
    next = linkSpecifier(options.manifest);
  } else {
    const version = options.version ?? readJson(path.join(bindingDir, 'package.json')).version;
    if (!/^\d+\.\d+\.\d+/.test(version)) {
      console.error(`::error::refusing to pin a non-SemVer version: ${version}`);
      process.exit(1);
    }
    next = `^${version}`;
  }

  if (current === next) {
    console.log(`${DEPENDENCY} already "${next}" in ${path.relative(repoRoot, options.manifest)}`);
    return;
  }

  if (options.check) {
    console.error(
      `::error file=${options.manifest}::${DEPENDENCY} is "${current}", expected "${next}"`,
    );
    process.exit(1);
  }

  manifest[field][DEPENDENCY] = next;
  writeFileSync(options.manifest, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(
    `${DEPENDENCY}: "${current}" -> "${next}" in ${path.relative(repoRoot, options.manifest)}`,
  );
}

try {
  main();
} catch (error) {
  console.error(`::error::${error.message}`);
  process.exit(1);
}
