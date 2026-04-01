/**
 * Tests for sync_entity_history local handler behavior.
 */

import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs/promises';
import os from 'os';
import path from 'path';

import Database from 'better-sqlite3';

import { syncTools } from '../../src/tools/sync.js';
import { createSyncConfig, saveSyncConfig } from '../../src/sync/config.js';
import { AgentKeyManager } from '../../src/sync/keys.js';
import { createOutbox } from '../../src/sync/outbox.js';
import {
  bufferToHex,
  computePayloadPlainHash,
  encryptPayloadHybrid,
  hasNativeHybridPqcDecryptionSupport,
  hasNativeHybridPqcSupport,
  ZERO_HASH,
} from '../../src/sync/crypto.js';

const TENANT_ID = '550e8400-e29b-41d4-a716-446655440301';
const STORE_ID = '550e8400-e29b-41d4-a716-446655440302';
const REMOTE_AGENT_ID = '550e8400-e29b-41d4-a716-446655440303';

function findTool(name) {
  return syncTools.find((tool) => tool.name === name);
}

async function createTempSyncEnv(securityProfile) {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), `sync-entity-history-${securityProfile}-`));
  const sequencerUrl =
    securityProfile === 'legacy'
      ? 'http://localhost:50051'
      : 'https://sequencer.example.com';
  const config = createSyncConfig({
    sequencerUrl,
    tenantId: TENANT_ID,
    storeId: STORE_ID,
    securityProfile,
  });
  saveSyncConfig(config, tempDir);
  return { tempDir, config };
}

describe('sync_entity_history — Local Handler', () => {
  let originalCwd;
  let tempDir;
  let config;
  let commerce;
  let outbox;

  before(async () => {
    originalCwd = process.cwd();
    ({ tempDir, config } = await createTempSyncEnv('legacy'));
    process.chdir(tempDir);

    commerce = { db: new Database(':memory:') };
    outbox = createOutbox(commerce.db);
  });

  after(async () => {
    commerce.db.close();
    process.chdir(originalCwd);
    await fs.rm(tempDir, { recursive: true, force: true });
  });

  it('returns local pulled history for an entity with plaintext payloads', async () => {
    const payloadA = { orderId: 'ord-local-entity-1', status: 'created' };
    const payloadB = { orderId: 'ord-local-entity-1', status: 'paid' };

    outbox.storePulledEvents([
      {
        sequenceNumber: 8,
        eventId: '550e8400-e29b-41d4-a716-446655440310',
        commandId: null,
        tenantId: config.identity.tenantId,
        storeId: config.identity.storeId,
        entityType: 'order',
        entityId: 'ord-local-entity-1',
        eventType: 'order.created',
        payload: payloadA,
        vesVersion: 1,
        payloadKind: 0,
        payloadEncrypted: null,
        payloadPlainHash: bufferToHex(computePayloadPlainHash(payloadA)),
        payloadCipherHash: bufferToHex(ZERO_HASH),
        agentKeyId: 5,
        agentSignature: '0x',
        agentSignatureScheme: 0,
        baseVersion: null,
        createdAt: '2026-03-31T03:00:00.000Z',
        sequencedAt: '2026-03-31T03:00:05.000Z',
        sourceAgent: REMOTE_AGENT_ID,
      },
      {
        sequenceNumber: 9,
        eventId: '550e8400-e29b-41d4-a716-446655440311',
        commandId: null,
        tenantId: config.identity.tenantId,
        storeId: config.identity.storeId,
        entityType: 'order',
        entityId: 'ord-local-entity-1',
        eventType: 'order.updated',
        payload: payloadB,
        vesVersion: 1,
        payloadKind: 0,
        payloadEncrypted: null,
        payloadPlainHash: bufferToHex(computePayloadPlainHash(payloadB)),
        payloadCipherHash: bufferToHex(ZERO_HASH),
        agentKeyId: 6,
        agentSignature: '0x',
        agentSignatureScheme: 0,
        baseVersion: null,
        createdAt: '2026-03-31T03:01:00.000Z',
        sequencedAt: '2026-03-31T03:01:05.000Z',
        sourceAgent: REMOTE_AGENT_ID,
      },
      {
        sequenceNumber: 10,
        eventId: '550e8400-e29b-41d4-a716-446655440312',
        commandId: null,
        tenantId: config.identity.tenantId,
        storeId: config.identity.storeId,
        entityType: 'order',
        entityId: 'ord-other-entity',
        eventType: 'order.updated',
        payload: { orderId: 'ord-other-entity', status: 'ignored' },
        vesVersion: 1,
        payloadKind: 0,
        payloadEncrypted: null,
        payloadPlainHash: bufferToHex(computePayloadPlainHash({ orderId: 'ord-other-entity', status: 'ignored' })),
        payloadCipherHash: bufferToHex(ZERO_HASH),
        agentKeyId: 7,
        agentSignature: '0x',
        agentSignatureScheme: 0,
        baseVersion: null,
        createdAt: '2026-03-31T03:02:00.000Z',
        sequencedAt: '2026-03-31T03:02:05.000Z',
        sourceAgent: REMOTE_AGENT_ID,
      },
    ]);

    const tool = findTool('sync_entity_history');
    const result = await tool.handler({
      commerce,
      params: {
        entityType: 'order',
        entityId: 'ord-local-entity-1',
        source: 'local',
        limit: 10,
        includePayloads: true,
      },
    });

    assert.equal(result.source, 'local');
    assert.equal(result.eventCount, 2);
    assert.equal(result.includePayloads, true);
    assert.equal(result.decryptPayloads, false);
    assert.deepEqual(
      result.events.map((event) => event.sequenceNumber),
      [8, 9],
    );
    assert.deepEqual(result.events[0].payload, payloadA);
    assert.deepEqual(result.events[1].payload, payloadB);
  });
});

