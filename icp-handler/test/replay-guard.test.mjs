// Unit tests for the bounded nonce replay guard (ICP-1.0-DRAFT §5.3).
//
// Run: PORT=0 node --test test/replay-guard.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { ReplayGuard } from '../src/replay-guard.mjs';

test('first sighting of a (aid, nonce) pair is accepted', () => {
  const g = new ReplayGuard();
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n1'), true);
  assert.equal(g.size(), 1);
});

test('duplicate (aid, nonce) within TTL is rejected', () => {
  const g = new ReplayGuard();
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n1'), true);
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n1'), false);
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n1'), false);
  assert.equal(g.size(), 1);
});

test('distinct nonces for the same AID all pass', () => {
  const g = new ReplayGuard();
  for (let i = 0; i < 10; i++) {
    assert.equal(g.checkAndRecord('aid:v1:zA', `n${i}`), true, `nonce n${i}`);
  }
  assert.equal(g.size(), 10);
});

test('same nonce under different AIDs does not collide', () => {
  const g = new ReplayGuard();
  assert.equal(g.checkAndRecord('aid:v1:zA', 'shared'), true);
  assert.equal(g.checkAndRecord('aid:v1:zB', 'shared'), true);
  // Re-seen under A → replay; still-fresh under C → ok.
  assert.equal(g.checkAndRecord('aid:v1:zA', 'shared'), false);
  assert.equal(g.checkAndRecord('aid:v1:zC', 'shared'), true);
});

test('a nonce containing the key separator is not confusable across AIDs', () => {
  // Key is `${aid} ${nonce}`. Ensure an attacker can't craft an AID/nonce
  // split that aliases a different pair.
  const g = new ReplayGuard();
  assert.equal(g.checkAndRecord('aid:v1:zA', 'x y'), true);
  // Different AID whose suffix + nonce could naively concatenate to the same
  // string only if the separator were ambiguous — these are distinct pairs.
  assert.equal(g.checkAndRecord('aid:v1:zA x', 'y'), true);
});

test('TTL expiry: a nonce becomes usable again after its window closes', () => {
  let clock = 1_000_000;
  const g = new ReplayGuard({ ttlMs: 1000, now: () => clock });
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n1'), true);
  clock += 500; // still inside window
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n1'), false);
  clock += 600; // now past ttlMs since first sighting (1100 > 1000)
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n1'), true, 'past TTL → fresh');
});

test('expired entries are lazily evicted, shrinking size()', () => {
  let clock = 0;
  const g = new ReplayGuard({ ttlMs: 100, now: () => clock });
  g.checkAndRecord('aid:v1:zA', 'a');
  g.checkAndRecord('aid:v1:zA', 'b');
  assert.equal(g.size(), 2);
  clock = 200; // both past TTL
  // size() runs lazy eviction.
  assert.equal(g.size(), 0);
});

test('a full guard rejects new nonces instead of evicting live ones', () => {
  let clock = 0;
  const g = new ReplayGuard({ maxEntries: 3, ttlMs: 1_000_000, now: () => (clock += 1) });
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n1'), true); // ts 1
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n2'), true); // ts 2
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n3'), true); // ts 3 → at cap (3)
  // A 4th live nonce must be rejected (fail closed), NOT admitted by
  // forgetting n1 — §5.3 requires already-seen nonces to stay rejected for
  // the full window, so an attacker must not be able to flush them early.
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n4'), false);
  assert.equal(g.size(), 3);
  // n1 is still remembered → replaying it is still rejected.
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n1'), false);
});

test('expired entries free capacity for new nonces', () => {
  let clock = 0;
  const g = new ReplayGuard({ maxEntries: 2, ttlMs: 100, now: () => clock });
  clock = 1;
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n1'), true);
  clock = 2;
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n2'), true);
  clock = 3;
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n3'), false); // full of live entries
  clock = 150; // n1 (ts 1) and n2 (ts 2) are now past their window
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n3'), true);
  assert.ok(g.size() <= 2);
});

test('flood load never exceeds maxEntries and never forgets live nonces', () => {
  let clock = 0;
  const g = new ReplayGuard({ maxEntries: 50, ttlMs: 1_000_000, now: () => (clock += 1) });
  let accepted = 0;
  for (let i = 0; i < 500; i++) {
    if (g.checkAndRecord('aid:v1:zLoad', `n${i}`)) accepted += 1;
  }
  assert.equal(accepted, 50, 'exactly the first maxEntries nonces are admitted');
  assert.ok(g.size() <= 50, `size ${g.size()} must be <= 50`);
  // Every admitted nonce is still remembered → replay rejected.
  assert.equal(g.checkAndRecord('aid:v1:zLoad', 'n0'), false);
  assert.equal(g.checkAndRecord('aid:v1:zLoad', 'n49'), false);
});

test('constructor rejects invalid config', () => {
  assert.throws(() => new ReplayGuard({ ttlMs: 0 }), /ttlMs/);
  assert.throws(() => new ReplayGuard({ ttlMs: -1 }), /ttlMs/);
  assert.throws(() => new ReplayGuard({ maxEntries: 0 }), /maxEntries/);
  assert.throws(() => new ReplayGuard({ maxEntries: 1.5 }), /maxEntries/);
});

test('clear() empties the guard', () => {
  const g = new ReplayGuard();
  g.checkAndRecord('aid:v1:zA', 'n1');
  g.clear();
  assert.equal(g.size(), 0);
  assert.equal(g.checkAndRecord('aid:v1:zA', 'n1'), true);
});
