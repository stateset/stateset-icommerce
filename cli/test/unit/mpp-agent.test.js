import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  MppPaymentPolicyError,
  createPaymentChallenge,
  createPaymentReceipt,
} from '../../src/mpp/index.js';
import {
  createMppHttpAgent,
  discoverMppHttpService,
  extractPayableHttpRoutes,
  fetchMppDiscoveryDocument,
  fetchMppServiceInfo,
  extractHttpPaymentChallenge,
  extractHttpPaymentReceipt,
  mppFetch,
} from '../../src/mpp/agent.js';

function createHeaders(init = {}) {
  const map = new Map(
    Object.entries(init).map(([key, value]) => [String(key).toLowerCase(), String(value)]),
  );
  return {
    get(name) {
      return map.get(String(name).toLowerCase()) || null;
    },
  };
}

function createResponse({ status = 200, body = null, headers = {} } = {}) {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: createHeaders(headers),
    async json() {
      if (body === undefined) {
        throw new Error('No JSON body');
      }
      return body;
    },
    clone() {
      return createResponse({ status, body, headers });
    },
  };
}

function encodeHeaderPayload(payload) {
  return Buffer.from(JSON.stringify(payload), 'utf8').toString('base64url');
}

function createMockDiscovery() {
  return {
    serviceInfo: {
      protocol: 'mpp',
      protocolVersion: 'draft-2026-03-18',
      transport: {
        type: 'http',
      },
      discovery: {
        canonicalOpenapiPath: '/openapi.json',
        serviceInfoPath: '/.well-known/service-info',
      },
    },
    openapi: {
      openapi: '3.1.0',
      'x-service-info': {
        protocol: 'mpp',
        transport: {
          type: 'http',
        },
      },
      paths: {
        '/payable': {
          post: {
            operationId: 'http_post_payable',
            summary: 'Payable route',
            'x-payment-info': {
              protocol: 'mpp',
              intent: 'charge',
              amount: {
                asset: 'BTC',
                network: 'bitcoin',
              },
            },
            'x-stateset-plugin-id': 'payments',
          },
        },
        '/free': {
          get: {
            operationId: 'http_get_free',
            summary: 'Free route',
          },
        },
      },
    },
  };
}

