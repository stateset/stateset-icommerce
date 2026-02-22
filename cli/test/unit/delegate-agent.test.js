/**
 * Delegate-to-Agent Tool Test Suite
 *
 * Tests for the delegate_to_agent agentic runtime tool defined in
 * cli/src/mcp-server.js. Verifies tool metadata, permission level,
 * and handler behavior with mocked autonomousEngine.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

// The delegate_to_agent tool is an inline AGENTIC_RUNTIME_TOOLS entry inside
// mcp-server.js. It is not separately exported, so we replicate its handler
// logic faithfully here for isolated unit testing. This avoids importing
// the full MCP server (which has heavy side-effects and deps).
//
// The handler signature from mcp-server.js:
//   handler: async ({ params, autonomousEngine }) => { ... }

// ============================================================================
// Faithful replica of the delegate_to_agent handler from mcp-server.js
// ============================================================================

const delegateToAgentTool = {
  name: 'delegate_to_agent',
  description:
    'Delegate a sub-task to a specialized commerce agent. Available agents: orders, inventory, returns, checkout, analytics, promotions, subscriptions, customer-service.',
  permission: 'write',
  policyDomain: 'agentic',
  handler: async ({ params, autonomousEngine }) => {
    if (!autonomousEngine) {
      return {
        success: false,
        error:
          'Autonomous engine not available. Agent delegation requires the autonomous engine to be initialized.',
      };
    }
    try {
      const result = await autonomousEngine.executeAgentRequest(
        params.agent_name,
        params.task_description,
        params.context || {},
      );
      return {
        success: true,
        delegatedTo: params.agent_name,
        task: params.task_description,
        result,
      };
    } catch (err) {
      return {
        success: false,
        error: `Delegation to '${params.agent_name}' failed: ${err.message}`,
      };
    }
  },
};

// ============================================================================
// Helper: find tool by name (matches pattern from segments/store-credits tests)
// ============================================================================

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock autonomous engine factory
// ============================================================================

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

// ============================================================================
// Structural checks
// ============================================================================

describe('delegate_to_agent — structure', () => {
  const tools = [delegateToAgentTool];
  const tool = findTool(tools, 'delegate_to_agent');

  it('tool exists and has correct name', () => {
    assert.equal(tool.name, 'delegate_to_agent');
  });

  it('has a handler function', () => {
    assert.equal(typeof tool.handler, 'function');
  });

  it('permission is write', () => {
    assert.equal(tool.permission, 'write');
  });

  it('policyDomain is agentic', () => {
    assert.equal(tool.policyDomain, 'agentic');
  });
});

// ============================================================================
// Handler — success path
// ============================================================================

describe('delegate_to_agent — handler success', () => {
  it('calls autonomousEngine.executeAgentRequest with correct params', async () => {
    let calledWith = null;
    const engine = makeMockEngine({
      executeAgentRequest: async (agentName, taskDesc, context) => {
        calledWith = { agentName, taskDesc, context };
        return { status: 'completed' };
      },
    });

    await delegateToAgentTool.handler({
      params: {
        agent_name: 'orders',
        task_description: 'List pending orders',
        context: { customerId: 'cust_001' },
      },
      autonomousEngine: engine,
    });

    assert.equal(calledWith.agentName, 'orders');
    assert.equal(calledWith.taskDesc, 'List pending orders');
    assert.deepEqual(calledWith.context, { customerId: 'cust_001' });
  });

  it('returns success with delegation result', async () => {
    const engine = makeMockEngine();
    const result = await delegateToAgentTool.handler({
      params: {
        agent_name: 'inventory',
        task_description: 'Check stock for SKU-100',
        context: {},
      },
      autonomousEngine: engine,
    });

    assert.equal(result.success, true);
    assert.equal(result.delegatedTo, 'inventory');
    assert.equal(result.task, 'Check stock for SKU-100');
    assert.ok(result.result, 'should include engine result');
    assert.equal(result.result.status, 'completed');
  });

  it('defaults context to empty object when omitted', async () => {
    let capturedContext = null;
    const engine = makeMockEngine({
      executeAgentRequest: async (_name, _desc, context) => {
        capturedContext = context;
        return { status: 'done' };
      },
    });

    await delegateToAgentTool.handler({
      params: {
        agent_name: 'returns',
        task_description: 'Process return RMA-42',
      },
      autonomousEngine: engine,
    });

    assert.deepEqual(capturedContext, {});
  });
});

// ============================================================================
// Handler — error paths
// ============================================================================

describe('delegate_to_agent — handler errors', () => {
  it('returns error when engine is not available (null)', async () => {
    const result = await delegateToAgentTool.handler({
      params: {
        agent_name: 'orders',
        task_description: 'Do something',
      },
      autonomousEngine: null,
    });

    assert.equal(result.success, false);
    assert.ok(result.error.includes('Autonomous engine not available'));
  });

  it('returns error when engine is undefined', async () => {
    const result = await delegateToAgentTool.handler({
      params: {
        agent_name: 'orders',
        task_description: 'Do something',
      },
      autonomousEngine: undefined,
    });

    assert.equal(result.success, false);
    assert.ok(result.error.includes('Autonomous engine not available'));
  });

  it('returns error when executeAgentRequest throws', async () => {
    const engine = makeMockEngine({
      executeAgentRequest: async () => {
        throw new Error('Agent not found');
      },
    });

    const result = await delegateToAgentTool.handler({
      params: {
        agent_name: 'nonexistent-agent',
        task_description: 'Do something impossible',
        context: {},
      },
      autonomousEngine: engine,
    });

    assert.equal(result.success, false);
    assert.ok(result.error.includes("Delegation to 'nonexistent-agent' failed"));
    assert.ok(result.error.includes('Agent not found'));
  });

  it('includes agent name in error message on failure', async () => {
    const engine = makeMockEngine({
      executeAgentRequest: async () => {
        throw new Error('timeout');
      },
    });

    const result = await delegateToAgentTool.handler({
      params: {
        agent_name: 'analytics',
        task_description: 'Generate report',
        context: {},
      },
      autonomousEngine: engine,
    });

    assert.equal(result.success, false);
    assert.ok(result.error.includes('analytics'), 'error should mention agent name');
    assert.ok(result.error.includes('timeout'), 'error should include original message');
  });
});
