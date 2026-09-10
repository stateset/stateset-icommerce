// The handler has two trust states, and no third.
//
//   enforce     — every Intent MUST carry a `principal_binding` the operator
//                 can verify. Durable handlers always enforce; an in-memory
//                 handler enforces with ICP_TRUST_MODE=enforce.
//   permissive  — an in-memory handler with ICP_TRUST_MODE unset. A binding
//                 naming a principal the operator never registered is waved
//                 through unverified. It is a localhost walkthrough posture,
//                 it says so loudly at startup, and it refuses to run under
//                 NODE_ENV=production.
//
// There used to be a third value, `ICP_TRUST_MODE=demo`, which selected the
// permissive path *explicitly* — including under NODE_ENV=production on a
// published port, which is exactly what icp-docker/docker-compose.yml did.
// "Deliberate" is not the same as "safe": the only thing that value bought was
// permission to publish a handler that accepts unverified delegations to the
// internet. It is now a startup error that names its replacement, because a
// stack that silently downgrades its own trust posture is worse than one that
// will not boot.
//
// The refusals happen at module evaluation, so each case runs in its own
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

test('ICP_TRUST_MODE=demo refuses to start and names its replacement', () => {
  const run = startServer({ ICP_TRUST_MODE: 'demo' });
  assert.notEqual(run.status, 0, `expected a refusal, got:\n${run.stdout}\n${run.stderr}`);
  // Both replacements, so the operator can pick without reading the source:
  // unset for the walkthrough they probably wanted, enforce for a deployment.
  assert.match(run.stderr, /no longer accepted/);
  assert.match(run.stderr, /unset ICP_TRUST_MODE/i);
  assert.match(run.stderr, /ICP_TRUST_MODE=enforce/);
});

test('ICP_TRUST_MODE=demo is refused under NODE_ENV=production too', () => {
  // The old build treated this as the *sanctioned* configuration: an
  // unverifying handler on a published port, blessed by an env var.
  const run = startServer({ NODE_ENV: 'production', ICP_TRUST_MODE: 'demo' });
  assert.notEqual(run.status, 0, `expected a refusal, got:\n${run.stdout}\n${run.stderr}`);
  assert.match(run.stderr, /no longer accepted/);
});

test('an unrecognized ICP_TRUST_MODE refuses to start', () => {
  const run = startServer({ ICP_TRUST_MODE: 'permissive' });
  assert.notEqual(run.status, 0, `expected a refusal, got:\n${run.stdout}\n${run.stderr}`);
  assert.match(run.stderr, /ICP_TRUST_MODE/);
});

test('ICP_TRUST_MODE=enforce starts in production', () => {
  const run = startServer({ NODE_ENV: 'production', ICP_TRUST_MODE: 'enforce' });
  assert.equal(run.status, 0, `expected a clean start, got:\n${run.stdout}\n${run.stderr}`);
});

test('a walkthrough outside production starts, and says it is permissive', () => {
  const run = startServer({});
  assert.equal(run.status, 0, `expected a clean start, got:\n${run.stdout}\n${run.stderr}`);
  // The banner is printed at module evaluation, not from the `listen`
  // callback: an operator must see it even if the port is already taken.
  assert.match(run.stderr, /PERMISSIVE TRUST/);
  assert.match(run.stderr, /WITHOUT verification/);
});
