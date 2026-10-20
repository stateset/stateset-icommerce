import { test } from 'node:test';
import assert from 'node:assert/strict';
import { reconcileCashFlow } from '../../src/simulation/cashflow.js';

const product = { sku: 'A', unitCost: '1.00', unitPrice: '3.00' };
const input = {
  startingCash: '2.00',
  days: 2,
  products: [product],
  timeline: [
    {
      day: 0,
      type: 'purchase_order_sent',
      sku: 'A',
      purchase: 'purchase-1',
      quantity: 2,
      amount: '2.00',
      cash: '0.00',
    },
    { day: 1, type: 'purchase_received', sku: 'A', quantity: 2 },
    { day: 1, type: 'demand', sku: 'A', sold: 2, cash: '6.00' },
    {
      day: 1,
      type: 'purchase_order_sent',
      sku: 'A',
      purchase: 'purchase-2',
      quantity: 2,
      amount: '2.00',
      cash: '4.00',
    },
    { day: 2, type: 'purchase_received', sku: 'A', quantity: 2 },
    { day: 2, type: 'demand', sku: 'A', sold: 0, cash: '4.00' },
  ],
  summary: {
    revenue: '6.00',
    purchasingSpend: '4.00',
    endingCash: '4.00',
    reserveBlockedPurchases: 0,
  },
};

test('cash flow exposes day-zero prepaid liquidity despite a higher ending cash balance', () => {
  const before = structuredClone(input);
  const result = reconcileCashFlow(input);
  assert.equal(result.minimumCash, '0.00');
  assert.deepEqual(result.minimumCashAt, {
    day: 0,
    type: 'purchase_order_sent',
    sku: 'A',
    purchase: 'purchase-1',
  });
  assert.deepEqual(result.daily, [
    {
      day: 0,
      openingCash: '2.00',
      salesReceipts: '0.00',
      purchasingSpend: '2.00',
      closingCash: '0.00',
      minimumCash: '0.00',
    },
    {
      day: 1,
      openingCash: '0.00',
      salesReceipts: '6.00',
      purchasingSpend: '2.00',
      closingCash: '4.00',
      minimumCash: '0.00',
    },
    {
      day: 2,
      openingCash: '4.00',
      salesReceipts: '0.00',
      purchasingSpend: '0.00',
      closingCash: '4.00',
      minimumCash: '4.00',
    },
  ]);
  assert.deepEqual(input, before);
});

test('opening cash participates in minima and quiet days carry forward without synthetic flows', () => {
  const result = reconcileCashFlow({
    ...input,
    startingCash: '0.000001',
    timeline: [],
    summary: {
      revenue: '0',
      purchasingSpend: '0',
      endingCash: '0.000001',
      reserveBlockedPurchases: 0,
    },
  });
  assert.equal(result.minimumCash, '0.000001');
  assert.deepEqual(result.minimumCashAt, { day: 0, type: 'opening' });
  assert.equal(result.daily.length, 3);
  for (const row of result.daily) {
    assert.equal(row.openingCash, '0.000001');
    assert.equal(row.closingCash, '0.000001');
    assert.equal(row.minimumCash, '0.000001');
    assert.equal(row.salesReceipts, '0.00');
    assert.equal(row.purchasingSpend, '0.00');
  }
});

test('cash-flow minima preserve sub-cent movements above JavaScript safe integer range', () => {
  const result = reconcileCashFlow({
    ...input,
    startingCash: '9007199254740993.000001',
    products: [{ ...product, unitCost: '0.000001', unitPrice: '0.000003' }],
    timeline: [
      { day: 1, type: 'demand', sku: 'A', sold: 1, cash: '9007199254740993.000004' },
      {
        day: 1,
        type: 'purchase_order_sent',
        sku: 'A',
        quantity: 4,
        amount: '0.000004',
        cash: '9007199254740993.000000',
      },
      { day: 2, type: 'demand', sku: 'A', sold: 1, cash: '9007199254740993.000003' },
    ],
    summary: {
      revenue: '0.000006',
      purchasingSpend: '0.000004',
      endingCash: '9007199254740993.000003',
      reserveBlockedPurchases: 0,
    },
  });
  assert.equal(result.minimumCash, '9007199254740993.00');
  assert.deepEqual(result.minimumCashAt, { day: 1, type: 'purchase_order_sent', sku: 'A' });
  assert.equal(result.daily[1].minimumCash, '9007199254740993.00');
  assert.equal(result.daily[2].closingCash, '9007199254740993.000003');
});

test('blocked attempts and missed deliveries neither spend nor refund cash', () => {
  const result = reconcileCashFlow({
    ...input,
    timeline: [
      ...input.timeline,
      { day: 2, type: 'purchase_budget_blocked', sku: 'A', required: '5.00', cash: '4.00' },
      { day: 2, type: 'purchase_constraint_blocked', sku: 'A', required: '1.00', cash: '4.00' },
      { day: 2, type: 'purchase_delivery_missed', sku: 'A', quantity: 0 },
    ],
  });
  assert.equal(result.daily[2].closingCash, '4.00');
  assert.equal(result.daily[2].purchasingSpend, '0.00');
  assert.equal(result.minimumCash, '0.00');
});

