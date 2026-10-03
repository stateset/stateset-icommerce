const assert = require('node:assert/strict');
const { test } = require('node:test');
const { spawnSync } = require('node:child_process');
const { mkdtempSync, readdirSync, rmSync } = require('node:fs');
const os = require('node:os');
const path = require('node:path');

test('installed engine check runs repeatedly without credentials or persistent files', (t) => {
  const cwd = mkdtempSync(path.join(os.tmpdir(), 'stateset-check-native-'));
  t.after(() => rmSync(cwd, { recursive: true, force: true }));
  const bin = path.join(__dirname, '../bin/stateset-embedded-check.mjs');
  // Deliberately pass no application/API-key/provider environment variables.
  const env = { PATH: process.env.PATH, SystemRoot: process.env.SystemRoot };
  for (let attempt = 0; attempt < 2; attempt++) {
    const result = spawnSync(process.execPath, [bin, '--json'], {
      cwd,
      env,
      encoding: 'utf8',
      timeout: 30_000,
    });
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.equal(result.stderr, '');
    const report = JSON.parse(result.stdout);
    assert.equal(report.schemaVersion, 1);
    assert.equal(report.ok, true);
    assert.equal(report.scope, 'local-engine-only');
    assert.equal(report.database, ':memory:');
    assert.equal(report.externalSettlementVerified, false);
    assert.equal(report.checks.length, 10);
    assert.ok(report.checks.every(({ status }) => status === 'passed'));
    assert.equal(report.checks.at(-1).id, 'cleanup');
    assert.deepEqual(readdirSync(cwd), []);
  }
  const text = spawnSync(process.execPath, [bin], { cwd, env, encoding: 'utf8', timeout: 30_000 });
  assert.equal(text.status, 0, text.stderr || text.stdout);
  assert.match(text.stdout, /StateSet embedded .*: PASS/);
  assert.match(text.stdout, /external settlement were not checked/);
  assert.deepEqual(readdirSync(cwd), []);
});
