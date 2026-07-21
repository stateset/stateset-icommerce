/**
 * Stablecoin Tools Test Suite
 *
 * Tests for tool definitions, schemas, permissions, and handler guards
 * in src/tools/stablecoin.js (4 tools).
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';
import { stablecoinTools } from '../../src/tools/stablecoin.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findTool(name) {
  const tool = stablecoinTools.find((t) => t.name === name);
  assert.ok(tool, `Tool "${name}" not found in stablecoinTools`);
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

describe('Stablecoin Tools — definitions', () => {
  it('exports exactly 4 tools', () => {
    assert.strictEqual(stablecoinTools.length, 4);
  });

  const expectedNames = [
    'get_agent_wallet',
    'get_wallet_balance',
    'create_stablecoin_payment',
    'list_supported_chains',
  ];

  for (const name of expectedNames) {
    it(`includes tool "${name}"`, () => {
      assert.ok(findTool(name));
    });
  }

  it('every tool has a handler function', () => {
    for (const tool of stablecoinTools) {
      assert.strictEqual(
        typeof tool.handler,
        'function',
        `${tool.name} handler should be a function`,
      );
    }
  });

  it('every tool has an inputSchema object', () => {
    for (const tool of stablecoinTools) {
      assert.strictEqual(
        typeof tool.inputSchema,
        'object',
        `${tool.name} should have an inputSchema`,
      );
    }
  });
});

describe('Stablecoin Tools — permissions', () => {
  it('get_agent_wallet is read', () => {
    assert.strictEqual(findTool('get_agent_wallet').permission, 'read');
  });

  it('get_wallet_balance is read', () => {
    assert.strictEqual(findTool('get_wallet_balance').permission, 'read');
  });

  it('create_stablecoin_payment is write', () => {
    assert.strictEqual(findTool('create_stablecoin_payment').permission, 'write');
  });

  it('list_supported_chains is read', () => {
    assert.strictEqual(findTool('list_supported_chains').permission, 'read');
  });
});

describe('Stablecoin Tools — get_agent_wallet schema', () => {
  it('accepts empty params (chain is optional)', () => {
    expectPass(getSchema('get_agent_wallet'), {});
  });

  it('accepts chain string', () => {
    expectPass(getSchema('get_agent_wallet'), { chain: 'solana' });
  });
});

describe('Stablecoin Tools — get_wallet_balance schema', () => {
  it('accepts empty params (all optional)', () => {
    expectPass(getSchema('get_wallet_balance'), {});
  });

  it('accepts chain and token', () => {
    expectPass(getSchema('get_wallet_balance'), { chain: 'base', token: 'USDC' });
  });
});

describe('Stablecoin Tools — create_stablecoin_payment schema', () => {
  it('toAddress has min 1', () => {
    expectFail(getSchema('create_stablecoin_payment'), { toAddress: '', amount: 10 });
  });

  it('rejects missing toAddress', () => {
    expectFail(getSchema('create_stablecoin_payment'), { amount: 10 });
  });

  it('amount must be positive', () => {
    expectFail(getSchema('create_stablecoin_payment'), { toAddress: '0xABC', amount: 0 });
    expectFail(getSchema('create_stablecoin_payment'), { toAddress: '0xABC', amount: -5 });
  });

  it('rejects missing amount', () => {
    expectFail(getSchema('create_stablecoin_payment'), { toAddress: '0xABC' });
  });

  it('accepts valid payment params', () => {
    expectPass(getSchema('create_stablecoin_payment'), {
      toAddress: '0xABC',
      amount: 50.0,
    });
  });

  it('accepts optional chain, token, orderId, customerId, memo', () => {
    expectPass(getSchema('create_stablecoin_payment'), {
      toAddress: '0xABC',
      amount: 100,
      chain: 'solana',
      token: 'USDC',
      orderId: 'ord-1',
      customerId: 'cust-1',
      memo: 'Widget purchase',
    });
  });
});

describe('Stablecoin Tools — list_supported_chains schema', () => {
  it('has empty inputSchema', () => {
    const tool = findTool('list_supported_chains');
    assert.deepStrictEqual(tool.inputSchema, {});
  });

  it('accepts empty object', () => {
    expectPass(getSchema('list_supported_chains'), {});
  });
});

describe('Stablecoin Tools — create_stablecoin_payment handler guard', () => {
  it('returns error when allowApply is false', async () => {
    const tool = findTool('create_stablecoin_payment');
    const result = await tool.handler({
      commerce: {},
      params: { toAddress: '0xABC', amount: 50, chain: 'solana' },
      allowApply: false,
    });
    assert.strictEqual(result.success, false);
    assert.ok(result.error.includes('--apply'), `Error should mention --apply: ${result.error}`);
    assert.ok(result.wouldSend, 'Should include wouldSend preview');
    assert.strictEqual(result.wouldSend.to, '0xABC');
    assert.strictEqual(result.wouldSend.amount, 50);
  });

  it('wouldSend defaults chain to set_chain when not provided', async () => {
    const tool = findTool('create_stablecoin_payment');
    const result = await tool.handler({
      commerce: {},
      params: { toAddress: '0xWallet', amount: 25 },
      allowApply: false,
    });
    assert.strictEqual(result.wouldSend.chain, 'set_chain');
  });
});
