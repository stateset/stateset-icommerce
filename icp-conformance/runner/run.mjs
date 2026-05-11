#!/usr/bin/env node
// ICP Conformance Runner
//
// Loads test vectors from `vectors/`, invokes the IUT adapter from `iut-adapters/`,
// compares stdout JSON to expected.json. Prints a pass/skip/fail report.
//
// Usage:
//   node runner/run.mjs [--profile <name>] [--iut <name>] [--vector <name>] [--verbose]
//
// Defaults:
//   --profile icp-1.0-core
//   --iut reference-demo
//
// Exit codes:
//   0 — all selected tests pass
//   1 — at least one test failed
//   2 — runner error (vector not found, adapter not found, etc.)

import { readFileSync } from 'node:fs';
import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');

const args = parseArgs(process.argv.slice(2));
const profileName = args.profile ?? 'icp-1.0-core';
const iutName = args.iut ?? 'reference-demo';
const onlyVector = args.vector ?? null;
const verbose = args.verbose === true;

const profile = loadJson(join(ROOT, 'profiles', `${profileName}.json`));
const registry = loadJson(join(ROOT, 'iut-adapters', 'registry.json'));

const iut = registry[iutName];
if (!iut) {
  fatal(`unknown IUT '${iutName}'. Known IUTs: ${Object.keys(registry).join(', ')}`);
}
if (!iut.supports.includes(profileName)) {
  fatal(`IUT '${iutName}' does not support profile '${profileName}'. Supported: ${iut.supports.join(', ')}`);
}

const vectors = onlyVector ? [onlyVector] : profile.vectors;

console.log(`ICP Conformance Runner — profile=${profileName} iut=${iutName} vectors=${vectors.length}`);
console.log(`Implementation: ${iut.implementation}`);
console.log('');

let passCount = 0;
let failCount = 0;
let skipCount = 0;

for (const vectorName of vectors) {
  const result = runVector(vectorName, iut);
  printResult(vectorName, result);
  if (result.kind === 'pass') passCount++;
  else if (result.kind === 'skip') skipCount++;
  else failCount++;
}

console.log('');
console.log(`Result: ${passCount} PASS, ${failCount} FAIL, ${skipCount} SKIP (of ${vectors.length} total)`);
process.exit(failCount > 0 ? 1 : 0);

// ===========================================================================
// Vector execution
// ===========================================================================

function runVector(vectorName, iut) {
  const dir = join(ROOT, 'vectors', profile.spec_version, vectorName);
  if (!existsSync(dir)) {
    return { kind: 'fail', reason: `vector directory not found: ${dir}` };
  }

  const inputs = loadJson(join(dir, 'inputs.json'));
  const expected = loadJson(join(dir, 'expected.json'));

  const [cmd, ...argv] = iut.command;
  const child = spawnSync(cmd, [...argv, vectorName], {
    cwd: ROOT,
    input: JSON.stringify(inputs),
    encoding: 'utf8',
    timeout: 30000,
  });

  if (child.status === 2) {
    let reason = 'unsupported';
    try {
      const e = JSON.parse(child.stderr);
      if (e.reason) reason = e.reason;
    } catch (_) {}
    return { kind: 'skip', reason };
  }

  if (child.status !== 0) {
    return { kind: 'fail', reason: `adapter exited with code ${child.status}`, stderr: child.stderr };
  }

  let actual;
  try {
    actual = JSON.parse(child.stdout);
  } catch (err) {
    return { kind: 'fail', reason: `adapter stdout is not JSON: ${err.message}`, stdout: child.stdout };
  }

  const checks = compareExpected(expected, actual);
  if (checks.failed.length === 0) {
    return { kind: 'pass', checks: checks.passed };
  }
  return { kind: 'fail', reason: 'output divergence', checks: checks.failed };
}

// Compare every key in `expected` (skipping ones starting with _) against actual.
// Extra keys in actual are ignored. Missing keys in actual are a FAIL.
function compareExpected(expected, actual) {
  const passed = [];
  const failed = [];
  for (const key of Object.keys(expected)) {
    if (key.startsWith('_')) continue; // metadata keys
    const want = expected[key];
    const got = actual[key];
    if (deepEqual(want, got)) {
      passed.push({ key, value: want });
    } else {
      failed.push({ key, expected: want, actual: got });
    }
  }
  return { passed, failed };
}

function deepEqual(a, b) {
  if (a === b) return true;
  if (typeof a !== typeof b) return false;
  if (a === null || b === null) return false;
  if (typeof a !== 'object') return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (Array.isArray(a)) {
    if (a.length !== b.length) return false;
    return a.every((v, i) => deepEqual(v, b[i]));
  }
  const ak = Object.keys(a).sort();
  const bk = Object.keys(b).sort();
  if (ak.length !== bk.length) return false;
  if (!ak.every((k, i) => k === bk[i])) return false;
  return ak.every((k) => deepEqual(a[k], b[k]));
}

// ===========================================================================
// Reporting
// ===========================================================================

function printResult(vectorName, result) {
  if (result.kind === 'pass') {
    console.log(`[${vectorName}] PASS — ${result.checks.length} field${result.checks.length === 1 ? '' : 's'} match expected`);
    if (verbose) {
      for (const c of result.checks) {
        const v = typeof c.value === 'string' && c.value.length > 60
          ? `${c.value.slice(0, 60)}…(${c.value.length} chars)`
          : JSON.stringify(c.value);
        console.log(`           · ${c.key} = ${v}`);
      }
    }
  } else if (result.kind === 'skip') {
    console.log(`[${vectorName}] SKIP — ${result.reason}`);
  } else {
    console.log(`[${vectorName}] FAIL — ${result.reason}`);
    if (result.checks) {
      for (const c of result.checks) {
        console.log(`           · ${c.key}`);
        console.log(`             expected: ${truncate(JSON.stringify(c.expected), 100)}`);
        console.log(`             actual:   ${truncate(JSON.stringify(c.actual), 100)}`);
      }
    }
    if (result.stderr) console.log(`           stderr: ${truncate(result.stderr, 200)}`);
    if (result.stdout) console.log(`           stdout: ${truncate(result.stdout, 200)}`);
  }
}

function truncate(s, n) {
  if (!s) return '';
  return s.length > n ? `${s.slice(0, n)}…(${s.length} chars)` : s;
}

// ===========================================================================
// Util
// ===========================================================================

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--verbose' || arg === '-v') out.verbose = true;
    else if (arg.startsWith('--')) {
      const key = arg.slice(2);
      const next = argv[i + 1];
      if (next && !next.startsWith('--')) {
        out[key] = next;
        i++;
      } else {
        out[key] = true;
      }
    }
  }
  return out;
}

function loadJson(path) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'));
  } catch (err) {
    fatal(`could not read ${path}: ${err.message}`);
  }
}

function fatal(msg) {
  process.stderr.write(`runner: ${msg}\n`);
  process.exit(2);
}
