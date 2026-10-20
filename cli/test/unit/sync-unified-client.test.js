import { test } from 'node:test';
import assert from 'node:assert/strict';
import { UnifiedSequencerClient } from '../../src/sync/unified-client.js';
import { GrpcSequencerClient } from '../../src/sync/grpc-client.js';
import { SyncConfig } from '../../src/sync/config.js';

// Node 20 (the supported floor) has no Promise.withResolvers.
function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function config() {
  return new SyncConfig({
    sequencer: { url: 'grpc://localhost:50051', insecure: true },
    identity: { tenantId: 'tenant', storeId: 'store', agentId: 'agent' },
    sync: { securityProfile: 'legacy' },
  });
}

test('retired gRPC callbacks cannot publish into a replacement unified connection', async (t) => {
  const started = deferred();
  const release = deferred();
  const retired = [];
  let first;
  t.mock.method(GrpcSequencerClient.prototype, 'connect', async function () {
    if (!first) {
      first = this;
      started.resolve();
      await release.promise;
    }
    this.connected = true;
    this.emit('connected');
  });
  t.mock.method(GrpcSequencerClient.prototype, 'disconnect', function () {
    retired.push(this);
    this.connected = false;
    this.emit('disconnected');
  });
  const client = new UnifiedSequencerClient({ config: config() });
  client._grpcAvailable = true;
  t.after(() => client.disconnect());
  const events = [];
  for (const event of ['connected', 'disconnected', 'error', 'event', 'push-ack', 'sync-state']) {
    client.on(event, () => events.push(event));
  }
  const cancelled = assert.rejects(client.connect(), { code: 'SEQUENCER_DISCONNECTED' });
  await started.promise;
  assert.equal(client.isConnected(), false);
  await client.disconnect();
  await client.connect();
  const active = client._client;
  assert.notEqual(active, first);
  release.resolve();
  await cancelled;
  for (const event of ['error', 'event', 'push-ack', 'sync-state'])
    first.emit(event, new Error('stale'));
  assert.deepEqual(events, ['disconnected', 'connected']);
  assert.equal(client.isConnected(), true);
  assert.equal(client._client, active);
  assert.equal(retired.filter((c) => c === first).length, 2);
  assert.equal(retired.includes(active), false);
});

test('a failed unified connection closes its transport before a later connect', async (t) => {
  let attempts = 0;
  const retired = [];
  t.mock.method(GrpcSequencerClient.prototype, 'connect', async function () {
    attempts++;
    if (attempts === 1) throw new Error('test handshake failed');
    this.connected = true;
  });
  t.mock.method(GrpcSequencerClient.prototype, 'disconnect', function () {
    retired.push(this);
    this.connected = false;
  });
  const client = new UnifiedSequencerClient({ config: config() });
  client._grpcAvailable = true;
  t.after(() => client.disconnect());
  await assert.rejects(client.connect(), /test handshake failed/);
  assert.equal(client.isConnected(), false);
  assert.equal(client._client, null);
  assert.ok(retired.length > 0);
  await client.connect();
  assert.equal(client.isConnected(), true);
  assert.equal(retired.includes(client._client), false);
});
