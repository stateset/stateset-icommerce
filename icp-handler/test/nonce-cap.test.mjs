// The §5.3 replay guard fails closed at capacity. A SINGLE global cap turns
// that into a cross-tenant denial of service: one signer that burns the cap
// (cheap — nonces are caller-chosen) locks every other agent out of the
// handler for the whole 24h window. The cap is therefore per signer AID, with
// a separate bound on how many distinct signers may hold live nonces.
//
// Run: node --test test/nonce-cap.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { ReplayGuard } from '../src/replay-guard.mjs';

test('one signer exhausting its cap does not block a second signer', () => {
  const guard = new ReplayGuard({ maxEntriesPerSigner: 2, now: () => 1_000 });

  assert.equal(guard.checkAndRecord('aid:v1:zFlooder', 'n1'), true);
  assert.equal(guard.checkAndRecord('aid:v1:zFlooder', 'n2'), true);
  // Flooder is at its own cap and fails closed…
  assert.equal(guard.checkAndRecord('aid:v1:zFlooder', 'n3'), false);
  // …while an unrelated agent still transacts.
  assert.equal(guard.checkAndRecord('aid:v1:zVictim', 'n1'), true);
  assert.equal(guard.checkAndRecord('aid:v1:zVictim', 'n2'), true);
  assert.equal(guard.checkAndRecord('aid:v1:zVictim', 'n3'), false);
  assert.equal(guard.size(), 4);
});

test('a replay is still rejected and expiry frees the signer cap again', () => {
  let clock = 1_000;
  const guard = new ReplayGuard({ ttlMs: 1_000, maxEntriesPerSigner: 1, now: () => clock });

  assert.equal(guard.checkAndRecord('aid:v1:zAgent', 'n1'), true);
  assert.equal(guard.checkAndRecord('aid:v1:zAgent', 'n1'), false, 'live replay');
  assert.equal(guard.checkAndRecord('aid:v1:zAgent', 'n2'), false, 'signer at cap');
  clock += 2_000;
  assert.equal(guard.checkAndRecord('aid:v1:zAgent', 'n2'), true, 'expiry frees the slot');
  assert.equal(guard.size(), 1);
});

test('distinct signers are bounded so minted AIDs cannot exhaust memory', () => {
  const guard = new ReplayGuard({ maxEntriesPerSigner: 4, maxSigners: 2, now: () => 1_000 });

  assert.equal(guard.checkAndRecord('aid:v1:zOne', 'n1'), true);
  assert.equal(guard.checkAndRecord('aid:v1:zTwo', 'n1'), true);
  // A third distinct signer exceeds the signer bound and fails closed…
  assert.equal(guard.checkAndRecord('aid:v1:zThree', 'n1'), false);
  // …but admitted signers keep working.
  assert.equal(guard.checkAndRecord('aid:v1:zOne', 'n2'), true);
});

test('the guard rejects invalid capacity policy', () => {
  assert.throws(() => new ReplayGuard({ maxEntriesPerSigner: 0 }), /maxEntriesPerSigner/);
  assert.throws(() => new ReplayGuard({ maxSigners: -1 }), /maxSigners/);
  assert.throws(() => new ReplayGuard({ ttlMs: 0 }), /ttlMs/);
});
