/**
 * Unit tests for treasury/pricing-store.js — pricing rule CRUD
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import {
  loadPricing,
  savePricing,
  upsertPricingRule,
  removePricingRule,
  getPricingRule,
} from '../../src/treasury/pricing-store.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function tmpPricingPath() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'price-test-'));
  return path.join(dir, 'pricing.json');
}

// ===========================================================================
// upsertPricingRule
// ===========================================================================

describe('upsertPricingRule', () => {
  it('adds a new rule', () => {
    const pricing = { rules: [] };
    const result = upsertPricingRule(pricing, {
      tool: 'list_orders',
      chainId: 1,
      tokenSymbol: 'usdc',
      amount: 10,
    });
    assert.strictEqual(result.rules.length, 1);
    assert.strictEqual(result.rules[0].tool, 'list_orders');
    assert.strictEqual(result.rules[0].tokenSymbol, 'USDC'); // Uppercased
  });

  it('trims tool name', () => {
    const pricing = { rules: [] };
    const result = upsertPricingRule(pricing, {
      tool: '  list_orders  ',
      chainId: 1,
      tokenSymbol: 'usdc',
    });
    assert.strictEqual(result.rules[0].tool, 'list_orders');
  });

  it('updates existing rule (same tool + chainId)', () => {
    const pricing = {
      rules: [{ tool: 'list_orders', chainId: 1, tokenSymbol: 'USDC', amount: 10 }],
    };
    const result = upsertPricingRule(pricing, {
      tool: 'list_orders',
      chainId: 1,
      tokenSymbol: 'DAI',
      amount: 5,
    });
    assert.strictEqual(result.rules.length, 1);
    assert.strictEqual(result.rules[0].tokenSymbol, 'DAI');
    assert.strictEqual(result.rules[0].amount, 5);
  });

  it('different chainId creates new entry', () => {
    const pricing = {
      rules: [{ tool: 'list_orders', chainId: 1, tokenSymbol: 'USDC', amount: 10 }],
    };
    const result = upsertPricingRule(pricing, {
      tool: 'list_orders',
      chainId: 8453,
      tokenSymbol: 'USDC',
      amount: 20,
    });
    assert.strictEqual(result.rules.length, 2);
  });
});

// ===========================================================================
// removePricingRule
// ===========================================================================

describe('removePricingRule', () => {
  it('removes matching rule', () => {
    const pricing = {
      rules: [
        { tool: 'list_orders', chainId: 1, amount: 10 },
        { tool: 'get_order', chainId: 1, amount: 5 },
      ],
    };
    const result = removePricingRule(pricing, 'list_orders', 1);
    assert.strictEqual(result.rules.length, 1);
    assert.strictEqual(result.rules[0].tool, 'get_order');
  });

  it('trims tool name before matching', () => {
    const pricing = { rules: [{ tool: 'list_orders', chainId: 1 }] };
    const result = removePricingRule(pricing, '  list_orders  ', 1);
    assert.strictEqual(result.rules.length, 0);
  });

  it('only removes matching chainId', () => {
    const pricing = {
      rules: [
        { tool: 'list_orders', chainId: 1 },
        { tool: 'list_orders', chainId: 8453 },
      ],
    };
    const result = removePricingRule(pricing, 'list_orders', 1);
    assert.strictEqual(result.rules.length, 1);
    assert.strictEqual(result.rules[0].chainId, 8453);
  });

  it('no-op for nonexistent rule', () => {
    const pricing = { rules: [{ tool: 'list_orders', chainId: 1 }] };
    const result = removePricingRule(pricing, 'get_order', 1);
    assert.strictEqual(result.rules.length, 1);
  });
});

// ===========================================================================
// getPricingRule
// ===========================================================================

describe('getPricingRule', () => {
  it('finds rule by tool name', () => {
    const pricing = {
      rules: [
        { tool: 'list_orders', chainId: 1, amount: 10 },
        { tool: 'get_order', chainId: 1, amount: 5 },
      ],
    };
    const rule = getPricingRule(pricing, 'get_order');
    assert.ok(rule);
    assert.strictEqual(rule.amount, 5);
  });

  it('trims tool name', () => {
    const pricing = { rules: [{ tool: 'list_orders', chainId: 1 }] };
    const rule = getPricingRule(pricing, '  list_orders  ');
    assert.ok(rule);
  });

  it('returns null for nonexistent tool', () => {
    const pricing = { rules: [{ tool: 'list_orders', chainId: 1 }] };
    const rule = getPricingRule(pricing, 'create_order');
    assert.strictEqual(rule, null);
  });

  it('returns null for empty rules', () => {
    assert.strictEqual(getPricingRule({ rules: [] }, 'any'), null);
  });
});

// ===========================================================================
// loadPricing / savePricing
// ===========================================================================

describe('loadPricing', () => {
  it('returns empty rules for nonexistent file', async () => {
    const result = await loadPricing('/tmp/nonexistent-pricing-file.json');
    assert.deepStrictEqual(result, { rules: [] });
  });

  it('round-trips through save and load', async () => {
    const p = tmpPricingPath();
    const pricing = {
      rules: [{ tool: 'list_orders', chainId: 1, tokenSymbol: 'USDC', amount: 10 }],
    };
    await savePricing(p, pricing);
    const loaded = await loadPricing(p);
    assert.strictEqual(loaded.rules.length, 1);
    assert.strictEqual(loaded.rules[0].tool, 'list_orders');
  });

  it('handles malformed JSON gracefully', async () => {
    const p = tmpPricingPath();
    fs.writeFileSync(p, 'invalid-json');
    const result = await loadPricing(p);
    assert.deepStrictEqual(result, { rules: [] });
  });
});
