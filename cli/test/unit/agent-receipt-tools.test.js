/**
 * Agent Receipt Tools — security-focused test suite for src/tools/agent-receipt.js.
 *
 * Focus: the demo-key gating on the on-chain signing tools (dispute / resolve /
 * release / refund / payout). The well-known public Anvil keys must NOT be usable
 * to sign value-bearing actions unless STATESET_ALLOW_DEMO_KEYS is set, and a real
 * configured key (SEQUENCER_KEY / BUYER_KEY / SELLER_KEY) must always be honored.
 *
 * The resolver is tested directly so no network I/O is performed. A synchronous
 * refusal path is also exercised end-to-end through agent_receipt_dispute, which
 * throws during key resolution (inside escrowAs) before any RPC call is made.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

// Provide deployed addresses so loadAddresses() succeeds and the dispute handler
// reaches the key-resolution path (where it refuses synchronously, no network).
process.env.ORDER_ESCROW = '0x' + '11'.repeat(20);
process.env.SSUSD_TOKEN = '0x' + '22'.repeat(20);

const { agentReceiptTools, resolveSigningKey } = await import('../../src/tools/agent-receipt.js');

const ORDER_ID_HASH = '0x' + 'ab'.repeat(32);
const KEY_ENV_VARS = ['SEQUENCER_KEY', 'BUYER_KEY', 'SELLER_KEY', 'STATESET_ALLOW_DEMO_KEYS'];

// Public, well-known Anvil keys — these are the demo fallbacks.
const ANVIL = {
  operator: '0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d',
  buyer: '0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a',
  seller: '0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6',
};
const REAL_KEY = '0x' + '11'.repeat(32);

function findTool(name) {
  const tool = agentReceiptTools.find((t) => t.name === name);
  assert.ok(tool, `Tool "${name}" not found in agentReceiptTools`);
  return tool;
}

describe('agent-receipt — resolveSigningKey gating', () => {
  /** @type {Record<string, string | undefined>} */
  let saved;

  beforeEach(() => {
    saved = {};
    for (const key of KEY_ENV_VARS) {
      saved[key] = process.env[key];
      delete process.env[key];
    }
  });

  afterEach(() => {
    for (const key of KEY_ENV_VARS) {
      if (saved[key] === undefined) delete process.env[key];
      else process.env[key] = saved[key];
    }
  });

  it('refuses each role with no configured key and demo mode off', () => {
    for (const role of /** @type {const} */ (['operator', 'buyer', 'seller'])) {
      assert.throws(
        () => resolveSigningKey(role),
        (err) =>
          err instanceof Error &&
          new RegExp(`No signing key configured for role "${role}"`).test(err.message) &&
          /STATESET_ALLOW_DEMO_KEYS/.test(err.message),
        `role ${role} should refuse`,
      );
    }
  });

  it('names the role-specific env var in the refusal', () => {
    assert.throws(() => resolveSigningKey('operator'), /SEQUENCER_KEY/);
    assert.throws(() => resolveSigningKey('buyer'), /BUYER_KEY/);
    assert.throws(() => resolveSigningKey('seller'), /SELLER_KEY/);
  });

  it('returns the public Anvil key only when STATESET_ALLOW_DEMO_KEYS is truthy', () => {
    for (const value of ['1', 'true', 'yes', 'on', 'TRUE', 'On']) {
      process.env.STATESET_ALLOW_DEMO_KEYS = value;
      assert.strictEqual(resolveSigningKey('seller'), ANVIL.seller, `value=${value}`);
      assert.strictEqual(resolveSigningKey('buyer'), ANVIL.buyer);
      assert.strictEqual(resolveSigningKey('operator'), ANVIL.operator);
    }
  });

  it('treats non-truthy STATESET_ALLOW_DEMO_KEYS values as disabled', () => {
    for (const value of ['', '0', 'false', 'no', 'off', 'nope']) {
      process.env.STATESET_ALLOW_DEMO_KEYS = value;
      assert.throws(
        () => resolveSigningKey('seller'),
        /No signing key configured/,
        `value=${value}`,
      );
    }
  });

  it('honors a configured role key without demo mode', () => {
    process.env.BUYER_KEY = REAL_KEY;
    assert.strictEqual(resolveSigningKey('buyer'), REAL_KEY);
    // Other roles remain gated.
    assert.throws(() => resolveSigningKey('seller'), /No signing key configured/);
  });

  it('prefers a configured key over the demo fallback even when demo mode is on', () => {
    process.env.STATESET_ALLOW_DEMO_KEYS = '1';
    process.env.SELLER_KEY = REAL_KEY;
    const resolved = resolveSigningKey('seller');
    assert.strictEqual(resolved, REAL_KEY);
    assert.notStrictEqual(resolved, ANVIL.seller);
  });

  it('trims surrounding whitespace from configured keys', () => {
    process.env.SELLER_KEY = `  ${REAL_KEY}  `;
    assert.strictEqual(resolveSigningKey('seller'), REAL_KEY);
  });
});

describe('agent-receipt — handler refuses synchronously before any RPC', () => {
  /** @type {Record<string, string | undefined>} */
  let saved;

  beforeEach(() => {
    saved = {};
    for (const key of KEY_ENV_VARS) {
      saved[key] = process.env[key];
      delete process.env[key];
    }
  });

  afterEach(() => {
    for (const key of KEY_ENV_VARS) {
      if (saved[key] === undefined) delete process.env[key];
      else process.env[key] = saved[key];
    }
  });

  it('agent_receipt_dispute returns the key-gating error, not a network error', async () => {
    const dispute = findTool('agent_receipt_dispute');
    const result = await dispute.handler({
      order_id_hash: ORDER_ID_HASH,
      reason: 'item never arrived',
    });
    assert.strictEqual(result.success, false);
    assert.match(result.error, /No signing key configured for role "buyer"/);
  });

  it('agent_receipt_resolve refuses for the operator role without a key', async () => {
    const resolve = findTool('agent_receipt_resolve');
    const result = await resolve.handler({
      order_id_hash: ORDER_ID_HASH,
      in_favor_of_seller: true,
    });
    assert.strictEqual(result.success, false);
    assert.match(result.error, /No signing key configured for role "operator"/);
  });
});

describe('agent-receipt — tool surface', () => {
  it('exposes the on-chain signing tools with expected permissions', () => {
    assert.strictEqual(findTool('agent_receipt_dispute').permission, 'write');
    assert.strictEqual(findTool('agent_receipt_resolve').permission, 'admin');
    assert.strictEqual(findTool('agent_receipt_release').permission, 'write');
    assert.strictEqual(findTool('agent_receipt_refund').permission, 'write');
    assert.strictEqual(findTool('agent_receipt_request_payout').permission, 'write');
  });
});
