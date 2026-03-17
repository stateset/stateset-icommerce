/**
 * Tests for escrow auto-release (processEscrows) in cli/src/a2a/escrow.js
 */

import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createEscrowService } from '../../src/a2a/escrow.js';

function createMockStore(config = {}) {
  const escrows = config.escrows || [];
  const quotes = config.quotes || [];
  const updates = [];

  return {
    getEscrow: async (id) => escrows.find((e) => e.id === id) || null,
    listEscrows: async (filter) =>
      escrows.filter((e) => (!filter?.status || e.status === filter.status)),
    updateEscrow: async (id, data) => {
      const e = escrows.find((esc) => esc.id === id);
      if (e) Object.assign(e, data, { updated_at: new Date().toISOString() });
      updates.push({ id, data });
      return e;
    },
    createEscrow: async (data) => {
      escrows.push(data);
      return data;
    },
    releaseEscrowAtomic: (id) => {
      const e = escrows.find((esc) => esc.id === id);
      if (e) {
        e.status = 'released';
        e.released_at = new Date().toISOString();
      }
      return e;
    },
    getQuote: async (id) => quotes.find((q) => q.id === id) || null,
    _updates: updates,
  };
}

describe('escrow processEscrows()', () => {
  it('auto-releases time-locked escrow when time has passed', async () => {
    const pastTime = new Date(Date.now() - 3600_000).toISOString();
    const store = createMockStore({
      escrows: [
        {
          id: 'esc-1',
          status: 'active',
          buyer_address: '0xBuyer',
          seller_address: '0xSeller',
          amount: 100,
          amount_decimal: 100,
          asset: 'USDC',
          network: 'set_chain',
          release_conditions: JSON.stringify([
            { type: 'time_lock', releaseAfter: pastTime, completed: false },
          ]),
          expires_at: new Date(Date.now() + 86400_000).toISOString(),
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
      ],
    });

    const escrow = createEscrowService(store);
    const result = await escrow.processEscrows();

    assert.equal(result.released, 1);
    assert.equal(result.expired, 0);
    assert.equal(result.actions[0].action, 'auto_released');
  });

  it('expires escrow past deadline', async () => {
    const pastExpiry = new Date(Date.now() - 3600_000).toISOString();
    const store = createMockStore({
      escrows: [
        {
          id: 'esc-2',
          status: 'active',
          buyer_address: '0xBuyer',
          seller_address: '0xSeller',
          amount: 50,
          amount_decimal: 50,
          release_conditions: '[]',
          expires_at: pastExpiry,
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
      ],
    });

    const escrow = createEscrowService(store);
    const result = await escrow.processEscrows();

    assert.equal(result.expired, 1);
    assert.equal(result.released, 0);
  });

  it('skips escrows with unmet conditions', async () => {
    const futureTime = new Date(Date.now() + 86400_000).toISOString();
    const store = createMockStore({
      escrows: [
        {
          id: 'esc-3',
          status: 'active',
          buyer_address: '0xBuyer',
          seller_address: '0xSeller',
          amount: 200,
          amount_decimal: 200,
          release_conditions: JSON.stringify([
            { type: 'time_lock', releaseAfter: futureTime },
            { type: 'buyer_confirmed', completed: false },
          ]),
          expires_at: new Date(Date.now() + 86400_000 * 7).toISOString(),
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
      ],
    });

    const escrow = createEscrowService(store);
    const result = await escrow.processEscrows();

    assert.equal(result.released, 0);
    assert.equal(result.expired, 0);
    assert.equal(result.checked, 1);
  });

  it('processes mix of active and funded escrows', async () => {
    const pastTime = new Date(Date.now() - 3600_000).toISOString();
    const store = createMockStore({
      escrows: [
        {
          id: 'esc-active',
          status: 'active',
          buyer_address: '0xA',
          seller_address: '0xB',
          amount: 100,
          amount_decimal: 100,
          release_conditions: JSON.stringify([
            { type: 'time_lock', releaseAfter: pastTime },
          ]),
          expires_at: new Date(Date.now() + 86400_000).toISOString(),
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
        {
          id: 'esc-funded',
          status: 'funded',
          buyer_address: '0xC',
          seller_address: '0xD',
          amount: 200,
          amount_decimal: 200,
          release_conditions: JSON.stringify([
            { type: 'buyer_confirmed', completed: true },
          ]),
          expires_at: new Date(Date.now() + 86400_000).toISOString(),
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
      ],
    });

    const escrow = createEscrowService(store);
    const result = await escrow.processEscrows();

    assert.equal(result.checked, 2);
    assert.equal(result.released, 2);
  });

  it('returns zeros when no active escrows exist', async () => {
    const store = createMockStore();
    const escrow = createEscrowService(store);
    const result = await escrow.processEscrows();

    assert.equal(result.checked, 0);
    assert.equal(result.released, 0);
    assert.equal(result.expired, 0);
  });

  it('continues processing on individual escrow error', async () => {
    const pastTime = new Date(Date.now() - 3600_000).toISOString();
    const store = createMockStore({
      escrows: [
        {
          id: 'esc-bad',
          status: 'active',
          buyer_address: '0xA',
          seller_address: '0xB',
          amount: 100,
          amount_decimal: 100,
          release_conditions: 'INVALID_JSON', // Will cause parse error
          expires_at: new Date(Date.now() + 86400_000).toISOString(),
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
        {
          id: 'esc-good',
          status: 'active',
          buyer_address: '0xC',
          seller_address: '0xD',
          amount: 200,
          amount_decimal: 200,
          release_conditions: JSON.stringify([
            { type: 'time_lock', releaseAfter: pastTime },
          ]),
          expires_at: new Date(Date.now() + 86400_000).toISOString(),
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
      ],
    });

    const escrow = createEscrowService(store);
    const result = await escrow.processEscrows();

    // Should still process esc-good even though esc-bad failed
    assert.equal(result.checked, 2);
    assert.ok(result.released >= 1);
  });

  it('auto-releases seller_fulfilled condition when quote is fulfilled', async () => {
    const store = createMockStore({
      escrows: [
        {
          id: 'esc-seller',
          status: 'active',
          buyer_address: '0xBuyer',
          seller_address: '0xSeller',
          amount: 500,
          amount_decimal: 500,
          release_conditions: JSON.stringify([
            { type: 'seller_fulfilled', quoteId: 'q-1' },
          ]),
          expires_at: new Date(Date.now() + 86400_000).toISOString(),
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
      ],
      quotes: [{ id: 'q-1', status: 'fulfilled' }],
    });

    const escrow = createEscrowService(store);
    const result = await escrow.processEscrows();

    assert.equal(result.released, 1);
  });
});
