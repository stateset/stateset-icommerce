import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  MPP_JSONRPC_PAYMENT_REQUIRED_CODE,
  attachPaymentMetadata,
  buildMppServiceInfo,
  buildPaymentInfoFromPricing,
  executeMppToolWithPayment,
  createPaymentChallenge,
  createPaymentCredential,
  createPaymentDiscoveryDocument,
  createPaymentReceipt,
  extractPaymentChallenge,
  validatePaymentChallenge,
  verifyPaymentCredential,
} from '../../src/mpp/index.js';
import {
  attachPaymentReceiptToHttpResponse,
  buildHttpRouteDiscoveryDocument,
  buildHttpPaymentHeaders,
  createMppHttpRouteHandler,
  extractHttpPaymentCredential,
} from '../../src/mpp/http.js';

describe('mpp helpers', () => {
  it('creates deterministic challenges for the same bound request', () => {
    const pricing = {
      chainId: 'bitcoin',
      tokenSymbol: 'BTC',
      amount: 0.0001,
      amountSmallest: '10000',
      token: { symbol: 'BTC', decimals: 8, address: null },
    };

    const a = createPaymentChallenge({
      toolName: 'list_customers',
      pricing,
      requestId: 'req-1',
      sessionId: 'sess-1',
      params: { limit: 5 },
    });
    const b = createPaymentChallenge({
      toolName: 'list_customers',
      pricing,
      requestId: 'req-1',
      sessionId: 'sess-1',
      params: { limit: 5 },
    });

    assert.equal(a.challengeId, b.challengeId);
    assert.equal(a.paymentMethods[0].kind, 'bitcoin');
  });

  it('verifies credentials against the matching challenge', () => {
    const challenge = createPaymentChallenge({
      toolName: 'list_customers',
      pricing: {
        chainId: 'zcash',
        tokenSymbol: 'ZEC',
        amount: 0.5,
        amountSmallest: '50000000',
        token: { symbol: 'ZEC', decimals: 8, address: null },
      },
      requestId: 'req-zec',
      sessionId: 'sess-zec',
    });
    const credential = createPaymentCredential({
      challenge,
      payer: 'agent-z',
      authorization: { type: 'test' },
    });
    const verified = verifyPaymentCredential(credential, challenge);

    assert.equal(verified.valid, true);

    const receipt = createPaymentReceipt({
      challenge,
      credential,
      charge: {
        charged: true,
        rule: { chainId: 'zcash', tokenSymbol: 'ZEC', amount: 0.5 },
      },
      toolName: 'list_customers',
    });
    const payload = attachPaymentMetadata({ success: true }, { receipt });

    assert.equal(payload._meta.payment.receipt.challengeId, challenge.challengeId);
  });

  it('builds discovery metadata for priced MCP tools', () => {
    const serviceInfo = buildMppServiceInfo();
    const paymentInfo = buildPaymentInfoFromPricing({
      toolName: 'list_customers',
      description: 'List customers',
      pricing: {
        chainId: 'bitcoin',
        tokenSymbol: 'BTC',
        amount: 0.0001,
        amountSmallest: '10000',
        token: { symbol: 'BTC', decimals: 8, address: null },
      },
    });
    const document = createPaymentDiscoveryDocument({
      serviceInfo,
      tools: [
        {
          name: 'list_customers',
          description: 'List customers',
          inputSchema: { type: 'object', additionalProperties: false },
          runtime: { policyDomain: 'customers' },
          paymentInfo,
        },
      ],
    });

    assert.equal(document['x-service-info'].protocol, 'mpp');
    assert.equal(
      document.paths['/mcp/tools/list_customers'].post['x-payment-info'].jsonrpc
        .paymentRequiredCode,
      MPP_JSONRPC_PAYMENT_REQUIRED_CODE,
    );
  });

  it('builds OpenAPI discovery metadata for payable HTTP routes', () => {
    const document = buildHttpRouteDiscoveryDocument({
      serviceInfo: buildMppServiceInfo({
        serviceId: 'stateset-http-gateway',
        serviceName: 'StateSet HTTP Gateway',
        serverName: 'stateset-http-gateway',
        serverUrl: 'https://gateway.example',
        transportType: 'http',
      }),
      serverUrl: 'https://gateway.example',
      routes: [
        {
          method: 'POST',
          path: '/payable',
          pluginId: 'payments',
          handler: createMppHttpRouteHandler({
            routeId: 'POST /payable',
            description: 'Payable HTTP endpoint',
            pricing: {
              chainId: 'bitcoin',
              tokenSymbol: 'BTC',
              amount: 0.0001,
              amountSmallest: '10000',
              token: { symbol: 'BTC', decimals: 8, address: null },
            },
            handler: async () => ({ ok: true }),
          }),
        },
      ],
    });

    assert.equal(document['x-service-info'].transport.type, 'http');
    assert.equal(document.paths['/payable'].post['x-payment-info'].amount.asset, 'BTC');
    assert.equal(document.paths['/payable'].post['x-stateset-plugin-id'], 'payments');
  });

  it('extracts and validates payment challenges from tool results', () => {
    const challenge = createPaymentChallenge({
      toolName: 'list_customers',
      pricing: {
        chainId: 'bitcoin',
        tokenSymbol: 'BTC',
        amount: 0.0001,
        amountSmallest: '10000',
        token: { symbol: 'BTC', decimals: 8, address: null },
      },
      requestId: 'req-extract',
      sessionId: 'sess-extract',
    });

    const extracted = extractPaymentChallenge({
      status: 'payment_required',
      charge: {
        challenge,
      },
    });
    const validation = validatePaymentChallenge(extracted, {
      acceptedMethods: ['bitcoin'],
      acceptedAssets: ['BTC'],
      acceptedNetworks: ['bitcoin'],
      maxAmountSmallest: '10000',
    });

    assert.equal(extracted.challengeId, challenge.challengeId);
    assert.equal(validation.valid, true);
  });

  it('retries tool execution automatically with a generated credential', async () => {
    const challenge = createPaymentChallenge({
      toolName: 'list_customers',
      pricing: {
        chainId: 'bitcoin',
        tokenSymbol: 'BTC',
        amount: 0.0001,
        amountSmallest: '10000',
        token: { symbol: 'BTC', decimals: 8, address: null },
      },
      requestId: 'req-auto',
      sessionId: 'sess-auto',
      params: { limit: 1 },
    });
    const calls = [];

    const result = await executeMppToolWithPayment({
      toolName: 'list_customers',
      params: { limit: 1 },
      payment: {
        payer: 'buyer-agent',
        acceptedMethods: ['bitcoin'],
        maxAmountSmallest: '10000',
      },
      executor: async (_toolName, _params, executionOptions) => {
        calls.push(executionOptions);
        if (calls.length === 1) {
          return {
            status: 'payment_required',
            requestId: 'req-auto',
            sessionId: 'sess-auto',
            charge: {
              challenge,
            },
            result: {
              paymentChallenge: challenge,
              _meta: {
                payment: {
                  challenge,
                },
              },
            },
          };
        }

        assert.equal(executionOptions.requestId, 'req-auto');
        assert.equal(executionOptions.sessionId, 'sess-auto');
        assert.equal(executionOptions.extra._meta.payment.challengeId, challenge.challengeId);
        assert.equal(executionOptions.extra._meta.payment.payer, 'buyer-agent');
        return {
          status: 'success',
          result: { ok: true },
        };
      },
    });

    assert.equal(calls.length, 2);
    assert.equal(result.status, 'success');
    assert.equal(result.result.ok, true);
  });

  it('extracts HTTP payment credentials from encoded headers', () => {
    const challenge = createPaymentChallenge({
      toolName: 'POST /payable',
      pricing: {
        chainId: 'bitcoin',
        tokenSymbol: 'BTC',
        amount: 0.0001,
        amountSmallest: '10000',
        token: { symbol: 'BTC', decimals: 8, address: null },
      },
      requestId: 'req-http-header',
      sessionId: 'sess-http-header',
    });
    const credential = createPaymentCredential({
      challenge,
      payer: 'buyer-agent',
      authorization: { type: 'header-test' },
    });
    const extracted = extractHttpPaymentCredential({
      headers: {
        payment: Buffer.from(JSON.stringify(credential), 'utf8').toString('base64url'),
      },
    });

    assert.equal(extracted.challengeId, challenge.challengeId);
    assert.equal(extracted.payer, 'buyer-agent');
  });

  it('builds HTTP payment responses with encoded headers and attached receipts', async () => {
    const route = createMppHttpRouteHandler({
      routeId: 'POST /payable',
      description: 'Payable HTTP endpoint',
      pricing: {
        chainId: 'bitcoin',
        tokenSymbol: 'BTC',
        amount: 0.0001,
        amountSmallest: '10000',
        token: { symbol: 'BTC', decimals: 8, address: null },
      },
      resolvePayer: ({ request }) => request.identity?.name || null,
      handler: async ({ payment }) => ({
        status: 200,
        body: {
          ok: true,
          payer: payment.payer,
        },
      }),
    });

    const challengeResponse = await route({
      method: 'POST',
      pathname: '/payable',
      body: { sku: 'abc' },
      query: {},
      params: {},
      headers: {},
      identity: { name: 'buyer-agent' },
    });
    assert.equal(challengeResponse.status, 402);
    assert.equal(typeof challengeResponse.headers['payment-required'], 'string');

    const challenge = challengeResponse.body.paymentChallenge;
    const credential = createPaymentCredential({
      challenge,
      payer: 'buyer-agent',
      authorization: { type: 'http-test' },
    });
    const successResponse = await route({
      method: 'POST',
      pathname: '/payable',
      body: {
        sku: 'abc',
      },
      query: {},
      params: {},
      headers: {
        payment: Buffer.from(JSON.stringify(credential), 'utf8').toString('base64url'),
      },
      identity: { name: 'buyer-agent' },
    });

    assert.equal(successResponse.status, 200);
    assert.equal(typeof successResponse.headers['payment-response'], 'string');
    assert.equal(successResponse.body._meta.payment.receipt.tool, 'POST /payable');
    assert.equal(successResponse.body._meta.payment.receipt.payer, 'buyer-agent');

    const receiptHeaders = buildHttpPaymentHeaders({
      receipt: successResponse.body._meta.payment.receipt,
      serviceInfo: buildMppServiceInfo(),
    });
    assert.equal(typeof receiptHeaders['payment-response'], 'string');

    const attached = attachPaymentReceiptToHttpResponse(
      { ok: true },
      {
        receipt: successResponse.body._meta.payment.receipt,
        credential,
        serviceInfo: buildMppServiceInfo(),
      },
    );
    assert.equal(attached.body._meta.payment.credentialId, credential.credentialId);
  });
});
