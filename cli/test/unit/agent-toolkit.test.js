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
        create: async (params) => ({
          id: 'cust_test_1',
          status: 'active',
          createdAt: '2026-04-08T00:00:00.000Z',
          ...params,
        }),
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

  function createMockAutonomousEngine(overrides = {}) {
    return {
      executeAgentRequest: async (agentName, taskDescription, context) => ({
        agentName,
        taskDescription,
        context,
        status: 'completed',
      }),
      ...overrides,
    };
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
    const anthropicTools = toolkit.getTools({ format: 'anthropic' });

    assert.ok(genericTools.length >= 100);
    assert.equal(genericTools[0].inputSchema.type, 'object');
    assert.ok(Array.isArray(genericTools[0].runtime.compensations));

    assert.ok(openAiTools.length >= 100);
    assert.equal(openAiTools[0].type, 'function');
    assert.equal(openAiTools[0].function.parameters.type, 'object');

    assert.ok(anthropicTools.length >= 100);
    assert.equal(typeof anthropicTools[0].name, 'string');
    assert.equal(anthropicTools[0].input_schema.type, 'object');
    assert.equal(anthropicTools[0].stateset.permission, genericTools[0].permission);
  });

  it('executes a direct tool call without MCP transport', async () => {
    const toolkit = createEmbeddedAgentToolkit({ commerce: mockCommerce });

    const result = await toolkit.executeTool('list_customers');

    assert.equal(result.success, true);
    assert.equal(result.status, 'success');
    assert.equal(result.tool, 'list_customers');
    assert.equal(result.result.count, 0);
  });

  it('delegates through the embedded toolkit when an autonomous engine is provided', async () => {
    let delegated = null;
    const toolkit = createEmbeddedAgentToolkit({
      commerce: mockCommerce,
      allowApply: true,
      capabilities: ['read:*', 'delegate_to_agent'],
      autonomousEngine: createMockAutonomousEngine({
        executeAgentRequest: async (agentName, taskDescription, context) => {
          delegated = { agentName, taskDescription, context };
          return {
            status: 'completed',
            summary: `Delegated ${taskDescription} to ${agentName}`,
          };
        },
      }),
    });

    const result = await toolkit.executeTool('delegate_to_agent', {
      agent_name: 'orders',
      task_description: 'Review pending orders over $500',
      context: { limit: 10 },
    });

    assert.equal(result.success, true);
    assert.equal(result.status, 'success');
    assert.equal(result.result.delegatedTo, 'orders');
    assert.equal(result.policy.domain, 'agentic');
    assert.equal(result.runtime.policyDomain, 'agentic');
    assert.deepEqual(delegated, {
      agentName: 'orders',
      taskDescription: 'Review pending orders over $500',
      context: { limit: 10 },
    });
  });

  it('keeps delegation in preview mode until apply is enabled', async () => {
    let delegated = false;
    const toolkit = createEmbeddedAgentToolkit({
      commerce: mockCommerce,
      allowApply: false,
      autonomousEngine: createMockAutonomousEngine({
        executeAgentRequest: async () => {
          delegated = true;
          return { status: 'completed' };
        },
      }),
    });

    const result = await toolkit.executeTool('delegate_to_agent', {
      agent_name: 'orders',
      task_description: 'Review pending orders over $500',
      context: { limit: 10 },
    });

    assert.equal(result.success, false);
    assert.equal(result.status, 'preview');
    assert.equal(result.preview, true);
    assert.equal(result.tool, 'delegate_to_agent');
    assert.equal(result.policy.domain, 'agentic');
    assert.equal(result.runtime.policyDomain, 'agentic');
    assert.match(result.error, /Preview mode: would execute 'delegate_to_agent'/);
    assert.deepEqual(result.wouldDo, {
      tool: 'delegate_to_agent',
      params: {
        agent_name: 'orders',
        task_description: 'Review pending orders over $500',
        context: { limit: 10 },
      },
    });
    assert.equal(delegated, false);
  });

  it('adapts prototype getter-based commerce APIs for tool execution', async () => {
    class GetterCommerce {
      get customers() {
        return {
          list: async () => [
            {
              id: 'cust_demo_1',
              email: 'buyer@example.com',
              firstName: 'Buyer',
              lastName: 'Agent',
              status: 'active',
              acceptsMarketing: false,
              createdAt: '2026-04-02T00:00:00.000Z',
            },
          ],
          count: async () => 1,
          get: async () => null,
        };
      }

      get x402() {
        return {
          getNextNonce: async () => 42,
        };
      }
    }

    const toolkit = createEmbeddedAgentToolkit({
      commerce: new GetterCommerce(),
      allowApply: true,
      capabilities: ['*'],
    });

    const customers = await toolkit.executeTool('list_customers');
    const nonce = await toolkit.executeTool('x402_get_next_nonce', {
      payerAddress: '0x1234567890abcdef1234567890abcdef12345678',
    });

    assert.equal(customers.success, true);
    assert.equal(customers.result.count, 1);
    assert.equal(customers.result.customers[0].email, 'buyer@example.com');
    assert.equal(nonce.success, true);
    assert.equal(nonce.result.nextNonce, 42);
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

  it('returns individual tool descriptors through helper accessors', () => {
    const toolkit = createEmbeddedAgentToolkit({ commerce: mockCommerce });

    const openAiTool = toolkit.getTool('mcp__stateset-commerce__list_customers', {
      format: 'openai',
    });
    const rawTool = toolkit.getRawTool('mcp__stateset-commerce__list_customers');

    assert.equal(openAiTool.type, 'function');
    assert.equal(openAiTool.function.name, 'list_customers');
    assert.equal(rawTool.name, 'list_customers');
    assert.equal(rawTool.permission, 'read');
  });

  it('normalizes prefixed tool names for runtime contracts and plan helpers', async () => {
    const toolkit = createEmbeddedAgentToolkit({
      commerce: mockCommerce,
      allowApply: true,
      capabilities: ['*'],
    });

    const contract = await toolkit.getRuntimeContract({
      tool: 'mcp__stateset-commerce__create_customer',
    });
    const simulation = await toolkit.simulatePlan({
      steps: [
        {
          tool: 'mcp__stateset-commerce__create_customer',
          params: {
            email: 'plan@example.com',
            firstName: 'Plan',
            lastName: 'User',
          },
        },
      ],
    });
    const execution = await toolkit.executePlan({
      dryRun: true,
      steps: [
        {
          tool: 'mcp__stateset-commerce__create_customer',
          params: {
            email: 'plan@example.com',
            firstName: 'Plan',
            lastName: 'User',
          },
        },
      ],
    });

    assert.equal(contract.totalTools, 1);
    assert.deepEqual(
      contract.tools.map((tool) => tool.name),
      ['create_customer'],
    );
    assert.equal(simulation.outcomes[0].tool, 'create_customer');
    assert.equal(simulation.outcomes[0].status, 'success');
    assert.equal(execution.steps[0].tool, 'create_customer');
    assert.equal(execution.steps[0].status, 'dry_run_success');
    assert.equal(execution.finalStatus, 'dry_run');
  });

  it('normalizes prefixed tool names for replay helpers', async () => {
    const tempDir = await mkdtemp(join(os.tmpdir(), 'stateset-agent-toolkit-replay-'));
    const dbPath = join(tempDir, 'store.db');

    try {
      const toolkit = createEmbeddedAgentToolkit({
        commerce: mockCommerce,
        dbPath,
        allowApply: true,
        capabilities: ['create_customer'],
      });

      const execution = await toolkit.executeTool('create_customer', {
        email: 'replay@example.com',
        firstName: 'Replay',
        lastName: 'User',
      });
      const replay = await toolkit.replayMutation({
        tool: 'mcp__stateset-commerce__create_customer',
        requestId: execution.requestId,
        dryRun: true,
      });
      const replayLog = await toolkit.getReplayLog({
        tool: 'mcp__stateset-commerce__create_customer',
        requestId: execution.requestId,
      });

      assert.equal(execution.status, 'success');
      assert.equal(replay.success, true);
      assert.equal(replay.sourceEvent.tool, 'create_customer');
      assert.equal(replay.replay.status, 'dry_run_success');
      assert.equal(replayLog.count, 1);
      assert.equal(replayLog.filters.tool, 'create_customer');
      assert.equal(replayLog.events[0].tool, 'create_customer');
    } finally {
      await rm(tempDir, { recursive: true, force: true });
    }
  });

  it('routes governed mutations through the native kernel receipt boundary', async () => {
    let captured = null;
    mockCommerce.executeKernelCommand = async (command, policy) => {
      captured = { command, policy };
      return {
        status: 'succeeded',
        receipt_id: 'receipt-1',
        command_type: command.command_type,
        result: { id: 'payment-1', amount: command.payload.amount },
      };
    };
    const policy = {
      version: 'agent-policy-1',
      commands: {
        'payments.create': {
          required_capabilities: ['payments.create'],
          requires_tenant: true,
          requires_store: true,
          requires_agent_delegation: true,
          requires_signed_authority: false,
          requires_approval: false,
        },
      },
      trusted_authority_keys: {},
    };
    const toolkit = createEmbeddedAgentToolkit({
      commerce: mockCommerce,
      allowApply: true,
      capabilities: ['payments.create'],
      kernel: {
        policy,
        storeId: 'store-1',
        principal: {
          id: 'agent-1',
          kind: 'agent',
          tenantId: 'tenant-1',
          delegatedBy: 'user-1',
          capabilities: ['payments.create'],
        },
      },
    });

    const result = await toolkit.executeTool(
      'create_payment',
      { orderId: 'order-1', amount: '12.34', currency: 'USD' },
      { idempotencyKey: 'payment-attempt-1' },
    );

    assert.equal(result.result.kernel, true);
    assert.equal(result.result.receipt.receipt_id, 'receipt-1');
    assert.equal(captured.policy, policy);
    assert.equal(captured.command.command_type, 'payments.create');
    assert.equal(captured.command.mode, 'apply');
    assert.equal(captured.command.idempotency_key, 'payment-attempt-1');
    assert.equal(captured.command.principal.tenant_id, 'tenant-1');
    assert.equal(captured.command.store_id, 'store-1');
    assert.equal(captured.command.payload.amount, '12.34');
  });

  it('routes product creation with exact-decimal variants through the kernel', async () => {
    const captured = [];
    mockCommerce.executeKernelCommand = async (command) => {
      captured.push(command);
      return {
        status: 'succeeded',
        receipt_id: 'product-receipt-1',
        result: { id: 'product-1', slug: 'agent-offer' },
      };
    };
    const rule = {
      required_capabilities: ['products.create'],
      requires_tenant: true,
      requires_store: true,
      requires_agent_delegation: true,
      requires_signed_authority: false,
      requires_approval: false,
    };
    const toolkit = createEmbeddedAgentToolkit({
      commerce: mockCommerce,
      allowApply: true,
      capabilities: ['products.create'],
      kernel: {
        strict: true,
        policy: {
          version: 'catalog-policy-1',
          commands: { 'products.create': rule },
          trusted_authority_keys: {},
        },
        storeId: 'store-1',
        principal: {
          id: 'agent-1',
          kind: 'agent',
          tenantId: 'tenant-1',
          delegatedBy: 'user-1',
          capabilities: ['products.create'],
        },
      },
    });

    const result = await toolkit.executeTool('create_product', {
      name: 'Agent Offer',
      variants: [
        {
          sku: 'AGENT-001',
          price: '9007199254740993.25',
          compareAtPrice: '9007199254740994.25',
        },
      ],
    });
    assert.equal(result.result.kernel, true);
    assert.equal(captured[0].command_type, 'products.create');
    assert.equal(captured[0].payload.variants[0].price, '9007199254740993.25');
    assert.equal(captured[0].payload.variants[0].compare_at_price, '9007199254740994.25');

    const unsafe = await toolkit.executeTool('create_product', {
      name: 'Unsafe Offer',
      variants: [{ sku: 'UNSAFE-001', price: 12.34 }],
    });
    assert.equal(unsafe.success, false);
    assert.match(unsafe.error, /exact decimal strings/);
    assert.equal(captured.length, 1);
  });

  it('routes exact-decimal initial inventory through the kernel', async () => {
    const captured = [];
    mockCommerce.executeKernelCommand = async (command) => {
      captured.push(command);
      return {
        status: 'succeeded',
        receipt_id: 'inventory-receipt-1',
        result: { id: 1, sku: command.payload.sku },
      };
    };
    const capability = 'inventory.item.create';
    const toolkit = createEmbeddedAgentToolkit({
      commerce: mockCommerce,
      allowApply: true,
      capabilities: [capability],
      kernel: {
        strict: true,
        policy: {
          version: 'inventory-policy-1',
          commands: {
            [capability]: {
              required_capabilities: [capability],
              requires_tenant: true,
              requires_store: true,
              requires_agent_delegation: true,
              requires_signed_authority: false,
              requires_approval: false,
            },
          },
          trusted_authority_keys: {},
        },
        storeId: 'store-1',
        principal: {
          id: 'agent-1',
          kind: 'agent',
          tenantId: 'tenant-1',
          delegatedBy: 'user-1',
          capabilities: [capability],
        },
      },
    });

    const result = await toolkit.executeTool('create_inventory_item', {
      sku: 'FRACTIONAL-001',
      name: 'Fractional inventory',
      initialQuantity: '9007199254740993.125',
      reorderPoint: '0.125',
      safetyStock: '0.025',
    });
    assert.equal(result.result.kernel, true);
    assert.equal(captured[0].command_type, capability);
    assert.equal(captured[0].payload.initial_quantity, '9007199254740993.125');
    assert.equal(captured[0].payload.reorder_point, '0.125');

    const unsafe = await toolkit.executeTool('create_inventory_item', {
      sku: 'UNSAFE-INVENTORY',
      name: 'Unsafe inventory',
      initialQuantity: 12.5,
    });
    assert.equal(unsafe.success, false);
    assert.match(unsafe.error, /exact decimal strings/);
    assert.equal(captured.length, 1);
  });

  it('routes the exact-decimal A2A escrow lifecycle through strict kernel commands', async () => {
    const captured = [];
    mockCommerce.executeKernelCommand = async (command) => {
      captured.push(command);
      return {
        status: 'succeeded',
        receipt_id: `receipt-${captured.length}`,
        command_type: command.command_type,
        result: { id: command.payload.escrow_id || 'escrow-1', status: 'active' },
      };
    };
    const capabilities = [
      'a2a.escrow.create',
      'a2a.escrow.dispute',
      'a2a.escrow.fund',
      'a2a.escrow.refund',
      'a2a.escrow.release',
      'a2a.dispute.file',
      'a2a.dispute.evidence.submit',
      'a2a.dispute.resolve',
    ];
    const commands = Object.fromEntries(
      capabilities.map((capability) => [
        capability,
        {
          required_capabilities: [capability],
          requires_tenant: true,
          requires_store: true,
          requires_agent_delegation: true,
          requires_signed_authority: false,
          requires_approval: false,
        },
      ]),
    );
    const toolkit = createEmbeddedAgentToolkit({
      commerce: mockCommerce,
      allowApply: true,
      capabilities: ['permission:write', ...capabilities],
      agentConfig: { walletAddress: 'did:key:buyer' },
      kernel: {
        strict: true,
        policy: { version: 'a2a-policy-1', commands, trusted_authority_keys: {} },
        storeId: 'store-a2a',
        principal: {
          id: 'agent-a2a',
          kind: 'agent',
          tenantId: 'tenant-a2a',
          delegatedBy: 'user-a2a',
          capabilities,
        },
      },
    });

    await toolkit.executeTool('a2a_create_escrow', {
      buyerAddress: 'did:key:buyer',
      sellerAddress: 'did:key:seller',
      amount: '0.123456',
      conditions: [{ type: 'buyer_confirmed' }],
      expiresInHours: 24,
    });
    await toolkit.executeTool('a2a_fund_escrow', { escrowId: 'escrow-1' });
    await toolkit.executeTool('a2a_dispute_escrow', {
      escrowId: 'escrow-1',
      reason: 'delivery evidence missing',
      category: 'non_delivery',
    });
    await toolkit.executeTool('a2a_refund_escrow', {
      escrowId: 'escrow-1',
      reason: 'buyer cancelled',
    });
    await toolkit.executeTool('a2a_file_dispute', {
      escrowId: 'escrow-2',
      reason: 'delivery evidence missing',
      category: 'non_delivery',
    });
    await toolkit.executeTool('a2a_submit_evidence', {
      disputeId: 'dispute-1',
      evidenceType: 'communication',
      title: 'Seller conversation',
      content: 'seller acknowledged non-delivery',
    });
    await toolkit.executeTool('a2a_resolve_dispute', {
      disputeId: 'dispute-1',
      resolutionType: 'split',
      buyerAmount: '0.100001',
      sellerAmount: '0.023455',
    });

    assert.deepEqual(
      captured.map((command) => command.command_type),
      [
        'a2a.escrow.create',
        'a2a.escrow.fund',
        'a2a.escrow.dispute',
        'a2a.escrow.refund',
        'a2a.dispute.file',
        'a2a.dispute.evidence.submit',
        'a2a.dispute.resolve',
      ],
    );
    assert.equal(captured[0].payload.amount, '0.123456');
    assert.equal(captured[0].payload.release_conditions[0].completed, false);
    assert.equal(captured[1].payload.escrow_id, 'escrow-1');
    assert.equal(captured[2].payload.reason, 'delivery evidence missing');
    assert.equal(captured[2].payload.category, 'non_delivery');
    assert.equal(captured[3].payload.reason, 'buyer cancelled');
    assert.equal(captured[4].payload.claimant_address, 'did:key:buyer');
    assert.equal(captured[5].payload.submitted_by, 'did:key:buyer');
    assert.equal(captured[6].payload.buyer_amount, '0.100001');
    assert.equal(captured[6].payload.seller_amount, '0.023455');

    const unsafeNumber = await toolkit.executeTool('a2a_create_escrow', {
      buyerAddress: 'did:key:buyer',
      sellerAddress: 'did:key:seller',
      amount: 0.123456,
    });
    assert.equal(unsafeNumber.success, false);
    assert.match(unsafeNumber.error, /exact decimal string/);
    assert.equal(captured.length, 7);
  });

  it('refuses governed apply mutations without trusted kernel configuration', async () => {
    const toolkit = createEmbeddedAgentToolkit({
      commerce: mockCommerce,
      allowApply: true,
      capabilities: ['payments.create'],
    });

    const result = await toolkit.executeTool('create_payment', {
      orderId: 'order-1',
      amount: '12.34',
    });
    assert.equal(result.success, false);
    assert.match(result.error, /apply mode requires trusted kernel configuration/);
  });

  it('strict kernel mode hides and rejects every unmapped mutation tool', async () => {
    mockCommerce.executeKernelCommand = async (command) => ({
      status: 'succeeded',
      receipt_id: 'strict-receipt-1',
      command_type: command.command_type,
      result: { id: 'payment-1' },
    });
    const toolkit = createEmbeddedAgentToolkit({
      commerce: mockCommerce,
      allowApply: true,
      capabilities: ['read:*', 'permission:write', 'permission:delete', 'permission:admin'],
      kernel: {
        policy: {
          version: 'strict-policy-1',
          commands: {
            'payments.create': {
              required_capabilities: ['payments.create'],
              requires_tenant: true,
              requires_store: true,
              requires_agent_delegation: true,
              requires_signed_authority: false,
              requires_approval: false,
            },
          },
          trusted_authority_keys: {},
        },
        storeId: 'store-1',
        principal: {
          id: 'agent-1',
          kind: 'agent',
          tenantId: 'tenant-1',
          delegatedBy: 'user-1',
          capabilities: ['payments.create'],
        },
      },
    });

    const names = toolkit.getRawTools().map((tool) => tool.name);
    assert.ok(names.includes('list_customers'));
    assert.ok(names.includes('create_payment'));
    assert.ok(!names.includes('create_customer'));
    assert.ok(!names.includes('delete_customer'));
    assert.ok(!names.includes('backup_database'));
    assert.ok(!names.includes('set_exchange_rate'));
    await assert.rejects(
      toolkit.executeTool('create_customer', { email: 'blocked@example.com' }),
      /outside this toolkit's capability scope/,
    );
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

  it('supports payable catalog and payment discovery helpers for prefixed tool names', async () => {
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

      const catalog = await toolkit.getPayableToolCatalog({
        tool: 'mcp__stateset-commerce__list_customers',
      });
      const discovery = await toolkit.getPaymentDiscovery({
        tool: 'mcp__stateset-commerce__list_customers',
        pricedOnly: true,
      });

      assert.equal(catalog.count, 1);
      assert.equal(catalog.tools[0].toolName, 'list_customers');
      assert.equal(catalog.tools[0].paymentInfo.amount.asset, 'BTC');
      assert.equal(discovery.tools.length, 1);
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

      const result = await toolkit.executeToolWithPayment(
        'list_customers',
        {},
        {
          payment: {
            acceptedMethods: ['bitcoin'],
            maxAmountSmallest: '10000',
          },
        },
      );

      assert.equal(result.success, true);
      assert.equal(result.status, 'success');
      assert.equal(Array.isArray(result.result.customers), true);
      assert.equal(result.result._meta.payment.receipt.tool, 'list_customers');
      assert.equal(result.result._meta.payment.receipt.payer, 'buyer-agent');
    });
  });

  it('executes paid-tool helpers with the same payment flow as executeToolWithPayment', async () => {
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

      const result = await toolkit.executePaidTool(
        'list_customers',
        {},
        {
          payment: {
            acceptedMethods: ['bitcoin'],
            maxAmountSmallest: '10000',
          },
        },
      );

      assert.equal(result.success, true);
      assert.equal(result.status, 'success');
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

    const [descriptor] = await toolkit.createRemoteHttpToolDescriptors('https://merchant.example', {
      fetch,
      executionOptions: {
        http: {
          fetch,
          validateUrl: false,
        },
      },
    });

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
