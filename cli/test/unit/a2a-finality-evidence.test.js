import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createFinalityTracker } from '../../src/a2a/settlement-finality.js';
import { a2aObservabilityTools } from '../../src/tools/a2a-observability.js';

const tool = a2aObservabilityTools.find(
  (candidate) => candidate.name === 'a2a_settlement_finality_metrics',
);

function payments(values, overrides = {}) {
  return values.map((confirmations, index) => ({
    id: `payment-${index}`,
    tx_hash: `transaction-${index}`,
    network: 'base',
    status: 'completed',
    block_number: 100,
    sender_address: 'buyer',
    metadata: JSON.stringify({ confirmations }),
    ...overrides,
  }));
}

async function metrics(rows, tracker) {
  return tool.handler({
    commerce: {
      ...(tracker ? { _finalityTracker: tracker } : {}),
      a2a: () => ({ listPayments: () => rows }),
    },
    params: { network: 'base' },
  });
}

test('completed status cannot fabricate missing or insufficient confirmations in MCP metrics', async () => {
  const result = await metrics(payments([undefined, 0, 1]));
  assert.equal(result.totalTracked, 3);
  assert.equal(result.totalFinal, 0);
  assert.equal(result.pendingCount, 3);
  assert.equal(result.finalityRate, 0);
});

test('fractional and unsafe persisted confirmation counts are not finality evidence', async () => {
  const result = await metrics(
    payments([2.5, Number.MAX_SAFE_INTEGER + 1, '2.5', 'Infinity'], { status: 'submitted' }),
  );
  assert.equal(result.totalFinal, 0);
  assert.equal(result.pendingCount, 4);
});

test('missing inclusion heights and overflowing observed windows cannot become final', async () => {
  const rows = [
    ...payments([2], { id: 'missing', block_number: null }),
    ...payments([2], { id: 'overflow', block_number: Number.MAX_SAFE_INTEGER }),
  ];
  const result = await metrics(rows);
  assert.equal(result.totalFinal, 0);
  assert.equal(result.pendingCount, 2);
});

test('persisted MCP metrics honor the same operator depth as the tracker', async () => {
  const tracker = createFinalityTracker({ confirmationRequirements: { base: 3 } });
  const rows = payments([2]);
  assert.equal((await metrics(rows, tracker)).totalFinal, 0);
  rows[0].metadata = JSON.stringify({ confirmations: 3 });
  assert.equal((await metrics(rows, tracker)).totalFinal, 1);
});

test('validated persisted observations still finalize and failed rows remain failed', async () => {
  const rows = payments([2, '2', 1]);
  rows.push(...payments([2], { id: 'failed', status: 'failed' }));
  const result = await metrics(rows);
  assert.equal(result.totalFinal, 2);
  assert.equal(result.totalFailed, 1);
  assert.equal(result.pendingCount, 1);
  assert.equal(result.finalityRate, 0.5);
});