describe(
  'sync_entity_history — Local Hybrid Handler',
  { skip: !hasNativeHybridPqcSupport() || !hasNativeHybridPqcDecryptionSupport() },
  () => {
    let originalCwd;
    let tempDir;
    let config;
    let commerce;
    let keyManager;
    let outbox;

    before(async () => {
      originalCwd = process.cwd();
      ({ tempDir, config } = await createTempSyncEnv('hybrid'));
      process.chdir(tempDir);

      commerce = { db: new Database(':memory:') };
      keyManager = new AgentKeyManager('.stateset', { securityProfile: 'hybrid' });
      outbox = createOutbox(commerce.db, { keyManager, securityProfile: 'hybrid' });
      await keyManager.ensureKeys(config.identity.agentId);
    });

    after(async () => {
      commerce.db.close();
      process.chdir(originalCwd);
      await fs.rm(tempDir, { recursive: true, force: true });
    });

    it('decrypts encrypted local entity history when requested', async () => {
      const encryptionKey = await keyManager.getCurrentEncryptionKey(config.identity.agentId);
      const payload = {
        orderId: 'ord-hybrid-entity-1',
        riskScore: 4,
      };
      const eventId = '550e8400-e29b-41d4-a716-446655440320';
      const createdAt = '2026-03-31T04:00:00.000Z';
      const encrypted = encryptPayloadHybrid(
        payload,
        {
          vesVersion: 1,
          tenantId: config.identity.tenantId,
          storeId: config.identity.storeId,
          eventId,
          sourceAgentId: REMOTE_AGENT_ID,
          agentKeyId: 12,
          entityType: 'order',
          entityId: 'ord-hybrid-entity-1',
          eventType: 'order.updated',
          createdAt,
        },
        [
          {
            kid: encryptionKey.keyId,
            x25519PublicKey: encryptionKey.publicKeyBundle.x25519PublicKey,
            mlKem768PublicKey: encryptionKey.publicKeyBundle.mlKem768PublicKey,
          },
        ],
      );

      outbox.storePulledEvent({
        sequenceNumber: 42,
        eventId,
        commandId: null,
        tenantId: config.identity.tenantId,
        storeId: config.identity.storeId,
        entityType: 'order',
        entityId: 'ord-hybrid-entity-1',
        eventType: 'order.updated',
        payload: null,
        vesVersion: 1,
        payloadKind: 1,
        payloadEncrypted: encrypted.payloadEncrypted,
        payloadPlainHash: bufferToHex(encrypted.payloadPlainHash),
        payloadCipherHash: bufferToHex(encrypted.payloadCipherHash),
        agentKeyId: 12,
        agentSignature: '0x',
        agentSignatureScheme: 0,
        baseVersion: null,
        createdAt,
        sequencedAt: '2026-03-31T04:00:05.000Z',
        sourceAgent: REMOTE_AGENT_ID,
      });

      const tool = findTool('sync_entity_history');
      const result = await tool.handler({
        commerce,
        params: {
          entityType: 'order',
          entityId: 'ord-hybrid-entity-1',
          source: 'local',
          decryptPayloads: true,
        },
      });

      assert.equal(result.source, 'local');
      assert.equal(result.eventCount, 1);
      assert.equal(result.includePayloads, true);
      assert.equal(result.decryptPayloads, true);
      assert.deepEqual(result.events[0].payload, payload);
      assert.equal(result.events[0].encryptionProfile, 'hybrid');
      assert.equal(result.events[0].recipientKeyId, encryptionKey.keyId);
    });

    it('reports local decryption errors per event', async () => {
      const encryptionKey = await keyManager.getCurrentEncryptionKey(config.identity.agentId);
      const extraKey = await keyManager.generateEncryptionKey(config.identity.agentId);
      const payload = { orderId: 'ord-hybrid-entity-2', total: 55 };
      const eventId = '550e8400-e29b-41d4-a716-446655440321';
      const createdAt = '2026-03-31T04:05:00.000Z';
      const encrypted = encryptPayloadHybrid(
        payload,
        {
          vesVersion: 1,
          tenantId: config.identity.tenantId,
          storeId: config.identity.storeId,
          eventId,
          sourceAgentId: REMOTE_AGENT_ID,
          agentKeyId: 13,
          entityType: 'order',
          entityId: 'ord-hybrid-entity-2',
          eventType: 'order.updated',
          createdAt,
        },
        [
          {
            kid: encryptionKey.keyId,
            x25519PublicKey: encryptionKey.publicKeyBundle.x25519PublicKey,
            mlKem768PublicKey: encryptionKey.publicKeyBundle.mlKem768PublicKey,
          },
        ],
      );

      outbox.storePulledEvent({
        sequenceNumber: 43,
        eventId,
        commandId: null,
        tenantId: config.identity.tenantId,
        storeId: config.identity.storeId,
        entityType: 'order',
        entityId: 'ord-hybrid-entity-2',
        eventType: 'order.updated',
        payload: null,
        vesVersion: 1,
        payloadKind: 1,
        payloadEncrypted: encrypted.payloadEncrypted,
        payloadPlainHash: bufferToHex(encrypted.payloadPlainHash),
        payloadCipherHash: bufferToHex(encrypted.payloadCipherHash),
        agentKeyId: 13,
        agentSignature: '0x',
        agentSignatureScheme: 0,
        baseVersion: null,
        createdAt,
        sequencedAt: '2026-03-31T04:05:05.000Z',
        sourceAgent: REMOTE_AGENT_ID,
      });

      const tool = findTool('sync_entity_history');
      const result = await tool.handler({
        commerce,
        params: {
          entityType: 'order',
          entityId: 'ord-hybrid-entity-2',
          source: 'local',
          decryptPayloads: true,
          keyId: extraKey.keyId,
        },
      });

      assert.equal(result.source, 'local');
      assert.equal(result.eventCount, 1);
      assert.equal(result.events[0].payload, undefined);
      assert.match(result.events[0].decryptionError, /not listed as a recipient/i);
    });
  },
);
