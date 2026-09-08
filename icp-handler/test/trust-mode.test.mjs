// Demo trust must be asked for by name.
//
// The handler enforces trust when it is durable or when ICP_TRUST_MODE=enforce.
// Everything else was permissive by omission: an in-memory handler with no
// ICP_TRUST_MODE accepted every `principal_binding` without checking who
// authorized the agent. icp-docker/docker-compose.yml ran exactly that
// configuration — NODE_ENV=production, port published, no ICP_TRUST_MODE — so
// the stack's trust posture was an accident of two defaults rather than a
// decision anyone made.
//
// The handler now refuses to start in that configuration. Explicit
// ICP_TRUST_MODE=demo still works (loudly), because that is a decision.
//
// The refusal happens at module evaluation, so each case runs in its own
// process rather than importing the server into this one.
//
// Run: node --test test/trust-mode.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const serverUrl = pathToFileURL(join(here, '..', 'src', 'server.mjs')).href;

/** Import the server in a child process and report how it went. */
function startServer(env) {
  return spawnSync(
    process.execPath,
    [
      '--input-type=module',
      '-e',
      // Exit as soon as the module finishes evaluating: `server.listen` has
      // already run by then, so a clean exit means "it started".
      `await import(${JSON.stringify(serverUrl)}); process.exit(0);`,
    ],
    {
      encoding: 'utf8',
      timeout: 20_000,
      env: { ...process.env, PORT: '0', ICP_TRUST_MODE: '', NODE_ENV: '', ...env },
    },
  );
}

test('NODE_ENV=production with no ICP_TRUST_MODE refuses to start', () => {
  const run = startServer({ NODE_ENV: 'production' });
  assert.notEqual(run.status, 0, `expected a refusal, got:\n${run.stdout}\n${run.stderr}`);
  assert.match(run.stderr, /refusing to start/);
  assert.match(run.stderr, /ICP_TRUST_MODE/);
});

test('explicit ICP_TRUST_MODE=demo still starts, loudly', () => {
  const run = startServer({ NODE_ENV: 'production', ICP_TRUST_MODE: 'demo' });
  assert.equal(run.status, 0, `expected a clean start, got:\n${run.stdout}\n${run.stderr}`);
  assert.match(run.stderr, /DEMO TRUST MODE/);
});

test('ICP_TRUST_MODE=enforce starts in production', () => {
  const run = startServer({ NODE_ENV: 'production', ICP_TRUST_MODE: 'enforce' });
  assert.equal(run.status, 0, `expected a clean start, got:\n${run.stdout}\n${run.stderr}`);
});

test('a walkthrough outside production is untouched', () => {
  const run = startServer({});
  assert.equal(run.status, 0, `expected a clean start, got:\n${run.stdout}\n${run.stderr}`);
});