describe('mpp HTTP agent', () => {
  it('extracts payment challenges and receipts from HTTP responses', async () => {
    const challenge = createPaymentChallenge({
      toolName: 'POST /payable',
      pricing: {
        chainId: 'bitcoin',
        tokenSymbol: 'BTC',
        amount: 0.0001,
        amountSmallest: '10000',
        token: { symbol: 'BTC', decimals: 8, address: null },
      },
      requestId: 'req-http-agent',
      sessionId: 'sess-http-agent',
    });
    const receipt = createPaymentReceipt({
      challenge,
      credential: {
        credentialId: 'cred-1',
        payer: 'buyer-agent',
      },
      toolName: 'POST /payable',
    });

    const challengeResponse = createResponse({
      status: 402,
      headers: {
        'payment-required': encodeHeaderPayload({ challenge }),
      },
      body: {
        paymentChallenge: challenge,
      },
    });
    const receiptResponse = createResponse({
      status: 200,
      headers: {
        'payment-response': encodeHeaderPayload({ receipt }),
      },
      body: {
        ok: true,
        _meta: {
          payment: {
            receipt,
          },
        },
      },
    });

    assert.equal((await extractHttpPaymentChallenge(challengeResponse)).challengeId, challenge.challengeId);
    assert.equal((await extractHttpPaymentReceipt(receiptResponse)).receiptId, receipt.receiptId);
  });

  it('retries a payable HTTP request with an encoded payment credential', async () => {
    const challenge = createPaymentChallenge({
      toolName: 'POST /payable',
      pricing: {
        chainId: 'bitcoin',
        tokenSymbol: 'BTC',
        amount: 0.0001,
        amountSmallest: '10000',
        token: { symbol: 'BTC', decimals: 8, address: null },
      },
      requestId: 'req-retry',
      sessionId: 'sess-retry',
      params: {
        method: 'POST',
        pathname: '/payable',
        params: {},
        query: {},
        body: { sku: 'sku_1' },
      },
    });

    let callCount = 0;
    const fetch = async (_url, options = {}) => {
      callCount += 1;
      if (callCount === 1) {
        assert.equal(options.headers['content-type'], 'application/json');
        return createResponse({
          status: 402,
          headers: {
            'payment-required': encodeHeaderPayload({ challenge }),
          },
          body: {
            paymentChallenge: challenge,
          },
        });
      }

      const encodedCredential = options.headers.payment;
      assert.equal(typeof encodedCredential, 'string');
      const credential = JSON.parse(Buffer.from(encodedCredential, 'base64url').toString('utf8'));
      assert.equal(credential.challengeId, challenge.challengeId);
      assert.equal(credential.payer, 'buyer-agent');

      const receipt = createPaymentReceipt({
        challenge,
        credential,
        toolName: 'POST /payable',
      });
      return createResponse({
        status: 200,
        headers: {
          'payment-response': encodeHeaderPayload({ receipt }),
        },
        body: {
          ok: true,
        },
      });
    };

    const response = await mppFetch(
      'https://merchant.example/payable',
      {
        method: 'POST',
        headers: {
          'content-type': 'application/json',
        },
        body: {
          sku: 'sku_1',
        },
      },
      {
        fetch,
        payer: 'buyer-agent',
        acceptedMethods: ['bitcoin'],
        maxAmountSmallest: '10000',
        requireReceipt: true,
      },
    );

    assert.equal(callCount, 2);
    assert.equal(response.status, 200);
    assert.equal(response.mpp.challenge.challengeId, challenge.challengeId);
    assert.equal(response.mpp.credential.payer, 'buyer-agent');
    assert.equal(response.mpp.receipt.tool, 'POST /payable');
  });

  it('rejects disallowed challenges before retrying', async () => {
    const challenge = createPaymentChallenge({
      toolName: 'POST /payable',
      pricing: {
        chainId: 'bitcoin',
        tokenSymbol: 'BTC',
        amount: 0.0001,
        amountSmallest: '10000',
        token: { symbol: 'BTC', decimals: 8, address: null },
      },
    });

    await assert.rejects(
      () =>
        mppFetch(
          'https://merchant.example/payable',
          {},
          {
            fetch: async () =>
              createResponse({
                status: 402,
                headers: {
                  'payment-required': encodeHeaderPayload({ challenge }),
                },
                body: {
                  paymentChallenge: challenge,
                },
              }),
            acceptedMethods: ['zcash'],
          },
        ),
      (error) => {
        assert.ok(error instanceof MppPaymentPolicyError);
        assert.equal(error.challenge.challengeId, challenge.challengeId);
        return true;
      },
    );
  });

  it('extracts payable routes from an HTTP OpenAPI discovery document', () => {
    const { openapi } = createMockDiscovery();
    const routes = extractPayableHttpRoutes(openapi, {
      asset: 'BTC',
      method: 'POST',
    });

    assert.equal(routes.length, 1);
    assert.equal(routes[0].path, '/payable');
    assert.equal(routes[0].method, 'POST');
    assert.equal(routes[0].paymentInfo.amount.network, 'bitcoin');
    assert.equal(routes[0].pluginId, 'payments');
  });

  it('fetches service info and discovery documents for HTTP payment services', async () => {
    const { serviceInfo, openapi } = createMockDiscovery();
    const calls = [];
    const fetch = async (url) => {
      calls.push(url);
      if (String(url).endsWith('/.well-known/service-info')) {
        return createResponse({
          status: 200,
          body: serviceInfo,
        });
      }
      if (String(url).endsWith('/openapi.json')) {
        return createResponse({
          status: 200,
          body: openapi,
        });
      }
      throw new Error(`Unexpected URL ${url}`);
    };

    const service = await fetchMppServiceInfo('https://merchant.example', {
      fetch,
    });
    const discoveryDocument = await fetchMppDiscoveryDocument('https://merchant.example', {
      fetch,
    });
    const discovery = await discoverMppHttpService('https://merchant.example', {
      fetch,
      asset: 'BTC',
    });

    assert.equal(service.serviceInfo.protocol, 'mpp');
    assert.equal(discoveryDocument.document.openapi, '3.1.0');
    assert.equal(discovery.serviceInfo.transport.type, 'http');
    assert.equal(discovery.payableRoutes.length, 1);
    assert.equal(discovery.payableRoutes[0].path, '/payable');
    assert.equal(calls.length, 4);
  });

  it('createMppHttpAgent delegates to mppFetch', async () => {
    const agent = createMppHttpAgent({
      fetch: async () =>
        createResponse({
          status: 200,
          body: { ok: true },
        }),
      validateUrl: false,
    });

    const response = await agent.fetch('http://127.0.0.1/ignored');
    assert.equal(response.status, 200);
    assert.equal((await response.json()).ok, true);
  });

  it('createMppHttpAgent exposes service discovery helpers', async () => {
    const { serviceInfo, openapi } = createMockDiscovery();
    const fetch = async (url) => {
      if (String(url).endsWith('/.well-known/service-info')) {
        return createResponse({
          status: 200,
          body: serviceInfo,
        });
      }
      return createResponse({
        status: 200,
        body: openapi,
      });
    };
    const agent = createMppHttpAgent({
      fetch,
      validateUrl: false,
    });

    const service = await agent.getServiceInfo('http://127.0.0.1:8080');
    const discovery = await agent.getDiscovery('http://127.0.0.1:8080');
    const routes = await agent.discoverPayableRoutes('http://127.0.0.1:8080', {
      method: 'POST',
    });

    assert.equal(service.serviceInfo.protocol, 'mpp');
    assert.equal(discovery.payableRoutes.length, 1);
    assert.equal(routes.length, 1);
    assert.equal(routes[0].path, '/payable');
  });
});
