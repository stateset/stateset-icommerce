// Unit tests for the cost-budget cluster extracted from mcp-server.js.

import { strict as assert } from 'node:assert';
import { describe, it } from 'node:test';

import {
  addCostSummaryEntry,
  createCostSummary,
  normalizeCostBudget,
  normalizeCostBudgetKey,
  normalizeCostBudgetValue,
  resolveCostBudgetLimit,
} from '../../src/mcp/cost-budget.js';

describe('cost-budget · normalizeCostBudgetValue', () => {
  it('passes through non-negative finite numbers', () => {
    assert.equal(normalizeCostBudgetValue(0), 0);
    assert.equal(normalizeCostBudgetValue(1.5), 1.5);
    assert.equal(normalizeCostBudgetValue(100), 100);
  });

  it('rejects negative numbers', () => {
    assert.equal(normalizeCostBudgetValue(-1), null);
  });

  it('rejects NaN and Infinity', () => {
    assert.equal(normalizeCostBudgetValue(NaN), null);
    assert.equal(normalizeCostBudgetValue(Infinity), null);
    assert.equal(normalizeCostBudgetValue(-Infinity), null);
  });

  it('parses non-empty numeric strings', () => {
    assert.equal(normalizeCostBudgetValue('42'), 42);
    assert.equal(normalizeCostBudgetValue('  3.14  '), 3.14);
  });

  it('rejects non-numeric strings, empty strings, and other types', () => {
    assert.equal(normalizeCostBudgetValue('abc'), null);
    assert.equal(normalizeCostBudgetValue(''), null);
    assert.equal(normalizeCostBudgetValue('   '), null);
    assert.equal(normalizeCostBudgetValue(null), null);
    assert.equal(normalizeCostBudgetValue(undefined), null);
    assert.equal(normalizeCostBudgetValue({}), null);
    assert.equal(normalizeCostBudgetValue([]), null);
  });
});

describe('cost-budget · normalizeCostBudgetKey', () => {
  it('normalises bare token names to uppercase', () => {
    assert.equal(normalizeCostBudgetKey('usdc'), 'USDC');
    assert.equal(normalizeCostBudgetKey('  USDC  '), 'USDC');
  });

  it('normalises chain:token pairs', () => {
    assert.equal(normalizeCostBudgetKey('solana:usdc'), 'SOLANA:USDC');
    assert.equal(normalizeCostBudgetKey('  base : usdc '), 'BASE:USDC');
  });

  it('preserves wildcards', () => {
    assert.equal(normalizeCostBudgetKey('*'), '*');
    assert.equal(normalizeCostBudgetKey('solana:*'), 'SOLANA:*');
    assert.equal(normalizeCostBudgetKey('*:USDC'), '*:USDC');
  });

  it('rejects non-strings, empty strings, and malformed compound keys', () => {
    assert.equal(normalizeCostBudgetKey(null), null);
    assert.equal(normalizeCostBudgetKey(undefined), null);
    assert.equal(normalizeCostBudgetKey(123), null);
    assert.equal(normalizeCostBudgetKey(''), null);
    assert.equal(normalizeCostBudgetKey('   '), null);
    assert.equal(normalizeCostBudgetKey(':USDC'), null);
    assert.equal(normalizeCostBudgetKey('SOLANA:'), null);
  });
});

describe('cost-budget · normalizeCostBudget', () => {
  it('returns empty object for non-objects', () => {
    assert.deepEqual(normalizeCostBudget(null), {});
    assert.deepEqual(normalizeCostBudget(undefined), {});
    assert.deepEqual(normalizeCostBudget('string'), {});
    assert.deepEqual(normalizeCostBudget([]), {});
  });

  it('drops invalid entries silently', () => {
    const out = normalizeCostBudget({
      'solana:usdc': 100,
      'invalid:': -1,
      bad_key_value: 'not-a-number',
      USDC: '50',
      '*': 1000,
    });
    assert.deepEqual(out, {
      'SOLANA:USDC': 100,
      USDC: 50,
      '*': 1000,
    });
  });

  it('canonicalizes keys to uppercase', () => {
    const out = normalizeCostBudget({ 'BASE:USDC': 10, 'base:usdc': 20 });
    // Last one wins because Object.entries preserves insertion order;
    // both keys normalize to the same canonical form.
    assert.equal(out['BASE:USDC'], 20);
  });
});

