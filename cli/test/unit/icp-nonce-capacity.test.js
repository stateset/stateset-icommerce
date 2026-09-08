// Durable (SQLite) half of the ICP §5.3 replay-guard capacity policy. The
// in-memory guard is covered by icp-handler/test/nonce-cap.test.mjs, which
// stays dependency-free; better-sqlite3 only resolves from the CLI workspace.
//
// One global 100k cap let a single signer's nonce flood lock every other agent
// out of the handler for the whole 24h window. The cap is per signer AID.
import assert from 'node:assert/strict';
import test from 'node:test';
import Database from 'better-sqlite3';
import { SqliteProtocolStore } from '../../../icp-handler/src/sqlite-store.mjs';

test('durable nonce capacity is per signer, not one global bound', (t) => {
  const db = new Database(':memory:');
  t.after(() => db.close());
  const store = new SqliteProtocolStore(db);
  const guard = store.replayGuard({ maxEntriesPerSigner: 2, now: () => 1_000 });

  assert.equal(guard.checkAndRecord('aid:v1:zFlooder', 'n1'), true);
  assert.equal(guard.checkAndRecord('aid:v1:zFlooder', 'n2'), true);
  assert.equal(guard.checkAndRecord('aid:v1:zFlooder', 'n3'), false, 'flooder fails closed');
  assert.equal(guard.checkAndRecord('aid:v1:zVictim', 'n1'), true, 'victim is unaffected');
  assert.equal(guard.checkAndRecord('aid:v1:zVictim', 'n2'), true);
  assert.equal(guard.checkAndRecord('aid:v1:zVictim', 'n3'), false);
  assert.equal(guard.size(), 4);
});

test('durable guard bounds distinct signers and validates its policy', (t) => {
  const db = new Database(':memory:');
  t.after(() => db.close());
  const store = new SqliteProtocolStore(db);
  const guard = store.replayGuard({ maxEntriesPerSigner: 4, maxSigners: 2, now: () => 1_000 });

  assert.equal(guard.checkAndRecord('aid:v1:zOne', 'n1'), true);
  assert.equal(guard.checkAndRecord('aid:v1:zTwo', 'n1'), true);
  assert.equal(guard.checkAndRecord('aid:v1:zThree', 'n1'), false, 'signer bound is enforced');
  assert.equal(guard.checkAndRecord('aid:v1:zOne', 'n2'), true, 'admitted signers keep working');

  // `maxEntries` remains accepted as the deprecated spelling of the same cap.
  const legacy = store.replayGuard({ maxEntries: 1, maxSigners: 2, now: () => 1_000 });
  assert.equal(legacy.checkAndRecord('aid:v1:zFour', 'n1'), false, 'signer bound still applies');
  assert.equal(legacy.checkAndRecord('aid:v1:zOne', 'n3'), false, 'legacy cap is per signer');
  assert.equal(legacy.sizeFor('aid:v1:zOne'), 2);
  assert.throws(() => store.replayGuard({ maxEntriesPerSigner: 0 }), /nonce policy/);
  assert.throws(() => store.replayGuard({ maxSigners: 0 }), /nonce policy/);
});

test('expiry frees a signer slot without resurrecting the old nonce', (t) => {
  const db = new Database(':memory:');
  t.after(() => db.close());
  const store = new SqliteProtocolStore(db);
  const day = 86_400_000;

  assert.equal(
    store.replayGuard({ maxEntriesPerSigner: 1, now: () => 1_000 }).checkAndRecord('aid:a', 'n1'),
    true,
  );
  const same = store.replayGuard({ maxEntriesPerSigner: 1, now: () => 2_000 });
  assert.equal(same.checkAndRecord('aid:a', 'n1'), false, 'replay');
  assert.equal(same.checkAndRecord('aid:a', 'n2'), false, 'at cap');
  const later = store.replayGuard({ maxEntriesPerSigner: 1, now: () => day + 2_000 });
  assert.equal(later.checkAndRecord('aid:a', 'n2'), true, 'window closed, slot freed');
});
