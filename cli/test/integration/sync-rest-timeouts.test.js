import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import crypto from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import Database from 'better-sqlite3';
import { SyncConfig } from '../../src/sync/config.js';
import { SequencerClient } from '../../src/sync/client.js';
import { SyncEngine } from '../../src/sync/engine.js';
import { UnifiedSequencerClient } from '../../src/sync/unified-client.js';

async function endpoint(t, handler) {
  const server = createServer(handler);
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  t.after(async () => {
    server.closeAllConnections();
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  });
  return new SyncConfig({
    sequencer: { url: `http://127.0.0.1:${server.address().port}`, insecure: true },
    identity: {
      tenantId: crypto.randomUUID(),
      storeId: crypto.randomUUID(),
      agentId: crypto.randomUUID(),
    },
    sync: {
      securityProfile: 'legacy',
      requestTimeoutMs: 500,
      retryPolicy: { maxRetries: 0, baseDelay: 1, maxDelay: 1 },
    },
  });
}

for (const stage of ['headers', 'success body', 'error body']) {
  test(
    `REST aborts a stalled ${stage} and a later request still succeeds`,
    { timeout: 10000 },
    async (t) => {
      let requests = 0;
      let aborted;
      const closed = new Promise((resolve) => {
        aborted = resolve;
      });
      const config = await endpoint(t, (req, res) => {
        requests++;
        if (requests > 1) {
          res.end(JSON.stringify({ head_sequence: 7 }));
          return;
        }
        res.on('close', aborted);
        if (stage !== 'headers') {
          res.writeHead(stage === 'error body' ? 503 : 200, { 'content-type': 'application/json' });
          res.write('{"head_sequence":');
        }
      });
      const client = new SequencerClient(config);
      await assert.rejects(client.getHead(), (error) => error.code === 'SEQUENCER_TIMEOUT');
      await closed;
      assert.equal(requests, 1);
      assert.equal((await client.getHead()).headSequence, 7);
    },
  );
}

test(
  'a timed-out attempt receives a fresh deadline for its configured retry',
  { timeout: 10000 },
  async (t) => {
    let requests = 0;
    const config = await endpoint(t, (req, res) => {
      req.resume();
      requests++;
      if (requests === 2)
        res.end(JSON.stringify({ eventsAccepted: 0, eventsRejected: 0, headSequence: 0 }));
    });
    config.sync.retryPolicy.maxRetries = 1;
    const client = new SequencerClient(config);
    const receipt = await client.pushWithRetry({ agentId: config.agentId, events: [] });
    assert.equal(requests, 2);
    assert.equal(receipt.eventsAccepted, 0);
  },
);

for (const interruption of ['timeout', 'shutdown']) {
  test(
    `${interruption} during a push leaves the exact signed event pending across restart`,
    { timeout: 10000 },
    async (t) => {
      const received = [];
      const firstRequest = Promise.withResolvers();
      const receipt = {
        eventsAccepted: 1,
        eventsRejected: 0,
        sequenceStart: 1,
        sequenceEnd: 1,
        headSequence: 1,
        rejections: [],
      };
      const config = await endpoint(t, (req, res) => {
        let body = '';
        req.setEncoding('utf8');
        req.on('data', (chunk) => {
          body += chunk;
        });
        req.on('end', () => {
          received.push(JSON.parse(body));
          firstRequest.resolve();
          // Simulate a receiver that accepted the event but lost the first reply.
          if (received.length > 1) res.end(JSON.stringify(receipt));
        });
      });
      if (interruption === 'shutdown') config.sync.requestTimeoutMs = 30000;
      const root = mkdtempSync(join(tmpdir(), 'stateset-rest-timeout-'));
      let db = new Database(join(root, 'store.db'));
      t.after(() => {
        db.close();
        rmSync(root, { recursive: true, force: true });
      });
      const pair = crypto.generateKeyPairSync('ed25519');
      const keyManager = {
        getCurrentSigningKey: async () => ({
          keyId: 1,
          publicKey: pair.publicKey.export({ type: 'spki', format: 'der' }).subarray(-32),
          privateKey: pair.privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32),
        }),
      };
      const build = () => {
        const engine = new SyncEngine({ db, config, keyManager });
        engine.client = new SequencerClient(config);
        engine.on('error', () => {});
        return engine;
      };
      let engine = build();
      await engine.outbox.append({
        eventId: crypto.randomUUID(),
        tenantId: config.tenantId,
        storeId: config.storeId,
        sourceAgent: config.agentId,
        entityType: 'order',
        entityId: 'order-1',
        eventType: 'order.created',
        payload: { amount: '19.99' },
      });
      const before = db.prepare('SELECT * FROM _ves_outbox').all();
      const pushing = engine.push();
      if (interruption === 'shutdown') {
        await firstRequest.promise;
        await engine.shutdown();
      }
      const failed = await pushing;
      assert.equal(failed.success, false);
      assert.match(failed.error, interruption === 'shutdown' ? /disconnected/ : /timed out/);
      assert.equal(received.length, 1);
      assert.deepEqual(db.prepare('SELECT * FROM _ves_outbox').all(), before);
      assert.equal(engine.outbox.getSyncState().lastPushedSequence, 0);
      db.close();
      db = new Database(join(root, 'store.db'));
      engine = build();
      assert.deepEqual(db.prepare('SELECT * FROM _ves_outbox').all(), before);
      assert.equal((await engine.push()).success, true);
      assert.equal(received.length, 2);
      assert.deepEqual(received[1], received[0]);
      assert.equal(engine.outbox.getPending().length, 0);
      assert.equal(engine.outbox.getSyncState().lastPushedSequence, 1);
    },
  );
}

