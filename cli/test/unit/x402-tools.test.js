/**
 * Unit tests for x402 MCP tool handlers.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { x402Tools } from '../../src/tools/x402.js';

function createMockCommerce(overrides = {}) {
  const x402Api = {
    signIntent: async (_intentId, payload) => ({
      id: 'intent_123',
      status: 'signed',
      payer_signature: payload.signature,
      payer_public_key: payload.public_key,
    }),
    ...overrides,
  };
  return {
    x402: () => x402Api,
  };
}

describe('x402_sign_intent tool', () => {
  it('returns preview details when allowApply is false', async () => {
    const tool = x402Tools.find((entry) => entry.name === 'x402_sign_intent');
    assert.ok(tool, 'x402_sign_intent should exist');

    const result = await tool.handler({
      commerce: createMockCommerce(),
      params: { intentId: 'intent_123' },
      allowApply: false,
    });

    assert.equal(result.error, 'Signing x402 intent requires --apply flag.');
    assert.equal(result.wouldSign.intentId, 'intent_123');
    assert.equal(result.wouldSign.mode, 'local_agent_key');
  });

  it('supports manual signing mode', async () => {
    const tool = x402Tools.find((entry) => entry.name === 'x402_sign_intent');
    assert.ok(tool, 'x402_sign_intent should exist');

    const result = await tool.handler({
      commerce: createMockCommerce(),
      params: {
        intentId: 'intent_123',
        signature: '0xabc123',
        publicKey: '0xdef456',
      },
      allowApply: true,
    });

    assert.equal(result.success, true);
    assert.equal(result.signing.mode, 'manual_signature');
    assert.equal(result.intent.id, 'intent_123');
  });

  it('rejects mixing local and manual signing inputs', async () => {
    const tool = x402Tools.find((entry) => entry.name === 'x402_sign_intent');
    assert.ok(tool, 'x402_sign_intent should exist');

    const result = await tool.handler({
      commerce: createMockCommerce(),
      params: {
        intentId: 'intent_123',
        signature: '0xabc123',
        publicKey: '0xdef456',
        agentId: 'agent-payer',
      },
      allowApply: true,
    });

    assert.ok(result.error.includes('either local signing params'));
  });
});

describe('x402_execute_agent_payment tool', () => {
  it('returns preview details when allowApply is false', async () => {
    const tool = x402Tools.find((entry) => entry.name === 'x402_execute_agent_payment');
    assert.ok(tool, 'x402_execute_agent_payment should exist');

    const result = await tool.handler({
      commerce: createMockCommerce(),
      params: {
        amount: 1000000,
        payeeAddress: '0xabc',
      },
      allowApply: false,
    });

    assert.equal(result.error, 'Executing an end-to-end x402 payment requires --apply flag.');
    assert.equal(result.wouldExecute.amount, 1000000);
    assert.equal(result.wouldExecute.payeeAddress, '0xabc');
    assert.equal(result.wouldExecute.network, 'set_chain');
  });
});
