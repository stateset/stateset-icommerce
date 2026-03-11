import assert from 'node:assert/strict';
import { beforeEach, describe, it } from 'node:test';
import { createEmbeddedAgentToolkit } from '../../src/agent-toolkit.js';

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
});
