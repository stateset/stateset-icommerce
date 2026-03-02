/**
 * Treasury Tools Test Suite
 *
 * Tests for tool definitions, schemas, permissions, and Zod constraints
 * in src/tools/treasury.js (6 tools).
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';
import { treasuryTools } from '../../src/tools/treasury.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findTool(name) {
  const tool = treasuryTools.find((t) => t.name === name);
  assert.ok(tool, `Tool "${name}" not found in treasuryTools`);
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
// Tests
// ---------------------------------------------------------------------------

describe('Treasury Tools — definitions', () => {
  it('exports exactly 6 tools', () => {
    assert.strictEqual(treasuryTools.length, 6);
  });

  const expectedNames = [
    'treasury_balance',
    'treasury_ledger',
    'treasury_deposit',
    'treasury_buy',
    'treasury_list_tokens',
    'treasury_register_token',
  ];

  for (const name of expectedNames) {
    it(`includes tool "${name}"`, () => {
      assert.ok(findTool(name));
    });
  }

  it('each tool has a handler function', () => {
    for (const tool of treasuryTools) {
      assert.strictEqual(typeof tool.handler, 'function', `${tool.name} handler should be a function`);
    }
  });

  it('each tool has an inputSchema object', () => {
    for (const tool of treasuryTools) {
      assert.strictEqual(typeof tool.inputSchema, 'object', `${tool.name} should have an inputSchema`);
    }
  });
});

describe('Treasury Tools — permissions', () => {
  it('treasury_balance is read', () => {
    assert.strictEqual(findTool('treasury_balance').permission, 'read');
  });

  it('treasury_ledger is read', () => {
    assert.strictEqual(findTool('treasury_ledger').permission, 'read');
  });

  it('treasury_deposit is write', () => {
    assert.strictEqual(findTool('treasury_deposit').permission, 'write');
  });

  it('treasury_buy is write', () => {
    assert.strictEqual(findTool('treasury_buy').permission, 'write');
  });

  it('treasury_list_tokens is read', () => {
    assert.strictEqual(findTool('treasury_list_tokens').permission, 'read');
  });

  it('treasury_register_token is write', () => {
    assert.strictEqual(findTool('treasury_register_token').permission, 'write');
  });
});

describe('Treasury Tools — treasury_balance schema', () => {
  it('accepts empty params (all optional)', () => {
    expectPass(getSchema('treasury_balance'), {});
  });

  it('accepts agentId, chainId, token', () => {
    expectPass(getSchema('treasury_balance'), {
      agentId: 'agent1',
      chainId: 'set_chain',
      token: 'USDC',
    });
  });

  it('accepts agentId alone', () => {
    expectPass(getSchema('treasury_balance'), { agentId: 'myagent' });
  });
});

describe('Treasury Tools — treasury_ledger schema', () => {
  it('requires agentId (min 1 char)', () => {
    expectFail(getSchema('treasury_ledger'), { agentId: '' });
  });

  it('rejects missing agentId', () => {
    expectFail(getSchema('treasury_ledger'), {});
  });

  it('accepts valid agentId', () => {
    expectPass(getSchema('treasury_ledger'), { agentId: 'a1' });
  });

  it('limit must be int', () => {
    expectFail(getSchema('treasury_ledger'), { agentId: 'a1', limit: 2.5 });
  });

  it('limit min is 1', () => {
    expectFail(getSchema('treasury_ledger'), { agentId: 'a1', limit: 0 });
  });

  it('limit max is 500', () => {
    expectFail(getSchema('treasury_ledger'), { agentId: 'a1', limit: 501 });
  });

  it('accepts limit within range', () => {
    expectPass(getSchema('treasury_ledger'), { agentId: 'a1', limit: 100 });
  });
});

describe('Treasury Tools — treasury_deposit schema', () => {
  it('requires agentId, chainId, token, amount', () => {
    expectFail(getSchema('treasury_deposit'), {});
    expectFail(getSchema('treasury_deposit'), { agentId: 'a' });
    expectFail(getSchema('treasury_deposit'), { agentId: 'a', chainId: 'c' });
    expectFail(getSchema('treasury_deposit'), { agentId: 'a', chainId: 'c', token: 't' });
  });

  it('amount must be positive', () => {
    expectFail(getSchema('treasury_deposit'), {
      agentId: 'a',
      chainId: 'c',
      token: 't',
      amount: 0,
    });
    expectFail(getSchema('treasury_deposit'), {
      agentId: 'a',
      chainId: 'c',
      token: 't',
      amount: -5,
    });
  });

  it('accepts valid deposit params', () => {
    expectPass(getSchema('treasury_deposit'), {
      agentId: 'a',
      chainId: 'set_chain',
      token: 'USDC',
      amount: 100,
    });
  });
});

describe('Treasury Tools — treasury_buy schema', () => {
  it('requires agentId, chainId, toToken, amount', () => {
    expectFail(getSchema('treasury_buy'), {});
  });

  it('amount must be positive', () => {
    expectFail(getSchema('treasury_buy'), {
      agentId: 'a',
      chainId: 'c',
      toToken: 'ETH',
      amount: 0,
    });
  });

  it('slippagePct defaults to 1', () => {
    const schema = getSchema('treasury_buy');
    const result = schema.safeParse({
      agentId: 'a',
      chainId: 'c',
      toToken: 'ETH',
      amount: 50,
    });
    assert.ok(result.success);
    assert.strictEqual(result.data.slippagePct, 1);
  });

  it('accepts custom slippagePct', () => {
    const schema = getSchema('treasury_buy');
    const result = schema.safeParse({
      agentId: 'a',
      chainId: 'c',
      toToken: 'ETH',
      amount: 50,
      slippagePct: 0.5,
    });
    assert.ok(result.success);
    assert.strictEqual(result.data.slippagePct, 0.5);
  });
});

describe('Treasury Tools — treasury_register_token schema', () => {
  it('requires symbol and chainId (min 1)', () => {
    expectFail(getSchema('treasury_register_token'), { symbol: '', chainId: 'c', decimals: 6 });
    expectFail(getSchema('treasury_register_token'), { symbol: 's', chainId: '', decimals: 6 });
  });

  it('requires decimals', () => {
    expectFail(getSchema('treasury_register_token'), { symbol: 'TOK', chainId: 'set_chain' });
  });

  it('accepts valid registration', () => {
    expectPass(getSchema('treasury_register_token'), {
      symbol: 'TOK',
      chainId: 'set_chain',
      decimals: 18,
    });
  });

  it('accepts optional address and priceUsd', () => {
    expectPass(getSchema('treasury_register_token'), {
      symbol: 'TOK',
      chainId: 'set_chain',
      decimals: 18,
      address: '0xABC',
      priceUsd: 1.0,
    });
  });
});
