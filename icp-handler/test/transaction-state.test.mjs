import test from 'node:test';
import assert from 'node:assert/strict';
import * as state from '../src/state.mjs';
import { stubFundingInstructions } from '../src/backend-stub.mjs';

test('rollback suppresses notifications and leaves no partial state', () => {
  const notices = [];
  assert.throws(
    () =>
      state.atomic(() => {
        state.createEscrow('rollback:test', { state: 'pending', seq: 0 });
        state.afterCommit(() => notices.push('committed'));
        state.atomic(() => state.afterCommit(() => notices.push('nested')));
        throw new Error('abort');
      }),
    /abort/,
  );
  assert.equal(state.getEscrow('rollback:test'), undefined);
  assert.deepEqual(notices, []);
  state.atomic(() => {
    state.afterCommit(() => notices.push('first'));
    state.atomic(() => state.afterCommit(() => notices.push('second')));
    assert.deepEqual(notices, []);
  });
  assert.deepEqual(notices, ['first', 'second']);
  assert.throws(() => state.atomic(async () => {}), /synchronous/);
});

test('intent, quote and settlement identities cannot be overwritten', () => {
  const intent = { intent_id: 'immutable:intent', buyer: 'buyer:one' };
  state.recordIntent(intent, 'sig', Buffer.alloc(32, 1));
  state.recordIntent(intent, 'sig', Buffer.alloc(32, 1));
  assert.throws(() => state.recordIntent({ ...intent, buyer: 'buyer:two' }, 'sig'), /immutable/);
  assert.equal(state.getIntent(intent.intent_id).signerPublicKey.toString('hex'), '01'.repeat(32));
  const quote = { quote_id: 'immutable:quote', total: '10' };
  state.recordQuote(quote, intent.intent_id, 'sig');
  const detached = state.getQuote(quote.quote_id);
  detached.quote.total = '0';
  assert.equal(state.getQuote(quote.quote_id).quote.total, '10');
  assert.throws(
    () => state.recordQuote({ ...quote, total: '1' }, intent.intent_id, 'sig'),
    /immutable/,
  );
  const settlement = { settlement_id: 'immutable:settlement', amount: '10' };
  state.recordSettlement(settlement);
  state.recordSettlement(settlement);
  assert.throws(() => state.recordSettlement({ ...settlement, amount: '1' }), /immutable/);
});

test('funding is stable and binds the full quote identity, not its shared prefix', () => {
  const quote = {
    intent_id: 'icp_int_' + 'x'.repeat(50),
    quote_id: 'quote:one',
    iat: '2026-09-01T00:00:00Z',
    total: { amount: '10', currency: 'USDC' },
    settler: 'settler',
  };
  const funding = stubFundingInstructions(quote);
  assert.deepEqual(stubFundingInstructions(quote), funding);
  const second = stubFundingInstructions({ ...quote, quote_id: 'quote:two' });
  assert.notEqual(second.escrow_id, funding.escrow_id);
  assert.match(funding.args.quoteHash, /^0x[0-9a-f]{64}$/);
  assert.notEqual(second.args.quoteHash, funding.args.quoteHash);
  const firstReservation = state.reserveInventory(funding.escrow_id, [
    { sku: 'SKU-100', quantity: 1 },
  ]);
  const secondReservation = state.reserveInventory(second.escrow_id, [
    { sku: 'SKU-100', quantity: 1 },
  ]);
  assert.notEqual(firstReservation.reservation_id, secondReservation.reservation_id);
});
