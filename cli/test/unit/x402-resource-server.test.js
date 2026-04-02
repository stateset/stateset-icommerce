import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {
  buildExactEvmPaymentRequired,
  createExactEvmResourceServerHandler,
} from '../../src/x402/resource-server.js';
import { createExactEvmPaymentPayload } from '../../src/x402/exact-evm.js';
import { deriveEvmWalletFromSeed } from '../../src/chains/wallet.js';
import { decodeBase64Json, encodeBase64Json } from '../../src/x402/crypto.js';

function generateEd25519Keypair() {
  const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
  const privDer = privateKey.export({ type: 'pkcs8', format: 'der' });
  const pubDer = publicKey.export({ type: 'spki', format: 'der' });
  return {
    privBytes: privDer.subarray(-32),
    pubBytes: pubDer.subarray(-32),
  };
}

function createMockRequest({ method = 'GET', url = '/', headers = {}, body = null } = {}) {
  const chunks =
    body === null ? [] : [Buffer.from(typeof body === 'string' ? body : JSON.stringify(body))];
  return {
    method,
    url,
    headers,
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

describe('x402 exact resource server helpers', () => {
  it('builds a standards-shaped PaymentRequired payload', () => {
    const paymentRequired = buildExactEvmPaymentRequired({
      url: 'https://api.example.com/premium',
      description: 'Premium data',
      mimeType: 'application/json',
      amount: '10000',
      asset: '0x036CbD53842c5426634e7929541eC2318f3dCF7e',
      network: 'eip155:84532',
      payTo: '0x1234567890123456789012345678901234567890',
    });

    assert.strictEqual(paymentRequired.x402Version, 2);
    assert.strictEqual(paymentRequired.resource.url, 'https://api.example.com/premium');
    assert.strictEqual(paymentRequired.accepts[0].scheme, 'exact');
    assert.strictEqual(paymentRequired.accepts[0].network, 'eip155:84532');
  });

  it('returns payment-required challenge when no payment is attached', async () => {
    const paymentRequired = buildExactEvmPaymentRequired({
      url: 'https://api.example.com/premium',
      description: 'Premium data',
      mimeType: 'application/json',
      amount: '10000',
      asset: '0x036CbD53842c5426634e7929541eC2318f3dCF7e',
      network: 'eip155:84532',
      payTo: '0x1234567890123456789012345678901234567890',
    });

    const handler = createExactEvmResourceServerHandler({
      paymentRequired,
      facilitatorPrivateKey: `0x${'33'.repeat(32)}`,
      settlePayment: async () => {
        throw new Error('should not settle');
      },
    });

    const req = createMockRequest({
      url: '/premium',
      headers: { host: 'api.example.com' },
    });
    const res = createMockResponse();
    await handler(req, res);

    assert.strictEqual(res.statusCode, 402);
    const challenge = decodeBase64Json(res.header('payment-required'));
    assert.strictEqual(challenge.x402Version, 2);
    assert.strictEqual(challenge.accepts[0].network, 'eip155:84532');
  });

  it('returns PAYMENT-RESPONSE when a valid payment is attached', async () => {
    const { privBytes, pubBytes } = generateEd25519Keypair();
    const wallet = deriveEvmWalletFromSeed(privBytes, 'base_sepolia');
    const paymentRequired = buildExactEvmPaymentRequired({
      url: 'https://api.example.com/premium',
      description: 'Premium data',
      mimeType: 'application/json',
      amount: '10000',
      asset: '0x036CbD53842c5426634e7929541eC2318f3dCF7e',
      network: 'eip155:84532',
      payTo: '0x9999999999999999999999999999999999999999',
    });
    const paymentPayload = await createExactEvmPaymentPayload({
      requirement: paymentRequired.accepts[0],
      paymentRequired,
      signingKey: { privateKey: privBytes, publicKey: pubBytes },
      payerAddress: wallet.address,
      resourceUrl: paymentRequired.resource.url,
    });

    const settlement = {
      success: true,
      payer: wallet.address,
      transaction: '0xabc123',
      network: 'eip155:84532',
    };

    const handler = createExactEvmResourceServerHandler({
      paymentRequired,
      checkOnchain: false,
      facilitatorPrivateKey: `0x${'44'.repeat(32)}`,
      settlePayment: async () => settlement,
      onRequest: async () => ({
        status: 200,
        body: { data: 'ok' },
        headers: { 'x-example': '1' },
      }),
    });

    const req = createMockRequest({
      url: '/premium',
      headers: {
        host: 'api.example.com',
        'payment-signature': encodeBase64Json(paymentPayload),
      },
    });
    const res = createMockResponse();
    await handler(req, res);

    assert.strictEqual(res.statusCode, 200);
    assert.deepStrictEqual(res.body(), { data: 'ok' });
    assert.strictEqual(res.header('x-example'), '1');
    assert.deepStrictEqual(decodeBase64Json(res.header('payment-response')), settlement);
  });
});
