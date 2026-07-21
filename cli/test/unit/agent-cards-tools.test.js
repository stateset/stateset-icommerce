/**
 * Agent Cards Tools Test Suite
 *
 * Tests for tool definitions, schemas, permissions, and handler guards
 * in src/tools/agent-cards.js (5 tools).
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';
import { agentCardTools } from '../../src/tools/agent-cards.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findTool(name) {
  const tool = agentCardTools.find((t) => t.name === name);
  assert.ok(tool, `Tool "${name}" not found in agentCardTools`);
  return tool;
}

function getSchema(name) {
  return z.object(findTool(name).inputSchema);
}

function expectFail(schema, data, msg) {
  const result = schema.safeParse(data);
  assert.ok(!result.success, msg || `Expected parse to fail for: ${JSON.stringify(data)}`);
}

function expectPass(schema, data, msg) {
  const result = schema.safeParse(data);
  assert.ok(
    result.success,
    msg ||
      `Expected parse to pass for: ${JSON.stringify(data)}, errors: ${JSON.stringify(result.error?.issues)}`,
  );
}

// ---------------------------------------------------------------------------
// All 5 tool names
// ---------------------------------------------------------------------------

const ALL_TOOL_NAMES = [
  'register_agent_card',
  'discover_agents',
  'get_agent_card',
  'verify_agent',
  'list_agent_cards',
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Agent Cards Tools — definitions', () => {
  it('exports exactly 5 tools', () => {
    assert.strictEqual(agentCardTools.length, 5);
  });

  for (const name of ALL_TOOL_NAMES) {
    it(`includes tool "${name}"`, () => {
      assert.ok(findTool(name));
    });
  }

  it('every tool has a handler function', () => {
    for (const tool of agentCardTools) {
      assert.strictEqual(
        typeof tool.handler,
        'function',
        `${tool.name} handler should be a function`,
      );
    }
  });

  it('every tool has an inputSchema object', () => {
    for (const tool of agentCardTools) {
      assert.strictEqual(
        typeof tool.inputSchema,
        'object',
        `${tool.name} should have an inputSchema`,
      );
    }
  });
});

describe('Agent Cards Tools — permissions', () => {
  it('register_agent_card is write', () => {
    assert.strictEqual(findTool('register_agent_card').permission, 'write');
  });

  it('discover_agents is read', () => {
    assert.strictEqual(findTool('discover_agents').permission, 'read');
  });

  it('get_agent_card is read', () => {
    assert.strictEqual(findTool('get_agent_card').permission, 'read');
  });

  it('verify_agent is write', () => {
    assert.strictEqual(findTool('verify_agent').permission, 'write');
  });

  it('list_agent_cards is read', () => {
    assert.strictEqual(findTool('list_agent_cards').permission, 'read');
  });
});

describe('Agent Cards Tools — register_agent_card schema', () => {
  it('name requires min 1 char', () => {
    expectFail(getSchema('register_agent_card'), {
      name: '',
      walletAddress: '0xABC',
      publicKey: 'pk1',
    });
  });

  it('walletAddress requires min 1 char', () => {
    expectFail(getSchema('register_agent_card'), {
      name: 'Bot',
      walletAddress: '',
      publicKey: 'pk1',
    });
  });

  it('publicKey requires min 1 char', () => {
    expectFail(getSchema('register_agent_card'), {
      name: 'Bot',
      walletAddress: '0xABC',
      publicKey: '',
    });
  });

  it('requires name, walletAddress, publicKey', () => {
    expectFail(getSchema('register_agent_card'), {});
    expectFail(getSchema('register_agent_card'), { name: 'Bot' });
    expectFail(getSchema('register_agent_card'), { name: 'Bot', walletAddress: '0xABC' });
  });

  it('accepts valid registration with required fields only', () => {
    expectPass(getSchema('register_agent_card'), {
      name: 'Widget Bot',
      walletAddress: '0xABC123',
      publicKey: 'ed25519:pk123',
    });
  });

  it('endpointUrl must be a valid URL', () => {
    expectFail(getSchema('register_agent_card'), {
      name: 'Bot',
      walletAddress: '0xABC',
      publicKey: 'pk1',
      endpointUrl: 'not-a-url',
    });
    expectPass(getSchema('register_agent_card'), {
      name: 'Bot',
      walletAddress: '0xABC',
      publicKey: 'pk1',
      endpointUrl: 'https://agent.example.com/a2a',
    });
  });

  it('supportedNetworks and supportedAssets are optional arrays', () => {
    expectPass(getSchema('register_agent_card'), {
      name: 'Bot',
      walletAddress: '0xABC',
      publicKey: 'pk1',
      supportedNetworks: ['set_chain', 'base'],
      supportedAssets: ['usdc', 'ssusd'],
    });
  });

  it('paymentAddresses is an optional network-to-address map', () => {
    expectPass(getSchema('register_agent_card'), {
      name: 'Bot',
      walletAddress: '0xABC',
      publicKey: 'pk1',
      paymentAddresses: {
        bitcoin: 'bc1qbot',
        zcash: 'u1bot',
      },
    });
  });

  it('skills is optional array of strings', () => {
    expectPass(getSchema('register_agent_card'), {
      name: 'Bot',
      walletAddress: '0xABC',
      publicKey: 'pk1',
      skills: ['sell', 'buy', 'quote'],
    });
  });

  it('description is optional string', () => {
    expectPass(getSchema('register_agent_card'), {
      name: 'Bot',
      walletAddress: '0xABC',
      publicKey: 'pk1',
      description: 'A commerce agent for widgets',
    });
  });
});

describe('Agent Cards Tools — discover_agents schema', () => {
  it('accepts empty params (all optional)', () => {
    expectPass(getSchema('discover_agents'), {});
  });

  it('accepts network, asset, skill, trustLevel', () => {
    expectPass(getSchema('discover_agents'), {
      network: 'base',
      asset: 'usdc',
      skill: 'sell',
      trustLevel: 'verified',
    });
  });
});

describe('Agent Cards Tools — get_agent_card schema', () => {
  it('accepts agentId', () => {
    expectPass(getSchema('get_agent_card'), { agentId: 'uuid-123' });
  });

  it('accepts walletAddress', () => {
    expectPass(getSchema('get_agent_card'), { walletAddress: '0xABC' });
  });

  it('accepts empty params (both optional)', () => {
    expectPass(getSchema('get_agent_card'), {});
  });
});

describe('Agent Cards Tools — verify_agent schema', () => {
  it('agentId requires min 1 char', () => {
    expectFail(getSchema('verify_agent'), { agentId: '' });
  });

  it('requires agentId', () => {
    expectFail(getSchema('verify_agent'), {});
  });

  it('accepts valid agentId', () => {
    expectPass(getSchema('verify_agent'), { agentId: 'agent-uuid-123' });
  });
});

describe('Agent Cards Tools — list_agent_cards schema', () => {
  it('accepts empty params', () => {
    expectPass(getSchema('list_agent_cards'), {});
  });

  it('active is optional boolean', () => {
    expectPass(getSchema('list_agent_cards'), { active: true });
    expectPass(getSchema('list_agent_cards'), { active: false });
  });

  it('trustLevel is optional string', () => {
    expectPass(getSchema('list_agent_cards'), { trustLevel: 'enterprise' });
  });

  it('limit is optional number', () => {
    expectPass(getSchema('list_agent_cards'), { limit: 10 });
  });
});

describe('Agent Cards Tools — register_agent_card handler guard', () => {
  it('returns error when allowApply is false', async () => {
    const tool = findTool('register_agent_card');
    const result = await tool.handler({
      commerce: {},
      params: { name: 'Bot', walletAddress: '0xABC', publicKey: 'pk1' },
      allowApply: false,
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'), `Error should mention --apply: ${result.error}`);
    assert.ok(result.wouldRegister, 'Should include wouldRegister preview');
    assert.strictEqual(result.wouldRegister.name, 'Bot');
  });
});

describe('Agent Cards Tools — verify_agent handler guard', () => {
  it('returns error when allowApply is false', async () => {
    const tool = findTool('verify_agent');
    const result = await tool.handler({
      commerce: {},
      params: { agentId: 'agent-123' },
      allowApply: false,
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'), `Error should mention --apply: ${result.error}`);
    assert.ok(result.wouldVerify, 'Should include wouldVerify preview');
    assert.strictEqual(result.wouldVerify.agentId, 'agent-123');
  });
});

describe('Agent Cards Tools — get_agent_card handler returns error for missing params', () => {
  it('returns error when neither agentId nor walletAddress provided', async () => {
    const tool = findTool('get_agent_card');
    const result = await tool.handler({
      commerce: {
        x402: () => ({
          getAgent: async () => null,
          getAgentByWallet: async () => null,
        }),
      },
      params: {},
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('agentId') || result.error.includes('walletAddress'));
  });
});
