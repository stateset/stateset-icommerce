/**
 * Tests for cli/src/treasury/store.js
 *
 * Covers: TreasuryStore (record, list, getBalances, getBalance).
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs';
import path from 'path';
import os from 'os';

import {
  TreasuryStore,
  defaultTreasuryDir,
  defaultTreasuryDbPath,
} from '../../src/treasury/store.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createStore() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'treasury-test-'));
  const dbPath = path.join(dir, 'treasury.db');
  const store = new TreasuryStore({ dbPath });
  store.init();
  return { store, dir };
}

function makeEntry(overrides = {}) {
  return {
    event_id: 'evt-001',
    agent_id: 'agent-1',
    chain_id: 'solana',
    token_symbol: 'USDC',
    token_address: '0xUSDC',
    token_decimals: 6,
    direction: 'deposit',
    amount_smallest: '1000000',
    amount_display: '1.00',
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// defaultTreasuryDir / defaultTreasuryDbPath
// ---------------------------------------------------------------------------

describe('defaultTreasuryDir', () => {
  it('returns a path under .stateset/treasury', () => {
    const dir = defaultTreasuryDir('/tmp/test');
    assert.ok(dir.includes('.stateset'));
    assert.ok(dir.includes('treasury'));
  });
});

describe('defaultTreasuryDbPath', () => {
  it('returns a .db path', () => {
    const p = defaultTreasuryDbPath('/tmp/test');
    assert.ok(p.endsWith('.db'));
  });
});

// ---------------------------------------------------------------------------
// TreasuryStore
// ---------------------------------------------------------------------------

describe('TreasuryStore', () => {
  let store;
  let dir;

  beforeEach(() => {
    ({ store, dir } = createStore());
  });

  afterEach(() => {
    store.close();
    try {
      fs.rmSync(dir, { recursive: true });
    } catch {
      /* ok */
    }
  });

  describe('record()', () => {
    it('records a transaction', () => {
      const result = store.record(makeEntry());
      assert.equal(result.event_id, 'evt-001');
      assert.equal(result.agent_id, 'agent-1');
    });

    it('stores metadata as JSON', () => {
      const result = store.record(makeEntry({ metadata: { key: 'val' } }));
      assert.equal(result.metadata, '{"key":"val"}');
    });

    it('sets created_at when not provided', () => {
      const result = store.record(makeEntry());
      assert.ok(result.created_at);
    });

    it('auto-initializes if db not open', () => {
      const dir2 = fs.mkdtempSync(path.join(os.tmpdir(), 'treasury-test2-'));
      const store2 = new TreasuryStore({ dbPath: path.join(dir2, 'test.db') });
      // Don't call init() — record should auto-init
      store2.record(makeEntry());
      const list = store2.list({ agentId: 'agent-1' });
      assert.equal(list.length, 1);
      store2.close();
      fs.rmSync(dir2, { recursive: true });
    });
  });

  describe('list()', () => {
    it('lists transactions by agent', () => {
      store.record(makeEntry({ event_id: 'e1' }));
      store.record(makeEntry({ event_id: 'e2' }));
      store.record(makeEntry({ event_id: 'e3', agent_id: 'other' }));

      const results = store.list({ agentId: 'agent-1' });
      assert.equal(results.length, 2);
    });

    it('filters by chainId', () => {
      store.record(makeEntry({ event_id: 'e1', chain_id: 'solana' }));
      store.record(makeEntry({ event_id: 'e2', chain_id: 'base' }));

      const results = store.list({ agentId: 'agent-1', chainId: 'solana' });
      assert.equal(results.length, 1);
    });

    it('filters by tokenSymbol', () => {
      store.record(makeEntry({ event_id: 'e1', token_symbol: 'USDC' }));
      store.record(makeEntry({ event_id: 'e2', token_symbol: 'ssUSD' }));

      const results = store.list({ agentId: 'agent-1', tokenSymbol: 'ssUSD' });
      assert.equal(results.length, 1);
    });

    it('respects limit', () => {
      for (let i = 0; i < 10; i++) {
        store.record(makeEntry({ event_id: `e${i}` }));
      }
      const results = store.list({ agentId: 'agent-1', limit: 3 });
      assert.equal(results.length, 3);
    });

    it('parses metadata JSON', () => {
      store.record(makeEntry({ metadata: { foo: 'bar' } }));
      const results = store.list({ agentId: 'agent-1' });
      assert.deepStrictEqual(results[0].metadata, { foo: 'bar' });
    });
  });

  describe('getBalances()', () => {
    it('computes net balance from deposits', () => {
      store.record(makeEntry({ event_id: 'e1', direction: 'deposit', amount_smallest: '1000000' }));
      store.record(makeEntry({ event_id: 'e2', direction: 'deposit', amount_smallest: '2000000' }));

      const balances = store.getBalances({ agentId: 'agent-1' });
      assert.equal(balances.length, 1);
      assert.equal(balances[0].balanceSmallest, 3000000n);
    });

    it('subtracts withdrawals', () => {
      store.record(makeEntry({ event_id: 'e1', direction: 'deposit', amount_smallest: '5000000' }));
      store.record(
        makeEntry({ event_id: 'e2', direction: 'withdraw', amount_smallest: '2000000' }),
      );

      const balances = store.getBalances({ agentId: 'agent-1' });
      assert.equal(balances[0].balanceSmallest, 3000000n);
    });

    it('subtracts fees', () => {
      store.record(makeEntry({ event_id: 'e1', direction: 'deposit', amount_smallest: '5000000' }));
      store.record(makeEntry({ event_id: 'e2', direction: 'fee', amount_smallest: '100000' }));

      const balances = store.getBalances({ agentId: 'agent-1' });
      assert.equal(balances[0].balanceSmallest, 4900000n);
    });

    it('groups by chain and token', () => {
      store.record(
        makeEntry({
          event_id: 'e1',
          chain_id: 'solana',
          token_symbol: 'USDC',
          direction: 'deposit',
          amount_smallest: '100',
        }),
      );
      store.record(
        makeEntry({
          event_id: 'e2',
          chain_id: 'base',
          token_symbol: 'USDC',
          direction: 'deposit',
          amount_smallest: '200',
        }),
      );

      const balances = store.getBalances({ agentId: 'agent-1' });
      assert.equal(balances.length, 2);
    });

    it('filters by chainId', () => {
      store.record(
        makeEntry({
          event_id: 'e1',
          chain_id: 'solana',
          direction: 'deposit',
          amount_smallest: '100',
        }),
      );
      store.record(
        makeEntry({
          event_id: 'e2',
          chain_id: 'base',
          direction: 'deposit',
          amount_smallest: '200',
        }),
      );

      const balances = store.getBalances({ agentId: 'agent-1', chainId: 'solana' });
      assert.equal(balances.length, 1);
      assert.equal(balances[0].chainId, 'solana');
    });
  });

  describe('getBalance()', () => {
    it('returns balance for specific token', () => {
      store.record(makeEntry({ event_id: 'e1', direction: 'deposit', amount_smallest: '1000' }));
      const balance = store.getBalance({
        agentId: 'agent-1',
        chainId: 'solana',
        tokenSymbol: 'USDC',
      });
      assert.equal(balance.balanceSmallest, 1000n);
    });

    it('returns 0 for unknown token', () => {
      const balance = store.getBalance({
        agentId: 'agent-1',
        chainId: 'solana',
        tokenSymbol: 'NOPE',
      });
      assert.equal(balance.balanceSmallest, 0n);
      assert.equal(balance.tokenSymbol, 'NOPE');
    });
  });

  describe('close()', () => {
    it('closes without error', () => {
      store.close();
      // Double close should not throw
      store.close();
    });
  });
});
