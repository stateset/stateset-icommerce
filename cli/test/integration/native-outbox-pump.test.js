/** Native Rust mutations -> durable facts -> signed VES rows -> REST wire envelopes.
 * Requires Cargo, Node and installed CLI dependencies; missing prerequisites fail.
 * The HTTP boundary is intercepted locally, not an external sequencer certification.
 */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import Database from 'better-sqlite3';
import { createOutboxPump, VES_OUTBOX_NAMESPACE, uuidv5 } from '../../src/sync/outbox-pump.js';
import { SyncEngine } from '../../src/sync/engine.js';
import { SyncConfig } from '../../src/sync/config.js';
import { SequencerClient } from '../../src/sync/client.js';
import {
  computeEventSigningHash,
  computePayloadPlainHash,
  hexToBuffer,
  generateHybridSigningKeypair,
  verifyEventSignatureHybrid,
} from '../../src/sync/crypto.js';

const identity = {
  tenantId: '22222222-2222-2222-2222-222222222222',
  storeId: '33333333-3333-3333-3333-333333333333',
  agentId: '44444444-4444-4444-4444-444444444444',
};

test('native recorded facts survive signing, restart replay and REST serialization', async (t) => {
  const dir = mkdtempSync(join(tmpdir(), 'stateset-native-outbox-'));
  let db;
  t.after(() => {
    if (db?.open) db.close();
    rmSync(dir, { recursive: true, force: true });
  });
  const path = join(dir, 'store.db');
  const result = spawnSync(
    'cargo',
    ['run', '--locked', '-p', 'stateset-db', '--example', 'recorded_outbox_fixture', '--', path],
    {
      cwd: fileURLToPath(new URL('../../../', import.meta.url)),
      encoding: 'utf8',
      timeout: 15 * 60 * 1000,
      maxBuffer: 16 * 1024 * 1024,
      env: { ...process.env, CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || '1' },
    },
  );
  assert.equal(result.error, undefined, String(result.error));
  assert.equal(result.status, 0, result.stderr + result.stdout);
  db = new Database(path);
  const facts = db
    .prepare("SELECT * FROM kernel_outbox WHERE tier = 'recorded' ORDER BY created_at, id")
    .all();
  const governed = db
    .prepare("SELECT * FROM kernel_outbox WHERE tier = 'governed' ORDER BY id")
    .all();
  assert.ok(governed.length > 0, 'native fixture must exercise tier isolation');
  const names = new Set(facts.map((row) => row.event_type));
  for (const name of [
    'shipments.created.v1',
    'shipments.updated.v1',
    'shipments.item_added.v1',
    'shipments.item_removed.v1',
    'shipments.event_added.v1',
    'shipment.status_changed',
    'promotion.condition_added',
  ]) {
    assert.ok(names.has(name), `native fixture must emit ${name}`);
  }
  assert.equal(
    JSON.parse(facts.find((row) => row.event_type === 'shipments.updated.v1').payload)
      .shipping_cost,
    '5.01',
  );
  assert.ok(
    facts.some((row) => new Date(row.created_at).toISOString() !== row.created_at),
    'fixture must exercise noncanonical JS timestamps',
  );

  // Exercise the default hybrid security profile without downgrading the engine.
  const signing = generateHybridSigningKeypair();
  const publicKey = crypto.createPublicKey({
    key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), signing.ed25519PublicKey]),
    type: 'spki',
    format: 'der',
  });
  const keyManager = {
    getCurrentSigningKey: async () => ({
      keyId: 1,
      privateKey: signing.ed25519PrivateKey,
      publicKey: signing.ed25519PublicKey,
      privateKeyBundle: {
        ed25519PrivateKey: signing.ed25519PrivateKey,
        mlDsa65Seed: signing.mlDsa65Seed,
      },
      publicKeyBundle: {
        ed25519PublicKey: signing.ed25519PublicKey,
        mlDsa65PublicKey: signing.mlDsa65PublicKey,
      },
    }),
  };
  const config = new SyncConfig({ sequencer: { url: 'https://sequencer.invalid' }, identity });
  let engine = new SyncEngine({ db, config, keyManager, preferGrpc: false });
  let pump = createOutboxPump(db, engine.outbox, { identity });
  const first = await pump.drain(1000);
  assert.equal(
    first.failed,
    0,
    JSON.stringify(
      db.prepare("SELECT event_type, last_error FROM kernel_outbox WHERE tier = 'recorded'").all(),
    ),
  );
  assert.deepEqual(first, {
    leased: facts.length,
    appended: facts.length,
    failed: 0,
    duplicates: 0,
  });
  const signed = db
    .prepare(
      'SELECT event_id, created_at, agent_signature, payload_plain_hash FROM _ves_outbox ORDER BY event_id',
    )
    .all();

  // Fault injection: signing committed but kernel settlement did not. Reopen both
  // objects to prove replay uses persisted identities and timestamps.
  db.prepare("UPDATE kernel_outbox SET published_at = NULL WHERE tier = 'recorded'").run();
  db.close();
  db = new Database(path);
  engine = new SyncEngine({ db, config, keyManager, preferGrpc: false });
  pump = createOutboxPump(db, engine.outbox, { identity });
  assert.deepEqual(await pump.drain(1000), {
    leased: facts.length,
    appended: 0,
    failed: 0,
    duplicates: facts.length,
  });
  assert.deepEqual(
    db
      .prepare(
        'SELECT event_id, created_at, agent_signature, payload_plain_hash FROM _ves_outbox ORDER BY event_id',
      )
      .all(),
    signed,
  );
  assert.deepEqual(
    db.prepare("SELECT * FROM kernel_outbox WHERE tier = 'governed' ORDER BY id").all(),
    governed,
  );

  const byId = new Map(facts.map((row) => [uuidv5(VES_OUTBOX_NAMESPACE, row.id), row]));
  let requests = 0;
  const client = new SequencerClient(config);
  client._request = async (method, route, payload) => {
    assert.equal(method, 'POST');
    assert.equal(route, '/api/v1/ves/events/ingest');
    requests++;
    // Exercise the JSON conversion a real REST request performs.
    const wire = JSON.parse(JSON.stringify(payload));
    assert.equal(wire.events.length, facts.length);
    for (const event of wire.events) {
      const fact = byId.get(event.event_id);
      assert.ok(fact);
      assert.equal(event.event_type, fact.event_type);
      assert.equal(event.created_at, fact.created_at);
      assert.deepEqual(event.payload, JSON.parse(fact.payload));
      assert.deepEqual(
        hexToBuffer(event.payload_plain_hash),
        computePayloadPlainHash(event.payload),
      );
      const hash = computeEventSigningHash({
        vesVersion: event.ves_version,
        tenantId: event.tenant_id,
        storeId: event.store_id,
        eventId: event.event_id,
        sourceAgentId: event.source_agent_id,
        agentKeyId: event.agent_key_id,
        entityType: event.entity_type,
        entityId: event.entity_id,
        eventType: event.event_type,
        createdAt: event.created_at,
        payloadKind: event.payload_kind,
        payloadPlainHash: hexToBuffer(event.payload_plain_hash),
        payloadCipherHash: hexToBuffer(event.payload_cipher_hash),
      });
      assert.equal(crypto.verify(null, hash, publicKey, hexToBuffer(event.agent_signature)), true);
      assert.equal(
        verifyEventSignatureHybrid(hash, event.agent_signature_bundle, {
          ed25519PublicKey: signing.ed25519PublicKey,
          mlDsa65PublicKey: signing.mlDsa65PublicKey,
        }),
        true,
      );
    }
    return {
      eventsAccepted: facts.length,
      eventsRejected: 0,
      sequenceStart: 1,
      sequenceEnd: facts.length,
      headSequence: facts.length,
    };
  };
  engine.client = client;
  const pushed = await engine.push({ batchSize: 1000 });
  assert.equal(pushed.success, true);
  assert.equal(pushed.pushed, facts.length);
  assert.equal(requests, 1);
  assert.equal(engine.outbox.getPending(1000).length, 0);
});
