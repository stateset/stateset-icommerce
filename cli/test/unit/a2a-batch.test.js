import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { A2AStore } from '../../src/a2a/store.js';
import { createBatchService } from '../../src/a2a/batch.js';

describe('A2A batch service', () => {
  it('forwards asset, network, message, and maxRounds when requesting quotes', async () => {
    const requestQuote = mock.fn(async (params) => ({
      success: true,
      quote: { id: `quote-${params.seller}` },
    }));
    const batch = createBatchService({ requestQuote }, {});

    const result = await batch.batchRequestQuotes([
      {
        seller: 'seller-a',
        items: [{ description: 'Compute' }],
        asset: 'BTC',
        network: 'bitcoin',
        message: 'Need native settlement',
        maxRounds: 3,
      },
    ]);

    assert.equal(result.sent, 1);
    assert.equal(requestQuote.mock.calls.length, 1);
    assert.deepEqual(requestQuote.mock.calls[0].arguments[0], {
      seller: 'seller-a',
      items: [{ description: 'Compute' }],
      asset: 'BTC',
      network: 'bitcoin',
      message: 'Need native settlement',
      maxRounds: 3,
    });
  });

  it('derives native escrow assets from the selected network', async () => {
    const store = new A2AStore({ dbPath: ':memory:' });
    store.init();
    const batch = createBatchService({}, store);

    const result = await batch.batchCreateEscrows([
      {
        buyerAddress: '0xBuyer',
        sellerAddress: 'bc1qseller',
        amount: 0.01,
        network: 'bitcoin',
      },
      {
        buyerAddress: '0xBuyer',
        sellerAddress: 'u1seller',
        amount: 1.25,
        network: 'zcash',
      },
    ]);

    assert.equal(result.created, 2);
    const first = store.getEscrow(result.escrowIds[0]);
    const second = store.getEscrow(result.escrowIds[1]);

    assert.equal(first.asset, 'BTC');
    assert.equal(first.network, 'bitcoin');
    assert.equal(first.amount_decimal, 0.01);

    assert.equal(second.asset, 'ZEC');
    assert.equal(second.network, 'zcash');
    assert.equal(second.amount_decimal, 1.25);

    store.close();
  });
});
