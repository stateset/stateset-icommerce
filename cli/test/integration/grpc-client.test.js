/**
 * gRPC Client Integration Tests
 *
 * Tests for the gRPC client and unified client functionality.
 * Note: These tests require the sequencer to be running for full integration testing.
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert';
import { EventEmitter } from 'events';

// Mock the gRPC modules since they might not be installed
let grpcMocked = false;
let protoLoaderMocked = false;

// Create mock implementations
const mockMetadata = class Metadata {
  constructor() {
    this.data = new Map();
  }
  set(key, value) {
    this.data.set(key, value);
  }
  get(key) {
    return this.data.get(key);
  }
};

const mockCredentials = {
  createSsl: () => ({ type: 'ssl' }),
  createInsecure: () => ({ type: 'insecure' }),
};

const mockGrpc = {
  Metadata: mockMetadata,
  credentials: mockCredentials,
  status: {
    OK: 0,
    CANCELLED: 1,
    UNKNOWN: 2,
    NOT_FOUND: 5,
  },
  loadPackageDefinition: () => ({
    stateset: {
      sequencer: {
        v2: {
          Sequencer: class MockSequencer {
            constructor(url, creds) {
              this.url = url;
              this.creds = creds;
            }
            waitForReady(deadline, callback) {
              callback(null);
            }
            push(request, metadata, callback) {
              callback(null, {
                batch_id: 'test-batch',
                events_accepted: request.events.length,
                events_rejected: 0,
                sequence_start: 1,
                sequence_end: request.events.length,
                head_sequence: request.events.length,
                rejections: [],
              });
            }
            pullEvents(request, metadata, callback) {
              callback(null, {
                events: [],
                next_sequence: request.from_sequence,
                has_more: false,
                head_sequence: 0,
              });
            }
            getSyncState(request, metadata, callback) {
              callback(null, {
                tenant_id: request.tenant_id,
                store_id: request.store_id,
                head_sequence: 0,
                state_root: Buffer.alloc(32),
              });
            }
            getHealth(request, metadata, callback) {
              callback(null, {
                healthy: true,
                version: 'test-1.0.0',
                timestamp: { seconds: Math.floor(Date.now() / 1000), nanos: 0 },
              });
            }
            streamEvents(request, metadata) {
              const emitter = new EventEmitter();
              emitter.cancel = () => {};
              return emitter;
            }
            syncStream(metadata) {
              const emitter = new EventEmitter();
              emitter.write = () => {};
              emitter.end = () => {};
              return emitter;
            }
          },
          KeyManagement: class MockKeyManagement {
            constructor(url, creds) {
              this.url = url;
              this.creds = creds;
            }
            waitForReady(deadline, callback) {
              callback(null);
            }
            registerAgentKey(request, metadata, callback) {
              callback(null, {
                success: true,
                message: 'Key registered',
                registered_at: { seconds: Math.floor(Date.now() / 1000), nanos: 0 },
              });
            }
            getAgentKeys(request, metadata, callback) {
              callback(null, { keys: [] });
            }
            revokeAgentKey(request, metadata, callback) {
              callback(null, {
                success: true,
                revoked_at: { seconds: Math.floor(Date.now() / 1000), nanos: 0 },
              });
            }
          },
        },
      },
    },
  }),
  closeClient: () => {},
};

const mockProtoLoader = {
  load: async () => ({}),
};

// Test configuration
const TEST_CONFIG = {
  url: 'localhost:50051',
  tenantId: '00000000-0000-0000-0000-000000000001',
  storeId: '00000000-0000-0000-0000-000000000002',
  agentId: '00000000-0000-0000-0000-000000000003',
  apiKey: 'test-api-key',
  tls: true,
};

describe('GrpcSequencerClient', () => {
  let GrpcSequencerClient;

  beforeEach(async () => {
    // Reset module cache and setup mocks
    try {
      // Try to load real gRPC - if not available, use mocks
      await import('@grpc/grpc-js');
      await import('@grpc/proto-loader');
      grpcMocked = false;
    } catch {
      // Mock the modules
      grpcMocked = true;
    }

    // Import the client (it will use real or mocked gRPC)
    const module = await import('../../src/sync/grpc-client.js');
    GrpcSequencerClient = module.GrpcSequencerClient;
  });

  it('should create client with valid config', () => {
    const client = new GrpcSequencerClient(TEST_CONFIG);
    assert.strictEqual(client.config.url, TEST_CONFIG.url);
    assert.strictEqual(client.config.tenantId, TEST_CONFIG.tenantId);
    assert.strictEqual(client.connected, false);
  });

  it('should throw on missing required config', () => {
    assert.throws(
      () => new GrpcSequencerClient({ tenantId: 'test' }),
      /url is required/
    );
    assert.throws(
      () => new GrpcSequencerClient({ url: 'localhost:50051' }),
      /tenantId is required/
    );
  });

  it('should reject insecure transport for hybrid profile', () => {
    assert.throws(
      () =>
        new GrpcSequencerClient({
          ...TEST_CONFIG,
          securityProfile: 'hybrid',
          tls: false,
        }),
      /must use TLS for hybrid sync profile/,
    );
  });

  it('should have default retry policy', () => {
    const client = new GrpcSequencerClient(TEST_CONFIG);
    assert.strictEqual(client.config.retryPolicy.maxRetries, 5);
    assert.strictEqual(client.config.retryPolicy.baseDelayMs, 1000);
  });

  it('should map encrypted payloads into proto envelopes', () => {
    const client = new GrpcSequencerClient({ ...TEST_CONFIG, tls: true });

    const protoEvent = client._toProtoEvent({
      eventId: 'evt-1',
      entityType: 'order',
      entityId: 'ORD-1',
      eventType: 'created',
      payloadKind: 1,
      payload: { secret: true },
      payloadEncrypted: {
        enc_version: 1,
        aead: 'AES-256-GCM',
        nonce_b64u: 'bm9uY2U',
        ciphertext_b64u: 'Y2lwaGVydGV4dA',
        tag_b64u: 'dGFn',
        hpke: {
          mode: 'base',
          kem: 'X25519-HKDF-SHA256',
          kdf: 'HKDF-SHA256',
          aead: 'AES-256-GCM',
        },
        recipients: [
          {
            recipient_kid: 11,
            enc_b64u: 'ZW5j',
            ct_b64u: 'd3JhcHBlZA',
          },
        ],
      },
      payloadPlainHash: 'aa'.repeat(32),
      payloadCipherHash: 'bb'.repeat(32),
      agentSignature: 'cc'.repeat(64),
    });

    assert.strictEqual(protoEvent.payload_kind, 2);
    assert.strictEqual(protoEvent.payload.length, 0);
    assert.ok(protoEvent.payload_encrypted);
    assert.strictEqual(protoEvent.payload_encrypted.recipients.length, 1);
    assert.strictEqual(protoEvent.payload_encrypted.key_wrap_params.scheme, 1);
    assert.strictEqual(protoEvent.payload_encrypted.recipient_wraps.length, 1);
    assert.strictEqual(protoEvent.payload_encrypted.recipient_wraps[0].wrap_scheme, 1);
  });

  it('should enforce hybrid signatures on pushEvents', async () => {
    const client = new GrpcSequencerClient({
      ...TEST_CONFIG,
      tls: true,
      securityProfile: 'hybrid',
    });
    client.connected = true;
    client.client = {
      push(_request, _metadata, callback) {
        callback(null, {
          sequence_end: 1,
          events_accepted: 1,
          events_rejected: 0,
        });
      },
    };

    await assert.rejects(
      () =>
        client.pushEvents([
          {
            eventId: 'evt-1',
            entityType: 'order',
            entityId: 'ORD-1',
            eventType: 'created',
            payload: { ok: true },
          },
        ]),
      /Hybrid profile requires SIGNATURE_SCHEME_ED25519_ML_DSA_65/,
    );
  });
});

describe('UnifiedSequencerClient', () => {
  let UnifiedSequencerClient, createUnifiedClient;

  beforeEach(async () => {
    const module = await import('../../src/sync/unified-client.js');
    UnifiedSequencerClient = module.UnifiedSequencerClient;
    createUnifiedClient = module.createUnifiedClient;
  });

  it('should create unified client', () => {
    const mockConfig = {
      sequencerUrl: 'grpc://localhost:50051',
      tenantId: TEST_CONFIG.tenantId,
      storeId: TEST_CONFIG.storeId,
      agentId: TEST_CONFIG.agentId,
      getCredentials: () => ({ apiKey: 'test' }),
      retryPolicy: { maxRetries: 3 },
    };

    const client = new UnifiedSequencerClient({ config: mockConfig });
    assert.strictEqual(client.transport, null);
    assert.strictEqual(client.isConnected(), false);
  });

  it('should detect gRPC transport from URL', () => {
    const mockConfig = {
      sequencerUrl: 'grpc://localhost:50051',
      tenantId: TEST_CONFIG.tenantId,
      storeId: TEST_CONFIG.storeId,
      agentId: TEST_CONFIG.agentId,
      getCredentials: () => ({}),
      retryPolicy: { maxRetries: 3 },
    };

    const client = new UnifiedSequencerClient({
      config: mockConfig,
      preferGrpc: true,
    });
    assert.strictEqual(client.preferGrpc, true);
  });

  it('should detect REST transport from URL', () => {
    const mockConfig = {
      sequencerUrl: 'https://api.stateset.io',
      tenantId: TEST_CONFIG.tenantId,
      storeId: TEST_CONFIG.storeId,
      agentId: TEST_CONFIG.agentId,
      getCredentials: () => ({}),
      retryPolicy: { maxRetries: 3 },
    };

    const client = new UnifiedSequencerClient({
      config: mockConfig,
      preferGrpc: false,
    });
    assert.strictEqual(client.preferGrpc, false);
  });

  it('should create client via factory function', () => {
    const mockConfig = {
      sequencerUrl: 'grpc://localhost:50051',
      tenantId: TEST_CONFIG.tenantId,
      storeId: TEST_CONFIG.storeId,
      agentId: TEST_CONFIG.agentId,
      getCredentials: () => ({}),
      retryPolicy: { maxRetries: 3 },
    };

    const client = createUnifiedClient(mockConfig, { preferGrpc: true });
    assert.ok(client instanceof UnifiedSequencerClient);
  });
});

describe('SyncEngine with gRPC', () => {
  let SyncEngine, createSyncEngine;
  let mockDb;

  beforeEach(async () => {
    const module = await import('../../src/sync/engine.js');
    SyncEngine = module.SyncEngine;
    createSyncEngine = module.createSyncEngine;

    // Create a mock database
    mockDb = {
      prepare: () => ({
        run: () => ({ changes: 1 }),
        get: () => null,
        all: () => [],
      }),
      exec: () => {},
    };
  });

  it('should create engine with gRPC preference', () => {
    const mockConfig = {
      sequencerUrl: 'grpc://localhost:50051',
      tenantId: TEST_CONFIG.tenantId,
      storeId: TEST_CONFIG.storeId,
      agentId: TEST_CONFIG.agentId,
      getCredentials: () => ({}),
      retryPolicy: { maxRetries: 3 },
      sync: { syncIntervalMs: 30000 },
    };

    // Create SyncConfig-like object
    const configObj = {
      ...mockConfig,
      get sequencer() {
        return { url: this.sequencerUrl };
      },
      get identity() {
        return {
          tenantId: this.tenantId,
          storeId: this.storeId,
          agentId: this.agentId,
        };
      },
      get batchSize() {
        return 100;
      },
    };

    const engine = new SyncEngine({
      db: mockDb,
      config: configObj,
      preferGrpc: true,
      enableStreaming: true,
    });

    assert.ok(engine);
    assert.strictEqual(engine._streamingEnabled, false);
    assert.ok(Array.isArray(engine._eventBuffer));
  });

  it('should report capabilities', () => {
    const mockConfig = {
      sequencerUrl: 'https://api.stateset.io',
      tenantId: TEST_CONFIG.tenantId,
      storeId: TEST_CONFIG.storeId,
      agentId: TEST_CONFIG.agentId,
      getCredentials: () => ({}),
      retryPolicy: { maxRetries: 3 },
      sync: { syncIntervalMs: 30000 },
    };

    const configObj = {
      ...mockConfig,
      get sequencer() {
        return { url: this.sequencerUrl };
      },
      get identity() {
        return {
          tenantId: this.tenantId,
          storeId: this.storeId,
          agentId: this.agentId,
        };
      },
      get batchSize() {
        return 100;
      },
    };

    const engine = new SyncEngine({
      db: mockDb,
      config: configObj,
      preferGrpc: false,
    });

    const capabilities = engine.getCapabilities();
    assert.strictEqual(capabilities.batchPush, true);
    assert.strictEqual(capabilities.inclusionProofs, true);
  });

  it('should have streaming methods', () => {
    const mockConfig = {
      sequencerUrl: 'grpc://localhost:50051',
      tenantId: TEST_CONFIG.tenantId,
      storeId: TEST_CONFIG.storeId,
      agentId: TEST_CONFIG.agentId,
      getCredentials: () => ({}),
      retryPolicy: { maxRetries: 3 },
      sync: { syncIntervalMs: 30000 },
    };

    const configObj = {
      ...mockConfig,
      get sequencer() {
        return { url: this.sequencerUrl };
      },
      get identity() {
        return {
          tenantId: this.tenantId,
          storeId: this.storeId,
          agentId: this.agentId,
        };
      },
      get batchSize() {
        return 100;
      },
    };

    const engine = new SyncEngine({
      db: mockDb,
      config: configObj,
    });

    assert.strictEqual(typeof engine.startStreamingSync, 'function');
    assert.strictEqual(typeof engine.stopStreamingSync, 'function');
    assert.strictEqual(typeof engine.isStreaming, 'function');
    assert.strictEqual(typeof engine.supportsStreaming, 'function');
    assert.strictEqual(typeof engine.onEvent, 'function');
    assert.strictEqual(typeof engine.getRecentEvents, 'function');
  });
});

describe('Transport Selection', () => {
  it('should prefer gRPC for grpc:// URLs', async () => {
    const { checkGrpcAvailability } = await import('../../src/sync/unified-client.js');
    const available = await checkGrpcAvailability();
    // Just verify the function works - actual availability depends on installation
    assert.strictEqual(typeof available, 'boolean');
  });
});
