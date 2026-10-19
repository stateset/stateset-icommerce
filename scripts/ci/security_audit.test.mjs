import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { AUDITS, STEP_COUNTS, auditCommands, runAudit } from './run_security_audit.mjs';
import { MARKER, TITLE, readAuditReport, reportSecurityAudit } from './report_security_audit.mjs';

const sha = 'a'.repeat(40);
const runUrl = 'https://github.com/stateset/stateset-icommerce/actions/runs/123/attempts/2';
function fixture(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-security-test-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  for (const id of AUDITS) {
    fs.writeFileSync(
      path.join(directory, `${id}.json`),
      JSON.stringify({
        schemaVersion: 1,
        id,
        sha,
        success: true,
        steps: Array.from({ length: STEP_COUNTS[id] }, () => ({ exitCode: 0, signal: null })),
      }),
    );
    fs.writeFileSync(path.join(directory, `${id}.log`), `${id}: no findings`);
  }
  return { directory, sha, runUrl, auditResult: 'success' };
}
function changeReport(options, id, change) {
  const file = path.join(options.directory, `${id}.json`);
  const report = JSON.parse(fs.readFileSync(file, 'utf8'));
  change(report);
  fs.writeFileSync(file, JSON.stringify(report));
}
function client(issues = []) {
  const calls = [];
  const github = {
    rest: {
      issues: {
        listForRepo: () => {},
        create: async (args) => calls.push(['create', args]),
        update: async (args) => calls.push(['update', args]),
      },
    },
    paginate: async (method, args) => {
      calls.push(['list', args]);
      return issues;
    },
  };
  return {
    github,
    calls,
    context: { repo: { owner: 'stateset', repo: 'stateset-icommerce' } },
    core: { setFailed: (text) => calls.push(['fail', text]) },
  };
}
const tracked = { number: 12, title: TITLE, body: MARKER, user: { type: 'Bot' } };

test('CLI and Node install complete lockfiles and audit all dependencies at high', () => {
  for (const id of ['cli', 'node']) {
    const commands = auditCommands(id, '/repo', '/scratch');
    assert.deepEqual(
      commands.map(({ file, args }) => [file, ...args]),
      [
        ['npm', 'ci'],
        ['npm', 'audit', '--audit-level=high'],
      ],
    );
    assert.equal(commands[0].cwd, id === 'cli' ? '/repo/cli' : '/repo/bindings/node');
  }
  assert.deepEqual(auditCommands('rust', '/repo', '/scratch')[0].args, [
    'scripts/ci/check_rust_audits.sh',
  ]);
  assert.deepEqual(auditCommands('deny', '/repo', '/scratch')[0].args, [
    'deny',
    'check',
    'advisories',
  ]);
});

test('storefront packs the public generator and uses the exact golden-path gate', () => {
  const root = path.resolve(import.meta.dirname, '../..');
  const commands = auditCommands('storefront', root, '/scratch');
  assert.equal(commands.length, 4);
  assert.deepEqual(commands[0].args, [
    'pack',
    './packages/create-stateset-app',
    '--pack-destination',
    '/scratch',
  ]);
  assert.ok(commands[1].args.includes('--skip-install'));
  assert.ok(
    commands[1].args.some((arg) => arg.startsWith('--package=/scratch/create-stateset-app-')),
  );
  assert.deepEqual(commands[2].args, ['install', '--no-fund', '--no-audit']);
  assert.deepEqual(commands[3].args, [
    path.join(root, 'scripts/ci/enforce_npm_audit_allowlist.mjs'),
    '--level',
    'high',
    '--allowlist',
    path.join(root, 'scripts/ci/npm-audit-allowlist.json'),
  ]);
  assert.equal(commands[2].cwd, '/scratch/audit-store');
  assert.equal(commands[3].cwd, commands[2].cwd);
});

test('a failed install records failure and never runs an audit on a partial tree', (t) => {
  const options = fixture(t);
  const calls = [];
  const success = runAudit({
    id: 'cli',
    output: options.directory,
    sha,
    execute: (file, args) => {
      calls.push([file, ...args]);
      return { status: 1, stderr: 'install unavailable' };
    },
  });
  assert.equal(success, false);
  assert.deepEqual(calls, [['npm', 'ci']]);
  assert.equal(readAuditReport(options).clean, false);
  assert.match(readAuditReport(options).body, /cli: failed/);
  assert.match(readAuditReport(options).body, /install unavailable/);
});

test('audit findings and process timeouts preserve output and fail', (t) => {
  const options = fixture(t);
  for (const result of [
    { status: 1, stdout: 'GHSA-example: new advisory' },
    { status: null, signal: 'SIGTERM', error: new Error('deadline exceeded') },
  ]) {
    assert.equal(
      runAudit({ id: 'deny', output: options.directory, sha, execute: () => result }),
      false,
    );
    const report = readAuditReport(options);
    assert.equal(report.clean, false);
    assert.match(report.body, /GHSA-example|deadline exceeded/);
  }
});

test('successful command sequence records complete evidence for the audited SHA', (t) => {
  const options = fixture(t);
  const calls = [];
  assert.equal(
    runAudit({
      id: 'node',
      output: options.directory,
      sha,
      execute: (file, args) => {
        calls.push([file, ...args]);
        return { status: 0, stdout: 'clean' };
      },
    }),
    true,
  );
  assert.equal(calls.length, 2);
  assert.equal(readAuditReport(options).clean, true);
});

