/**
 * Side-table stores on the real engine's side store.
 *
 * Every store here keeps auxiliary tables in the engine's store file. With an
 * engine in the process they must run on `commerce.openSideStore()` -- the
 * engine's own SQLite library -- never on a second library such as
 * better-sqlite3, whose close drops the engine's file locks and unlinks its
 * WAL. These tests drive each migrated store against `@stateset/embedded`
 * (this checkout's binding) and check the engine's file is intact afterwards.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';

import { engineSideStore, resolveSideDb, supportsSideStore } from '../../src/side-store.js';
import { createOutbox } from '../../src/sync/outbox.js';
import { createOutboxPump, VES_OUTBOX_NAMESPACE, uuidv5 } from '../../src/sync/outbox-pump.js';
import { EngineSyncDatabase, openSyncDatabase } from '../../src/sync/store-db.js';
import { syncTools } from '../../src/tools/sync.js';
import { execute as executeSyncCommand } from '../../src/commands/sync.js';
import { createComplianceService } from '../../src/compliance/exports.js';
import { KernelMarketplaceBridge, SqliteBridgeStore } from '../../src/marketplace/kernel-bridge.js';
import { IdMapStore } from '../../src/adapters/id-map-store.js';
import { createExpressCheckout } from '../../src/checkout/express.js';
import { createAgentCatalog } from '../../src/catalog/agent-catalog.js';
import { adaptCommerceApis } from '../../src/commerce.js';
import { adaptCommerceForTools, extendCommerceWithApis } from '../../src/mcp/commerce-adapter.js';
import { A2AStore } from '../../src/a2a/store.js';

const require = createRequire(import.meta.url);
const { Commerce } = require('@stateset/embedded');

const IDENTITY = {
  tenantId: '22222222-2222-2222-2222-222222222222',
  storeId: '33333333-3333-3333-3333-333333333333',
  agentId: '44444444-4444-4444-4444-444444444444',
};

function rawKeyPair(type) {
  const { publicKey, privateKey } = crypto.generateKeyPairSync(type);
  return {
    publicKey: Buffer.from(publicKey.export({ type: 'spki', format: 'der' }).subarray(-32)),
    privateKey: Buffer.from(privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32)),
  };
}

/** In-memory signing/encryption keys, stable for the life of the manager. */
function keyManager() {
  const signing = rawKeyPair('ed25519');
  const encryption = rawKeyPair('x25519');
  return {
    getCurrentSigningKey: async () => ({
      keyId: 1,
      ...signing,
      createdAt: '2026-11-17T00:00:00.000Z',
    }),
    getCurrentEncryptionKey: async () => ({
      keyId: 1,
      ...encryption,
      createdAt: '2026-11-17T00:00:00.000Z',
    }),
  };
}

function event(overrides = {}) {
  return {
    tenantId: IDENTITY.tenantId,
    storeId: IDENTITY.storeId,
    entityType: 'order',
    entityId: 'ord-1',
    eventType: 'order.created',
    payload: { orderId: 'ord-1', total: '19.99' },
    sourceAgent: IDENTITY.agentId,
    ...overrides,
  };
}

function tempStore(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-side-store-cli-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  return path.join(dir, 'store.db');
}

async function customer(commerce, email) {
  return commerce.customers.create({ email, firstName: 'Side', lastName: 'Store' });
}

/**
 * Close the engine, reopen the file with a fresh one, and check it is intact:
 * every customer the test created is there and SQLite's integrity check
 * passes. A second SQLite library in the process would have lost the later
 * commits.
 */
async function assertFileIntact(commerce, file, emails) {
  await commerce.close();
  const reopened = await Commerce.open(file);
  try {
    const seen = (await reopened.customers.list({ limit: 1000 })).map((c) => c.email).sort();
    assert.deepEqual(seen, [...emails].sort());
    assert.equal(reopened.openSideStore().pragma('integrity_check', { simple: true }), 'ok');
  } finally {
    await reopened.close();
  }
}

