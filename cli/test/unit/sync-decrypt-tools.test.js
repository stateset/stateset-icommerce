/**
 * Tests for sync_decrypt_event tool definitions and handlers.
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
  encryptPayloadHybrid,
  hasNativeHybridPqcDecryptionSupport,
  hasNativeHybridPqcSupport,
} from '../../src/sync/crypto.js';

const TENANT_ID = '550e8400-e29b-41d4-a716-446655440001';
const STORE_ID = '550e8400-e29b-41d4-a716-446655440002';
const REMOTE_AGENT_ID = '550e8400-e29b-41d4-a716-446655440003';

function findTool(name) {
  return syncTools.find((tool) => tool.name === name);
}

async function createTempSyncEnv(securityProfile) {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), `sync-decrypt-${securityProfile}-`));
  const sequencerUrl =
    securityProfile === 'legacy' ? 'http://localhost:50051' : 'https://sequencer.example.com';
  const config = createSyncConfig({
    sequencerUrl,
    tenantId: TENANT_ID,
    storeId: STORE_ID,
    securityProfile,
    allowInsecureTransport: securityProfile === 'legacy',
  });
  saveSyncConfig(config, tempDir);
  return { tempDir, config };
}

describe('sync_decrypt_event — Definition', () => {
  it('tool exists with read permission', () => {
    const tool = findTool('sync_decrypt_event');
    assert.ok(tool);
    assert.equal(tool.permission, 'read');
  });

  it('tool exposes event lookup fields in schema', () => {
    const tool = findTool('sync_decrypt_event');
    assert.ok(tool.inputSchema.eventId);
    assert.ok(tool.inputSchema.sequenceNumber);
    assert.ok(tool.inputSchema.source);
    assert.ok(tool.inputSchema.keyId);
  });
});

describe('sync_decrypt_event — Legacy Handler', () => {
  let originalCwd;
  let tempDir;
  let config;
  let commerce;
  let keyManager;
  let outbox;

  before(async () => {
    originalCwd = process.cwd();
    ({ tempDir, config } = await createTempSyncEnv('legacy'));
    process.chdir(tempDir);

    commerce = { db: new Database(':memory:') };
    keyManager = new AgentKeyManager('.stateset');
    outbox = createOutbox(commerce.db, { keyManager });
    await keyManager.ensureKeys(config.identity.agentId);
  });

  after(async () => {
    commerce.db.close();
    process.chdir(originalCwd);
    await fs.rm(tempDir, { recursive: true, force: true });
  });

  it('decrypts an encrypted outbox event by eventId', async () => {
    const payload = {
      orderId: 'ord-legacy-001',
      total: 125.5,
      status: 'created',
    };
    const recipientKey = await keyManager.getCurrentEncryptionKey(config.identity.agentId);

    await outbox.append(
      {
        tenantId: config.identity.tenantId,
        storeId: config.identity.storeId,
        entityType: 'order',
        entityId: 'ord-legacy-001',
        eventType: 'order.created',
        payload,
        sourceAgent: config.identity.agentId,
      },
      {
        encrypt: true,
        recipientKeyId: recipientKey.keyId,
        recipientPublicKey: recipientKey.publicKey,
      },
    );

    const [event] = outbox.getPending();
    const tool = findTool('sync_decrypt_event');
    const result = await tool.handler({
      commerce,
      params: {
        eventId: event.eventId,
        source: 'outbox',
      },
    });

    assert.equal(result.success, true);
    assert.equal(result.source, 'outbox');
    assert.equal(result.encryptionProfile, 'legacy');
    assert.equal(result.recipientKeyId, recipientKey.keyId);
    assert.deepEqual(result.payload, payload);
  });

  it('returns a clear error for plaintext events', async () => {
    await outbox.append({
      tenantId: config.identity.tenantId,
      storeId: config.identity.storeId,
      entityType: 'order',
      entityId: 'ord-plain-001',
      eventType: 'order.updated',
      payload: { status: 'plain' },
      sourceAgent: config.identity.agentId,
    });

    const [event] = outbox.getPending(10).filter((entry) => entry.entityId === 'ord-plain-001');
    const tool = findTool('sync_decrypt_event');
    const result = await tool.handler({
      commerce,
      params: {
        eventId: event.eventId,
        source: 'outbox',
      },
    });

    assert.equal(result.success, false);
    assert.match(result.error, /not encrypted/i);
  });
});

describe(
  'sync_decrypt_event — Hybrid Handler',
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

    it('decrypts a pulled hybrid-encrypted event by sequence number', async () => {
      const encryptionKey = await keyManager.getCurrentEncryptionKey(config.identity.agentId);
      const payload = {
        orderId: 'ord-hybrid-001',
        total: 229.99,
        riskScore: 3,
      };
      const eventId = '550e8400-e29b-41d4-a716-446655440010';
      const createdAt = '2026-03-30T10:00:00.000Z';
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
          entityId: 'ord-hybrid-001',
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
        entityId: 'ord-hybrid-001',
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
        sequencedAt: '2026-03-30T10:00:05.000Z',
        sourceAgent: REMOTE_AGENT_ID,
      });

      const tool = findTool('sync_decrypt_event');
      const result = await tool.handler({
        commerce,
        params: {
          sequenceNumber: 42,
          source: 'pulled',
        },
      });

      assert.equal(result.success, true);
      assert.equal(result.source, 'pulled');
      assert.equal(result.encryptionProfile, 'hybrid');
      assert.equal(result.recipientKeyId, encryptionKey.keyId);
      assert.deepEqual(result.payload, payload);
    });

    it('rejects an explicit keyId that is not a listed recipient', async () => {
      const encryptionKey = await keyManager.getCurrentEncryptionKey(config.identity.agentId);
      const extraKey = await keyManager.generateEncryptionKey(config.identity.agentId);
      const payload = { orderId: 'ord-hybrid-002', total: 19.99 };
      const eventId = '550e8400-e29b-41d4-a716-446655440011';
      const createdAt = '2026-03-30T11:00:00.000Z';
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
          entityId: 'ord-hybrid-002',
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
        entityId: 'ord-hybrid-002',
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
        sequencedAt: '2026-03-30T11:00:05.000Z',
        sourceAgent: REMOTE_AGENT_ID,
      });

      const tool = findTool('sync_decrypt_event');
      const result = await tool.handler({
        commerce,
        params: {
          sequenceNumber: 43,
          source: 'pulled',
          keyId: extraKey.keyId,
        },
      });

      assert.equal(result.success, false);
      assert.match(result.error, /not listed as a recipient/i);
    });
  },
);
