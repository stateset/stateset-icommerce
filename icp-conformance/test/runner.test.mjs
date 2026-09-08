// Runner behaviour tests.
//
// The conformance runner is itself a gate: CI trusts its exit code. Two
// properties matter and neither was covered before:
//
//   1. A SKIP (adapter exits 2 — "no handler for this vector") must be able
//      to fail the build. Without that, an IUT can claim a profile in
//      `registry.json`, implement none of it, and every CI leg still goes
//      green: `skipped > 0` was counted and then ignored.
//   2. The registry must be overridable so a suite can be run against an
//      out-of-tree adapter (which is also what lets this file test #1
//      without adding a fake IUT to the shipped registry).

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync, chmodSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const RUNNER = join(ROOT, 'runner', 'run.mjs');

/**
 * Build a temp registry whose single IUT is a stub adapter with the given
 * behaviour, and return the registry path plus a cleanup function.
 *
 * @param {'skip'|'pass'} mode
 */
function stubRegistry(mode) {
  const dir = mkdtempSync(join(tmpdir(), 'icp-runner-test-'));
  const adapter = join(dir, 'adapter.mjs');

  if (mode === 'skip') {
    // Exit code 2 is the protocol's "no handler → SKIP" signal
    // (iut-adapters/iut.protocol.md).
    writeFileSync(
      adapter,
      "process.stderr.write(JSON.stringify({ reason: 'no handler in stub' }));\nprocess.exit(2);\n",
    );
  } else {
    // Emit exactly the reference-demo's expected.json so the vector passes.
    writeFileSync(
      adapter,
      [
        "import { readFileSync } from 'node:fs';",
        `const expected = JSON.parse(readFileSync(${JSON.stringify(
          join(ROOT, 'vectors', 'icp-1.0', '01-aid-derivation', 'expected.json'),
        )}, 'utf8'));`,
        'for (const k of Object.keys(expected)) if (k.startsWith("_")) delete expected[k];',
        'process.stdout.write(JSON.stringify(expected));',
      ].join('\n'),
    );
  }
  chmodSync(adapter, 0o644);

  const registry = join(dir, 'registry.json');
  writeFileSync(
    registry,
    JSON.stringify({
      stub: {
        command: [process.execPath, adapter],
        language: 'javascript',
        implementation: `stub adapter (${mode})`,
        version: '0.0.0',
        supports: ['icp-1.0-core'],
      },
    }),
  );

  return { registry, cleanup: () => rmSync(dir, { recursive: true, force: true }) };
}

function runRunner(extraArgs, env = {}) {
  return spawnSync(
    process.execPath,
    [RUNNER, '--iut', 'stub', '--vector', '01-aid-derivation', ...extraArgs],
    { cwd: ROOT, encoding: 'utf8', env: { ...process.env, ...env } },
  );
}

test('--registry points the runner at an out-of-tree adapter registry', () => {
  const { registry, cleanup } = stubRegistry('pass');
  try {
    const r = runRunner(['--registry', registry]);
    assert.equal(r.status, 0, r.stdout + r.stderr);
    assert.match(r.stdout, /1 PASS, 0 FAIL, 0 SKIP/);
  } finally {
    cleanup();
  }
});

test('a SKIP is tolerated by default (back-compat)', () => {
  const { registry, cleanup } = stubRegistry('skip');
  try {
    const r = runRunner(['--registry', registry]);
    assert.equal(r.status, 0, r.stdout + r.stderr);
    assert.match(r.stdout, /0 PASS, 0 FAIL, 1 SKIP/);
  } finally {
    cleanup();
  }
});

test('--fail-on-skip turns a SKIP into a non-zero exit', () => {
  const { registry, cleanup } = stubRegistry('skip');
  try {
    const r = runRunner(['--registry', registry, '--fail-on-skip']);
    assert.equal(r.status, 1, `expected exit 1, got ${r.status}\n${r.stdout}${r.stderr}`);
    assert.match(r.stdout, /0 PASS, 0 FAIL, 1 SKIP/);
    assert.match(r.stdout + r.stderr, /--fail-on-skip/);
  } finally {
    cleanup();
  }
});

test('--fail-on-skip cannot be silently disabled by a trailing value', () => {
  // `--fail-on-skip 1` and `--fail-on-skip=1` both parse to a string value.
  // Reading the flag by value rather than by presence would turn either of
  // them into "no gate" on a command line that plainly asks for one.
  const { registry, cleanup } = stubRegistry('skip');
  try {
    for (const form of [['--fail-on-skip', '1'], ['--fail-on-skip=1'], ['--fail-on-skip=false']]) {
      const r = runRunner(['--registry', registry, ...form]);
      assert.equal(
        r.status,
        1,
        `expected exit 1 for ${JSON.stringify(form)}, got ${r.status}\n${r.stdout}${r.stderr}`,
      );
    }
  } finally {
    cleanup();
  }
});

test('--key=value is equivalent to --key value', () => {
  const { registry, cleanup } = stubRegistry('pass');
  try {
    const r = spawnSync(
      process.execPath,
      [RUNNER, `--iut=stub`, `--registry=${registry}`, '--vector=01-aid-derivation'],
      { cwd: ROOT, encoding: 'utf8' },
    );
    assert.equal(r.status, 0, r.stdout + r.stderr);
    assert.match(r.stdout, /1 PASS, 0 FAIL, 0 SKIP/);
  } finally {
    cleanup();
  }
});

test('ICP_CONFORMANCE_FAIL_ON_SKIP=1 is equivalent to --fail-on-skip', () => {
  const { registry, cleanup } = stubRegistry('skip');
  try {
    const r = runRunner(['--registry', registry], { ICP_CONFORMANCE_FAIL_ON_SKIP: '1' });
    assert.equal(r.status, 1, `expected exit 1, got ${r.status}\n${r.stdout}${r.stderr}`);
  } finally {
    cleanup();
  }
});

test('--fail-on-skip does not affect an all-pass run', () => {
  const { registry, cleanup } = stubRegistry('pass');
  try {
    const r = runRunner(['--registry', registry, '--fail-on-skip']);
    assert.equal(r.status, 0, r.stdout + r.stderr);
  } finally {
    cleanup();
  }
});

test('every shipped IUT completes icp-1.0-core with zero skips under --fail-on-skip', () => {
  // The registry-level claim ("supports icp-1.0-core") is only meaningful if
  // no vector in the profile silently no-ops. This is the property CI now
  // enforces per IUT; assert it for the pure-JS reference here so a
  // regression is caught without a toolchain.
  const r = spawnSync(
    process.execPath,
    [RUNNER, '--profile', 'icp-1.0-core', '--iut', 'reference-demo', '--fail-on-skip'],
    { cwd: ROOT, encoding: 'utf8' },
  );
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /9 PASS, 0 FAIL, 0 SKIP/);
});

test('the reference IUT passes icp-1.0-commerce with zero skips', () => {
  const r = spawnSync(
    process.execPath,
    [RUNNER, '--profile', 'icp-1.0-commerce', '--iut', 'reference-demo', '--fail-on-skip'],
    { cwd: ROOT, encoding: 'utf8' },
  );
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /1 PASS, 0 FAIL, 0 SKIP/);
});
