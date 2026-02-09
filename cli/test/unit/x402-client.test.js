/**
 * Unit tests for x402/client.js — X402SequencerClient
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { X402SequencerClient } from '../../src/x402/client.js';

// ===========================================================================
// Helpers
// ===========================================================================

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

// ===========================================================================
// Constructor
// ===========================================================================

describe('X402SequencerClient — constructor', () => {
  it('accepts a string URL', () => {
    const client = new X402SequencerClient('https://seq.example.com');
    assert.strictEqual(client.baseUrl, 'https://seq.example.com');
  });

  it('extracts sequencerUrl from object config', () => {
    const client = new X402SequencerClient({ sequencerUrl: 'https://seq.example.com' });
    assert.strictEqual(client.baseUrl, 'https://seq.example.com');
  });

  it('extracts sequencer.url from nested config', () => {
    const client = new X402SequencerClient({ sequencer: { url: 'https://seq.example.com' } });
    assert.strictEqual(client.baseUrl, 'https://seq.example.com');
  });

  it('converts grpc:// to http://', () => {
    const client = new X402SequencerClient('grpc://seq.example.com:9090');
    assert.strictEqual(client.baseUrl, 'http://seq.example.com:9090');
  });

  it('converts grpcs:// to https://', () => {
    const client = new X402SequencerClient('grpcs://seq.example.com:443');
    assert.strictEqual(client.baseUrl, 'https://seq.example.com:443');
  });

  it('strips trailing slash from URL', () => {
    const client = new X402SequencerClient('https://seq.example.com/');
    assert.ok(!client.baseUrl.endsWith('/'));
  });

  it('throws when URL is missing', () => {
    assert.throws(() => new X402SequencerClient({}), /Sequencer URL is required/);
  });

  it('throws when config is null', () => {
    assert.throws(() => new X402SequencerClient(null), /Sequencer URL is required/);
  });
});

// ===========================================================================
// _getHeaders
// ===========================================================================

describe('X402SequencerClient — _getHeaders', () => {
  it('includes Content-Type', () => {
    const client = new X402SequencerClient('https://seq.example.com');
    const headers = client._getHeaders();
    assert.strictEqual(headers['Content-Type'], 'application/json');
  });

  it('includes Authorization with apiKey', () => {
    const client = new X402SequencerClient({
      sequencerUrl: 'https://seq.example.com',
      auth: { apiKey: 'my-api-key' },
    });
    const headers = client._getHeaders();
    assert.strictEqual(headers.Authorization, 'Bearer my-api-key');
  });

  it('includes Authorization with jwt when no apiKey', () => {
    const client = new X402SequencerClient({
      sequencerUrl: 'https://seq.example.com',
      auth: { jwt: 'my-jwt-token' },
    });
    const headers = client._getHeaders();
    assert.strictEqual(headers.Authorization, 'Bearer my-jwt-token');
  });

  it('prefers apiKey over jwt', () => {
    const client = new X402SequencerClient({
      sequencerUrl: 'https://seq.example.com',
      auth: { apiKey: 'key', jwt: 'jwt' },
    });
    const headers = client._getHeaders();
    assert.strictEqual(headers.Authorization, 'Bearer key');
  });

  it('has no Authorization header when no auth', () => {
    const client = new X402SequencerClient('https://seq.example.com');
    const headers = client._getHeaders();
    assert.strictEqual(headers.Authorization, undefined);
  });

  it('uses getCredentials() when config provides it', () => {
    const client = new X402SequencerClient({
      sequencerUrl: 'https://seq.example.com',
      getCredentials: () => ({ apiKey: 'fn-key', jwt: null }),
    });
    const headers = client._getHeaders();
    assert.strictEqual(headers.Authorization, 'Bearer fn-key');
  });
});

// ===========================================================================
// _request
// ===========================================================================

describe('X402SequencerClient — _request', () => {
  afterEach(() => restoreFetch());

  it('calls fetch with correct URL and method', async () => {
    let capturedUrl, capturedOptions;
    mockFetch((url, opts) => {
      capturedUrl = url;
      capturedOptions = opts;
      return okResponse({ ok: true });
    });

    const client = new X402SequencerClient('https://seq.example.com');
    await client._request('GET', '/api/v1/test');

    assert.strictEqual(capturedUrl, 'https://seq.example.com/api/v1/test');
    assert.strictEqual(capturedOptions.method, 'GET');
  });

  it('sends body as JSON for POST', async () => {
    let capturedOptions;
    mockFetch((_url, opts) => {
      capturedOptions = opts;
      return okResponse({ ok: true });
    });

    const client = new X402SequencerClient('https://seq.example.com');
    await client._request('POST', '/api/v1/test', { foo: 'bar' });

    assert.strictEqual(capturedOptions.body, JSON.stringify({ foo: 'bar' }));
  });

  it('does not include body for GET', async () => {
    let capturedOptions;
    mockFetch((_url, opts) => {
      capturedOptions = opts;
      return okResponse({ ok: true });
    });

    const client = new X402SequencerClient('https://seq.example.com');
    await client._request('GET', '/api/v1/test');

    assert.strictEqual(capturedOptions.body, undefined);
  });

  it('returns parsed JSON on success', async () => {
    mockFetch(() => okResponse({ result: 42 }));

    const client = new X402SequencerClient('https://seq.example.com');
    const result = await client._request('GET', '/test');

    assert.deepStrictEqual(result, { result: 42 });
  });

  it('throws on non-OK response', async () => {
    mockFetch(() => errorResponse(500, 'Internal Server Error'));

    const client = new X402SequencerClient('https://seq.example.com');
    await assert.rejects(() => client._request('GET', '/test'), /Sequencer request failed: 500/);
  });
});

// ===========================================================================
// API methods
// ===========================================================================

describe('X402SequencerClient — submitPaymentIntent', () => {
  afterEach(() => restoreFetch());

  it('calls POST /api/v1/x402/payments', async () => {
    let capturedUrl, capturedMethod;
    mockFetch((url, opts) => {
      capturedUrl = url;
      capturedMethod = opts.method;
      return okResponse({ intent_id: 'INT-1' });
    });

    const client = new X402SequencerClient('https://seq.example.com');
    const result = await client.submitPaymentIntent({ amount: 100 });

    assert.ok(capturedUrl.endsWith('/api/v1/x402/payments'));
    assert.strictEqual(capturedMethod, 'POST');
    assert.strictEqual(result.intent_id, 'INT-1');
  });
});

describe('X402SequencerClient — getPaymentStatus', () => {
  afterEach(() => restoreFetch());

  it('calls GET /api/v1/x402/payments/{id}', async () => {
    let capturedUrl;
    mockFetch((url) => {
      capturedUrl = url;
      return okResponse({ status: 'pending' });
    });

    const client = new X402SequencerClient('https://seq.example.com');
    await client.getPaymentStatus('INT-42');

    assert.ok(capturedUrl.endsWith('/api/v1/x402/payments/INT-42'));
  });
});

describe('X402SequencerClient — getPaymentReceipt', () => {
  afterEach(() => restoreFetch());

  it('calls GET /api/v1/x402/payments/{id}/receipt', async () => {
    let capturedUrl;
    mockFetch((url) => {
      capturedUrl = url;
      return okResponse({ receipt: { txHash: '0xabc' } });
    });

    const client = new X402SequencerClient('https://seq.example.com');
    await client.getPaymentReceipt('INT-42');

    assert.ok(capturedUrl.endsWith('/api/v1/x402/payments/INT-42/receipt'));
  });
});

describe('X402SequencerClient — createBatch', () => {
  afterEach(() => restoreFetch());

  it('calls POST /api/v1/x402/batches', async () => {
    let capturedUrl;
    mockFetch((url) => {
      capturedUrl = url;
      return okResponse({ batch_id: 'B-1' });
    });

    const client = new X402SequencerClient('https://seq.example.com');
    await client.createBatch({ tenant_id: 'T1' });

    assert.ok(capturedUrl.endsWith('/api/v1/x402/batches'));
  });
});

describe('X402SequencerClient — settleBatch', () => {
  afterEach(() => restoreFetch());

  it('calls POST /api/v1/x402/batches/settle', async () => {
    let capturedUrl;
    mockFetch((url) => {
      capturedUrl = url;
      return okResponse({ settled: true });
    });

    const client = new X402SequencerClient('https://seq.example.com');
    await client.settleBatch({ batch_id: 'B-1' });

    assert.ok(capturedUrl.endsWith('/api/v1/x402/batches/settle'));
  });
});

// ===========================================================================
// waitForReceipt
// ===========================================================================

describe('X402SequencerClient — waitForReceipt', () => {
  afterEach(() => restoreFetch());

  it('returns receipt when available immediately', async () => {
    mockFetch(() => okResponse({ receipt: { txHash: '0xfoo' } }));

    const client = new X402SequencerClient('https://seq.example.com');
    const receipt = await client.waitForReceipt('INT-1', {
      timeoutMs: 5000,
      intervalMs: 10,
    });

    assert.deepStrictEqual(receipt, { txHash: '0xfoo' });
  });

  it('throws on timeout when receipt never available', async () => {
    mockFetch(() => okResponse({ receipt: null }));

    const client = new X402SequencerClient('https://seq.example.com');
    await assert.rejects(
      () => client.waitForReceipt('INT-1', { timeoutMs: 50, intervalMs: 10 }),
      /Timed out/,
    );
  });

  it('logs warning on poll error and continues polling', async () => {
    let callCount = 0;
    const warnings = [];
    const origWarn = console.warn;
    console.warn = (...args) => warnings.push(args.join(' '));

    mockFetch(() => {
      callCount++;
      if (callCount <= 2) {
        throw new Error('network error');
      }
      return okResponse({ receipt: { txHash: '0xok' } });
    });

    try {
      const client = new X402SequencerClient('https://seq.example.com');
      const receipt = await client.waitForReceipt('INT-1', {
        timeoutMs: 5000,
        intervalMs: 10,
      });
      assert.deepStrictEqual(receipt, { txHash: '0xok' });
      assert.ok(warnings.some((w) => w.includes('poll error')));
    } finally {
      console.warn = origWarn;
    }
  });
});
