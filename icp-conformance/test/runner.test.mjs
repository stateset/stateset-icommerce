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
import { mkdtempSync, readFileSync, writeFileSync, rmSync, chmodSync } from 'node:fs';
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

// ===========================================================================
// --version and --report
// ===========================================================================

const PKG = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8'));
const FAILING_ADAPTER = join(__dirname, 'fixtures', 'failing-adapter.mjs');

function loadProfile(name) {
  return JSON.parse(readFileSync(join(ROOT, 'profiles', `${name}.json`), 'utf8'));
}

/** Parse the runner's `Result:` line into the same shape as report.totals. */
function totalsFromStdout(stdout) {
  const m = stdout.match(/Result: (\d+) PASS, (\d+) FAIL, (\d+) SKIP \(of (\d+) total\)/);
  assert.ok(m, `no Result line in:\n${stdout}`);
  return { pass: Number(m[1]), fail: Number(m[2]), skip: Number(m[3]), total: Number(m[4]) };
}

function withTempDir(fn) {
  const dir = mkdtempSync(join(tmpdir(), 'icp-runner-report-'));
  try {
    return fn(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** A registry whose single IUT (`broken`) is the always-diverging fixture. */
function failingRegistry(dir) {
  const registry = join(dir, 'registry.json');
  writeFileSync(
    registry,
    JSON.stringify({
      broken: {
        command: [process.execPath, FAILING_ADAPTER],
        language: 'javascript',
        implementation: 'test fixture (always diverges)',
        version: '0.0.0',
        supports: ['icp-1.0-core'],
      },
    }),
  );
  return registry;
}

test('--version prints the package version and nothing else', () => {
  const r = spawnSync(process.execPath, [RUNNER, '--version'], { cwd: ROOT, encoding: 'utf8' });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.equal(r.stdout, `${PKG.version}\n`);
  assert.equal(r.stderr, '');
});

test('--version does not need a readable registry or profile', () => {
  const r = spawnSync(
    process.execPath,
    [RUNNER, '--version', '--registry', '/nonexistent/registry.json', '--profile', 'no-such-profile'],
    { cwd: ROOT, encoding: 'utf8' },
  );
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.equal(r.stdout, `${PKG.version}\n`);
});

test('the package version is the release the README announces', () => {
  const readme = readFileSync(join(ROOT, 'README.md'), 'utf8');
  const m = readme.match(/current release is `icp-conformance (\d+\.\d+\.\d+)`/);
  assert.ok(m, 'README no longer states the current release');
  assert.equal(PKG.version, m[1]);
});

test('--report writes totals matching the run and one entry per profile vector', () => {
  withTempDir((dir) => {
    // A nested path: the runner creates missing parent directories.
    const reportPath = join(dir, 'nested', 'report.json');
    const before = Date.now();
    const r = spawnSync(
      process.execPath,
      [RUNNER, '--profile', 'icp-1.0-core', '--iut', 'reference-demo', '--fail-on-skip', '--report', reportPath],
      { cwd: ROOT, encoding: 'utf8' },
    );
    assert.equal(r.status, 0, r.stdout + r.stderr);

    const report = JSON.parse(readFileSync(reportPath, 'utf8'));
    const profile = loadProfile('icp-1.0-core');

    assert.equal(report.report_format, 'icp-conformance-report/1');
    assert.equal(report.suite.version, PKG.version);
    assert.equal(report.suite.name, PKG.name);
    assert.equal(report.spec_version, profile.spec_version);
    assert.equal(report.profile, 'icp-1.0-core');
    assert.equal(report.iut.name, 'reference-demo');
    assert.equal(report.fail_on_skip, true);
    assert.equal(report.complete_profile, true);
    assert.equal(report.exit_code, 0);

    const ts = Date.parse(report.timestamp);
    assert.ok(report.timestamp.endsWith('Z'), report.timestamp);
    assert.ok(ts >= before - 1000 && ts <= Date.now() + 1000, report.timestamp);

    assert.deepEqual(report.totals, totalsFromStdout(r.stdout));
    assert.deepEqual(report.totals, { pass: 9, fail: 0, skip: 0, total: 9 });
    assert.deepEqual(
      report.vectors.map((v) => v.name),
      profile.vectors,
    );
    for (const v of report.vectors) {
      assert.equal(v.result, 'pass', v.name);
      assert.ok(v.checks_passed > 0, v.name);
    }
  });
});

test('--report does not change stdout, stderr or the exit code', () => {
  withTempDir((dir) => {
    const base = [RUNNER, '--profile', 'icp-1.0-commerce', '--iut', 'reference-demo', '--verbose'];
    const plain = spawnSync(process.execPath, base, { cwd: ROOT, encoding: 'utf8' });
    const withReport = spawnSync(process.execPath, [...base, '--report', join(dir, 'r.json')], {
      cwd: ROOT,
      encoding: 'utf8',
    });
    assert.equal(withReport.status, plain.status);
    assert.equal(withReport.stdout, plain.stdout);
    assert.equal(withReport.stderr, plain.stderr);
  });
});

test('a failing adapter exits non-zero and the report records every failure', () => {
  withTempDir((dir) => {
    const reportPath = join(dir, 'report.json');
    const r = spawnSync(
      process.execPath,
      [RUNNER, '--profile', 'icp-1.0-core', '--iut', 'broken', '--registry', failingRegistry(dir), '--report', reportPath],
      { cwd: ROOT, encoding: 'utf8' },
    );
    assert.equal(r.status, 1, `expected exit 1, got ${r.status}\n${r.stdout}${r.stderr}`);

    const report = JSON.parse(readFileSync(reportPath, 'utf8'));
    const profile = loadProfile('icp-1.0-core');
    const n = profile.vectors.length;

    assert.equal(report.exit_code, 1);
    assert.equal(report.iut.name, 'broken');
    assert.equal(report.iut.implementation, 'test fixture (always diverges)');
    assert.deepEqual(report.totals, totalsFromStdout(r.stdout));
    assert.deepEqual(report.totals, { pass: 0, fail: n, skip: 0, total: n });
    assert.deepEqual(
      report.vectors.map((v) => v.name),
      profile.vectors,
    );
    for (const v of report.vectors) {
      assert.equal(v.result, 'fail', v.name);
      assert.equal(v.reason, 'output divergence', v.name);
      assert.ok(Array.isArray(v.failures) && v.failures.length > 0, v.name);
      for (const f of v.failures) {
        assert.equal(typeof f.key, 'string');
        assert.ok('expected' in f, `${v.name}/${f.key} has no expected value`);
      }
    }
  });
});

test('the report records a SKIP with its reason, and the --fail-on-skip exit code', () => {
  const { registry, cleanup } = stubRegistry('skip');
  try {
    withTempDir((dir) => {
      const reportPath = join(dir, 'report.json');
      const r = runRunner(['--registry', registry, '--fail-on-skip', '--report', reportPath]);
      assert.equal(r.status, 1, r.stdout + r.stderr);

      const report = JSON.parse(readFileSync(reportPath, 'utf8'));
      assert.equal(report.exit_code, 1);
      assert.equal(report.fail_on_skip, true);
      // --vector narrows the run, so this is not a whole-profile result.
      assert.equal(report.complete_profile, false);
      assert.deepEqual(report.totals, { pass: 0, fail: 0, skip: 1, total: 1 });
      assert.deepEqual(report.vectors, [
        { name: '01-aid-derivation', result: 'skip', reason: 'no handler in stub' },
      ]);
    });
  } finally {
    cleanup();
  }
});

test('--report without a path is a runner error, not a silently missing report', () => {
  const r = spawnSync(process.execPath, [RUNNER, '--iut', 'reference-demo', '--report'], {
    cwd: ROOT,
    encoding: 'utf8',
  });
  assert.equal(r.status, 2, r.stdout + r.stderr);
  assert.match(r.stderr, /--report/);
});