describe('engine side store helper', () => {
  it('shares one side store per engine and leaves closing to the engine', async () => {
    const commerce = new Commerce(':memory:');
    assert.equal(supportsSideStore(commerce), true);
    const first = engineSideStore(commerce);
    assert.equal(engineSideStore(commerce), first);
    assert.equal(resolveSideDb({ commerce }), first);
    // An explicit caller-owned handle, then commerce.db, win over the engine.
    const explicit = { prepare() {} };
    assert.equal(resolveSideDb({ db: explicit, commerce }), explicit);
    assert.equal(resolveSideDb({ commerce: { db: explicit } }), explicit);
    // A side store closed by someone else is replaced, not handed out dead.
    first.close();
    const second = engineSideStore(commerce);
    assert.notEqual(second, first);
    assert.equal(second.open, true);
    await commerce.close();
    assert.equal(second.open, false);
    assert.throws(
      () => engineSideStore(commerce),
      (error) => error.code === 'PRECONDITION_FAILED',
    );
  });

  it('is one store per native engine, whatever wrappers a host builds around it', async () => {
    const native = new Commerce(':memory:');
    // stateset-mcp-http builds a fresh wrapper chain like this for every
    // request; a store per wrapper would leak one connection per request.
    const wrap = () =>
      adaptCommerceApis(extendCommerceWithApis(adaptCommerceForTools(native), { a2a: {} }));
    const first = wrap();
    const second = wrap();
    try {
      const side = engineSideStore(first);
      assert.equal(engineSideStore(second), side);
      assert.equal(engineSideStore(native), side);
      assert.equal(native.sharedSideStore(), side);
      assert.deepEqual(side.prepare('SELECT count(*) AS n FROM customers').get(), { n: 0 });
    } finally {
      await native.close();
    }
    assert.equal(supportsSideStore({ orders: {} }), false);
    assert.equal(engineSideStore({ orders: {} }), null);
    assert.equal(resolveSideDb({ commerce: { db: {} } }), null);
    assert.equal(resolveSideDb(), null);
  });
});

