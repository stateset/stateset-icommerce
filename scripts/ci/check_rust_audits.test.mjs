import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import {
  chmodSync,
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

const locks = [
  'Cargo.lock',
  'bindings/php/Cargo.lock',
  'bindings/ruby/Cargo.lock',
  'crates/crypto/fuzz/Cargo.lock',
];
function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'stateset-rust-audit-test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(path.join(root, 'scripts/ci'), { recursive: true });
  copyFileSync(
    new URL('./check_rust_audits.sh', import.meta.url),
    path.join(root, 'scripts/ci/check_rust_audits.sh'),
  );
  execFileSync('git', ['init', '-q'], { cwd: root });
  for (const lock of locks) {
    mkdirSync(path.dirname(path.join(root, lock)), { recursive: true });
    writeFileSync(path.join(root, lock), '# fixture\n');
    writeFileSync(path.join(root, lock.replace('Cargo.lock', 'Cargo.toml')), '# fixture\n');
  }
  execFileSync('git', ['add', ...locks], { cwd: root });
  const bin = path.join(root, 'bin');
  mkdirSync(bin);
  writeFileSync(
    path.join(bin, 'cargo'),
    `#!/bin/sh
set -eu
printf '%s\\0' "$@" >> "$AUDIT_CALLS"
printf '\\n' >> "$AUDIT_CALLS"
operation=$1
shift
subject=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    --file|--manifest-path) subject=$2; shift 2 ;;
    *) shift ;;
  esac
done
case "$operation" in
  audit) [ "$subject" != "$FAIL_AUDIT" ] ;;
  tree)
    [ "$subject" != "$FAIL_GRAPH" ] || exit 1
    if [ "$subject" = "$COMPILED_EXCEPTION" ]; then
      printf 'rsa v0.9.10\\n'
    else
      printf 'stateset-core v1.39.0\\n'
    fi
    ;;
  *) exit 64 ;;
esac
`,
  );
  chmodSync(path.join(bin, 'cargo'), 0o755);
  const calls = path.join(root, 'calls.log');
  return {
    root,
    bin,
    run(extra = {}) {
      return spawnSync('bash', ['scripts/ci/check_rust_audits.sh'], {
        cwd: root,
        env: {
          ...process.env,
          PATH: `${bin}:${process.env.PATH}`,
          AUDIT_CALLS: calls,
          FAIL_AUDIT: '',
          FAIL_GRAPH: '',
          COMPILED_EXCEPTION: '',
          ...extra,
        },
        encoding: 'utf8',
        timeout: 10_000,
      });
    },
    calls() {
      return readFileSync(calls, 'utf8')
        .trimEnd()
        .split('\n')
        .map((line) => line.split('\0').slice(0, -1));
    },
  };
}

test('audits every tracked lockfile and checks each complete build graph', (t) => {
  const f = fixture(t);
  const result = f.run();
  assert.equal(result.status, 0, result.stderr);
  const audits = f.calls().filter((args) => args[0] === 'audit');
  assert.deepEqual(
    audits.map((args) => args[args.indexOf('--file') + 1]),
    locks,
  );
  assert.equal(audits[0].includes('--no-fetch'), false);
  assert.ok(audits.slice(1).every((args) => args.includes('--no-fetch')));
  const trees = f.calls().filter((args) => args[0] === 'tree');
  assert.equal(trees.length, locks.length);
  for (const args of trees) {
    assert.ok(args.includes('--locked') && args.includes('--all-features'));
    assert.equal(args[args.indexOf('--target') + 1], 'all');
    assert.equal(args[args.indexOf('--edges') + 1], 'normal,build');
  }
});

test('a vulnerable standalone binding fails while remaining lockfiles still run', (t) => {
  const f = fixture(t);
  assert.equal(f.run({ FAIL_AUDIT: 'bindings/php/Cargo.lock' }).status, 1);
  assert.equal(f.calls().filter((args) => args[0] === 'audit').length, locks.length);
});

test('a compiled exception in a standalone binding is refused', (t) => {
  const f = fixture(t);
  const result = f.run({ COMPILED_EXCEPTION: 'bindings/ruby/Cargo.toml' });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /advisory-ignored package is compiled/);
});

test('a failed dependency graph cannot establish an exception', (t) => {
  const f = fixture(t);
  const result = f.run({ FAIL_GRAPH: 'bindings/php/Cargo.toml' });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Unable to verify advisory exceptions/);
});

test('failed lockfile discovery fails closed', (t) => {
  const f = fixture(t);
  writeFileSync(path.join(f.bin, 'git'), '#!/bin/sh\nexit 42\n');
  chmodSync(path.join(f.bin, 'git'), 0o755);
  assert.equal(f.run().status, 42);
});

test('an empty lockfile inventory fails closed', (t) => {
  const f = fixture(t);
  execFileSync('git', ['rm', '--cached', ...locks], { cwd: f.root });
  const result = f.run();
  assert.equal(result.status, 1);
  assert.match(result.stderr, /No tracked Rust lockfiles/);
});

test('a missing standalone manifest is refused', (t) => {
  const f = fixture(t);
  rmSync(path.join(f.root, 'bindings/php/Cargo.toml'));
  const result = f.run();
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Missing lockfile or manifest/);
});
