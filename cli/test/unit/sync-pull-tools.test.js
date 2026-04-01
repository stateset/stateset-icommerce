/**
 * Tests for sync_pull tool handler response shaping.
 */

import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs/promises';
import os from 'os';
import path from 'path';

import { syncTools } from '../../src/tools/sync.js';
import { createSyncConfig, saveSyncConfig } from '../../src/sync/config.js';
import { SyncEngine } from '../../src/sync/engine.js';
import { KEY_WRAP_SCHEME_X25519_ML_KEM_768 } from '../../src/sync/pqc.js';

const TENANT_ID = '550e8400-e29b-41d4-a716-446655440201';
const STORE_ID = '550e8400-e29b-41d4-a716-446655440202';
const REMOTE_AGENT_ID = '550e8400-e29b-41d4-a716-446655440204';

const mockDb = {
  prepare: () => ({
    run: () => ({ changes: 1 }),
    get: () => null,
    all: () => [],
  }),
  exec: () => {},
};

function findTool(name) {
  return syncTools.find((tool) => tool.name === name);
}

function stubSyncEngine(overrides) {
  const methodNames = ['initialize', 'pull', 'shutdown', 'getStoredEvent', 'decryptStoredEvent'];
  const originals = Object.fromEntries(
    methodNames.map((methodName) => [methodName, SyncEngine.prototype[methodName]]),
  );

  for (const methodName of methodNames) {
    if (Object.hasOwn(overrides, methodName)) {
      SyncEngine.prototype[methodName] = overrides[methodName];
    }
  }

  return () => {
    for (const methodName of methodNames) {
      SyncEngine.prototype[methodName] = originals[methodName];
    }
  };
}

async function createTempSyncEnv() {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'sync-pull-tool-'));
  const config = createSyncConfig({
    sequencerUrl: 'https://sequencer.example.com',
    tenantId: TENANT_ID,
    storeId: STORE_ID,
    securityProfile: 'hybrid',
  });
  saveSyncConfig(config, tempDir);
  return { tempDir, config };
}

