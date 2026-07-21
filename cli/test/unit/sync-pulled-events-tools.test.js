/**
 * Tests for sync_pulled_events tool definitions and handlers.
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

const TENANT_ID = '550e8400-e29b-41d4-a716-446655440101';
const STORE_ID = '550e8400-e29b-41d4-a716-446655440102';
const REMOTE_AGENT_ID = '550e8400-e29b-41d4-a716-446655440103';

function findTool(name) {
  return syncTools.find((tool) => tool.name === name);
}

async function createTempSyncEnv(securityProfile) {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), `sync-pulled-${securityProfile}-`));
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

describe('sync_pulled_events — Definition', () => {
  it('tool exists with read permission', () => {
    const tool = findTool('sync_pulled_events');
    assert.ok(tool);
    assert.equal(tool.permission, 'read');
  });

  it('tool exposes pulled-event listing fields in schema', () => {
    const tool = findTool('sync_pulled_events');
    assert.ok(tool.inputSchema.limit);
    assert.ok(tool.inputSchema.includePayloads);
    assert.ok(tool.inputSchema.decryptPayloads);
    assert.ok(tool.inputSchema.keyId);
  });
});

describe('sync_pulled_events — Legacy Handler', () => {
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

  it('lists pulled events without payloads by default', async () => {
    const payload = { orderId: 'ord-pulled-001', total: 42.5 };

    outbox.storePulledEvent({
      sequenceNumber: 10,
      eventId: '550e8400-e29b-41d4-a716-446655440110',
      commandId: null,
      tenantId: config.identity.tenantId,
      storeId: config.identity.storeId,
      entityType: 'order',
      entityId: 'ord-pulled-001',
      eventType: 'order.created',
      payload,
      vesVersion: 1,
      payloadKind: 0,
      payloadEncrypted: null,
      payloadPlainHash: bufferToHex(computePayloadPlainHash(payload)),
      payloadCipherHash: bufferToHex(ZERO_HASH),
      agentKeyId: 5,
      agentSignature: '0x',
      agentSignatureScheme: 0,
      baseVersion: null,
      createdAt: '2026-03-30T12:00:00.000Z',
      sequencedAt: '2026-03-30T12:00:05.000Z',
      sourceAgent: REMOTE_AGENT_ID,
    });

    const tool = findTool('sync_pulled_events');
    const result = await tool.handler({
      commerce,
      params: { limit: 10 },
    });

    assert.equal(result.success, true);
    assert.equal(result.source, 'pulled');
    assert.equal(result.count, 1);
    assert.equal(result.includePayloads, false);
    assert.equal(result.decryptPayloads, false);
    assert.deepEqual(result.events[0].payload, undefined);
    assert.equal(result.events[0].encrypted, false);
    assert.equal(result.events[0].sequenceNumber, 10);
  });

  it('includes plaintext payloads when requested', async () => {
    const payload = { orderId: 'ord-pulled-002', status: 'paid' };

    outbox.storePulledEvent({
      sequenceNumber: 11,
      eventId: '550e8400-e29b-41d4-a716-446655440111',
      commandId: null,
      tenantId: config.identity.tenantId,
      storeId: config.identity.storeId,
      entityType: 'order',
      entityId: 'ord-pulled-002',
      eventType: 'order.updated',
      payload,
      vesVersion: 1,
      payloadKind: 0,
      payloadEncrypted: null,
      payloadPlainHash: bufferToHex(computePayloadPlainHash(payload)),
      payloadCipherHash: bufferToHex(ZERO_HASH),
      agentKeyId: 6,
      agentSignature: '0x',
      agentSignatureScheme: 0,
      baseVersion: null,
      createdAt: '2026-03-30T12:05:00.000Z',
      sequencedAt: '2026-03-30T12:05:05.000Z',
      sourceAgent: REMOTE_AGENT_ID,
    });

    const tool = findTool('sync_pulled_events');
    const result = await tool.handler({
      commerce,
      params: { limit: 10, includePayloads: true },
    });

    assert.equal(result.success, true);
    const event = result.events.find((entry) => entry.sequenceNumber === 11);
    assert.ok(event);
    assert.deepEqual(event.payload, payload);
    assert.equal(event.encrypted, false);
    assert.equal(event.wrapScheme, null);
  });
});

describe(
  'sync_pulled_events — Hybrid Handler',
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

    it('decrypts hybrid pulled events when requested', async () => {
      const encryptionKey = await keyManager.getCurrentEncryptionKey(config.identity.agentId);
      const payload = {
        orderId: 'ord-hybrid-pulled-001',
        total: 229.99,
        riskScore: 2,
      };
      const eventId = '550e8400-e29b-41d4-a716-446655440142';
      const createdAt = '2026-03-30T13:00:00.000Z';
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
          entityId: 'ord-hybrid-pulled-001',
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
        entityId: 'ord-hybrid-pulled-001',
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
        sequencedAt: '2026-03-30T13:00:05.000Z',
        sourceAgent: REMOTE_AGENT_ID,
      });

      const tool = findTool('sync_pulled_events');
      const result = await tool.handler({
        commerce,
        params: { limit: 10, decryptPayloads: true },
      });

      assert.equal(result.success, true);
      assert.equal(result.includePayloads, true);
      const event = result.events.find((entry) => entry.sequenceNumber === 42);
      assert.ok(event);
      assert.equal(event.encrypted, true);
      assert.equal(event.encryptionProfile, 'hybrid');
      assert.equal(event.recipientKeyId, encryptionKey.keyId);
      assert.deepEqual(event.payload, payload);
      assert.equal(event.decryptionError, undefined);
    });

    it('reports decryption errors per event when the wrong keyId is forced', async () => {
      const encryptionKey = await keyManager.getCurrentEncryptionKey(config.identity.agentId);
      const extraKey = await keyManager.generateEncryptionKey(config.identity.agentId);
      const payload = { orderId: 'ord-hybrid-pulled-002', total: 19.99 };
      const eventId = '550e8400-e29b-41d4-a716-446655440143';
      const createdAt = '2026-03-30T13:05:00.000Z';
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
          entityId: 'ord-hybrid-pulled-002',
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
        entityId: 'ord-hybrid-pulled-002',
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
        sequencedAt: '2026-03-30T13:05:05.000Z',
        sourceAgent: REMOTE_AGENT_ID,
      });

      const tool = findTool('sync_pulled_events');
      const result = await tool.handler({
        commerce,
        params: { limit: 10, decryptPayloads: true, keyId: extraKey.keyId },
      });

      assert.equal(result.success, true);
      const event = result.events.find((entry) => entry.sequenceNumber === 43);
      assert.ok(event);
      assert.equal(event.payload, undefined);
      assert.match(event.decryptionError, /not listed as a recipient/i);
    });
  },
);