describe('cost-budget · resolveCostBudgetLimit', () => {
  const budget = {
    'SOLANA:USDC': 100,
    USDC: 50,
    'BASE:*': 25,
    '*': 10,
  };

  it('matches exact chain:token first', () => {
    assert.equal(resolveCostBudgetLimit(budget, 'SOLANA', 'USDC'), 100);
  });

  it('falls back to token-only', () => {
    assert.equal(resolveCostBudgetLimit(budget, 'ETHEREUM', 'USDC'), 50);
  });

  it('falls back to chain-only wildcard', () => {
    assert.equal(resolveCostBudgetLimit(budget, 'BASE', 'DAI'), 25);
  });

  it('falls back to global wildcard', () => {
    assert.equal(resolveCostBudgetLimit(budget, 'ARBITRUM', 'XYZ'), 10);
  });

  it('returns null when no rule matches and no global wildcard exists', () => {
    const sparse = { 'SOLANA:USDC': 100 };
    assert.equal(resolveCostBudgetLimit(sparse, 'BASE', 'DAI'), null);
  });

  it('handles null/undefined chain and token defaults', () => {
    assert.equal(resolveCostBudgetLimit(budget, null, null), 10); // global
  });

  it('uppercases token symbol for lookup', () => {
    assert.equal(resolveCostBudgetLimit(budget, 'SOLANA', 'usdc'), 100);
  });
});

describe('cost-budget · createCostSummary', () => {
  it('returns an empty summary keyed by mode', () => {
    const s = createCostSummary('simulate');
    assert.equal(s.mode, 'simulate');
    assert.equal(s.totalEntries, 0);
    assert.equal(s.chargedEntries, 0);
    assert.equal(s.blockedEntries, 0);
    assert.deepEqual(s.entries, []);
    assert.deepEqual(s.totals, {});
  });
});

describe('cost-budget · addCostSummaryEntry', () => {
  it('aggregates numeric amounts per chain:token bucket', () => {
    const s = createCostSummary('simulate');
    addCostSummaryEntry(s, { chainId: 'SOLANA', tokenSymbol: 'USDC', amount: 5 });
    addCostSummaryEntry(s, { chainId: 'SOLANA', tokenSymbol: 'USDC', amount: '7' });
    assert.equal(s.totals['SOLANA:USDC'].amount, 12);
    assert.equal(s.totals['SOLANA:USDC'].entries, 2);
    assert.equal(s.totalEntries, 2);
    assert.equal(s.entries.length, 2);
  });

  it('falls back to amountText when amount is not numeric', () => {
    const s = createCostSummary('simulate');
    addCostSummaryEntry(s, {
      chainId: 'SOLANA',
      tokenSymbol: 'USDC',
      amount: 'pending-quote',
    });
    assert.equal(s.totals['SOLANA:USDC'].amount, 0);
    assert.equal(s.totals['SOLANA:USDC'].amountText, 'pending-quote');
  });

  it('uses defaults for missing chainId / tokenSymbol', () => {
    const s = createCostSummary('execute');
    addCostSummaryEntry(s, { amount: 1 });
    assert.ok(s.totals['unknown:UNKNOWN']);
    assert.equal(s.totals['unknown:UNKNOWN'].entries, 1);
  });

  it('counts charged and blocked entries', () => {
    const s = createCostSummary('execute');
    addCostSummaryEntry(s, {
      chainId: 'SOLANA',
      tokenSymbol: 'USDC',
      amount: 5,
      charged: true,
    });
    addCostSummaryEntry(s, {
      chainId: 'SOLANA',
      tokenSymbol: 'USDC',
      amount: 0,
      blocked: true,
      blockedReason: 'budget-exceeded',
    });
    assert.equal(s.chargedEntries, 1);
    assert.equal(s.blockedEntries, 1);
    assert.equal(s.entries[1].blockedReason, 'budget-exceeded');
  });

  it('preserves stepIndex, tool, source, rule, status fields on each entry', () => {
    const s = createCostSummary('simulate');
    addCostSummaryEntry(s, {
      chainId: 'SOLANA',
      tokenSymbol: 'USDC',
      amount: 1,
      stepIndex: 3,
      tool: 'pay',
      status: 'ok',
      source: 'inferred',
      rule: 'SOLANA:USDC',
    });
    assert.equal(s.entries[0].step, 3);
    assert.equal(s.entries[0].tool, 'pay');
    assert.equal(s.entries[0].status, 'ok');
    assert.equal(s.entries[0].source, 'inferred');
    assert.equal(s.entries[0].rule, 'SOLANA:USDC');
    assert.equal(s.entries[0].amountNumeric, 1);
  });

  it('records null amountNumeric for unparseable amounts', () => {
    const s = createCostSummary('simulate');
    addCostSummaryEntry(s, { amount: 'pending', tool: 'quote' });
    assert.equal(s.entries[0].amountNumeric, null);
  });
});
