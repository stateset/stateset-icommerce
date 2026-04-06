/**
 * Unit tests for sync/client.js — SequencerClient
 *
 * Tests: constructor, URL parsing, authentication headers, HTTP requests,
 * push events, pull events, Merkle proof verification, event signature
 * verification, retry logic, pagination, getHead, getCommitment,
 * getEntityHistory, registerAgentKey, getAgentKeys, connect/disconnect.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'crypto';

import { SequencerClient, createSequencerClient } from '../../src/sync/client.js';
import {
  computeLeafHash,
  computeNodeHash,
  computePayloadPlainHash,
  computeEventSigningHash,
  generateHybridSigningKeypair,
  hasNativeHybridPqcVerificationSupport,
  hexToBuffer,
  signEventHashHybrid,
} from '../../src/sync/crypto.js';
import { SIGNATURE_SCHEME_ED25519_ML_DSA_65 } from '../../src/sync/pqc.js';

// =============================================================================
// Helpers
// =============================================================================

const originalFetch = globalThis.fetch;

function mockFetch(handler) {
  globalThis.fetch = async (...args) => handler(...args);
}

function restoreFetch() {
  globalThis.fetch = originalFetch;
}

function okResponse(body) {
  return {
    ok: true,
    status: 200,
    json: async () => body,
    text: async () => JSON.stringify(body),
  };
}

function errorResponse(status, text) {
  return {
    ok: false,
    status,
    json: async () => ({ error: text }),
    text: async () => text,
  };
}

// Minimal SyncConfig-compatible object for testing
function makeConfig({
  url = 'https://seq.example.com',
  apiKey = null,
  jwt = null,
  securityProfile = 'legacy',
  allowInsecureTransport = false,
  tenantId = '550e8400-e29b-41d4-a716-446655440001',
  storeId = '550e8400-e29b-41d4-a716-446655440002',
  maxRetries = 3,
  baseDelay = 10,
  maxDelay = 100,
} = {}) {
  return {
    sequencerUrl: url,
    securityProfile,
    allowInsecureTransport,
    sequencer: { url, insecure: allowInsecureTransport },
    tenantId,
    storeId,
    getCredentials: () => ({ apiKey, jwt }),
    retryPolicy: { maxRetries, baseDelay, maxDelay },
  };
}

const UUID1 = '550e8400-e29b-41d4-a716-446655440001';
const UUID2 = '550e8400-e29b-41d4-a716-446655440002';
const UUID3 = '550e8400-e29b-41d4-a716-446655440003';

// Build a minimal VES v1.0 envelope for testing
function makeEnvelope(overrides = {}) {
  const payloadHash = computePayloadPlainHash({ test: true }).toString('hex');
  return {
    eventId: UUID1,
    commandId: null,
    tenantId: UUID2,
    storeId: UUID3,
    entityType: 'order',
    entityId: 'ORD-1',
    eventType: 'created',
    payload: { test: true },
    vesVersion: 1,
    payloadKind: 0,
    payloadPlainHash: payloadHash,
    payloadCipherHash: '0'.repeat(64),
    agentKeyId: 1,
    agentSignature: '0'.repeat(128),
    baseVersion: null,
    createdAt: '2024-01-01T00:00:00Z',
    sourceAgent: UUID1,
    ...overrides,
  };
}

// Build a minimal VES v1.0 sequenced event as returned by the server
function makeServerEvent(seqNum = 1) {
  const payloadHash = computePayloadPlainHash({ order: seqNum }).toString('hex');
  return {
    envelope: {
      event_id: UUID1,
      command_id: null,
      tenant_id: UUID2,
      store_id: UUID3,
      entity_type: 'order',
      entity_id: `ORD-${seqNum}`,
      event_type: 'created',
      payload: { order: seqNum },
      ves_version: 1,
      payload_kind: 0,
      payload_encrypted: null,
      payload_plain_hash: payloadHash,
      payload_cipher_hash: '0'.repeat(64),
      agent_key_id: 1,
      agent_signature: '0'.repeat(128),
      base_version: null,
      created_at: '2024-01-01T00:00:00Z',
      source_agent: UUID1,
      sequence_number: seqNum,
    },
    sequenced_at: '2024-01-01T00:00:01Z',
    receipt_hash: 'abc123',
  };
}

// =============================================================================
// Constructor / URL parsing
// =============================================================================

describe('SequencerClient — constructor', () => {
  it('stores config reference', () => {
    const cfg = makeConfig();
    const client = new SequencerClient(cfg);
    assert.strictEqual(client.config, cfg);
  });

  it('starts disconnected', () => {
    const client = new SequencerClient(makeConfig());
    assert.strictEqual(client.isConnected(), false);
  });

  it('defaults missing securityProfile to hybrid', () => {
    const config = makeConfig();
    delete config.securityProfile;
    const client = new SequencerClient(config);
    assert.strictEqual(client.securityProfile, 'hybrid');
  });

  it('sets baseUrl for https:// URL', () => {
    const client = new SequencerClient(makeConfig({ url: 'https://seq.example.com' }));
    assert.strictEqual(client.baseUrl, 'https://seq.example.com');
  });

  it('strips trailing slash from http:// URL', () => {
    const client = new SequencerClient(
      makeConfig({
        url: 'http://seq.example.com/',
        securityProfile: 'legacy',
        allowInsecureTransport: true,
      }),
    );
    assert.ok(!client.baseUrl.endsWith('/'));
  });

  it('converts grpc:// to http://', () => {
    const client = new SequencerClient(
      makeConfig({
        url: 'grpc://seq.example.com:50051',
        securityProfile: 'legacy',
        allowInsecureTransport: true,
      }),
    );
    assert.strictEqual(client.baseUrl, 'http://seq.example.com:50051');
  });

  it('converts grpcs:// to https://', () => {
    const client = new SequencerClient(makeConfig({ url: 'grpcs://seq.example.com:443' }));
    assert.strictEqual(client.baseUrl, 'https://seq.example.com:443');
  });

  it('throws when URL is empty string', () => {
    assert.throws(
      () => new SequencerClient(makeConfig({ url: '' })),
      /non-empty string/,
    );
  });

  it('throws when URL is whitespace only', () => {
    assert.throws(
      () => new SequencerClient(makeConfig({ url: '   ' })),
      /non-empty string/,
    );
  });

  it('throws on unsupported protocol (ftp://)', () => {
    assert.throws(
      () => new SequencerClient(makeConfig({ url: 'ftp://seq.example.com' })),
      /Unsupported sequencer protocol/,
    );
  });

  it('throws on unsupported protocol (ws://)', () => {
    assert.throws(
      () => new SequencerClient(makeConfig({ url: 'ws://seq.example.com' })),
      /Unsupported sequencer protocol/,
    );
  });

  it('rejects insecure transport for hybrid profile', () => {
    assert.throws(
      () => new SequencerClient(makeConfig({ url: 'http://seq.example.com', securityProfile: 'hybrid' })),
      /must use TLS for hybrid sync profile/,
    );
  });

  it('rejects insecure transport for legacy without explicit allow flag', () => {
    assert.throws(
      () => new SequencerClient(makeConfig({ url: 'http://seq.example.com', securityProfile: 'legacy' })),
      /explicitly allowed/,
    );
  });

  it('preserves port in baseUrl', () => {
    const client = new SequencerClient(makeConfig({ url: 'https://seq.example.com:8443' }));
    assert.ok(client.baseUrl.includes(':8443'));
  });
});

// =============================================================================
// createSequencerClient factory
// =============================================================================

describe('createSequencerClient', () => {
  it('returns a SequencerClient instance', () => {
    const client = createSequencerClient(makeConfig());
    assert.ok(client instanceof SequencerClient);
  });

  it('passes config through to instance', () => {
    const cfg = makeConfig({ apiKey: 'factory-key' });
    const client = createSequencerClient(cfg);
    assert.strictEqual(client.config, cfg);
  });
});

// =============================================================================
// _getHeaders
// =============================================================================

describe('SequencerClient — _getHeaders', () => {
  it('always includes Content-Type: application/json', () => {
    const client = new SequencerClient(makeConfig());
    const headers = client._getHeaders();
    assert.strictEqual(headers['Content-Type'], 'application/json');
  });

  it('sets Authorization Bearer with apiKey', () => {
    const client = new SequencerClient(makeConfig({ apiKey: 'my-api-key' }));
    const headers = client._getHeaders();
    assert.strictEqual(headers['Authorization'], 'Bearer my-api-key');
  });

  it('sets Authorization Bearer with jwt when no apiKey', () => {
    const client = new SequencerClient(makeConfig({ jwt: 'my-jwt' }));
    const headers = client._getHeaders();
    assert.strictEqual(headers['Authorization'], 'Bearer my-jwt');
  });

  it('prefers apiKey over jwt when both present', () => {
    const cfg = makeConfig();
    cfg.getCredentials = () => ({ apiKey: 'key', jwt: 'jwt' });
    const client = new SequencerClient(cfg);
    const headers = client._getHeaders();
    assert.strictEqual(headers['Authorization'], 'Bearer key');
  });

  it('omits Authorization when no credentials', () => {
    const client = new SequencerClient(makeConfig());
    const headers = client._getHeaders();
    assert.strictEqual(headers['Authorization'], undefined);
  });
});

// =============================================================================
// _request
// =============================================================================

describe('SequencerClient — _request', () => {
  afterEach(() => restoreFetch());

  it('calls fetch with the correct full URL', async () => {
    let capturedUrl;
    mockFetch((url) => {
      capturedUrl = url;
      return okResponse({ ok: true });
    });

    const client = new SequencerClient(makeConfig({ url: 'https://seq.example.com' }));
    await client._request('GET', '/api/v1/health');
    assert.strictEqual(capturedUrl, 'https://seq.example.com/api/v1/health');
  });

  it('passes method to fetch', async () => {
    let capturedMethod;
    mockFetch((_url, opts) => {
      capturedMethod = opts.method;
      return okResponse({});
    });

    const client = new SequencerClient(makeConfig());
    await client._request('POST', '/api/v1/test');
    assert.strictEqual(capturedMethod, 'POST');
  });

  it('serialises body as JSON for POST', async () => {
    let capturedBody;
    mockFetch((_url, opts) => {
      capturedBody = opts.body;
      return okResponse({});
    });

    const client = new SequencerClient(makeConfig());
    await client._request('POST', '/api/v1/test', { foo: 'bar' });
    assert.strictEqual(capturedBody, JSON.stringify({ foo: 'bar' }));
  });

  it('omits body for GET', async () => {
    let capturedBody;
    mockFetch((_url, opts) => {
      capturedBody = opts.body;
      return okResponse({});
    });

    const client = new SequencerClient(makeConfig());
    await client._request('GET', '/api/v1/test');
    assert.strictEqual(capturedBody, undefined);
  });

  it('returns parsed JSON on success', async () => {
    mockFetch(() => okResponse({ headSequence: 42 }));

    const client = new SequencerClient(makeConfig());
    const result = await client._request('GET', '/api/v1/test');
    assert.deepStrictEqual(result, { headSequence: 42 });
  });

  it('throws on 4xx response with status code in message', async () => {
    mockFetch(() => errorResponse(404, 'Not Found'));

    const client = new SequencerClient(makeConfig());
    await assert.rejects(
      () => client._request('GET', '/api/v1/missing'),
      /404/,
    );
  });

  it('throws on 5xx response with status code in message', async () => {
    mockFetch(() => errorResponse(503, 'Service Unavailable'));

    const client = new SequencerClient(makeConfig());
    await assert.rejects(
      () => client._request('GET', '/api/v1/test'),
      /503/,
    );
  });

  it('includes auth header in request', async () => {
    let capturedHeaders;
    mockFetch((_url, opts) => {
      capturedHeaders = opts.headers;
      return okResponse({});
    });

    const client = new SequencerClient(makeConfig({ apiKey: 'test-key' }));
    await client._request('GET', '/api/v1/test');
    assert.strictEqual(capturedHeaders['Authorization'], 'Bearer test-key');
  });
});

// =============================================================================
// connect / disconnect / isConnected
// =============================================================================

describe('SequencerClient — connect/disconnect', () => {
  afterEach(() => restoreFetch());

  it('sets _connected to true on successful GET /health', async () => {
    mockFetch(() => okResponse({ status: 'ok' }));

    const client = new SequencerClient(makeConfig());
    await client.connect();
    assert.strictEqual(client.isConnected(), true);
  });

  it('calls GET /health endpoint', async () => {
    let capturedUrl;
    mockFetch((url) => {
      capturedUrl = url;
      return okResponse({ status: 'ok' });
    });

    const client = new SequencerClient(makeConfig({ url: 'https://seq.example.com' }));
    await client.connect();
    assert.ok(capturedUrl.endsWith('/health'));
  });

  it('throws and stays disconnected when health check fails', async () => {
    mockFetch(() => errorResponse(503, 'Unavailable'));

    const client = new SequencerClient(makeConfig());
    await assert.rejects(() => client.connect(), /Failed to connect to sequencer/);
    assert.strictEqual(client.isConnected(), false);
  });

  it('sets _connected to false after disconnect', async () => {
    mockFetch(() => okResponse({ status: 'ok' }));

    const client = new SequencerClient(makeConfig());
    await client.connect();
    assert.strictEqual(client.isConnected(), true);
    await client.disconnect();
    assert.strictEqual(client.isConnected(), false);
  });

  it('disconnect is safe to call when already disconnected', async () => {
    const client = new SequencerClient(makeConfig());
    await client.disconnect(); // no error
    assert.strictEqual(client.isConnected(), false);
  });
});

// =============================================================================
// push
// =============================================================================

describe('SequencerClient — push', () => {
  afterEach(() => restoreFetch());

  it('calls POST /api/v1/ves/events/ingest', async () => {
    let capturedUrl, capturedMethod;
    mockFetch((url, opts) => {
      capturedUrl = url;
      capturedMethod = opts.method;
      return okResponse({
        batchId: 'B-1',
        eventsAccepted: 1,
        headSequence: 10,
      });
    });

    const client = new SequencerClient(makeConfig({ url: 'https://seq.example.com' }));
    await client.push({ agentId: UUID1, events: [makeEnvelope()] });

    assert.ok(capturedUrl.endsWith('/api/v1/ves/events/ingest'));
    assert.strictEqual(capturedMethod, 'POST');
  });

  it('maps camelCase envelope fields to snake_case VesEventEnvelope', async () => {
    let capturedBody;
    mockFetch((_url, opts) => {
      capturedBody = JSON.parse(opts.body);
      return okResponse({ batchId: 'B-1', eventsAccepted: 1, headSequence: 1 });
    });

    const envelope = makeEnvelope({ commandId: 'CMD-42' });
    const client = new SequencerClient(makeConfig());
    await client.push({ agentId: UUID1, events: [envelope] });

    const ev = capturedBody.events[0];
    assert.strictEqual(ev.event_id, envelope.eventId);
    assert.strictEqual(ev.command_id, 'CMD-42');
    assert.strictEqual(ev.tenant_id, envelope.tenantId);
    assert.strictEqual(ev.store_id, envelope.storeId);
    assert.strictEqual(ev.entity_type, envelope.entityType);
    assert.strictEqual(ev.entity_id, envelope.entityId);
    assert.strictEqual(ev.event_type, envelope.eventType);
    assert.strictEqual(ev.ves_version, 1);
    assert.strictEqual(ev.payload_kind, 0);
    assert.strictEqual(ev.agent_key_id, envelope.agentKeyId);
    assert.strictEqual(ev.agent_signature, envelope.agentSignature);
  });

  it('includes PQ signature bundle fields when provided', async () => {
    let capturedBody;
    mockFetch((_url, opts) => {
      capturedBody = JSON.parse(opts.body);
      return okResponse({ batchId: 'B-1', eventsAccepted: 1, headSequence: 1 });
    });

    const envelope = makeEnvelope({
      agentSignatureScheme: 3,
      agentSignatureBundle: {
        ed25519Signature: 'aa'.repeat(64),
        mlDsa65Signature: 'bb'.repeat(32),
      },
    });
    const client = new SequencerClient(makeConfig());
    await client.push({ agentId: UUID1, events: [envelope] });

    const ev = capturedBody.events[0];
    assert.strictEqual(ev.agent_signature_scheme, 3);
    assert.deepStrictEqual(ev.agent_signature_bundle, {
      ed25519_signature: 'aa'.repeat(64),
      ml_dsa_65_signature: 'bb'.repeat(32),
    });
  });

  it('derives generalized recipient wrap fields from legacy encrypted recipients', async () => {
    let capturedBody;
    mockFetch((_url, opts) => {
      capturedBody = JSON.parse(opts.body);
      return okResponse({ batchId: 'B-1', eventsAccepted: 1, headSequence: 1 });
    });

    const envelope = makeEnvelope({
      payloadKind: 1,
      payload: { secret: 'data' },
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
            recipient_kid: 9,
            enc_b64u: 'ZW5j',
            ct_b64u: 'd3JhcHBlZA',
          },
        ],
      },
    });
    const client = new SequencerClient(makeConfig());
    await client.push({ agentId: UUID1, events: [envelope] });

    const ev = capturedBody.events[0];
    assert.deepStrictEqual(ev.payload_encrypted.key_wrap_params, {
      scheme: 1,
      kdf: 'HKDF-SHA256',
      aead: 'AES-256-GCM',
    });
    assert.deepStrictEqual(ev.payload_encrypted.recipient_wraps, [
      {
        recipient_kid: 9,
        wrap_scheme: 1,
        x25519_enc_b64u: 'ZW5j',
        ml_kem_ciphertext_b64u: null,
        wrap_nonce_b64u: null,
        wrapped_key_b64u: 'd3JhcHBlZA',
      },
    ]);
  });

  it('rejects legacy-only signatures under hybrid profile', async () => {
    const client = new SequencerClient(makeConfig({ securityProfile: 'hybrid' }));

    await assert.rejects(
      () => client.push({ agentId: UUID1, events: [makeEnvelope()] }),
      /Hybrid profile requires SIGNATURE_SCHEME_ED25519_ML_DSA_65/,
    );
  });

  it('sends payload for plaintext events (payload_kind=0)', async () => {
    let capturedBody;
    mockFetch((_url, opts) => {
      capturedBody = JSON.parse(opts.body);
      return okResponse({ batchId: 'B-1', eventsAccepted: 1, headSequence: 1 });
    });

    const envelope = makeEnvelope({ payloadKind: 0, payload: { amount: 99 } });
    const client = new SequencerClient(makeConfig());
    await client.push({ agentId: UUID1, events: [envelope] });

    assert.deepStrictEqual(capturedBody.events[0].payload, { amount: 99 });
  });

  it('sends null payload for encrypted events (payload_kind=1)', async () => {
    let capturedBody;
    mockFetch((_url, opts) => {
      capturedBody = JSON.parse(opts.body);
      return okResponse({ batchId: 'B-1', eventsAccepted: 1, headSequence: 1 });
    });

    const envelope = makeEnvelope({
      payloadKind: 1,
      payload: { secret: 'data' },
      payloadEncrypted: { ciphertext: 'abc' },
    });
    const client = new SequencerClient(makeConfig());
    await client.push({ agentId: UUID1, events: [envelope] });

    assert.strictEqual(capturedBody.events[0].payload, null);
    assert.deepStrictEqual(capturedBody.events[0].payload_encrypted, { ciphertext: 'abc' });
  });

  it('returns IngestReceipt with batchId and counts', async () => {
    mockFetch(() =>
      okResponse({
        batchId: 'BATCH-99',
        eventsAccepted: 3,
        eventsRejected: 1,
        sequenceStart: 10,
        sequenceEnd: 12,
        headSequence: 12,
        rejections: [{ eventId: UUID1, reason: 'duplicate' }],
        receipts: [],
      }),
    );

    const client = new SequencerClient(makeConfig());
    const receipt = await client.push({ agentId: UUID1, events: [makeEnvelope()] });

    assert.strictEqual(receipt.batchId, 'BATCH-99');
    assert.strictEqual(receipt.eventsAccepted, 3);
    assert.strictEqual(receipt.eventsRejected, 1);
    assert.strictEqual(receipt.sequenceStart, 10);
    assert.strictEqual(receipt.sequenceEnd, 12);
    assert.strictEqual(receipt.headSequence, 12);
    assert.strictEqual(receipt.rejections.length, 1);
  });

  it('defaults eventsRejected to 0 when missing from response', async () => {
    mockFetch(() =>
      okResponse({ batchId: 'B-1', eventsAccepted: 1, headSequence: 1 }),
    );

    const client = new SequencerClient(makeConfig());
    const receipt = await client.push({ agentId: UUID1, events: [makeEnvelope()] });
    assert.strictEqual(receipt.eventsRejected, 0);
  });

  it('defaults rejections to [] when missing from response', async () => {
    mockFetch(() =>
      okResponse({ batchId: 'B-1', eventsAccepted: 1, headSequence: 1 }),
    );

    const client = new SequencerClient(makeConfig());
    const receipt = await client.push({ agentId: UUID1, events: [makeEnvelope()] });
    assert.deepStrictEqual(receipt.rejections, []);
  });

  it('sends multiple events in a single batch', async () => {
    let capturedBody;
    mockFetch((_url, opts) => {
      capturedBody = JSON.parse(opts.body);
      return okResponse({ batchId: 'B-1', eventsAccepted: 2, headSequence: 2 });
    });

    const events = [makeEnvelope(), makeEnvelope({ entityId: 'ORD-2' })];
    const client = new SequencerClient(makeConfig());
    await client.push({ agentId: UUID1, events });

    assert.strictEqual(capturedBody.events.length, 2);
    assert.strictEqual(capturedBody.agentId, UUID1);
  });

  it('propagates HTTP errors', async () => {
    mockFetch(() => errorResponse(422, 'Validation failed'));

    const client = new SequencerClient(makeConfig());
    await assert.rejects(
      () => client.push({ agentId: UUID1, events: [makeEnvelope()] }),
      /422/,
    );
  });
});

// =============================================================================
// pushWithRetry
// =============================================================================

describe('SequencerClient — pushWithRetry', () => {
  afterEach(() => restoreFetch());

  it('returns receipt immediately on first success', async () => {
    mockFetch(() =>
      okResponse({ batchId: 'B-1', eventsAccepted: 1, headSequence: 1 }),
    );

    const client = new SequencerClient(makeConfig());
    const receipt = await client.pushWithRetry({ agentId: UUID1, events: [makeEnvelope()] }, 3);
    assert.strictEqual(receipt.batchId, 'B-1');
  });

  it('retries on failure and eventually succeeds', async () => {
    let attempts = 0;
    mockFetch(() => {
      attempts++;
      if (attempts < 3) throw new Error('transient error');
      return okResponse({ batchId: 'B-retry', eventsAccepted: 1, headSequence: 3 });
    });

    const client = new SequencerClient(
      makeConfig({ maxRetries: 3, baseDelay: 1, maxDelay: 10 }),
    );
    const receipt = await client.pushWithRetry(
      { agentId: UUID1, events: [makeEnvelope()] },
      3,
    );
    assert.strictEqual(receipt.batchId, 'B-retry');
    assert.strictEqual(attempts, 3);
  });

  it('throws the last error when all retries are exhausted', async () => {
    mockFetch(() => {
      throw new Error('permanent failure');
    });

    const client = new SequencerClient(
      makeConfig({ maxRetries: 2, baseDelay: 1, maxDelay: 10 }),
    );
    await assert.rejects(
      () => client.pushWithRetry({ agentId: UUID1, events: [makeEnvelope()] }, 2),
      /permanent failure/,
    );
  });

  it('makes maxRetries+1 total attempts before giving up', async () => {
    let attempts = 0;
    mockFetch(() => {
      attempts++;
      throw new Error('always fails');
    });

    const client = new SequencerClient(
      makeConfig({ maxRetries: 2, baseDelay: 1, maxDelay: 5 }),
    );
    await assert.rejects(
      () => client.pushWithRetry({ agentId: UUID1, events: [makeEnvelope()] }, 2),
    );
    assert.strictEqual(attempts, 3); // attempt 0, 1, 2
  });
});

// =============================================================================
// pull
// =============================================================================

describe('SequencerClient — pull', () => {
  afterEach(() => restoreFetch());

  it('calls GET /api/v1/events with correct query params', async () => {
    let capturedUrl;
    mockFetch((url) => {
      capturedUrl = url;
      return okResponse({ events: [], head_sequence: 0 });
    });

    const client = new SequencerClient(
      makeConfig({ tenantId: UUID2, storeId: UUID3, url: 'https://seq.example.com' }),
    );
    await client.pull(5, 50);

    const url = new URL(capturedUrl);
    assert.strictEqual(url.pathname, '/api/v1/events');
    assert.strictEqual(url.searchParams.get('tenant_id'), UUID2);
    assert.strictEqual(url.searchParams.get('store_id'), UUID3);
    assert.strictEqual(url.searchParams.get('from'), '5');
    assert.strictEqual(url.searchParams.get('limit'), '50');
  });

  it('uses default limit of 100 when not specified', async () => {
    let capturedUrl;
    mockFetch((url) => {
      capturedUrl = url;
      return okResponse({ events: [], head_sequence: 0 });
    });

    const client = new SequencerClient(makeConfig());
    await client.pull(0);

    const url = new URL(capturedUrl);
    assert.strictEqual(url.searchParams.get('limit'), '100');
  });

  it('maps snake_case server response to camelCase envelope', async () => {
    const serverEvent = makeServerEvent(7);
    mockFetch(() => okResponse({ events: [serverEvent], head_sequence: 7 }));

    const client = new SequencerClient(makeConfig());
    const result = await client.pull(7);

    assert.strictEqual(result.events.length, 1);
    const ev = result.events[0];
    assert.strictEqual(ev.envelope.eventId, UUID1);
    assert.strictEqual(ev.envelope.tenantId, UUID2);
    assert.strictEqual(ev.envelope.storeId, UUID3);
    assert.strictEqual(ev.envelope.entityType, 'order');
    assert.strictEqual(ev.envelope.entityId, 'ORD-7');
    assert.strictEqual(ev.envelope.eventType, 'created');
    assert.strictEqual(ev.envelope.vesVersion, 1);
    assert.strictEqual(ev.envelope.payloadKind, 0);
    assert.strictEqual(ev.sequenceNumber, 7);
    assert.strictEqual(ev.sequencedAt, '2024-01-01T00:00:01Z');
    assert.strictEqual(ev.receiptHash, 'abc123');
  });

  it('falls back to payload_hash for payloadPlainHash (legacy compat)', async () => {
    const serverEvent = makeServerEvent(1);
    const legacyHash = 'a'.repeat(64);
    delete serverEvent.envelope.payload_plain_hash;
    serverEvent.envelope.payload_hash = legacyHash;

    mockFetch(() => okResponse({ events: [serverEvent], head_sequence: 1 }));

    const client = new SequencerClient(makeConfig());
    const result = await client.pull(1);
    assert.strictEqual(result.events[0].envelope.payloadPlainHash, legacyHash);
  });

  it('sets payloadCipherHash to zero hash when missing (legacy compat)', async () => {
    const serverEvent = makeServerEvent(1);
    delete serverEvent.envelope.payload_cipher_hash;

    mockFetch(() => okResponse({ events: [serverEvent], head_sequence: 1 }));

    const client = new SequencerClient(makeConfig());
    const result = await client.pull(1);
    assert.strictEqual(result.events[0].envelope.payloadCipherHash, '0'.repeat(64));
  });

  it('returns hasMore=true when events.length === limit', async () => {
    const events = Array.from({ length: 3 }, (_, i) => makeServerEvent(i + 1));
    mockFetch(() => okResponse({ events, head_sequence: 3 }));

    const client = new SequencerClient(makeConfig());
    const result = await client.pull(1, 3);
    assert.strictEqual(result.hasMore, true);
  });

  it('returns hasMore=false when events.length < limit', async () => {
    const events = [makeServerEvent(1), makeServerEvent(2)];
    mockFetch(() => okResponse({ events, head_sequence: 2 }));

    const client = new SequencerClient(makeConfig());
    const result = await client.pull(1, 10);
    assert.strictEqual(result.hasMore, false);
  });

  it('returns nextSequence = maxSequenceNumber + 1', async () => {
    const events = [makeServerEvent(5), makeServerEvent(8)];
    mockFetch(() => okResponse({ events, head_sequence: 8 }));

    const client = new SequencerClient(makeConfig());
    const result = await client.pull(5, 10);
    assert.strictEqual(result.nextSequence, 9);
  });

  it('returns nextSequence = fromSequence when no events', async () => {
    mockFetch(() => okResponse({ events: [], head_sequence: 0 }));

    const client = new SequencerClient(makeConfig());
    const result = await client.pull(42, 10);
    assert.strictEqual(result.nextSequence, 43);
  });

  it('returns headSequence from response', async () => {
    const events = [makeServerEvent(3)];
    mockFetch(() => okResponse({ events, head_sequence: 99 }));

    const client = new SequencerClient(makeConfig());
    const result = await client.pull(3);
    assert.strictEqual(result.headSequence, 99);
  });

  it('sets agentKeyId=0 for legacy events missing the field', async () => {
    const serverEvent = makeServerEvent(1);
    delete serverEvent.envelope.agent_key_id;

    mockFetch(() => okResponse({ events: [serverEvent], head_sequence: 1 }));

    const client = new SequencerClient(makeConfig());
    const result = await client.pull(1);
    assert.strictEqual(result.events[0].envelope.agentKeyId, 0);
  });
});

// =============================================================================
// pullStream (async iterator / pagination)
// =============================================================================

describe('SequencerClient — pullStream', () => {
  afterEach(() => restoreFetch());

  it('yields all events from a single page', async () => {
    const events = [makeServerEvent(1), makeServerEvent(2)];
    mockFetch(() => okResponse({ events, head_sequence: 2 }));

    const client = new SequencerClient(makeConfig());
    const collected = [];
    for await (const ev of client.pullStream(1)) {
      collected.push(ev);
    }
    assert.strictEqual(collected.length, 2);
  });

  it('follows pagination across multiple pages', async () => {
    // Page 1: 3 events with limit=3 → hasMore=true
    // Page 2: 2 events with limit=3 → hasMore=false
    let pageCount = 0;
    mockFetch(() => {
      pageCount++;
      if (pageCount === 1) {
        const events = [makeServerEvent(1), makeServerEvent(2), makeServerEvent(3)];
        return okResponse({ events, head_sequence: 3 });
      }
      const events = [makeServerEvent(4), makeServerEvent(5)];
      return okResponse({ events, head_sequence: 5 });
    });

    // Override pull limit to 3 for this test by monkeypatching
    const client = new SequencerClient(makeConfig());
    const origPull = client.pull.bind(client);
    client.pull = (from, _limit) => origPull(from, 3);

    const collected = [];
    for await (const ev of client.pullStream(1)) {
      collected.push(ev);
    }
    assert.strictEqual(collected.length, 5);
    assert.strictEqual(pageCount, 2);
  });

  it('yields nothing when no events available', async () => {
    mockFetch(() => okResponse({ events: [], head_sequence: 0 }));

    const client = new SequencerClient(makeConfig());
    const collected = [];
    for await (const ev of client.pullStream(1)) {
      collected.push(ev);
    }
    assert.strictEqual(collected.length, 0);
  });
});

// =============================================================================
// getHead
// =============================================================================

describe('SequencerClient — getHead', () => {
  afterEach(() => restoreFetch());

  it('calls GET /api/v1/head with tenant and store query params', async () => {
    let capturedUrl;
    mockFetch((url) => {
      capturedUrl = url;
      return okResponse({ head_sequence: 0 });
    });

    const client = new SequencerClient(
      makeConfig({ tenantId: UUID2, storeId: UUID3, url: 'https://seq.example.com' }),
    );
    await client.getHead();

    const url = new URL(capturedUrl);
    assert.strictEqual(url.pathname, '/api/v1/head');
    assert.strictEqual(url.searchParams.get('tenant_id'), UUID2);
    assert.strictEqual(url.searchParams.get('store_id'), UUID3);
  });

  it('returns SyncState with headSequence', async () => {
    mockFetch(() =>
      okResponse({
        head_sequence: 42,
        state_root: 'abc123',
        latest_commitment: { batch_id: 'BATCH-7' },
      }),
    );

    const client = new SequencerClient(makeConfig({ tenantId: UUID2, storeId: UUID3 }));
    const state = await client.getHead();

    assert.strictEqual(state.headSequence, 42);
    assert.strictEqual(state.stateRoot, 'abc123');
    assert.strictEqual(state.lastCommitmentId, 'BATCH-7');
    assert.strictEqual(state.tenantId, UUID2);
    assert.strictEqual(state.storeId, UUID3);
  });

  it('defaults headSequence to 0 when missing', async () => {
    mockFetch(() => okResponse({}));

    const client = new SequencerClient(makeConfig());
    const state = await client.getHead();
    assert.strictEqual(state.headSequence, 0);
  });

  it('sets lastCommitmentId to undefined when no latest_commitment', async () => {
    mockFetch(() => okResponse({ head_sequence: 5 }));

    const client = new SequencerClient(makeConfig());
    const state = await client.getHead();
    assert.strictEqual(state.lastCommitmentId, undefined);
  });
});

// =============================================================================
// getCommitment
// =============================================================================

describe('SequencerClient — getCommitment', () => {
  afterEach(() => restoreFetch());

  it('calls GET /api/v1/commitments/{batchId}', async () => {
    let capturedUrl;
    mockFetch((url) => {
      capturedUrl = url;
      return okResponse({
        batch_id: 'BATCH-1',
        merkle_root: 'root123',
        start_sequence: 1,
        end_sequence: 10,
        event_count: 10,
        committed_at: '2024-01-01T00:00:00Z',
      });
    });

    const client = new SequencerClient(
      makeConfig({ url: 'https://seq.example.com' }),
    );
    await client.getCommitment('BATCH-1');
    assert.ok(capturedUrl.endsWith('/api/v1/commitments/BATCH-1'));
  });

  it('returns BatchCommitment with correct fields', async () => {
    mockFetch(() =>
      okResponse({
        batch_id: 'BATCH-42',
        merkle_root: 'merkle-hex',
        start_sequence: 1,
        end_sequence: 100,
        event_count: 100,
        committed_at: '2024-02-01T00:00:00Z',
      }),
    );

    const client = new SequencerClient(makeConfig());
    const commitment = await client.getCommitment('BATCH-42');

    assert.strictEqual(commitment.batchId, 'BATCH-42');
    assert.strictEqual(commitment.merkleRoot, 'merkle-hex');
    assert.strictEqual(commitment.startSequence, 1);
    assert.strictEqual(commitment.endSequence, 100);
    assert.strictEqual(commitment.eventCount, 100);
    assert.strictEqual(commitment.committedAt, '2024-02-01T00:00:00Z');
  });

  it('returns null for 404 response', async () => {
    mockFetch(() => errorResponse(404, 'Not Found'));

    const client = new SequencerClient(makeConfig());
    const result = await client.getCommitment('BATCH-MISSING');
    assert.strictEqual(result, null);
  });

  it('re-throws non-404 errors', async () => {
    mockFetch(() => errorResponse(500, 'Internal Server Error'));

    const client = new SequencerClient(makeConfig());
    await assert.rejects(() => client.getCommitment('BATCH-1'), /500/);
  });
});

// =============================================================================
// getEntityHistory
// =============================================================================

describe('SequencerClient — getEntityHistory', () => {
  afterEach(() => restoreFetch());

  it('calls GET /api/v1/entities/{type}/{id}', async () => {
    let capturedUrl;
    mockFetch((url) => {
      capturedUrl = url;
      return okResponse({ events: [] });
    });

    const client = new SequencerClient(
      makeConfig({ url: 'https://seq.example.com' }),
    );
    await client.getEntityHistory('order', 'ORD-99');
    assert.ok(capturedUrl.includes('/api/v1/entities/order/ORD-99'));
  });

  it('includes tenant and store in query params', async () => {
    let capturedUrl;
    mockFetch((url) => {
      capturedUrl = url;
      return okResponse({ events: [] });
    });

    const client = new SequencerClient(makeConfig({ tenantId: UUID2, storeId: UUID3 }));
    await client.getEntityHistory('order', 'ORD-1');

    const url = new URL(capturedUrl);
    assert.strictEqual(url.searchParams.get('tenant_id'), UUID2);
    assert.strictEqual(url.searchParams.get('store_id'), UUID3);
  });

  it('maps entity events to SequencedEvent camelCase shape', async () => {
    const payloadHash = computePayloadPlainHash({ qty: 1 }).toString('hex');
    mockFetch(() =>
      okResponse({
        events: [
          {
            event_id: UUID1,
            command_id: null,
            tenant_id: UUID2,
            store_id: UUID3,
            entity_type: 'order',
            entity_id: 'ORD-1',
            event_type: 'fulfilled',
            payload: { qty: 1 },
            ves_version: 1,
            payload_kind: 0,
            payload_encrypted: null,
            payload_plain_hash: payloadHash,
            payload_cipher_hash: '0'.repeat(64),
            agent_key_id: 2,
            agent_signature: 'f'.repeat(128),
            base_version: 5,
            created_at: '2024-03-01T00:00:00Z',
            source_agent: UUID1,
            sequence_number: 3,
            sequenced_at: '2024-03-01T00:00:02Z',
            receipt_hash: 'rcpt-abc',
          },
        ],
      }),
    );

    const client = new SequencerClient(makeConfig());
    const events = await client.getEntityHistory('order', 'ORD-1');

    assert.strictEqual(events.length, 1);
    const ev = events[0];
    assert.strictEqual(ev.envelope.eventId, UUID1);
    assert.strictEqual(ev.envelope.entityType, 'order');
    assert.strictEqual(ev.envelope.eventType, 'fulfilled');
    assert.strictEqual(ev.envelope.agentKeyId, 2);
    assert.strictEqual(ev.envelope.baseVersion, 5);
    assert.strictEqual(ev.sequenceNumber, 3);
    assert.strictEqual(ev.receiptHash, 'rcpt-abc');
  });

  it('returns empty array when no events', async () => {
    mockFetch(() => okResponse({ events: [] }));

    const client = new SequencerClient(makeConfig());
    const events = await client.getEntityHistory('product', 'SKU-1');
    assert.deepStrictEqual(events, []);
  });
});

// =============================================================================
// registerAgentKey
// =============================================================================

describe('SequencerClient — registerAgentKey', () => {
  afterEach(() => restoreFetch());

  it('calls POST /api/v1/agents/keys', async () => {
    let capturedUrl, capturedMethod;
    mockFetch((url, opts) => {
      capturedUrl = url;
      capturedMethod = opts.method;
      return okResponse({ success: true });
    });

    const client = new SequencerClient(
      makeConfig({ url: 'https://seq.example.com' }),
    );
    await client.registerAgentKey({
      agentId: UUID1,
      keyId: 1,
      publicKey: 'a'.repeat(64),
    });

    assert.ok(capturedUrl.endsWith('/api/v1/agents/keys'));
    assert.strictEqual(capturedMethod, 'POST');
  });

  it('sends snake_case fields in body', async () => {
    let capturedBody;
    mockFetch((_url, opts) => {
      capturedBody = JSON.parse(opts.body);
      return okResponse({ success: true });
    });

    const client = new SequencerClient(makeConfig({ tenantId: UUID2 }));
    await client.registerAgentKey({
      agentId: UUID1,
      keyId: 3,
      publicKey: 'pubkey-hex',
      validFrom: '2024-01-01T00:00:00Z',
      validTo: '2025-01-01T00:00:00Z',
    });

    assert.strictEqual(capturedBody.tenant_id, UUID2);
    assert.strictEqual(capturedBody.agent_id, UUID1);
    assert.strictEqual(capturedBody.key_id, 3);
    assert.strictEqual(capturedBody.public_key, 'pubkey-hex');
    assert.strictEqual(capturedBody.valid_from, '2024-01-01T00:00:00Z');
    assert.strictEqual(capturedBody.valid_to, '2025-01-01T00:00:00Z');
  });

  it('sends bundle-aware PQ key registration fields when provided', async () => {
    let capturedBody;
    mockFetch((_url, opts) => {
      capturedBody = JSON.parse(opts.body);
      return okResponse({ success: true });
    });

    const client = new SequencerClient(makeConfig({ tenantId: UUID2 }));
    await client.registerAgentKey({
      agentId: UUID1,
      keyId: 4,
      keyType: 1,
      keyAlgorithm: 5,
      publicKey: 'legacy-ed25519-key',
      publicKeyBundle: {
        ed25519PublicKey: 'aa'.repeat(32),
        mlDsa65PublicKey: 'bb'.repeat(64),
      },
      proofOfPossession: 'cc'.repeat(64),
      proofOfPossessionBundle: {
        ed25519Pop: 'dd'.repeat(64),
        mlDsa65Pop: 'ee'.repeat(32),
      },
    });

    assert.strictEqual(capturedBody.key_type, 1);
    assert.strictEqual(capturedBody.key_algorithm, 5);
    assert.deepStrictEqual(capturedBody.public_key_bundle, {
      ed25519_public_key: 'aa'.repeat(32),
      ml_dsa_65_public_key: 'bb'.repeat(64),
      x25519_public_key: null,
      ml_kem_768_public_key: null,
    });
    assert.strictEqual(capturedBody.proof_of_possession, 'cc'.repeat(64));
    assert.deepStrictEqual(capturedBody.proof_of_possession_bundle, {
      ed25519_pop: 'dd'.repeat(64),
      ml_dsa_65_pop: 'ee'.repeat(32),
    });
  });

  it('rejects legacy key registration under hybrid profile', async () => {
    const client = new SequencerClient(makeConfig({ securityProfile: 'hybrid' }));

    await assert.rejects(
      () =>
        client.registerAgentKey({
          agentId: UUID1,
          keyId: 1,
          keyType: 1,
          keyAlgorithm: 1,
          publicKey: 'legacy-ed25519-key',
        }),
      /Hybrid profile requires KEY_ALGORITHM_ED25519_ML_DSA_65/,
    );
  });

  it('returns { success: true } when response.success is truthy', async () => {
    mockFetch(() => okResponse({ success: true }));

    const client = new SequencerClient(makeConfig());
    const result = await client.registerAgentKey({ agentId: UUID1, keyId: 1, publicKey: 'pk' });
    assert.strictEqual(result.success, true);
  });

  it('returns { success: true } when response omits success field', async () => {
    mockFetch(() => okResponse({}));

    const client = new SequencerClient(makeConfig());
    const result = await client.registerAgentKey({ agentId: UUID1, keyId: 1, publicKey: 'pk' });
    assert.strictEqual(result.success, true);
  });
});

// =============================================================================
// getAgentKeys
// =============================================================================

describe('SequencerClient — getAgentKeys', () => {
  afterEach(() => restoreFetch());

  it('calls GET /api/v1/agents/keys with query params', async () => {
    let capturedUrl;
    mockFetch((url) => {
      capturedUrl = url;
      return okResponse({ keys: [] });
    });

    const client = new SequencerClient(
      makeConfig({ tenantId: UUID2, url: 'https://seq.example.com' }),
    );
    await client.getAgentKeys(UUID1);

    const url = new URL(capturedUrl);
    assert.strictEqual(url.pathname, '/api/v1/agents/keys');
    assert.strictEqual(url.searchParams.get('tenant_id'), UUID2);
    assert.strictEqual(url.searchParams.get('agent_id'), UUID1);
  });

  it('maps snake_case key fields to camelCase', async () => {
    mockFetch(() =>
      okResponse({
        keys: [
          {
            key_id: 2,
            public_key: 'pub-hex',
            status: 'active',
            created_at: '2024-01-01T00:00:00Z',
            valid_from: '2024-01-01T00:00:00Z',
            valid_to: '2025-01-01T00:00:00Z',
          },
        ],
      }),
    );

    const client = new SequencerClient(makeConfig());
    const keys = await client.getAgentKeys(UUID1);

    assert.strictEqual(keys.length, 1);
    assert.strictEqual(keys[0].keyId, 2);
    assert.strictEqual(keys[0].publicKey, 'pub-hex');
    assert.strictEqual(keys[0].status, 'active');
    assert.strictEqual(keys[0].createdAt, '2024-01-01T00:00:00Z');
    assert.strictEqual(keys[0].validFrom, '2024-01-01T00:00:00Z');
    assert.strictEqual(keys[0].validTo, '2025-01-01T00:00:00Z');
  });

  it('maps PQ key bundle fields to camelCase', async () => {
    mockFetch(() =>
      okResponse({
        keys: [
          {
            key_id: 7,
            key_type: 1,
            key_algorithm: 5,
            public_key: 'legacy-pk',
            public_key_bundle: {
              ed25519_public_key: 'aa'.repeat(32),
              ml_dsa_65_public_key: 'bb'.repeat(64),
              x25519_public_key: null,
              ml_kem_768_public_key: null,
            },
            status: 'active',
            created_at: '2024-01-01T00:00:00Z',
          },
        ],
      }),
    );

    const client = new SequencerClient(makeConfig());
    const keys = await client.getAgentKeys(UUID1);

    assert.strictEqual(keys[0].keyType, 1);
    assert.strictEqual(keys[0].keyAlgorithm, 5);
    assert.deepStrictEqual(keys[0].publicKeyBundle, {
      ed25519PublicKey: 'aa'.repeat(32),
      mlDsa65PublicKey: 'bb'.repeat(64),
      x25519PublicKey: null,
      mlKem768PublicKey: null,
    });
  });

  it('returns empty array when no keys registered', async () => {
    mockFetch(() => okResponse({ keys: [] }));

    const client = new SequencerClient(makeConfig());
    const keys = await client.getAgentKeys(UUID1);
    assert.deepStrictEqual(keys, []);
  });
});

// =============================================================================
// verifyInclusion (Merkle proof verification)
// =============================================================================

describe('SequencerClient — verifyInclusion', () => {
  /**
   * Build a real Merkle tree and verify inclusion proofs using the actual
   * computeLeafHash / computeNodeHash / computeEventSigningHash functions from
   * crypto.js.  verifyInclusion now correctly maps envelope fields to the
   * params expected by computeLeafHash.
   */

  /**
   * Helper: compute the leaf hash the same way verifyInclusion does.
   * This mirrors the fixed implementation so the tests can build valid roots.
   */
  function makeLeafHash(envelope, sequenceNumber) {
    const eventSigningHash = computeEventSigningHash({
      vesVersion: envelope.vesVersion || 1,
      tenantId: envelope.tenantId,
      storeId: envelope.storeId,
      eventId: envelope.eventId,
      commandId: envelope.commandId || null,
      sourceAgentId: envelope.sourceAgent,
      agentKeyId: envelope.agentKeyId,
      entityType: envelope.entityType,
      entityId: envelope.entityId,
      eventType: envelope.eventType,
      baseVersion: envelope.baseVersion || null,
      createdAt: envelope.createdAt,
      payloadPlainHash: hexToBuffer(envelope.payloadPlainHash),
      payloadCipherHash: hexToBuffer(envelope.payloadCipherHash),
    });

    return computeLeafHash({
      tenantId: envelope.tenantId,
      storeId: envelope.storeId,
      sequenceNumber,
      eventSigningHash,
      agentSignature: hexToBuffer(envelope.agentSignature),
    });
  }

  it('returns true for a single-leaf tree where leaf hash equals the root', () => {
    const client = new SequencerClient(makeConfig());
    const envelope = makeEnvelope({ sequenceNumber: 0 });

    // With an empty proof the leaf hash IS the root.
    const root = makeLeafHash(envelope, 0).toString('hex');

    const result = client.verifyInclusion(
      envelope,
      { merkleRoot: root, leafIndex: 0, proofHashes: [], leafCount: 1 },
      root,
    );
    assert.strictEqual(result, true);
  });

  it('returns false for a single-leaf tree when expectedRoot does not match', () => {
    const client = new SequencerClient(makeConfig());
    const envelope = makeEnvelope({ sequenceNumber: 0 });

    const wrongRoot = 'bad'.padEnd(64, '0');

    const result = client.verifyInclusion(
      envelope,
      { merkleRoot: wrongRoot, leafIndex: 0, proofHashes: [], leafCount: 1 },
      wrongRoot,
    );
    assert.strictEqual(result, false);
  });

  it('returns true for a 2-leaf tree with leaf at index 0 (right sibling)', () => {
    const client = new SequencerClient(makeConfig());
    const envelope = makeEnvelope({ sequenceNumber: 0 });

    const leafA = makeLeafHash(envelope, 0);
    const sibling = Buffer.alloc(32, 0xab); // arbitrary right sibling
    // leafIndex=0 → bit0=0 → sibling is to the RIGHT → root = node(leafA, sibling)
    const root = computeNodeHash(leafA, sibling).toString('hex');

    const result = client.verifyInclusion(
      envelope,
      { merkleRoot: root, leafIndex: 0, proofHashes: [sibling.toString('hex')], leafCount: 2 },
      root,
    );
    assert.strictEqual(result, true);
  });

  it('returns true for a 2-leaf tree with leaf at index 1 (left sibling)', () => {
    // leafIndex=1 → bit0=1 → sibling is to the LEFT → root = node(sibling, leafA)
    const client = new SequencerClient(makeConfig());
    const envelope = makeEnvelope({ sequenceNumber: 1 });

    const leafA = makeLeafHash(envelope, 1);
    const sibling = Buffer.alloc(32, 0xff); // arbitrary left sibling
    const root = computeNodeHash(sibling, leafA).toString('hex');

    const result = client.verifyInclusion(
      envelope,
      { merkleRoot: root, leafIndex: 1, proofHashes: [sibling.toString('hex')], leafCount: 2 },
      root,
    );
    assert.strictEqual(result, true);
  });

  it('verifies a correct 2-leaf proof end-to-end with real crypto', () => {
    const client = new SequencerClient(makeConfig());
    const envelope = makeEnvelope({ sequenceNumber: 5 });

    const leafA = makeLeafHash(envelope, 5);
    const siblingBuf = Buffer.alloc(32, 0xcc);
    // leafIndex=4 → bit0=0 → sibling right → root = node(leafA, sibling)
    const root = computeNodeHash(leafA, siblingBuf).toString('hex');

    assert.strictEqual(
      client.verifyInclusion(
        envelope,
        { merkleRoot: root, leafIndex: 4, proofHashes: [siblingBuf.toString('hex')], leafCount: 2 },
        root,
      ),
      true,
    );

    // A tampered expected root must return false
    assert.strictEqual(
      client.verifyInclusion(
        envelope,
        { merkleRoot: root, leafIndex: 4, proofHashes: [siblingBuf.toString('hex')], leafCount: 2 },
        'dead'.padEnd(64, 'beef'),
      ),
      false,
    );
  });

  it('returns true for zero-sibling proof using envelope.sequenceNumber over proof.leafIndex', () => {
    // verifyInclusion uses envelope.sequenceNumber when present, falling back to
    // proof.leafIndex only when the field is absent.
    const client = new SequencerClient(makeConfig());

    // Attach sequenceNumber directly on the envelope (as a SequencedEvent caller would)
    const envelope = makeEnvelope({ sequenceNumber: 42 });

    // Build root using sequence 42
    const root = makeLeafHash(envelope, 42).toString('hex');

    // Pass a different leafIndex — it must be ignored because envelope.sequenceNumber wins
    const result = client.verifyInclusion(
      envelope,
      { merkleRoot: root, leafIndex: 99, proofHashes: [], leafCount: 1 },
      root,
    );
    assert.strictEqual(result, true);
  });
});