for (const stage of ['headers', 'success body', 'error body']) {
  test(`disconnect cancels an active REST ${stage}`, { timeout: 5000 }, async (t) => {
    const received = Promise.withResolvers();
    const closed = Promise.withResolvers();
    let requests = 0;
    const config = await endpoint(t, (req, res) => {
      requests++;
      if (requests > 1) {
        res.end('{}');
        return;
      }
      res.on('close', closed.resolve);
      if (stage !== 'headers') {
        res.writeHead(stage === 'error body' ? 503 : 200);
        res.write('{');
      }
      received.resolve();
    });
    config.sync.requestTimeoutMs = 30000;
    const client = new SequencerClient(config);
    const rejected = assert.rejects(client.getHead(), { code: 'SEQUENCER_DISCONNECTED' });
    await received.promise;
    await client.disconnect();
    await rejected;
    await closed.promise;
    await assert.rejects(client.getHead(), { code: 'SEQUENCER_DISCONNECTED' });
    assert.equal(requests, 1);
    await client.connect();
    assert.equal(client.isConnected(), true);
    assert.equal(requests, 2);
  });
}

test(
  'unified disconnect cancels a pending connection without resurrecting it on reconnect',
  { timeout: 5000 },
  async (t) => {
    const received = Promise.withResolvers();
    let requests = 0;
    const config = await endpoint(t, (req, res) => {
      requests++;
      if (requests === 1) received.resolve();
      else res.end('{}');
    });
    config.sync.requestTimeoutMs = 30000;
    const client = new UnifiedSequencerClient({ config, preferGrpc: false });
    t.after(() => client.disconnect());
    let connected = 0;
    client.on('connected', () => connected++);
    const first = assert.rejects(client.connect(), { code: 'SEQUENCER_DISCONNECTED' });
    const shared = assert.rejects(client.connect(), { code: 'SEQUENCER_DISCONNECTED' });
    await received.promise;
    await client.disconnect();
    const reconnect = client.connect();
    await Promise.all([first, shared, reconnect]);
    assert.equal(requests, 2);
    assert.equal(connected, 1);
    assert.equal(client.isConnected(), true);
    assert.equal(client.transport, 'rest');
  },
);

test(
  'disconnect before transport discovery completes starts no connection',
  { timeout: 5000 },
  async (t) => {
    let requests = 0;
    const config = await endpoint(t, (req, res) => {
      requests++;
      res.end('{}');
    });
    const client = new UnifiedSequencerClient({ config, preferGrpc: false });
    const rejected = assert.rejects(client.connect(), { code: 'SEQUENCER_DISCONNECTED' });
    await client.disconnect();
    await rejected;
    assert.equal(requests, 0);
    assert.equal(client.isConnected(), false);
    assert.equal(client.transport, null);
  },
);