describe('sync outbox family on the engine side store', () => {
  it('appends, batches, settles and replays outbox events on the engine file', async (t) => {
    const file = tempStore(t);
    const commerce = await Commerce.open(file);
    await customer(commerce, 'before@example.com');
    const outbox = createOutbox(engineSideStore(commerce), { keyManager: keyManager() });

    const first = await outbox.append(event());
    assert.equal(typeof first, 'number');
    const batch = await outbox.appendBatch([
      event({ entityId: 'ord-2' }),
      event({ entityId: 'ord-3', eventType: 'order.updated' }),
    ]);
    assert.equal(batch.length, 2);
    assert.equal(outbox.getPendingCount(), 3);
    assert.deepEqual(
      outbox.getPending().map((pending) => pending.entityId),
      ['ord-1', 'ord-2', 'ord-3'],
    );

    // A duplicate event id surfaces SQLite's constraint code, which the pump
    // matches structurally.
    const [existing] = outbox.getPending(1);
    await assert.rejects(outbox.append(event({ eventId: existing.eventId })), (error) =>
      String(error.code).startsWith('SQLITE_CONSTRAINT'),
    );

    assert.equal(outbox.markSynced([{ localSeq: first, sequenceNumber: 10 }]), 1);
    assert.equal(outbox.markFailed(batch[0], 'sequencer offline'), true);
    assert.equal(outbox.retryFailed(), 1);
    const stats = outbox.getStats();
    assert.equal(stats.pending, 2);
    assert.equal(stats.synced, 1);

    outbox.updateSyncState({ agentId: IDENTITY.agentId, lastPushedSequence: 10 });
    assert.equal(outbox.getSyncState().lastPushedSequence, 10);
    outbox.updateEntityVersion(IDENTITY.tenantId, IDENTITY.storeId, 'order', 'ord-1', 3);
    assert.equal(outbox.getEntityVersion(IDENTITY.tenantId, IDENTITY.storeId, 'order', 'ord-1'), 3);

    // The engine kept committing throughout, and the file is intact.
    await customer(commerce, 'after@example.com');
    await assertFileIntact(commerce, file, ['before@example.com', 'after@example.com']);
  });

  it('drains the engine’s own recorded kernel_outbox facts through the side store', async (t) => {
    const file = tempStore(t);
    const commerce = await Commerce.open(file);
    const created = await customer(commerce, 'pumped@example.com');
    const side = engineSideStore(commerce);
    const outbox = createOutbox(side, { keyManager: keyManager() });
    const pump = createOutboxPump(side, outbox, { identity: IDENTITY });

    const facts = side
      .prepare("SELECT id, event_type FROM kernel_outbox WHERE tier = 'recorded'")
      .all();
    const fact = facts.find((row) => row.event_type === 'customer.created');
    assert.ok(fact, `the engine records customer.created; saw ${JSON.stringify(facts)}`);

    const result = await pump.drain();
    assert.ok(result.appended >= 1, JSON.stringify(result));
    const envelope = outbox.getByEventId(uuidv5(VES_OUTBOX_NAMESPACE, fact.id));
    assert.equal(envelope?.eventType, 'customer.created');
    assert.equal(envelope.entityId, created.id);
    assert.ok(
      side.prepare('SELECT published_at FROM kernel_outbox WHERE id = ?').get(fact.id).published_at,
    );

    // A replayed drain is absorbed: nothing new is appended.
    const replay = await pump.drain();
    assert.equal(replay.appended, 0);

    await customer(commerce, 'later@example.com');
    await assertFileIntact(commerce, file, ['pumped@example.com', 'later@example.com']);
  });

  it('serves the sync_outbox tool and the sync outbox command from a binding Commerce', async (t) => {
    const cwd = process.cwd();
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-side-store-sync-'));
    // isSyncConfigured() only checks that .stateset/sync.json exists.
    fs.mkdirSync(path.join(dir, '.stateset'));
    fs.writeFileSync(path.join(dir, '.stateset', 'sync.json'), '{}');
    process.chdir(dir);
    t.after(() => {
      process.chdir(cwd);
      fs.rmSync(dir, { recursive: true, force: true });
    });
    const commerce = new Commerce(':memory:');
    t.after(() => commerce.close());
    const tool = syncTools.find((candidate) => candidate.name === 'sync_outbox');
    assert.deepEqual(await tool.handler({ commerce, params: {} }), {
      count: 0,
      filter: 'all',
      events: [],
    });
    const outbox = createOutbox(engineSideStore(commerce), { keyManager: keyManager() });
    await outbox.append(event());
    const listed = await tool.handler({ commerce, params: {} });
    assert.equal(listed.count, 1);
    assert.equal(listed.events[0].entityId, 'ord-1');

    const command = await executeSyncCommand('outbox', [], { commerce, jsonOutput: true });
    assert.deepEqual(
      command.map((row) => row.entityId),
      ['ord-1'],
    );
  });

  it('openSyncDatabase opens stateset-sync stores through the engine', async (t) => {
    const file = tempStore(t);
    const db = await openSyncDatabase(file);
    assert.ok(db instanceof EngineSyncDatabase);
    const outbox = createOutbox(db, { keyManager: keyManager() });
    await outbox.append(event());
    assert.equal(db.prepare('SELECT count(*) AS n FROM _ves_outbox').get().n, 1);
    assert.equal(db.pragma('journal_mode', { simple: true }), 'wal');
    // The file is the engine's own store.
    assert.equal(db.prepare('SELECT count(*) AS n FROM customers').get().n, 0);
    db.close();
    db.close();
    await db.closed;
    assert.equal(db.open, false);
    assert.equal(fs.existsSync(`${file}-wal`), false, 'closing released the file');

    const reopened = await openSyncDatabase(file);
    try {
      assert.equal(createOutbox(reopened, { keyManager: keyManager() }).getPendingCount(), 1);
    } finally {
      await reopened.close().closed;
    }

    // Without a loadable engine it falls back to the caller's SQLite library.
    const opened = [];
    class FakeDatabase {
      constructor(dbPath) {
        opened.push(dbPath);
      }
    }
    const fallback = await openSyncDatabase('/tmp/x.db', {
      loadEngine: () => null,
      loadDatabase: async () => FakeDatabase,
    });
    assert.ok(fallback instanceof FakeDatabase);
    assert.deepEqual(opened, ['/tmp/x.db']);
  });
});

describe('compliance exports on the engine side store', () => {
  it('reads and erases commerce tables through the engine, not commerceDbPath', async (t) => {
    const file = tempStore(t);
    const commerce = await Commerce.open(file);
    const subject = await customer(commerce, 'gdpr@example.com');
    await customer(commerce, 'keep@example.com');

    // The A2A half stays on its own (separate, in-memory) store here.
    const a2a = new A2AStore({ dbPath: ':memory:' });
    a2a.init();
    t.after(() => a2a.close());
    const service = createComplianceService(
      a2a,
      // A path that does not exist: commerce data can only come from the
      // engine's side store.
      { commerce, commerceDbPath: `${file}.missing` },
    );

    const exported = service.generateGDPRExport(subject.id);
    assert.equal(exported.commerceData.customers[0].email, 'gdpr@example.com');

    const erased = service.deleteGDPRData(subject.id);
    assert.ok(
      erased.retained.some((entry) => entry.table === 'customers'),
      JSON.stringify(erased),
    );
    const side = engineSideStore(commerce);
    const emails = side.prepare('SELECT email FROM customers ORDER BY email').all();
    assert.equal(
      emails.some((row) => row.email === 'gdpr@example.com'),
      false,
    );
    assert.ok(emails.some((row) => row.email === 'keep@example.com'));
    // close() leaves the engine's side store alone: it is the engine's.
    service.close();
    assert.equal(side.open, true);

    await customer(commerce, 'after@example.com');
    await commerce.close();
    const reopened = await Commerce.open(file);
    try {
      const seen = (await reopened.customers.list({ limit: 100 })).map((c) => c.email);
      assert.ok(seen.includes('after@example.com'), 'commits after the export were kept');
      assert.ok(seen.includes('keep@example.com'));
      assert.equal(reopened.openSideStore().pragma('integrity_check', { simple: true }), 'ok');
    } finally {
      await reopened.close();
    }
  });
});

