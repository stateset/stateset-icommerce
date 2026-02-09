/**
 * Unit tests for x402/agent.js — BudgetExceededError, verifyPaymentHeader,
 * decodePaymentHeader, decodeReceiptHeader, x402Fetch, createX402Agent
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {
  BudgetExceededError,
  x402Fetch,
  createX402Agent,
  decodePaymentHeader,
  decodeReceiptHeader,
  verifyPaymentHeader,
} from '../../src/x402/agent.js';
import {
  computeX402SigningHash,
  signX402Hash,
  hashToHex,
  hexToBytes,
  encodeBase64Json,
  networkChainId,
} from '../../src/x402/crypto.js';

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

/**
 * Generate a raw 32-byte Ed25519 keypair for testing.
 * Returns { privBytes: Buffer, pubBytes: Buffer }
 */
function generateEd25519Keypair() {
  const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
  const privDer = privateKey.export({ type: 'pkcs8', format: 'der' });
  const pubDer = publicKey.export({ type: 'spki', format: 'der' });
  return {
    privBytes: privDer.subarray(-32),
    pubBytes: pubDer.subarray(-32),
  };
}

// ===========================================================================
// BudgetExceededError
// ===========================================================================

describe('BudgetExceededError', () => {
  it('is an instance of Error', () => {
    const err = new BudgetExceededError('over budget');
    assert.ok(err instanceof Error);
  });

  it('has name BudgetExceededError', () => {
    const err = new BudgetExceededError('over budget');
    assert.strictEqual(err.name, 'BudgetExceededError');
  });

  it('has the correct message', () => {
    const err = new BudgetExceededError('test message');
    assert.strictEqual(err.message, 'test message');
  });

  it('has a stack trace', () => {
    const err = new BudgetExceededError('test');
    assert.ok(err.stack);
  });
});

// ===========================================================================
// decodePaymentHeader / decodeReceiptHeader
// ===========================================================================

describe('decodePaymentHeader', () => {
  it('decodes a base64-encoded JSON object', () => {
    const original = { amount: 100, network: 'set_chain' };
    const encoded = encodeBase64Json(original);
    const result = decodePaymentHeader(encoded);
    assert.deepStrictEqual(result, original);
  });

  it('decodes a base64-encoded JSON array', () => {
    const original = [1, 2, 3];
    const encoded = encodeBase64Json(original);
    const result = decodePaymentHeader(encoded);
    assert.deepStrictEqual(result, original);
  });

  it('throws on invalid base64', () => {
    assert.throws(() => decodePaymentHeader('!!!not-base64!!!'));
  });
});

describe('decodeReceiptHeader', () => {
  it('decodes a base64-encoded JSON object', () => {
    const original = { receipt: { txHash: '0xabc' } };
    const encoded = encodeBase64Json(original);
    const result = decodeReceiptHeader(encoded);
    assert.deepStrictEqual(result, original);
  });
});

// ===========================================================================
// verifyPaymentHeader
// ===========================================================================

