/**
 * Unit tests for x402/budget.js — createBudgetState
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import { createBudgetState, getDefaultBudgetStateFile } from '../../src/x402/budget.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function tmpBudgetPath() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'budget-test-'));
  const p = path.join(dir, 'budget.json');
  // Write a clean initial state so each test gets its own daily/history objects
  // (avoids shared DEFAULT_STATE reference mutation across tests)
  fs.writeFileSync(p, JSON.stringify({ version: 1, daily: {}, history: [], balance: null }));
  return p;
}

// ===========================================================================
// getDefaultBudgetStateFile
// ===========================================================================

describe('getDefaultBudgetStateFile', () => {
  it('returns a path under ~/.stateset/x402/', () => {
    const p = getDefaultBudgetStateFile();
    assert.ok(p.includes('.stateset'));
    assert.ok(p.includes('x402'));
    assert.ok(p.endsWith('budget.json'));
  });
});

// ===========================================================================
// createBudgetState
// ===========================================================================

describe('createBudgetState', () => {
  it('creates with default state', () => {
    const budget = createBudgetState({ filePath: tmpBudgetPath() });
    assert.strictEqual(budget.getBalance(), null);
    assert.strictEqual(budget.getSpentToday(), 0);
    assert.deepStrictEqual(budget.listHistory(), []);
  });

  it('sets starting balance', () => {
    const budget = createBudgetState({ filePath: tmpBudgetPath(), startingBalance: 1000 });
    assert.strictEqual(budget.getBalance(), 1000);
  });

  it('does not override existing balance with startingBalance', () => {
    const p = tmpBudgetPath();
    // Write existing state
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, JSON.stringify({ version: 1, daily: {}, history: [], balance: 500 }));

    const budget = createBudgetState({ filePath: p, startingBalance: 1000 });
    assert.strictEqual(budget.getBalance(), 500); // Existing balance preserved
  });

  it('recordSpend decrements balance', () => {
    const budget = createBudgetState({ filePath: tmpBudgetPath(), startingBalance: 100 });
    budget.recordSpend(30);
    assert.strictEqual(budget.getBalance(), 70);
    budget.recordSpend(20);
    assert.strictEqual(budget.getBalance(), 50);
  });

  it('recordSpend tracks daily spending', () => {
    const budget = createBudgetState({ filePath: tmpBudgetPath() });
    budget.recordSpend(10);
    budget.recordSpend(25);
    assert.strictEqual(budget.getSpentToday(), 35);
  });

  it('recordSpend adds to history', () => {
    const budget = createBudgetState({ filePath: tmpBudgetPath() });
    budget.recordSpend(50, { tool: 'list_orders', network: 'set_chain' });
    const history = budget.listHistory();
    assert.strictEqual(history.length, 1);
    assert.strictEqual(history[0].amount, 50);
    assert.strictEqual(history[0].tool, 'list_orders');
    assert.ok(history[0].timestamp);
  });

  it('listHistory returns most recent first', () => {
    const budget = createBudgetState({ filePath: tmpBudgetPath() });
    budget.recordSpend(10, { label: 'first' });
    budget.recordSpend(20, { label: 'second' });
    budget.recordSpend(30, { label: 'third' });
    const history = budget.listHistory();
    assert.strictEqual(history[0].label, 'third');
    assert.strictEqual(history[2].label, 'first');
  });

  it('listHistory respects limit', () => {
    const budget = createBudgetState({ filePath: tmpBudgetPath() });
    for (let i = 0; i < 10; i++) {
      budget.recordSpend(i);
    }
    const history = budget.listHistory(3);
    assert.strictEqual(history.length, 3);
  });

  it('recordSpend works with null balance', () => {
    const budget = createBudgetState({ filePath: tmpBudgetPath() });
    budget.recordSpend(100);
    assert.strictEqual(budget.getBalance(), null); // Still null
    assert.strictEqual(budget.getSpentToday(), 100);
  });

  it('persists across instances', () => {
    const p = tmpBudgetPath();
    const budget1 = createBudgetState({ filePath: p, startingBalance: 500 });
    budget1.recordSpend(100);

    const budget2 = createBudgetState({ filePath: p });
    assert.strictEqual(budget2.getBalance(), 400);
  });

  it('history capped at 1000 entries', () => {
    const budget = createBudgetState({ filePath: tmpBudgetPath() });
    for (let i = 0; i < 1010; i++) {
      budget.recordSpend(1);
    }
    assert.strictEqual(budget.state.history.length, 1000);
  });
});
