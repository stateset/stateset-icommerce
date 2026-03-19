import assert from 'node:assert/strict';
import { beforeEach, describe, it } from 'node:test';
import os from 'node:os';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { createEmbeddedAgentToolkit } from '../../src/agent-toolkit.js';
import { createPaymentChallenge, createPaymentReceipt } from '../../src/mpp/index.js';
import { loadTreasuryContext, recordDeposit } from '../../src/treasury/index.js';

describe('agent-toolkit', () => {
  let mockCommerce;

  beforeEach(() => {
    mockCommerce = {
      customers: {
        list: async () => [],
        count: async () => 0,
        get: async () => null,
      },
      orders: {
        list: async () => [],
        count: async () => 0,
        get: async () => null,
      },
      products: {
        list: async () => [],
        count: async () => 0,
        get: async () => null,
      },
      inventory: {
        getStock: async () => null,
      },
    };
  });

  async function withTempPricing(callback) {
    const tempDir = await mkdtemp(join(os.tmpdir(), 'stateset-agent-toolkit-mpp-'));
    const pricingPath = join(tempDir, 'pricing.json');
    const dbPath = join(tempDir, 'treasury.db');

    await writeFile(
      pricingPath,
      JSON.stringify(
        {
          rules: [
            {
              tool: 'list_customers',
              chainId: 'bitcoin',
              tokenSymbol: 'BTC',
              amount: 0.0001,
            },
          ],
        },
        null,
        2,
      ),
    );

    try {
      return await callback({ pricingPath, dbPath });
    } finally {
      await rm(tempDir, { recursive: true, force: true });
    }
  }

  async function seedTreasuryBalance({ dbPath, pricingPath, agentId = 'buyer-agent', amount = 1 }) {
    const ctx = await loadTreasuryContext({ dbPath, pricingPath });
    await recordDeposit(
      {
        agentId,
        chainId: 'bitcoin',
        tokenSymbol: 'BTC',
        amount,
        source: 'test_seed',
      },
      ctx,
    );
  }

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

  function createMockRemoteDiscovery() {
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

  it('returns JSON-schema tool definitions for generic and OpenAI formats', () => {
    const toolkit = createEmbeddedAgentToolkit({ commerce: mockCommerce });

    const genericTools = toolkit.getTools();
    const openAiTools = toolkit.getTools({ format: 'openai' });

    assert.ok(genericTools.length >= 100);
    assert.equal(genericTools[0].inputSchema.type, 'object');
    assert.ok(Array.isArray(genericTools[0].runtime.compensations));

    assert.ok(openAiTools.length >= 100);
    assert.equal(openAiTools[0].type, 'function');
    assert.equal(openAiTools[0].function.parameters.type, 'object');
  });

  it('executes a direct tool call without MCP transport', async () => {
    const toolkit = createEmbeddedAgentToolkit({ commerce: mockCommerce });

    const result = await toolkit.executeTool('list_customers');

    assert.equal(result.success, true);
    assert.equal(result.status, 'success');
    assert.equal(result.tool, 'list_customers');
    assert.equal(result.result.count, 0);
  });

  it('normalizes OpenAI tool calls and returns a function_call_output payload', async () => {
    const toolkit = createEmbeddedAgentToolkit({ commerce: mockCommerce });

    const execution = await toolkit.executeOpenAIToolCall({
      call_id: 'call_123',
      function: {
        name: 'list_customers',
        arguments: '{}',
      },
    });

    assert.equal(execution.name, 'list_customers');
    assert.equal(execution.callId, 'call_123');
    assert.equal(execution.outputMessage.type, 'function_call_output');

    const payload = JSON.parse(execution.outputMessage.output);
    assert.equal(payload.status, 'success');
    assert.equal(payload.tool, 'list_customers');
  });

  it('creates Vercel AI tools with executable handlers', async () => {
    const toolkit = createEmbeddedAgentToolkit({ commerce: mockCommerce });

    const tools = toolkit.createVercelAITools({
      tool: (definition) => definition,
      filter: ['list_customers'],
    });

    assert.deepEqual(Object.keys(tools), ['list_customers']);
    assert.equal(typeof tools.list_customers.execute, 'function');

    const result = await tools.list_customers.execute({});
    assert.equal(result.status, 'success');
    assert.equal(result.tool, 'list_customers');
    assert.equal(typeof tools.list_customers.parameters.safeParse, 'function');
  });

  it('creates LangChain-compatible DynamicStructuredTool instances', async () => {
    const toolkit = createEmbeddedAgentToolkit({ commerce: mockCommerce });

    class DynamicStructuredTool {
      constructor(config) {
        Object.assign(this, config);
      }
    }

    const tools = toolkit.createLangChainTools({
      DynamicStructuredTool,
      filter: ['list_customers'],
    });

    assert.equal(tools.length, 1);
    assert.equal(tools[0].name, 'list_customers');
    assert.equal(typeof tools[0].func, 'function');
    assert.equal(typeof tools[0].schema.safeParse, 'function');

    const result = JSON.parse(await tools[0].func({}));
    assert.equal(result.status, 'success');
    assert.equal(result.tool, 'list_customers');
  });

  it('executes batches of OpenAI and direct tool calls', async () => {
    const toolkit = createEmbeddedAgentToolkit({ commerce: mockCommerce });

    const results = await toolkit.executeToolCalls([
      {
        call_id: 'call_1',
        function: {
          name: 'list_customers',
          arguments: '{}',
        },
      },
      {
        id: 'call_2',
        name: 'list_orders',
        params: {},
      },
    ]);

    assert.equal(results.length, 2);
    assert.equal(results[0].outputMessage.type, 'function_call_output');
    assert.equal(results[0].result.tool, 'list_customers');
    assert.equal(results[1].result.tool, 'list_orders');
  });

  it('discovers payable tools through the embedded toolkit', async () => {
    await withTempPricing(async ({ pricingPath, dbPath }) => {
      const toolkit = createEmbeddedAgentToolkit({
        commerce: mockCommerce,
        treasury: {
          enabled: true,
          agentId: 'buyer-agent',
          dbPath,
          pricingPath,
        },
      });

      const discovery = await toolkit.discoverPayableTools();

      assert.equal(discovery.protocol, 'mpp');
      assert.equal(Array.isArray(discovery.tools), true);
      assert.equal(discovery.tools[0].name, 'list_customers');
      assert.equal(discovery.tools[0].paymentInfo.amount.asset, 'BTC');
    });
  });

  it('returns a detailed tool catalog through the embedded toolkit', async () => {
    await withTempPricing(async ({ pricingPath, dbPath }) => {
      const toolkit = createEmbeddedAgentToolkit({
        commerce: mockCommerce,
        treasury: {
          enabled: true,
          agentId: 'buyer-agent',
          dbPath,
          pricingPath,
        },
      });

      const catalog = await toolkit.getToolCatalog({
        tool: 'list_customers',
        payableOnly: true,
      });

      assert.equal(catalog.count, 1);
      assert.equal(catalog.tools[0].toolName, 'list_customers');
      assert.equal(catalog.tools[0].payable, true);
      assert.equal(catalog.tools[0].paymentInfo.amount.asset, 'BTC');
    });
  });

  it('prepares a bound tool payment through the embedded toolkit', async () => {
    await withTempPricing(async ({ pricingPath, dbPath }) => {
      const toolkit = createEmbeddedAgentToolkit({
        commerce: mockCommerce,
        treasury: {
          enabled: true,
          agentId: 'buyer-agent',
          dbPath,
          pricingPath,
        },
      });

      const prepared = await toolkit.prepareToolPayment({
        tool: 'list_customers',
        params: {},
        requestId: 'toolkit-req-1',
        sessionId: 'toolkit-sess-1',
      });

      assert.equal(prepared.success, true);
      assert.equal(prepared.payable, true);
      assert.equal(prepared.challenge.tool, 'list_customers');
      assert.equal(prepared.retryExample._meta.payment.challengeId, prepared.challenge.challengeId);
    });
  });

  it('executes priced tools with automatic MPP retries through the toolkit', async () => {
    await withTempPricing(async ({ pricingPath, dbPath }) => {
      await seedTreasuryBalance({ dbPath, pricingPath });
      const toolkit = createEmbeddedAgentToolkit({
        commerce: mockCommerce,
        treasury: {
          enabled: true,
          agentId: 'buyer-agent',
          dbPath,
          pricingPath,
        },
      });

      const result = await toolkit.executeToolWithPayment('list_customers', {}, {
        payment: {
          acceptedMethods: ['bitcoin'],
          maxAmountSmallest: '10000',
        },
      });

      assert.equal(result.success, true);
      assert.equal(result.status, 'success');
      assert.equal(Array.isArray(result.result.customers), true);
      assert.equal(result.result._meta.payment.receipt.tool, 'list_customers');
      assert.equal(result.result._meta.payment.receipt.payer, 'buyer-agent');
    });
  });

  it('auto-pays priced OpenAI tool calls when payment options are provided', async () => {
    await withTempPricing(async ({ pricingPath, dbPath }) => {
      await seedTreasuryBalance({ dbPath, pricingPath });
      const toolkit = createEmbeddedAgentToolkit({
        commerce: mockCommerce,
        treasury: {
          enabled: true,
          agentId: 'buyer-agent',
          dbPath,
          pricingPath,
        },
      });

      const execution = await toolkit.executePaidOpenAIToolCall(
        {
          call_id: 'call_paid_1',
          function: {
            name: 'list_customers',
            arguments: '{}',
          },
        },
        {
          payment: {
            acceptedMethods: ['bitcoin'],
            maxAmountSmallest: '10000',
          },
        },
      );

      assert.equal(execution.result.status, 'success');
      assert.equal(execution.result.result._meta.payment.receipt.tool, 'list_customers');
      assert.equal(execution.outputMessage.type, 'function_call_output');
    });
  });

  it('adds payment preparation to tool descriptors', async () => {
    await withTempPricing(async ({ pricingPath, dbPath }) => {
      await seedTreasuryBalance({ dbPath, pricingPath });
      const toolkit = createEmbeddedAgentToolkit({
        commerce: mockCommerce,
        treasury: {
          enabled: true,
          agentId: 'buyer-agent',
          dbPath,
          pricingPath,
        },
      });

      const [descriptor] = toolkit.createToolDescriptors({ filter: ['list_customers'] });
      const prepared = await descriptor.preparePayment({
        params: {},
        requestId: 'descriptor-req-1',
        sessionId: 'descriptor-sess-1',
      });
      const result = await descriptor.executeWithPayment(
        {},
        {
          acceptedMethods: ['bitcoin'],
          maxAmountSmallest: '10000',
        },
      );

      assert.equal(descriptor.name, 'list_customers');
      assert.equal(typeof descriptor.preparePayment, 'function');
      assert.equal(typeof descriptor.executeWithPayment, 'function');
      assert.equal(prepared.payable, true);
      assert.equal(prepared.challenge.tool, 'list_customers');
      assert.equal(result.status, 'success');
      assert.equal(result.result._meta.payment.receipt.tool, 'list_customers');
    });
  });

  it('discovers remote payable HTTP services through the embedded toolkit', async () => {
    const { serviceInfo, openapi } = createMockRemoteDiscovery();
    const fetch = async (url) => {
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

    const toolkit = createEmbeddedAgentToolkit({
      commerce: mockCommerce,
      mpp: {
        payer: 'buyer-agent',
      },
    });

    const discovery = await toolkit.discoverRemotePaymentService('https://merchant.example', {
      fetch,
    });
    const routes = await toolkit.discoverRemotePayableRoutes('https://merchant.example', {
      fetch,
      method: 'POST',
    });

    assert.equal(discovery.serviceInfo.transport.type, 'http');
    assert.equal(discovery.payableRoutes.length, 1);
    assert.equal(routes.length, 1);
    assert.equal(routes[0].path, '/payable');
    assert.equal(routes[0].paymentInfo.amount.asset, 'BTC');
  });

  it('creates remote HTTP descriptors with auto-paying execute helpers', async () => {
    const { serviceInfo, openapi } = createMockRemoteDiscovery();
    const challenge = createPaymentChallenge({
      toolName: 'POST /payable',
      pricing: {
        chainId: 'bitcoin',
        tokenSymbol: 'BTC',
        amount: 0.0001,
        amountSmallest: '10000',
        token: { symbol: 'BTC', decimals: 8, address: null },
      },
      requestId: 'remote-toolkit-req-1',
      sessionId: 'remote-toolkit-sess-1',
      params: {
        method: 'POST',
        pathname: '/payable',
        body: { sku: 'sku_remote_1' },
      },
    });

    let routeCallCount = 0;
    const fetch = async (url, options = {}) => {
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
      if (String(url) === 'https://merchant.example/payable') {
        routeCallCount += 1;
        if (routeCallCount === 1) {
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

        const credential = JSON.parse(
          Buffer.from(options.headers.payment, 'base64url').toString('utf8'),
        );
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
      }
      throw new Error(`Unexpected URL ${url}`);
    };

    const toolkit = createEmbeddedAgentToolkit({
      commerce: mockCommerce,
      mpp: {
        payer: 'buyer-agent',
      },
    });

    const [descriptor] = await toolkit.createRemoteHttpToolDescriptors(
      'https://merchant.example',
      {
        fetch,
        executionOptions: {
          http: {
            fetch,
            validateUrl: false,
          },
        },
      },
    );

    const response = await descriptor.executeWithPayment(
      {
        body: {
          sku: 'sku_remote_1',
        },
      },
      {
        acceptedMethods: ['bitcoin'],
        maxAmountSmallest: '10000',
      },
    );

    assert.equal(descriptor.name, 'http_post_payable');
    assert.equal(descriptor.payable, true);
    assert.equal(routeCallCount, 2);
    assert.equal(response.status, 200);
    assert.equal(response.mpp.challenge.challengeId, challenge.challengeId);
    assert.equal(response.mpp.credential.payer, 'buyer-agent');
    assert.equal(response.mpp.receipt.tool, 'POST /payable');
  });
});