describe('verifyPaymentHeader', () => {
  // Build a valid payload for testing
  function buildValidPayload() {
    const { privBytes, pubBytes } = generateEd25519Keypair();
    const now = Math.floor(Date.now() / 1000);
    const validUntil = now + 3600;
    const nonce = 42;
    const network = 'set_chain';
    const chainId = networkChainId(network);

    const signingHash = computeX402SigningHash({
      payerAddress: '0xPayer',
      payeeAddress: '0xPayee',
      amount: 1000,
      asset: 'usdc',
      network,
      chainId,
      validUntil,
      nonce,
    });

    const signature = signX402Hash(signingHash, privBytes);

    return {
      payer_address: '0xPayer',
      payee_address: '0xPayee',
      amount: 1000,
      asset: 'usdc',
      network,
      chain_id: chainId,
      valid_until: validUntil,
      nonce,
      signing_hash: hashToHex(signingHash),
      payer_signature: hashToHex(signature),
      payer_public_key: hashToHex(pubBytes),
    };
  }

  it('returns ok:true for valid payload', () => {
    const payload = buildValidPayload();
    const result = verifyPaymentHeader(payload);
    assert.strictEqual(result.ok, true);
    assert.ok(result.signingHash);
  });

  it('returns ok:false when expired (valid_until in past)', () => {
    const payload = buildValidPayload();
    payload.valid_until = Math.floor(Date.now() / 1000) - 3600; // 1 hour ago

    // Recompute signing hash and signature for expired valid_until
    const { privBytes, pubBytes } = (() => {
      const pk = hexToBytes(payload.payer_public_key);
      return { pubBytes: pk };
    })();

    // Since we changed valid_until, the signing hash won't match the original.
    // But the expiration check happens first, so it'll fail on expiration.
    const result = verifyPaymentHeader(payload);
    assert.strictEqual(result.ok, false);
    assert.ok(result.reason.includes('expired'));
  });

  it('returns ok:false for chain_id mismatch', () => {
    const payload = buildValidPayload();
    payload.chain_id = 99999; // Wrong chain ID
    const result = verifyPaymentHeader(payload);
    assert.strictEqual(result.ok, false);
    assert.ok(result.reason.includes('Chain id mismatch'));
  });

  it('returns ok:false for wrong signing_hash', () => {
    const payload = buildValidPayload();
    // Tamper with signing hash
    payload.signing_hash = hashToHex(crypto.randomBytes(32));
    const result = verifyPaymentHeader(payload);
    assert.strictEqual(result.ok, false);
    assert.ok(result.reason.includes('Signing hash mismatch'));
  });

  it('returns ok:false when payer_public_key is missing', () => {
    const payload = buildValidPayload();
    delete payload.payer_public_key;
    const result = verifyPaymentHeader(payload);
    assert.strictEqual(result.ok, false);
    assert.ok(result.reason.includes('Missing payer_public_key'));
  });

  it('returns ok:false for bad signature', () => {
    const payload = buildValidPayload();
    // Replace signature with random bytes
    payload.payer_signature = hashToHex(crypto.randomBytes(64));
    const result = verifyPaymentHeader(payload);
    assert.strictEqual(result.ok, false);
    assert.ok(result.reason.includes('Signature verification failed'));
  });

  it('returns ok:false when signature is from different key', () => {
    const payload = buildValidPayload();
    // Generate a different keypair and use its public key
    const { pubBytes: otherPub } = generateEd25519Keypair();
    payload.payer_public_key = hashToHex(otherPub);
    const result = verifyPaymentHeader(payload);
    assert.strictEqual(result.ok, false);
    assert.ok(result.reason.includes('Signature verification failed'));
  });
});

// ===========================================================================
// x402Fetch — validation
// ===========================================================================

