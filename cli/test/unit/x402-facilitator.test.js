import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {
  buildFacilitatorSupportedResponse,
  createFacilitatorHttpHandler,
  verifyFacilitatedPayment,
} from '../../src/x402/facilitator.js';
import { createExactEvmPaymentPayload } from '../../src/x402/exact-evm.js';
import { deriveEvmWalletFromSeed } from '../../src/chains/wallet.js';

function generateEd25519Keypair() {
  const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
  const privDer = privateKey.export({ type: 'pkcs8', format: 'der' });
  const pubDer = publicKey.export({ type: 'spki', format: 'der' });
  return {
    privBytes: privDer.subarray(-32),
    pubBytes: pubDer.subarray(-32),
  };
}

function createMockRequest({ method = 'GET', url = '/', body = null } = {}) {
  const chunks =
    body === null ? [] : [Buffer.from(typeof body === 'string' ? body : JSON.stringify(body))];
  return {
    method,
    url,
    async *[Symbol.asyncIterator]() {
      for (const chunk of chunks) {
        yield chunk;
      }
    },
  };
}

function createMockResponse() {
  const headers = new Map();
  let rawBody = '';
  return {
    statusCode: 200,
    setHeader(name, value) {
      headers.set(String(name).toLowerCase(), String(value));
    },
    end(value = '') {
      rawBody += String(value);
    },
    body() {
      return rawBody ? JSON.parse(rawBody) : null;
    },
    header(name) {
      return headers.get(String(name).toLowerCase()) || null;
    },
  };
}

describe('x402 facilitator helpers', () => {
  it('builds a supported response with signer metadata', () => {
    const response = buildFacilitatorSupportedResponse({
      facilitatorPrivateKey: `0x${'11'.repeat(32)}`,
    });

    assert.ok(Array.isArray(response.kinds));
    assert.ok(response.kinds.some((kind) => kind.network === 'eip155:84532'));
    assert.ok(response.kinds.some((kind) => kind.network === 'eip155:11155111'));
    assert.deepStrictEqual(response.extensions, []);
    assert.ok(Array.isArray(response.signers['eip155:*']));
    assert.strictEqual(response.signers['eip155:*'][0].length, 42);
  });

  it('verifies an exact payment payload without onchain checks', async () => {
    const { privBytes, pubBytes } = generateEd25519Keypair();
    const wallet = deriveEvmWalletFromSeed(privBytes, 'base');
    const paymentRequirements = {
      scheme: 'exact',
      network: 'eip155:8453',
      amount: '10000',
      asset: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
      payTo: '0x3333333333333333333333333333333333333333',
      maxTimeoutSeconds: 60,
      extra: {
        assetTransferMethod: 'eip3009',
        name: 'USD Coin',
        version: '2',
      },
    };

    const paymentPayload = await createExactEvmPaymentPayload({
      requirement: paymentRequirements,
      paymentRequired: {
        resource: {
          url: 'https://api.example.com/premium',
          description: 'Premium data',
        },
      },
      signingKey: { privateKey: privBytes, publicKey: pubBytes },
      payerAddress: wallet.address,
      resourceUrl: 'https://api.example.com/premium',
    });

    const response = await verifyFacilitatedPayment({
      x402Version: 2,
      paymentPayload,
      paymentRequirements,
      checkOnchain: false,
    });

    assert.deepStrictEqual(response, { isValid: true, payer: wallet.address });
  });

  it('serves /supported and /verify over HTTP handler', async () => {
    const { privBytes, pubBytes } = generateEd25519Keypair();
    const wallet = deriveEvmWalletFromSeed(privBytes, 'base');
    const paymentRequirements = {
      scheme: 'exact',
      network: 'eip155:8453',
      amount: '10000',
      asset: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
      payTo: '0x4444444444444444444444444444444444444444',
      maxTimeoutSeconds: 60,
      extra: {
        assetTransferMethod: 'eip3009',
        name: 'USD Coin',
        version: '2',
      },
    };
    const paymentPayload = await createExactEvmPaymentPayload({
      requirement: paymentRequirements,
      paymentRequired: {
        resource: {
          url: 'https://api.example.com/premium',
          description: 'Premium data',
        },
      },
      signingKey: { privateKey: privBytes, publicKey: pubBytes },
      payerAddress: wallet.address,
      resourceUrl: 'https://api.example.com/premium',
    });
    const handler = createFacilitatorHttpHandler({
      facilitatorPrivateKey: `0x${'22'.repeat(32)}`,
      defaultCheckOnchain: false,
    });

    const supportedReq = createMockRequest({ method: 'GET', url: '/supported' });
    const supportedRes = createMockResponse();
    await handler(supportedReq, supportedRes);
    assert.strictEqual(supportedRes.statusCode, 200);
    assert.ok(Array.isArray(supportedRes.body().kinds));
    assert.strictEqual(supportedRes.header('content-type'), 'application/json');

    const verifyReq = createMockRequest({
      method: 'POST',
      url: '/verify',
      body: {
        x402Version: 2,
        paymentPayload,
        paymentRequirements,
      },
    });
    const verifyRes = createMockResponse();
    await handler(verifyReq, verifyRes);

    assert.strictEqual(verifyRes.statusCode, 200);
    assert.deepStrictEqual(verifyRes.body(), { isValid: true, payer: wallet.address });
  });
});
