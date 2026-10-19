import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { runStress, readStressEvidence, reportStress } from './nightly_stress.mjs';

const sha = 'a'.repeat(40);
const runUrl = 'https://github.com/stateset/stateset-icommerce/actions/runs/123/attempts/1';
const names = Array.from({ length: 12 }, (_, i) => `stress_test_${i}`);
const listing = names.map((name) => `${name}: test`).join('\n');
const summary =
  'test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1s';
const ok = (stdout) => ({ status: 0, stdout });
function fixture(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-stress-test-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  fs.writeFileSync(
    path.join(directory, 'stress.json'),
    JSON.stringify({ schemaVersion: 1, sha, success: true, selected: names, passed: 12 }),
  );
  fs.writeFileSync(path.join(directory, 'stress.log'), summary);
  return { directory, sha, result: 'success', runUrl };
}
function executeResults(results, calls = []) {
  return (file, args) => {
    calls.push([file, ...args]);
    return results.shift();
  };
}
function client(issues = []) {
  const calls = [];
  return {
    calls,
    context: { repo: { owner: 'stateset', repo: 'stateset-icommerce' } },
    core: { setFailed: (message) => calls.push(['fail', message]) },
    github: {
      paginate: async () => issues,
      rest: {
        issues: {
          listForRepo: () => {},
          create: async (args) => calls.push(['create', args]),
          update: async (args) => calls.push(['update', args]),
        },
      },
    },
  };
}
const tracked = {
  number: 44,
  user: { type: 'Bot' },
  body: '<!-- stateset-nightly-embedded-stress:v1 -->',
};

test('discovers and runs every ignored case with locked dependencies and serial cases', (t) => {
  const options = fixture(t);
  const calls = [];
  const result = runStress({
    output: options.directory,
    sha,
    execute: executeResults([ok(listing), ok(summary)], calls),
  });
  assert.equal(result.success, true);
  assert.deepEqual(result.selected, names);
  assert.deepEqual(
    calls.map((call) => call.slice(0, 8)),
    Array(2).fill([
      'cargo',
      'test',
      '--locked',
      '-p',
      'stateset-embedded',
      '--test',
      'stress_test',
      '--',
    ]),
  );
  assert.deepEqual(calls[0].slice(8), ['--ignored', '--list']);
  assert.deepEqual(calls[1].slice(8), ['--ignored', '--nocapture', '--test-threads=1']);
});

test('empty, reduced or duplicate discovery cannot yield a green zero-test run', (t) => {
  for (const list of [
    '',
    names
      .slice(0, 11)
      .map((name) => `${name}: test`)
      .join('\n'),
    Array(12).fill('same: test').join('\n'),
  ]) {
    const options = fixture(t);
    const calls = [];
    assert.equal(
      runStress({ output: options.directory, sha, execute: executeResults([ok(list)], calls) })
        .success,
      false,
    );
    assert.equal(calls.length, 1);
  }
});

test('a failed build or timeout records failure without starting another command', (t) => {
  for (const result of [
    { status: 1, stderr: 'compile failed' },
    { status: null, signal: 'SIGTERM', error: new Error('deadline exceeded') },
  ]) {
    const options = fixture(t);
    const calls = [];
    const evidence = runStress({
      output: options.directory,
      sha,
      execute: executeResults([result], calls),
    });
    assert.equal(evidence.success, false);
    assert.equal(calls.length, 1);
    assert.equal(readStressEvidence(options).success, false);
  }
});

test('panic, filtering, skipping and incomplete output never count as full acceptance', (t) => {
  for (const result of [
    { status: 101, stdout: 'stress_test_concurrent_orders panicked' },
    ok(''),
    ok(summary.replace('12 passed', '0 passed')),
    ok(summary.replace('0 ignored', '1 ignored')),
    ok(summary.replace('0 filtered out', '1 filtered out')),
  ]) {
    const options = fixture(t);
    assert.equal(
      runStress({ output: options.directory, sha, execute: executeResults([ok(listing), result]) })
        .success,
      false,
    );
  }
});

test('no run identity cannot create evidence', (t) => {
  assert.throws(() => runStress({ output: fixture(t).directory, sha: '' }), /SHA/);
});

test('missing, empty, stale and malformed evidence never signal recovery', (t) => {
  for (const scenario of [
    'missing-json',
    'missing-log',
    'empty-log',
    'malformed',
    'stale',
    'partial',
    'duplicates',
  ]) {
    const options = fixture(t);
    const file = path.join(options.directory, 'stress.json');
    if (scenario === 'missing-json') fs.unlinkSync(file);
    else if (scenario === 'missing-log') fs.unlinkSync(path.join(options.directory, 'stress.log'));
    else if (scenario === 'empty-log')
      fs.writeFileSync(path.join(options.directory, 'stress.log'), '');
    else if (scenario === 'malformed') fs.writeFileSync(file, '{');
    else {
      const data = JSON.parse(fs.readFileSync(file));
      if (scenario === 'stale') data.sha = 'b'.repeat(40);
      if (scenario === 'partial') data.passed = 11;
      if (scenario === 'duplicates') data.selected = Array(12).fill('duplicate');
      fs.writeFileSync(file, JSON.stringify(data));
    }
    assert.equal(readStressEvidence(options).success, false, scenario);
    if (scenario === 'missing-log')
      assert.doesNotMatch(readStressEvidence(options).description, /All 12/);
  }
  for (const result of ['failure', 'skipped', 'cancelled'])
    assert.equal(readStressEvidence({ ...fixture(t), result }).success, false);
});

test('failure opens one tracking issue and fails the reporting job', async (t) => {
  const api = client();
  await reportStress({ ...fixture(t), ...api, result: 'failure' });
  assert.deepEqual(
    api.calls.map(([kind]) => kind),
    ['create', 'fail'],
  );
  assert.match(api.calls[0][1].body, /actions\/runs\/123/);
});

test('repeated failure updates the bot-owned issue, without touching a human issue', async (t) => {
  const api = client([{ number: 1, body: tracked.body, user: { type: 'User' } }, tracked]);
  await reportStress({ ...fixture(t), ...api, result: 'failure' });
  assert.deepEqual(
    api.calls.map(([kind]) => kind),
    ['update', 'fail'],
  );
  assert.equal(api.calls[0][1].issue_number, 44);
  assert.equal(api.calls[0][1].state, undefined);
});

test('recovery saves fresh evidence before closing the issue', async (t) => {
  const api = client([tracked]);
  await reportStress({ ...fixture(t), ...api });
  assert.equal(api.calls.length, 2);
  assert.match(api.calls[0][1].body, /All 12 discovered/);
  assert.equal(api.calls[1][1].state, 'closed');
});

test('API errors cannot close a tracking issue before recovery evidence is saved', async (t) => {
  const api = client([tracked]);
  api.github.rest.issues.update = async () => {
    throw new Error('offline');
  };
  await assert.rejects(reportStress({ ...fixture(t), ...api }), /offline/);
});

test('clean run without an issue has no write side effects', async (t) => {
  const api = client();
  await reportStress({ ...fixture(t), ...api });
  assert.equal(api.calls.length, 0);
});

test('verbose dependency output is escaped and bounded in issue bodies', async (t) => {
  const options = fixture(t);
  fs.writeFileSync(
    path.join(options.directory, 'stress.log'),
    '<@&>'.repeat(25000) + '\nfinal panic',
  );
  const api = client();
  await reportStress({ ...options, ...api, result: 'failure' });
  const body = api.calls[0][1].body;
  assert.ok(body.length < 20000);
  assert.ok(!body.includes('<@&>'));
  assert.match(body, /final panic/);
});
