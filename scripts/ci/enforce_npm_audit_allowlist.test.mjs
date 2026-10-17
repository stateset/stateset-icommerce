import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const gate = fileURLToPath(new URL('./enforce_npm_audit_allowlist.mjs', import.meta.url));
const advisoryId = 'GHSA-vfj7-8cjw-p6xm';
const severities = ['info', 'low', 'moderate', 'high', 'critical'];

// Exercise the actual shell wrapper without building a storefront or contacting
// a registry. Each fake gate result is recorded across its command substitution.
for (const [name, exits, expectedStatus, attempts] of [
  ['success', [0], 0, 1],
  ['unavailable then successful', [2, 0], 0, 2],
  ['unavailable on every attempt', [2, 2, 2], 1, 3],
  ['policy refusal', [1], 1, 1],
  ['unavailable then policy refusal', [2, 1], 1, 2],
]) {
  test(`storefront retries: ${name}`, (t) => {
    const dir = mkdtempSync(path.join(os.tmpdir(), 'stateset-audit-retries-'));
    t.after(() => rmSync(dir, { recursive: true, force: true }));
    const calls = path.join(dir, 'attempts');
    writeFileSync(calls, '0\n');
    const source = readFileSync(
      new URL('./run_storefront_golden_path.sh', import.meta.url),
      'utf8',
    );
    const wrapper = source.match(/^run_audit_with_retries\(\) \{[\s\S]*?^\}/m)?.[0];
    assert.ok(wrapper, 'storefront audit retry function must be present');
    const result = spawnSync(
      'bash',
      [
        '-c',
        `
set -euo pipefail
results=(${exits.join(' ')})
node() {
  local count
  read -r count < "$AUDIT_TEST_CALLS"
  printf '%s\\n' "$((count + 1))" > "$AUDIT_TEST_CALLS"
  printf 'gate result %s\\n' "\${results[$count]}"
  return "\${results[$count]}"
}
sleep() { :; }
${wrapper}
run_audit_with_retries
`,
      ],
      {
        encoding: 'utf8',
        timeout: 10000,
        env: {
          ...process.env,
          REPO_ROOT: dir,
          LOG_PATH: path.join(dir, 'audit.log'),
          AUDIT_TEST_CALLS: calls,
        },
      },
    );
    assert.ifError(result.error);
    assert.equal(result.status, expectedStatus, result.stdout + result.stderr);
    assert.equal(Number(readFileSync(calls, 'utf8')), attempts);
    if (attempts === 3) assert.match(result.stderr, /failed after 3 attempts/);
  });
}

function report(vulnerabilities = {}) {
  const counts = Object.fromEntries(severities.map((severity) => [severity, 0]));
  for (const vulnerability of Object.values(vulnerabilities)) counts[vulnerability.severity]++;
  return {
    auditReportVersion: 2,
    vulnerabilities,
    metadata: { vulnerabilities: { ...counts, total: Object.keys(vulnerabilities).length } },
  };
}

function vulnerability(severity = 'high') {
  return {
    name: 'fixture-package',
    severity,
    via: [
      {
        name: 'fixture-package',
        severity,
        url: `https://github.com/advisories/${advisoryId}`,
        range: '<=1.0.0',
      },
    ],
    fixAvailable: false,
  };
}

function runGate(
  t,
  { audit = report(), status = 0, signal, allowed = [], views = {}, raw, level = 'high' } = {},
) {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'stateset-audit-gate-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  writeFileSync(
    path.join(dir, 'fixture.json'),
    JSON.stringify({ audit, status, signal, views, raw }),
  );
  writeFileSync(path.join(dir, 'allowlist.json'), JSON.stringify({ allowed }));
  writeFileSync(path.join(dir, 'calls.jsonl'), '');
  writeFileSync(
    path.join(dir, 'npm'),
    `#!${process.execPath}
const fs = require('node:fs');
const fixture = JSON.parse(fs.readFileSync(process.env.AUDIT_TEST_FIXTURE, 'utf8'));
const args = process.argv.slice(2);
fs.appendFileSync(process.env.AUDIT_TEST_CALLS, JSON.stringify(args) + '\\n');
if (args[0] === 'audit') {
  fs.writeSync(1, fixture.raw === undefined ? JSON.stringify(fixture.audit) : fixture.raw);
  if (fixture.signal) process.kill(process.pid, fixture.signal);
  else process.exit(fixture.status);
} else if (args[0] === 'view') {
  const response = fixture.views[args.slice(1, -1).join(' ')];
  if (!response) { console.error('Unexpected registry lookup'); process.exit(99); }
  fs.writeSync(1, response.raw === undefined ? JSON.stringify(response.value) : response.raw);
  process.exit(response.status || 0);
} else { process.exit(98); }
`,
    { mode: 0o755 },
  );
  const result = spawnSync(
    process.execPath,
    [gate, '--level', level, '--allowlist', path.join(dir, 'allowlist.json')],
    {
      encoding: 'utf8',
      timeout: 10000,
      env: {
        ...process.env,
        PATH: `${dir}${path.delimiter}${process.env.PATH}`,
        AUDIT_TEST_FIXTURE: path.join(dir, 'fixture.json'),
        AUDIT_TEST_CALLS: path.join(dir, 'calls.jsonl'),
      },
    },
  );
  assert.ifError(result.error);
  return {
    ...result,
    calls: readFileSync(path.join(dir, 'calls.jsonl'), 'utf8')
      .trim()
      .split('\n')
      .filter(Boolean)
      .map(JSON.parse),
  };
}