test('unknown tasks and missing commit identities cannot create acceptance', (t) => {
  const options = fixture(t);
  assert.throws(() => runAudit({ id: 'other', output: options.directory, sha }), /Unknown/);
  assert.throws(() => runAudit({ id: 'node', output: options.directory, sha: '' }), /SHA/);
});

test('only complete successful evidence and a successful matrix count as recovery', (t) => {
  const options = fixture(t);
  assert.equal(readAuditReport(options).clean, true);
  for (const auditResult of ['failure', 'cancelled', 'skipped', undefined])
    assert.equal(readAuditReport({ ...options, auditResult }).clean, false);
  for (const mutate of [
    (r) => {
      r.sha = 'b'.repeat(40);
    },
    (r) => {
      r.id = 'deny';
    },
    (r) => {
      r.success = 'true';
    },
    (r) => {
      r.steps = [];
    },
    (r) => {
      r.steps[0].exitCode = 1;
    },
    (r) => {
      r.steps[0].signal = 'SIGTERM';
    },
  ]) {
    const current = fixture(t);
    changeReport(current, 'cli', mutate);
    assert.equal(readAuditReport(current).clean, false);
  }
});

test('missing, malformed and truncated artifacts never close the issue', (t) => {
  for (const variant of ['missing-json', 'missing-log', 'bad-json', 'partial-steps']) {
    const options = fixture(t);
    if (variant === 'missing-json') fs.unlinkSync(path.join(options.directory, 'rust.json'));
    if (variant === 'missing-log') fs.unlinkSync(path.join(options.directory, 'rust.log'));
    if (variant === 'bad-json') fs.writeFileSync(path.join(options.directory, 'rust.json'), '{');
    if (variant === 'partial-steps')
      changeReport(options, 'node', (r) => {
        r.steps.pop();
      });
    assert.equal(readAuditReport(options).clean, false);
  }
});

test('issue excerpts escape dependency markup, bound size and retain final findings', (t) => {
  const options = fixture(t);
  for (const id of AUDITS)
    fs.writeFileSync(
      path.join(options.directory, `${id}.log`),
      '<@&>'.repeat(30000) + '\nGHSA-final-finding',
    );
  const { body } = readAuditReport(options);
  assert.ok(body.length < 60000);
  assert.ok(!body.includes('<@&>'));
  assert.match(body, /GHSA-final-finding/);
  assert.match(body, /complete findings/);
});

test('new finding creates one tracking issue and fails the run', async (t) => {
  const options = fixture(t);
  changeReport(options, 'node', (r) => {
    r.success = false;
  });
  const api = client();
  await reportSecurityAudit({ ...options, ...api });
  assert.deepEqual(
    api.calls.map(([kind]) => kind),
    ['list', 'create', 'fail'],
  );
  assert.equal(api.calls[1][1].title, TITLE);
  assert.match(api.calls[1][1].body, /node: failed/);
  assert.equal(api.calls[0][1].per_page, 100);
});

test('recurring failure updates the existing issue without comments or duplicates', async (t) => {
  const options = fixture(t);
  const api = client([
    { number: 99, title: TITLE, body: 'manual issue', user: { type: 'User' } },
    tracked,
  ]);
  await reportSecurityAudit({ ...options, ...api, auditResult: 'failure' });
  assert.deepEqual(
    api.calls.map(([kind]) => kind),
    ['list', 'update', 'fail'],
  );
  assert.equal(api.calls[1][1].issue_number, 12);
  assert.equal(api.calls[1][1].state, undefined);
});

test('recovery updates acceptance evidence then closes only the bot-owned issue', async (t) => {
  const options = fixture(t);
  const api = client([{ ...tracked, number: 44, pull_request: {} }, tracked]);
  await reportSecurityAudit({ ...options, ...api });
  assert.deepEqual(
    api.calls.map(([kind]) => kind),
    ['list', 'update', 'update'],
  );
  assert.match(api.calls[1][1].body, /All audit gates passed/);
  assert.equal(api.calls[2][1].state, 'closed');
  assert.equal(api.calls[2][1].issue_number, 12);
});

test('clean run without an issue has no issue-write side effects', async (t) => {
  const api = client();
  await reportSecurityAudit({ ...fixture(t), ...api });
  assert.deepEqual(
    api.calls.map(([kind]) => kind),
    ['list'],
  );
});

test('failure to save recovery evidence never closes an issue', async (t) => {
  const api = client([tracked]);
  api.github.rest.issues.update = async (args) => {
    api.calls.push(['update', args]);
    throw new Error('API unavailable');
  };
  await assert.rejects(reportSecurityAudit({ ...fixture(t), ...api }), /API unavailable/);
  assert.equal(api.calls.filter(([kind]) => kind === 'update').length, 1);
  assert.equal(api.calls[1][1].state, undefined);
});

test('ambiguous tracking issues fail closed instead of updating unrelated issues', async (t) => {
  const api = client([tracked, { ...tracked, number: 13 }]);
  await assert.rejects(reportSecurityAudit({ ...fixture(t), ...api }), /Multiple/);
  assert.deepEqual(
    api.calls.map(([kind]) => kind),
    ['list'],
  );
});
