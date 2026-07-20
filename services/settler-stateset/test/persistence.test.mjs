// Restart durability tests. Spawns the daemon as a real child process,
// drives it over HTTP, kills it, respawns with the same env, and verifies
// that escrow state (§8 event log) and the signing identity both survive.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtempSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const SERVER = join(dirname(fileURLToPath(import.meta.url)), '..', 'src', 'server.mjs');

function startDaemon(env) {
  const child = spawn(process.execPath, [SERVER], {
    env: { ...process.env, PORT: '0', ...env },
    stdio: ['ignore', 'ignore', 'pipe'],
  });
  const baseUrl = new Promise((resolve, reject) => {
    let buffer = '';
    child.stderr.on('data', (chunk) => {
      buffer += chunk;
      const match = buffer.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);
      if (match) resolve(match[1]);
    });
    child.on('exit', (code) =>
      reject(new Error(`daemon exited before listening (code ${code}): ${buffer}`)),
    );
  });
  return { child, baseUrl };
}

function stopDaemon(child, signal = 'SIGTERM') {
  return new Promise((resolve) => {
    child.on('exit', (code, sig) => resolve({ code, sig }));
    child.kill(signal);
  });
}

test('escrow state and signing identity survive a restart', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'settler-persist-'));
  const env = {
    SETTLER_STATE_FILE: join(dir, 'state.json'),
    SETTLER_KEY_FILE: join(dir, 'settler.key'),
  };
  const escrowId = '0xpersist' + 'beef'.repeat(14);

  // --- First life: create an escrow, capture identity, terminate. ---------
  const first = startDaemon(env);
  const base1 = await first.baseUrl;

  const fund = await fetch(`${base1}/admin/escrow/event`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      escrow_id: escrowId,
      kind: 'fund',
      init: { intent_id: 'icp_int_PERSIST01', amount: { amount: '42.00', currency: 'USDC' } },
      rail_event: { rail: 'base-sepolia', block_number: 1, tx_hash: '0xdead' },
    }),
  });
  assert.equal(fund.status, 200);

  const disco1 = await (await fetch(`${base1}/.well-known/icp-settler`)).json();
  const pub1 = disco1.signing_keys[0].pub_hex;
  const kid1 = disco1.signing_keys[0].kid;

  const keyMode = statSync(env.SETTLER_KEY_FILE).mode & 0o777;
  assert.equal(keyMode, 0o600, 'persisted signing key must be owner-only');

  const stopped = await stopDaemon(first.child, 'SIGTERM');
  assert.equal(stopped.code, 0, 'SIGTERM must produce a clean exit');

  // --- Second life: same env, state and identity must be intact. ----------
  const second = startDaemon(env);
  const base2 = await second.baseUrl;

  const disco2 = await (await fetch(`${base2}/.well-known/icp-settler`)).json();
  assert.equal(disco2.signing_keys[0].pub_hex, pub1, 'signing key must survive restart');
  assert.equal(disco2.signing_keys[0].kid, kid1, 'kid must be stable across restarts');

  const escrow = await (await fetch(`${base2}/icp/v1/escrows/${escrowId}`)).json();
  assert.equal(escrow.state, 'funded', `escrow state must survive restart: ${JSON.stringify(escrow)}`);
  assert.equal(escrow.seq, 1);

  // The observe stream replays the persisted event log on connect (SSE —
  // read the first chunk, then abort; the stream itself never ends).
  const controller = new AbortController();
  const sse = await fetch(`${base2}/icp/v1/escrows/${escrowId}/events`, {
    signal: controller.signal,
  });
  const reader = sse.body.getReader();
  const { value } = await reader.read();
  controller.abort();
  const chunk = new TextDecoder().decode(value);
  assert.ok(
    chunk.includes('"to_state":"funded"') && chunk.includes(escrowId),
    `SSE replay must contain the persisted event: ${chunk.slice(0, 200)}`,
  );

  await stopDaemon(second.child);
});

test('ephemeral mode (no env) still works and generates a fresh key', async () => {
  const first = startDaemon({});
  const base = await first.baseUrl;
  const disco = await (await fetch(`${base}/.well-known/icp-settler`)).json();
  assert.equal(disco.signing_keys.length, 1);
  assert.ok(disco.signing_keys[0].kid.startsWith('settler-stateset-'));
  const stopped = await stopDaemon(first.child, 'SIGINT');
  assert.equal(stopped.code, 0, 'SIGINT must produce a clean exit');
});