test('a complete clean report passes without registry lookups', (t) => {
  const result = runGate(t);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.calls.length, 1);
});

test('findings below the requested threshold remain permitted', (t) => {
  const result = runGate(t, { audit: report({ example: vulnerability('moderate') }) });
  assert.equal(result.status, 0, result.stderr);
});

for (const [name, audit] of [
  ['JSON registry error', { error: { code: 'E503', summary: 'Service unavailable' } }],
  ['error alongside clean data', { ...report(), error: { code: 'EAUDITNOLOCK' } }],
  ['missing report', {}],
  ['array report', []],
  ['missing vulnerability map', { auditReportVersion: 2, metadata: report().metadata }],
  ['array vulnerability map', { ...report(), vulnerabilities: [] }],
  ['unknown report revision', { ...report(), auditReportVersion: 3 }],
  ['missing summary', { auditReportVersion: 2, vulnerabilities: {} }],
  [
    'contradictory summary',
    {
      ...report(),
      metadata: { vulnerabilities: { ...report().metadata.vulnerabilities, high: 1, total: 1 } },
    },
  ],
  ['unknown severity', report({ example: vulnerability('urgent') })],
  ['invalid vulnerability entry', { ...report(), vulnerabilities: { example: null } }],
]) {
  test(`${name} refuses as unavailable evidence`, (t) => {
    const result = runGate(t, { audit, status: 1 });
    assert.equal(result.status, 2, result.stdout + result.stderr);
    assert.doesNotMatch(result.stdout, /no high .*advisories found/);
    assert.equal(result.calls.length, 1);
  });
}

test('invalid JSON refuses as unavailable evidence', (t) => {
  const result = runGate(t, { raw: '<html>Unavailable</html>', status: 1 });
  assert.equal(result.status, 2, result.stdout + result.stderr);
});

for (const status of [1, 17]) {
  test(`a clean-looking report with exit ${status} cannot pass`, (t) => {
    const result = runGate(t, { status });
    assert.equal(result.status, 2, result.stdout + result.stderr);
  });
}

test('a terminated audit cannot pass even after writing valid JSON', (t) => {
  const result = runGate(t, { signal: 'SIGTERM' });
  assert.equal(result.status, 2, result.stdout + result.stderr);
});

test('findings cannot be paired with a contradictory successful exit', (t) => {
  const result = runGate(t, { audit: report({ example: vulnerability() }) });
  assert.equal(result.status, 2, result.stdout + result.stderr);
});

test('a finding above threshold is rejected without an allowlist entry', (t) => {
  const result = runGate(t, { audit: report({ example: vulnerability() }), status: 1 });
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /id not on allowlist/);
});

const unpatchedViews = {
  'fixture-package version': { value: '1.0.0' },
  'fixture-package@<=1.0.0 version': { value: ['0.9.0', '1.0.0'] },
};

test('an allowlisted finding passes only with registry evidence of no patched latest version', (t) => {
  const result = runGate(t, {
    audit: report({ example: vulnerability() }),
    status: 1,
    allowed: [advisoryId],
    views: unpatchedViews,
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.calls.length, 3);
});

test('a published patch invalidates the exception', (t) => {
  const result = runGate(t, {
    audit: report({ example: vulnerability() }),
    status: 1,
    allowed: [advisoryId],
    views: { ...unpatchedViews, 'fixture-package version': { value: '1.0.1' } },
  });
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /patched fixture-package@1.0.1/);
});

test('transitive findings resolve their advisory and share registry lookups', (t) => {
  const result = runGate(t, {
    audit: report({ example: vulnerability(), parent: { severity: 'high', via: ['example'] } }),
    status: 1,
    allowed: [advisoryId],
    views: unpatchedViews,
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.calls.length, 3);
});

test('allowlisted IDs without package and range evidence cannot pass', (t) => {
  const v = vulnerability();
  delete v.via[0].range;
  const result = runGate(t, { audit: report({ example: v }), status: 1, allowed: [advisoryId] });
  assert.equal(result.status, 1, result.stdout + result.stderr);
});

for (const [name, response] of [
  ['null latest version', { value: null }],
  ['object latest version', { value: { error: 'unavailable' } }],
  ['empty latest version', { value: '' }],
  ['unparseable registry response', { raw: 'unavailable' }],
  ['registry lookup failure', { value: null, status: 1 }],
]) {
  test(`${name} cannot authorize an exception`, (t) => {
    const result = runGate(t, {
      audit: report({ example: vulnerability() }),
      status: 1,
      allowed: [advisoryId],
      views: { ...unpatchedViews, 'fixture-package version': response },
    });
    assert.equal(result.status, 1, result.stdout + result.stderr);
  });
}

for (const value of [null, [], ['1.0.0', null], { version: '1.0.0' }]) {
  test(`invalid vulnerable-version data ${JSON.stringify(value)} cannot authorize an exception`, (t) => {
    const result = runGate(t, {
      audit: report({ example: vulnerability() }),
      status: 1,
      allowed: [advisoryId],
      views: { ...unpatchedViews, 'fixture-package@<=1.0.0 version': { value } },
    });
    assert.equal(result.status, 1, result.stdout + result.stderr);
  });
}
