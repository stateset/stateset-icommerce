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
import {
  createExactEvmPaymentPayload,
  verifyExactEvmPaymentPayload,
} from '../../src/x402/exact-evm.js';
import { deriveEvmWalletFromSeed } from '../../src/chains/wallet.js';

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
  function buildValidPayload(overrides = {}) {
    const { privBytes, pubBytes } = generateEd25519Keypair();
    const now = Math.floor(Date.now() / 1000);
    const validUntil = now + 3600;
    const nonce = 42;
    const network = 'set_chain';
    const chainId = networkChainId(network);
    const resourceUri = overrides.resource_uri ?? '/premium';
    const resourceMethod = overrides.resource_method ?? 'GET';

    const signingHash = computeX402SigningHash({
      payerAddress: '0xPayer',
      payeeAddress: '0xPayee',
      amount: 1000,
      asset: 'usdc',
      network,
      chainId,
      validUntil,
      nonce,
      resourceUri,
      resourceMethod,
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
      resource_uri: resourceUri,
      resource_method: resourceMethod,
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

  it('returns ok:false when resource_uri changes after signing', () => {
    const payload = buildValidPayload();
    payload.resource_uri = '/other';
    const result = verifyPaymentHeader(payload);
    assert.strictEqual(result.ok, false);
    assert.ok(result.reason.includes('Signing hash mismatch'));
  });

  it('returns ok:false when resource_method changes after signing', () => {
    const payload = buildValidPayload();
    payload.resource_method = 'POST';
    const result = verifyPaymentHeader(payload);
    assert.strictEqual(result.ok, false);
    assert.ok(result.reason.includes('Signing hash mismatch'));
  });
});

// ===========================================================================
// x402Fetch — validation
// ===========================================================================

describe('x402Fetch — validation', () => {
  afterEach(() => restoreFetch());

  it('throws on missing payerAddress', async () => {
    await assert.rejects(
      () =>
        x402Fetch(
          'https://api.example.com/resource',
          {},
          {
            agentId: 'A',
            signingKey: { privateKey: Buffer.alloc(32), publicKey: Buffer.alloc(32) },
          },
        ),
      /payerAddress is required/,
    );
  });

  it('throws on missing agentId explicitly', async () => {
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
      /agentId is required/,
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

  it('throws before fetch when DNS resolves to a private address', async () => {
    let fetchCalled = false;
    global.fetch = async () => {
      fetchCalled = true;
      throw new Error('fetch should not be called');
    };

    await assert.rejects(
      () =>
        x402Fetch(
          'https://merchant.stateset.test/resource',
          {},
          {
            sequencerClient: {},
            tenantId: 'T',
            storeId: 'S',
            agentId: 'A',
            payerAddress: '0xPayer',
            signingKey: { privateKey: Buffer.alloc(32), publicKey: Buffer.alloc(32) },
            urlLookup: async () => [{ address: '172.16.0.5', family: 4 }],
          },
        ),
      /resolves to internal address/,
    );
    assert.equal(fetchCalled, false);
  });

  it('throws before following redirects to private addresses', async () => {
    const calls = [];
    global.fetch = async (url) => {
      calls.push(String(url));
      return new Response('', {
        status: 302,
        headers: { location: 'http://169.254.169.254/latest/meta-data' },
      });
    };

    await assert.rejects(
      () =>
        x402Fetch(
          'https://redirector.stateset.test/resource',
          {},
          {
            sequencerClient: {},
            tenantId: 'T',
            storeId: 'S',
            agentId: 'A',
            payerAddress: '0xPayer',
            signingKey: { privateKey: Buffer.alloc(32), publicKey: Buffer.alloc(32) },
            urlLookup: async () => [{ address: '8.8.8.8', family: 4 }],
          },
        ),
      /SSRF|blocked|internal/i,
    );
    assert.deepEqual(calls, ['https://redirector.stateset.test/resource']);
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

  it('allows local URLs when validateUrl is false', async () => {
    mockFetch(async () => {
      return new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      });
    });

    const response = await x402Fetch(
      'http://127.0.0.1/resource',
      {},
      {
        sequencerClient: {},
        tenantId: 'T',
        storeId: 'S',
        agentId: 'A',
        payerAddress: '0xPayer',
        signingKey: { privateKey: Buffer.alloc(32), publicKey: Buffer.alloc(32) },
        validateUrl: false,
      },
    );

    assert.strictEqual(response.status, 200);
    assert.deepStrictEqual(await response.json(), { ok: true });
  });
});

describe('x402Fetch — exact EVM flow', () => {
  afterEach(() => restoreFetch());

  it('retries with PAYMENT-SIGNATURE carrying an x402 v2 PaymentPayload', async () => {
    const { privBytes, pubBytes } = generateEd25519Keypair();
    const payerWallet = deriveEvmWalletFromSeed(privBytes, 'base');
    let callCount = 0;
    let capturedHeaders = null;

    const paymentRequired = {
      x402Version: 2,
      error: 'PAYMENT-SIGNATURE header is required',
      resource: {
        url: 'https://api.example.com/premium',
        description: 'Premium data',
        mimeType: 'application/json',
      },
      accepts: [
        {
          scheme: 'exact',
          network: 'eip155:8453',
          amount: '10000',
          asset: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
          payTo: '0x1111111111111111111111111111111111111111',
          maxTimeoutSeconds: 60,
          extra: {
            assetTransferMethod: 'eip3009',
            name: 'USD Coin',
            version: '2',
          },
        },
      ],
      extensions: {},
    };

    mockFetch((_url, options = {}) => {
      callCount += 1;
      if (callCount === 1) {
        return {
          ok: false,
          status: 402,
          headers: {
            get(name) {
              return String(name).toLowerCase() === 'payment-required'
                ? encodeBase64Json(paymentRequired)
                : null;
            },
          },
          clone() {
            return this;
          },
          async json() {
            return paymentRequired;
          },
        };
      }

      capturedHeaders = options.headers;
      return {
        ok: true,
        status: 200,
        headers: new Headers(),
        async json() {
          return { success: true };
        },
      };
    });

    const response = await x402Fetch(
      'https://api.example.com/premium',
      { method: 'GET' },
      {
        agentId: 'agent-test',
        payerAddress: payerWallet.address,
        signingKey: { privateKey: privBytes, publicKey: pubBytes },
      },
    );

    assert.strictEqual(response.status, 200);
    assert.ok(capturedHeaders['PAYMENT-SIGNATURE']);
    const paymentPayload = decodePaymentHeader(capturedHeaders['PAYMENT-SIGNATURE']);
    assert.strictEqual(paymentPayload.x402Version, 2);
    assert.strictEqual(paymentPayload.accepted.scheme, 'exact');
    assert.strictEqual(paymentPayload.accepted.network, 'eip155:8453');
    assert.ok(!('meta' in paymentPayload));
    assert.strictEqual(paymentPayload.payload.authorization.from, payerWallet.address);
    assert.strictEqual(paymentPayload.payload.authorization.to, paymentRequired.accepts[0].payTo);
  });

  it('fails legacy 402 handling without sequencer configuration', async () => {
    mockFetch(() => ({
      ok: false,
      status: 402,
      headers: {
        get(name) {
          return String(name).toLowerCase() === 'x-payment-required'
            ? encodeBase64Json({
                payee_address: '0xPayee',
                amount: 1000,
                asset: 'usdc',
                network: 'set_chain',
              })
            : null;
        },
      },
      clone() {
        return this;
      },
      async json() {
        return {};
      },
    }));

    await assert.rejects(
      () =>
        x402Fetch(
          'https://api.example.com/premium',
          { method: 'GET' },
          {
            agentId: 'agent-test',
            payerAddress: '0xPayer',
            signingKey: { privateKey: Buffer.alloc(32, 1), publicKey: Buffer.alloc(32, 2) },
          },
        ),
      /sequencerClient is required for legacy sequencer-backed x402 payments/,
    );
  });
});

describe('exact EVM helpers', () => {
  it('creates and verifies an exact EVM payment payload without onchain checks', async () => {
    const { privBytes, pubBytes } = generateEd25519Keypair();
    const wallet = deriveEvmWalletFromSeed(privBytes, 'base');
    const requirement = {
      scheme: 'exact',
      network: 'eip155:8453',
      amount: '10000',
      asset: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
      payTo: '0x2222222222222222222222222222222222222222',
      maxTimeoutSeconds: 60,
      extra: {
        assetTransferMethod: 'eip3009',
        name: 'USD Coin',
        version: '2',
      },
    };

    const payload = await createExactEvmPaymentPayload({
      requirement,
      paymentRequired: {
        resource: {
          url: 'https://api.example.com/data',
          description: 'Premium data',
        },
      },
      signingKey: { privateKey: privBytes, publicKey: pubBytes },
      payerAddress: wallet.address,
      resourceUrl: 'https://api.example.com/data',
      method: 'GET',
    });

    const verification = await verifyExactEvmPaymentPayload({
      paymentPayload: payload,
      paymentRequirements: requirement,
      checkOnchain: false,
    });

    assert.deepStrictEqual(verification, { isValid: true, payer: wallet.address });
    assert.ok(!('meta' in payload));
  });

  it('rejects authorizations that exceed maxTimeoutSeconds', async () => {
    const { privBytes, pubBytes } = generateEd25519Keypair();
    const wallet = deriveEvmWalletFromSeed(privBytes, 'base');
    const requirement = {
      scheme: 'exact',
      network: 'eip155:8453',
      amount: '10000',
      asset: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
      payTo: '0x2222222222222222222222222222222222222222',
      maxTimeoutSeconds: 60,
      extra: {
        assetTransferMethod: 'eip3009',
        name: 'USD Coin',
        version: '2',
      },
    };

    const payload = await createExactEvmPaymentPayload({
      requirement,
      paymentRequired: {
        resource: {
          url: 'https://api.example.com/data',
          description: 'Premium data',
        },
      },
      signingKey: { privateKey: privBytes, publicKey: pubBytes },
      payerAddress: wallet.address,
      resourceUrl: 'https://api.example.com/data',
      method: 'GET',
    });

    payload.payload.authorization.validBefore = String(
      Number(payload.payload.authorization.validAfter) + 120,
    );

    const verification = await verifyExactEvmPaymentPayload({
      paymentPayload: payload,
      paymentRequirements: requirement,
      checkOnchain: false,
    });

    assert.deepStrictEqual(verification, {
      isValid: false,
      invalidReason: 'invalid_payment_requirements',
      payer: wallet.address,
    });
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