describe('x402Fetch — validation', () => {
  afterEach(() => restoreFetch());

  it('throws on missing sequencerClient', async () => {
    await assert.rejects(
      () =>
        x402Fetch(
          'https://api.example.com/resource',
          {},
          {
            tenantId: 'T',
            storeId: 'S',
            agentId: 'A',
            payerAddress: '0xPayer',
            signingKey: { privateKey: Buffer.alloc(32), publicKey: Buffer.alloc(32) },
          },
        ),
      /sequencerClient is required/,
    );
  });

  it('throws on missing tenantId/storeId/agentId', async () => {
    await assert.rejects(
      () =>
        x402Fetch(
          'https://api.example.com/resource',
          {},
          {
            sequencerClient: {},
            payerAddress: '0xPayer',
            signingKey: { privateKey: Buffer.alloc(32), publicKey: Buffer.alloc(32) },
          },
        ),
      /tenantId, storeId, and agentId are required/,
    );
  });

  it('throws on missing payerAddress', async () => {
    await assert.rejects(
      () =>
        x402Fetch(
          'https://api.example.com/resource',
          {},
          {
            sequencerClient: {},
            tenantId: 'T',
            storeId: 'S',
            agentId: 'A',
            signingKey: { privateKey: Buffer.alloc(32), publicKey: Buffer.alloc(32) },
          },
        ),
      /payerAddress is required/,
    );
  });

  it('throws on missing signingKey', async () => {
    await assert.rejects(
      () =>
        x402Fetch(
          'https://api.example.com/resource',
          {},
          {
            sequencerClient: {},
            tenantId: 'T',
            storeId: 'S',
            agentId: 'A',
            payerAddress: '0xPayer',
          },
        ),
      /signingKey with privateKey\/publicKey is required/,
    );
  });

  it('throws on SSRF attempt (localhost)', async () => {
    await assert.rejects(
      () =>
        x402Fetch(
          'http://127.0.0.1/resource',
          {},
          {
            sequencerClient: {},
            tenantId: 'T',
            storeId: 'S',
            agentId: 'A',
            payerAddress: '0xPayer',
            signingKey: { privateKey: Buffer.alloc(32), publicKey: Buffer.alloc(32) },
          },
        ),
      /SSRF/,
    );
  });

  it('throws on SSRF attempt (private IP)', async () => {
    await assert.rejects(
      () =>
        x402Fetch(
          'http://192.168.1.1/resource',
          {},
          {
            sequencerClient: {},
            tenantId: 'T',
            storeId: 'S',
            agentId: 'A',
            payerAddress: '0xPayer',
            signingKey: { privateKey: Buffer.alloc(32), publicKey: Buffer.alloc(32) },
          },
        ),
      /SSRF/,
    );
  });
});

// ===========================================================================
// x402Fetch — non-402 response pass-through
// ===========================================================================

describe('x402Fetch — non-402 response', () => {
  afterEach(() => restoreFetch());

  it('returns the response directly when status is not 402', async () => {
    const expectedBody = { data: 'hello' };
    mockFetch(() => ({
      ok: true,
      status: 200,
      headers: new Map(),
      json: async () => expectedBody,
    }));

    const response = await x402Fetch(
      'https://api.example.com/resource',
      {},
      {
        sequencerClient: {},
        tenantId: 'T',
        storeId: 'S',
        agentId: 'A',
        payerAddress: '0xPayer',
        signingKey: { privateKey: Buffer.alloc(32), publicKey: Buffer.alloc(32) },
      },
    );

    assert.strictEqual(response.status, 200);
    const body = await response.json();
    assert.deepStrictEqual(body, expectedBody);
  });
});

// ===========================================================================
// createX402Agent
// ===========================================================================

describe('createX402Agent', () => {
  it('returns object with fetch and budget properties', () => {
    const agent = createX402Agent({
      sequencerClient: {},
      tenantId: 'T',
      storeId: 'S',
      agentId: 'A',
      payerAddress: '0xPayer',
      signingKey: { privateKey: Buffer.alloc(32), publicKey: Buffer.alloc(32) },
    });
    assert.ok(typeof agent.fetch === 'function');
    assert.ok('budget' in agent);
  });

  it('returns null budget when no budget config provided', () => {
    const agent = createX402Agent({
      sequencerClient: {},
      tenantId: 'T',
      storeId: 'S',
      agentId: 'A',
      payerAddress: '0xPayer',
      signingKey: { privateKey: Buffer.alloc(32), publicKey: Buffer.alloc(32) },
    });
    assert.strictEqual(agent.budget, null);
  });

  it('fetch function delegates to x402Fetch', async () => {
    mockFetch(() => ({
      ok: true,
      status: 200,
      headers: new Map(),
    }));

    try {
      const agent = createX402Agent({
        sequencerClient: {},
        tenantId: 'T',
        storeId: 'S',
        agentId: 'A',
        payerAddress: '0xPayer',
        signingKey: { privateKey: Buffer.alloc(32), publicKey: Buffer.alloc(32) },
      });

      const response = await agent.fetch('https://api.example.com/resource');
      assert.strictEqual(response.status, 200);
    } finally {
      restoreFetch();
    }
  });
});