test('cash-flow reconciliation rejects corrupt event balances, amounts, chronology and summary totals', () => {
  for (const corrupt of [
    (value) => {
      value.timeline[0].cash = '0.01';
    },
    (value) => {
      value.timeline[0].amount = '1.99';
    },
    (value) => {
      value.timeline[2].sold = 1;
    },
    (value) => {
      value.timeline[2].sku = 'UNKNOWN';
    },
    (value) => {
      value.timeline[0].quantity = 2.5;
    },
    (value) => {
      value.timeline[0].day = -1;
    },
    (value) => {
      value.timeline[0].day = 1.5;
    },
    (value) => {
      value.timeline.reverse();
    },
    (value) => {
      value.timeline[0].type = 'refund';
    },
    (value) => {
      value.timeline[5].day = 3;
    },
    (value) => {
      value.summary.revenue = '6.01';
    },
    (value) => {
      value.summary.purchasingSpend = '3.99';
    },
    (value) => {
      value.summary.endingCash = '4.01';
    },
    (value) => {
      value.startingCash = '1.99';
    },
    (value) => {
      value.days = 0;
    },
  ]) {
    const broken = structuredClone(input);
    corrupt(broken);
    assert.throws(() => reconcileCashFlow(broken), /cash-flow reconciliation/);
  }
});

test('cash-flow reconciliation independently checks reserve blocks and rejects reserve breaches', () => {
  const reserved = {
    ...input,
    cashReserve: '1.00',
    timeline: [
      {
        day: 0,
        type: 'purchase_reserve_blocked',
        sku: 'A',
        quantity: 2,
        required: '2.00',
        cash: '2.00',
        cashReserve: '1.00',
        spendableCash: '1.00',
      },
      { day: 1, type: 'demand', sku: 'A', sold: 1, cash: '5.00' },
      { day: 1, type: 'purchase_order_sent', sku: 'A', quantity: 3, amount: '3.00', cash: '2.00' },
      { day: 2, type: 'demand', sku: 'A', sold: 1, cash: '5.00' },
    ],
    summary: {
      revenue: '6.00',
      purchasingSpend: '3.00',
      endingCash: '5.00',
      reserveBlockedPurchases: 1,
    },
  };
  assert.equal(reconcileCashFlow(reserved).minimumCash, '2.00');
  for (const corrupt of [
    (value) => {
      value.timeline[0].cash = '2.01';
    },
    (value) => {
      value.timeline[0].spendableCash = '1.000001';
    },
    (value) => {
      value.timeline[0].cashReserve = '0.00';
    },
    (value) => {
      value.timeline[0].required = '2.000001';
    },
    (value) => {
      value.timeline[0].quantity = 1;
    },
    (value) => {
      value.cashReserve = '0';
    },
    (value) => {
      value.summary.reserveBlockedPurchases = 0;
    },
    (value) => {
      value.timeline[0] = {
        ...value.timeline[0],
        type: 'purchase_order_sent',
        amount: '2.00',
        cash: '0.00',
      };
    },
  ]) {
    const broken = structuredClone(reserved);
    corrupt(broken);
    assert.throws(() => reconcileCashFlow(broken), /cash-flow reconciliation/);
  }
});

test('a reserve above opening cash does not fabricate funds or block zero-cost purchases', () => {
  const result = reconcileCashFlow({
    ...input,
    startingCash: '0.50',
    cashReserve: '1.00',
    products: [{ ...product, unitCost: '0' }],
    timeline: [
      { day: 0, type: 'purchase_order_sent', sku: 'A', quantity: 2, amount: '0', cash: '0.50' },
    ],
    summary: { revenue: '0', purchasingSpend: '0', endingCash: '0.50', reserveBlockedPurchases: 0 },
  });
  assert.equal(result.minimumCash, '0.50');
  assert.equal(result.daily[0].purchasingSpend, '0.00');
});

test('deferred purchase reviews carry cash without a purchase payment or refund', () => {
  const deferred = {
    ...input,
    timeline: [
      { day: 1, type: 'demand', sku: 'A', sold: 1, cash: '5.00' },
      { day: 1, type: 'purchase_review_deferred', sku: 'A', cash: '5.00' },
      { day: 2, type: 'demand', sku: 'A', sold: 0, cash: '5.00' },
      { day: 2, type: 'purchase_review_deferred', sku: 'A', cash: '5.00' },
    ],
    summary: {
      revenue: '3.00',
      purchasingSpend: '0.00',
      endingCash: '5.00',
      reserveBlockedPurchases: 0,
    },
  };
  const result = reconcileCashFlow(deferred);
  assert.equal(result.minimumCash, '2.00');
  assert.deepEqual(
    result.daily.map((day) => day.closingCash),
    ['2.00', '5.00', '5.00'],
  );
  assert.ok(result.daily.every((day) => day.purchasingSpend === '0.00'));
  deferred.timeline[1].cash = '4.00';
  assert.throws(() => reconcileCashFlow(deferred), /cash-flow reconciliation/);
});