// =============================================================================
// verifyEventSignature
// =============================================================================

describe('SequencerClient — verifyEventSignature', () => {
  it('returns false for a zeroed-out signature against a random key', () => {
    const client = new SequencerClient(makeConfig());
    const envelope = makeEnvelope({
      agentSignature: '0'.repeat(128),
      payloadPlainHash: '0'.repeat(64),
      payloadCipherHash: '0'.repeat(64),
    });

    // Generate a random Ed25519 key pair
    const { publicKey } = crypto.generateKeyPairSync('ed25519');
    const rawPublicKey = publicKey.export({ type: 'spki', format: 'der' }).slice(-32);

    const result = client.verifyEventSignature(envelope, rawPublicKey);
    assert.strictEqual(result, false);
  });

  it('returns true for a valid Ed25519 signature over the signing hash', () => {
    const client = new SequencerClient(makeConfig());

    // Generate a real Ed25519 key pair
    const { privateKey, publicKey } = crypto.generateKeyPairSync('ed25519');
    const rawPublicKey = publicKey.export({ type: 'spki', format: 'der' }).slice(-32);

    // Build envelope hashes
    const payloadPlainHash = computePayloadPlainHash({ test: 'sign' });
    const zeroCipherHash = Buffer.alloc(32, 0);

    const envelope = makeEnvelope({
      payloadPlainHash: payloadPlainHash.toString('hex'),
      payloadCipherHash: zeroCipherHash.toString('hex'),
      agentKeyId: 1,
    });

    // Compute the signing hash the same way the client will
    const signingHash = computeEventSigningHash({
      vesVersion: envelope.vesVersion,
      tenantId: envelope.tenantId,
      storeId: envelope.storeId,
      eventId: envelope.eventId,
      commandId: null,
      sourceAgentId: envelope.sourceAgent,
      agentKeyId: envelope.agentKeyId,
      entityType: envelope.entityType,
      entityId: envelope.entityId,
      eventType: envelope.eventType,
      baseVersion: null,
      createdAt: envelope.createdAt,
      payloadPlainHash: payloadPlainHash,
      payloadCipherHash: zeroCipherHash,
    });

    // Sign with Ed25519
    const signature = crypto.sign(null, signingHash, privateKey);
    envelope.agentSignature = signature.toString('hex');

    const result = client.verifyEventSignature(envelope, rawPublicKey);
    assert.strictEqual(result, true);
  });

  it('returns false when signature is from a different key', () => {
    const client = new SequencerClient(makeConfig());

    const { privateKey: wrongKey } = crypto.generateKeyPairSync('ed25519');
    const { publicKey: correctPublicKey } = crypto.generateKeyPairSync('ed25519');
    const rawCorrectPublicKey = correctPublicKey.export({ type: 'spki', format: 'der' }).slice(-32);

    const payloadPlainHash = computePayloadPlainHash({ x: 1 });
    const zeroCipherHash = Buffer.alloc(32, 0);
    const envelope = makeEnvelope({
      payloadPlainHash: payloadPlainHash.toString('hex'),
      payloadCipherHash: zeroCipherHash.toString('hex'),
    });

    const signingHash = computeEventSigningHash({
      vesVersion: 1,
      tenantId: envelope.tenantId,
      storeId: envelope.storeId,
      eventId: envelope.eventId,
      commandId: null,
      sourceAgentId: envelope.sourceAgent,
      agentKeyId: envelope.agentKeyId,
      entityType: envelope.entityType,
      entityId: envelope.entityId,
      eventType: envelope.eventType,
      baseVersion: null,
      createdAt: envelope.createdAt,
      payloadPlainHash,
      payloadCipherHash: zeroCipherHash,
    });

    const signature = crypto.sign(null, signingHash, wrongKey);
    envelope.agentSignature = signature.toString('hex');

    const result = client.verifyEventSignature(envelope, rawCorrectPublicKey);
    assert.strictEqual(result, false);
  });

  it('returns false when payload has been tampered', () => {
    const client = new SequencerClient(makeConfig());

    const { privateKey, publicKey } = crypto.generateKeyPairSync('ed25519');
    const rawPublicKey = publicKey.export({ type: 'spki', format: 'der' }).slice(-32);

    const originalPayload = { amount: 100 };
    const payloadPlainHash = computePayloadPlainHash(originalPayload);
    const zeroCipherHash = Buffer.alloc(32, 0);

    const envelope = makeEnvelope({
      payloadPlainHash: payloadPlainHash.toString('hex'),
      payloadCipherHash: zeroCipherHash.toString('hex'),
    });

    // Sign over the original hash
    const signingHash = computeEventSigningHash({
      vesVersion: 1,
      tenantId: envelope.tenantId,
      storeId: envelope.storeId,
      eventId: envelope.eventId,
      commandId: null,
      sourceAgentId: envelope.sourceAgent,
      agentKeyId: envelope.agentKeyId,
      entityType: envelope.entityType,
      entityId: envelope.entityId,
      eventType: envelope.eventType,
      baseVersion: null,
      createdAt: envelope.createdAt,
      payloadPlainHash,
      payloadCipherHash: zeroCipherHash,
    });
    const signature = crypto.sign(null, signingHash, privateKey);
    envelope.agentSignature = signature.toString('hex');

    // Tamper: change the payload hash to simulate modified data
    const tamperedHash = computePayloadPlainHash({ amount: 999 });
    envelope.payloadPlainHash = tamperedHash.toString('hex');

    const result = client.verifyEventSignature(envelope, rawPublicKey);
    assert.strictEqual(result, false);
  });

  it(
    'returns true for a valid hybrid signature when given a hybrid public-key bundle',
    { skip: !hasNativeHybridPqcVerificationSupport() },
    () => {
      const client = new SequencerClient(makeConfig());
      const hybrid = generateHybridSigningKeypair();

      const payloadPlainHash = computePayloadPlainHash({ amount: 123 });
      const zeroCipherHash = Buffer.alloc(32, 0);
      const envelope = makeEnvelope({
        payloadPlainHash: payloadPlainHash.toString('hex'),
        payloadCipherHash: zeroCipherHash.toString('hex'),
        agentSignatureScheme: SIGNATURE_SCHEME_ED25519_ML_DSA_65,
      });

      const signingHash = computeEventSigningHash({
        vesVersion: envelope.vesVersion,
        tenantId: envelope.tenantId,
        storeId: envelope.storeId,
        eventId: envelope.eventId,
        commandId: null,
        sourceAgentId: envelope.sourceAgent,
        agentKeyId: envelope.agentKeyId,
        entityType: envelope.entityType,
        entityId: envelope.entityId,
        eventType: envelope.eventType,
        baseVersion: null,
        createdAt: envelope.createdAt,
        payloadPlainHash,
        payloadCipherHash: zeroCipherHash,
      });

      const signatureBundle = signEventHashHybrid(signingHash, {
        ed25519PrivateKey: hybrid.ed25519PrivateKey,
        mlDsa65Seed: hybrid.mlDsa65Seed,
      });
      envelope.agentSignature = signatureBundle.ed25519Signature.toString('hex');
      envelope.agentSignatureBundle = {
        ed25519Signature: signatureBundle.ed25519Signature.toString('hex'),
        mlDsa65Signature: signatureBundle.mlDsa65Signature.toString('hex'),
      };

      const result = client.verifyEventSignature(envelope, {
        ed25519PublicKey: hybrid.ed25519PublicKey.toString('hex'),
        mlDsa65PublicKey: hybrid.mlDsa65PublicKey.toString('hex'),
      });
      assert.strictEqual(result, true);
    },
  );

  it(
    'returns false when the ML-DSA component of a hybrid signature is tampered',
    { skip: !hasNativeHybridPqcVerificationSupport() },
    () => {
      const client = new SequencerClient(makeConfig());
      const hybrid = generateHybridSigningKeypair();

      const payloadPlainHash = computePayloadPlainHash({ amount: 321 });
      const zeroCipherHash = Buffer.alloc(32, 0);
      const envelope = makeEnvelope({
        payloadPlainHash: payloadPlainHash.toString('hex'),
        payloadCipherHash: zeroCipherHash.toString('hex'),
        agentSignatureScheme: SIGNATURE_SCHEME_ED25519_ML_DSA_65,
      });

      const signingHash = computeEventSigningHash({
        vesVersion: envelope.vesVersion,
        tenantId: envelope.tenantId,
        storeId: envelope.storeId,
        eventId: envelope.eventId,
        commandId: null,
        sourceAgentId: envelope.sourceAgent,
        agentKeyId: envelope.agentKeyId,
        entityType: envelope.entityType,
        entityId: envelope.entityId,
        eventType: envelope.eventType,
        baseVersion: null,
        createdAt: envelope.createdAt,
        payloadPlainHash,
        payloadCipherHash: zeroCipherHash,
      });

      const signatureBundle = signEventHashHybrid(signingHash, {
        ed25519PrivateKey: hybrid.ed25519PrivateKey,
        mlDsa65Seed: hybrid.mlDsa65Seed,
      });
      const tamperedMlDsa = Buffer.from(signatureBundle.mlDsa65Signature);
      tamperedMlDsa[0] ^= 0xff;

      envelope.agentSignature = signatureBundle.ed25519Signature.toString('hex');
      envelope.agentSignatureBundle = {
        ed25519Signature: signatureBundle.ed25519Signature.toString('hex'),
        mlDsa65Signature: tamperedMlDsa.toString('hex'),
      };

      const result = client.verifyEventSignature(envelope, {
        ed25519PublicKey: hybrid.ed25519PublicKey.toString('hex'),
        mlDsa65PublicKey: hybrid.mlDsa65PublicKey.toString('hex'),
      });
      assert.strictEqual(result, false);
    },
  );
});