describe('other side-table stores on the engine side store', () => {
  it('keeps the import id map in the engine file', async (t) => {
    const file = tempStore(t);
    const commerce = await Commerce.open(file);
    const created = await customer(commerce, 'imported@example.com');
    const ids = new IdMapStore(resolveSideDb({ commerce }));
    // Reads never create the table: list_id_mappings is a read tool.
    assert.deepEqual(ids.listByPlatform('shopify'), []);
    assert.equal(ids.lookup('shopify', 'customer', 'gid://shopify/Customer/1'), null);
    assert.equal(ids.count('shopify'), 0);
    const tables = () =>
      resolveSideDb({ commerce })
        .prepare("SELECT count(*) AS n FROM sqlite_master WHERE name = 'id_map'")
        .get().n;
    assert.equal(tables(), 0);
    ids.store('shopify', 'customer', 'gid://shopify/Customer/1', created.id, { source: 'test' });
    assert.equal(tables(), 1);
    assert.equal(
      ids.lookup('shopify', 'customer', 'gid://shopify/Customer/1').statesetId,
      created.id,
    );
    assert.equal(ids.listByPlatform('shopify').length, 1);
    await customer(commerce, 'after@example.com');
    await assertFileIntact(commerce, file, ['imported@example.com', 'after@example.com']);
  });

  it('runs the marketplace bridge inbox on the engine, durable across bridge rebuilds', async (t) => {
    const file = tempStore(t);
    const commerce = await Commerce.open(file);
    const store = new SqliteBridgeStore(commerce);
    assert.equal(store.db, engineSideStore(commerce));
    const claimed = store.claim('bridge-1', 'evt-1', 1, 'digest-1');
    assert.equal(claimed.execute, true);
    store.complete('bridge-1', 'evt-1', 'completed', { ok: true });
    store.advance('bridge-1', 2);

    const rebuilt = new SqliteBridgeStore(commerce);
    assert.equal(rebuilt.getCursor('bridge-1'), 2);
    assert.equal(rebuilt.claim('bridge-1', 'evt-1', 1, 'digest-1').execute, false);
    assert.equal(rebuilt.record('bridge-1', 'evt-1').status, 'completed');
    assert.throws(() => rebuilt.claim('bridge-1', 'evt-1', 1, 'tampered'), /changed after receipt/);
    assert.throws(() => new SqliteBridgeStore({}), /openSideStore/);
    assert.equal(typeof KernelMarketplaceBridge, 'function');

    await customer(commerce, 'bridge@example.com');
    await assertFileIntact(commerce, file, ['bridge@example.com']);
  });

  it('express checkout links and the agent catalog run on a side store unchanged', async (t) => {
    const commerce = new Commerce(':memory:');
    t.after(() => commerce.close());
    const store = { db: engineSideStore(commerce) };

    const checkout = createExpressCheckout(store);
    const link = checkout.createPaymentLink({
      items: [{ name: 'Widget', quantity: 2, unitPrice: 5 }],
      currency: 'USD',
    });
    assert.equal(checkout.getPaymentLinkStatus(link.linkId).status, 'active');
    assert.equal(checkout.resolvePaymentLink(link.shortCode).link.id, link.linkId);

    const catalog = createAgentCatalog(store);
    const listing = catalog.publishProduct({
      productId: 'prod-1',
      name: 'Widget',
      capabilities: ['purchase'],
      maxPrice: 5,
    });
    assert.equal(listing.status, 'active');
    const found = catalog.queryProducts({ capability: 'purchase', maxPrice: 10 });
    assert.equal(found.total, 1);
    assert.equal(found.products[0].productId ?? found.products[0].product_id, 'prod-1');
  });
});