describe('sync_pull — Handler', () => {
  let originalCwd;
  let tempDir;

  before(async () => {
    originalCwd = process.cwd();
    ({ tempDir } = await createTempSyncEnv());
    process.chdir(tempDir);
  });

  after(async () => {
    process.chdir(originalCwd);
    await fs.rm(tempDir, { recursive: true, force: true });
  });

  it('includes stored pulled events and plaintext payloads when requested', async () => {
    const restore = stubSyncEngine({
      async initialize() {},
      async shutdown() {},
      async pull(options) {
        assert.equal(options.fromSequence, 10);
        assert.equal(options.limit, 5);
        assert.equal(options.includeEvents, true);
        return {
          success: true,
          pulled: 1,
          applied: 1,
          conflicts: 0,
          sequenceNumbers: [12],
        };
      },
      getStoredEvent({ sequenceNumber, source }) {
        assert.equal(sequenceNumber, 12);
        assert.equal(source, 'pulled');
        return {
          source: 'pulled',
          sequenceNumber,
          eventId: 'evt-12',
          entityType: 'order',
          entityId: 'ord-12',
          eventType: 'order.updated',
          sourceAgent: REMOTE_AGENT_ID,
          createdAt: '2026-03-31T01:00:00.000Z',
          sequencedAt: '2026-03-31T01:00:05.000Z',
          payloadKind: 0,
          payload: { orderId: 'ord-12', status: 'paid' },
          payloadEncrypted: null,
        };
      },
    });

    try {
      const tool = findTool('sync_pull');
      const result = await tool.handler({
        commerce: { db: mockDb },
        params: {
          fromSequence: 10,
          limit: 5,
          includeEvents: true,
          includePayloads: true,
        },
      });

      assert.equal(result.success, true);
      assert.equal(result.includeEvents, true);
      assert.equal(result.includePayloads, true);
      assert.equal(result.decryptPayloads, false);
      assert.equal(result.events.length, 1);
      assert.deepEqual(result.events[0].payload, { orderId: 'ord-12', status: 'paid' });
      assert.equal(result.events[0].encrypted, false);
    } finally {
      restore();
    }
  });

  it('implicitly includes events and decrypts encrypted pulled payloads', async () => {
    const restore = stubSyncEngine({
      async initialize() {},
      async shutdown() {},
      async pull(options) {
        assert.equal(options.includeEvents, true);
        return {
          success: true,
          pulled: 1,
          applied: 1,
          conflicts: 0,
          sequenceNumbers: [21],
        };
      },
      getStoredEvent({ sequenceNumber, source }) {
        assert.equal(sequenceNumber, 21);
        assert.equal(source, 'pulled');
        return {
          source: 'pulled',
          sequenceNumber,
          eventId: 'evt-21',
          entityType: 'order',
          entityId: 'ord-21',
          eventType: 'order.updated',
          sourceAgent: REMOTE_AGENT_ID,
          createdAt: '2026-03-31T02:00:00.000Z',
          sequencedAt: '2026-03-31T02:00:05.000Z',
          payloadKind: 1,
          payload: null,
          payloadEncrypted: {
            keyWrapParams: { scheme: KEY_WRAP_SCHEME_X25519_ML_KEM_768 },
          },
        };
      },
      async decryptStoredEvent({ sequenceNumber, source, keyId }) {
        assert.equal(sequenceNumber, 21);
        assert.equal(source, 'pulled');
        assert.equal(keyId, 7);
        return {
          payload: { orderId: 'ord-21', secretTotal: 199 },
          encryptionProfile: 'hybrid',
          wrapScheme: KEY_WRAP_SCHEME_X25519_ML_KEM_768,
          recipientKeyId: 7,
          recipientKeyCandidates: [7],
        };
      },
    });

    try {
      const tool = findTool('sync_pull');
      const result = await tool.handler({
        commerce: { db: mockDb },
        params: {
          limit: 3,
          decryptPayloads: true,
          keyId: 7,
        },
      });

      assert.equal(result.success, true);
      assert.equal(result.includeEvents, true);
      assert.equal(result.includePayloads, true);
      assert.equal(result.decryptPayloads, true);
      assert.equal(result.events.length, 1);
      assert.deepEqual(result.events[0].payload, { orderId: 'ord-21', secretTotal: 199 });
      assert.equal(result.events[0].encryptionProfile, 'hybrid');
      assert.equal(result.events[0].recipientKeyId, 7);
    } finally {
      restore();
    }
  });

  it('surfaces per-event decryption errors without failing the pull', async () => {
    const restore = stubSyncEngine({
      async initialize() {},
      async shutdown() {},
      async pull() {
        return {
          success: true,
          pulled: 1,
          applied: 1,
          conflicts: 0,
          sequenceNumbers: [22],
        };
      },
      getStoredEvent({ sequenceNumber }) {
        return {
          source: 'pulled',
          sequenceNumber,
          eventId: 'evt-22',
          entityType: 'order',
          entityId: 'ord-22',
          eventType: 'order.updated',
          sourceAgent: REMOTE_AGENT_ID,
          createdAt: '2026-03-31T02:05:00.000Z',
          sequencedAt: '2026-03-31T02:05:05.000Z',
          payloadKind: 1,
          payload: null,
          payloadEncrypted: {
            keyWrapParams: { scheme: KEY_WRAP_SCHEME_X25519_ML_KEM_768 },
          },
        };
      },
      async decryptStoredEvent() {
        throw new Error('Encryption key 99 is not listed as a recipient for this event');
      },
    });

    try {
      const tool = findTool('sync_pull');
      const result = await tool.handler({
        commerce: { db: mockDb },
        params: {
          decryptPayloads: true,
        },
      });

      assert.equal(result.success, true);
      assert.equal(result.events.length, 1);
      assert.equal(result.events[0].payload, undefined);
      assert.match(result.events[0].decryptionError, /not listed as a recipient/i);
    } finally {
      restore();
    }
  });
});
