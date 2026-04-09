/**
 * Delegate-to-Agent Tool Test Suite
 *
 * Exercises the real delegate_to_agent MCP tool exported by mcp-server.js
 * so metadata, schema, and handler behavior stay aligned with production.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { SUPPORTED_AGENT_NAMES, SUPPORTED_AGENT_NAMES_DESCRIPTION } from '../../src/agent-catalog.js';
import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { createToolInputSchema } from '../../src/tool-schema.js';

function makeMockCommerce() {
  return {
    customers: {
      list: async () => [],
      count: async () => 0,
      get: async () => null,
      create: async (data) => ({ id: 'cust-1', ...data }),
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
}

function findTool(tools, name) {
  const tool = tools.find((entry) => entry.name === name);
  if (!tool) {
    throw new Error(`Tool '${name}' not found`);
  }
  return tool;
}

function makeMockEngine(overrides = {}) {
  return {
    executeAgentRequest: async (agentName, taskDesc, context) => ({
      agentName,
      taskDesc,
      context,
      status: 'completed',
      output: `Executed ${taskDesc} via ${agentName}`,
    }),
    ...overrides,
  };
}

describe('delegate_to_agent', () => {
  let rawTool;
  let registeredTool;
  let inputSchema;

  beforeEach(() => {
    const server = createStatesetMcpServer({
      commerce: makeMockCommerce(),
      allowApply: true,
      autonomousEngine: makeMockEngine(),
    });
    rawTool = findTool(server.getRawToolDefinitions(), 'delegate_to_agent');
    registeredTool = server.instance._registeredTools.delegate_to_agent;
    inputSchema = createToolInputSchema(rawTool.inputSchema);
  });

  describe('structure', () => {
    it('exists and has correct metadata', () => {
      assert.equal(rawTool.name, 'delegate_to_agent');
      assert.equal(rawTool.permission, 'write');
      assert.equal(rawTool.policyDomain, 'agentic');
      assert.equal(typeof registeredTool.handler, 'function');
    });

    it('describes the full supported agent set', () => {
      assert.match(rawTool.description, /Available agents:/);
      assert.ok(rawTool.description.includes(SUPPORTED_AGENT_NAMES_DESCRIPTION));
    });
  });

  describe('input schema', () => {
    it('accepts every supported agent name', () => {
      for (const agentName of SUPPORTED_AGENT_NAMES) {
        const parsed = inputSchema.safeParse({
          agent_name: agentName,
          task_description: `Delegate a task to ${agentName}`,
        });
        assert.equal(parsed.success, true, `Expected ${agentName} to validate`);
      }
    });

    it('rejects unsupported agent names', () => {
      const parsed = inputSchema.safeParse({
        agent_name: 'nonexistent-agent',
        task_description: 'Do something impossible',
      });
      assert.equal(parsed.success, false);
    });
  });

  describe('handler success', () => {
    it('calls autonomousEngine.executeAgentRequest with correct params', async () => {
      let calledWith = null;
      const engine = makeMockEngine({
        executeAgentRequest: async (agentName, taskDesc, context) => {
          calledWith = { agentName, taskDesc, context };
          return { status: 'completed' };
        },
      });

      const server = createStatesetMcpServer({
        commerce: makeMockCommerce(),
        allowApply: true,
        autonomousEngine: engine,
      });
      const tool = server.instance._registeredTools.delegate_to_agent;
      await tool.handler({
        agent_name: 'orders',
        task_description: 'List pending orders',
        context: { customerId: 'cust_001' },
      });

      assert.deepEqual(calledWith, {
        agentName: 'orders',
        taskDesc: 'List pending orders',
        context: { customerId: 'cust_001' },
      });
    });

    it('returns success with delegation result', async () => {
      const result = await registeredTool.handler({
          agent_name: 'inventory',
          task_description: 'Check stock for SKU-100',
          context: {},
      });
      const payload = JSON.parse(result.content[0].text);

      assert.equal(payload.success, true);
      assert.equal(payload.delegatedTo, 'inventory');
      assert.equal(payload.task, 'Check stock for SKU-100');
      assert.ok(payload.result);
      assert.equal(payload.result.status, 'completed');
    });

    it('defaults context to empty object when omitted', async () => {
      let capturedContext = null;
      const engine = makeMockEngine({
        executeAgentRequest: async (_name, _desc, context) => {
          capturedContext = context;
          return { status: 'done' };
        },
      });

      const server = createStatesetMcpServer({
        commerce: makeMockCommerce(),
        allowApply: true,
        autonomousEngine: engine,
      });
      const tool = server.instance._registeredTools.delegate_to_agent;
      await tool.handler({
          agent_name: 'returns',
          task_description: 'Process return RMA-42',
      });

      assert.deepEqual(capturedContext, {});
    });
  });

  describe('handler errors', () => {
    it('returns error when engine is not available', async () => {
      const server = createStatesetMcpServer({
        commerce: makeMockCommerce(),
        allowApply: true,
        autonomousEngine: null,
      });
      const tool = server.instance._registeredTools.delegate_to_agent;
      const result = await tool.handler({
          agent_name: 'orders',
          task_description: 'Do something',
      });
      const payload = JSON.parse(result.content[0].text);

      assert.equal(payload.success, false);
      assert.match(payload.error, /Autonomous engine not available/);
    });

    it('returns error when executeAgentRequest throws', async () => {
      const engine = makeMockEngine({
        executeAgentRequest: async () => {
          throw new Error('Agent not found');
        },
      });

      const server = createStatesetMcpServer({
        commerce: makeMockCommerce(),
        allowApply: true,
        autonomousEngine: engine,
      });
      const tool = server.instance._registeredTools.delegate_to_agent;
      const result = await tool.handler({
          agent_name: 'orders',
          task_description: 'Do something impossible',
          context: {},
      });
      const payload = JSON.parse(result.content[0].text);

      assert.equal(payload.success, false);
      assert.match(payload.error, /Delegation to 'orders' failed/);
      assert.match(payload.error, /Agent not found/);
    });
  });
});
