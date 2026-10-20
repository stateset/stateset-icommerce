import assert from 'node:assert/strict';
import { test } from 'node:test';
import { cashBudgetRefusals, purchasingExceptions } from '../../src/tui/report.js';
import { initialState, navigate } from '../../src/tui/model.js';
import { resultLines, render, panelLimit } from '../../src/tui/render.js';
import { width } from '../../src/tui/text.js';

const fixture = () => ({
  name: 'Baseline',
  currency: 'USD',
  days: 2,
  summary: {
    demanded: 2,
    sold: 0,
    lostSales: 2,
    endingCash: '9007199254740993.01',
    purchasingSpend: '10.00',
    grossMargin: '0.00',
  },
  cashFlow: {
    minimumCash: '9007199254740992.01',
    minimumCashAt: { day: 0, type: 'purchase_order_sent', sku: 'FIRST' },
  },
  inventory: [
    {
      sku: 'SECOND',
      sold: 0,
      lostSales: 2,
      available: 0,
      onOrder: 0,
      purchasedUnits: 0,
      receivedUnits: 0,
      stockoutDays: 2,
    },
  ],
  timeline: [
    { type: 'purchase_order_sent', day: 0, sku: 'FIRST', amount: '10.00' },
    {
      type: 'purchase_budget_blocked',
      day: 0,
      sku: 'SECOND',
      required: '9007199254740993.02',
      cash: '9007199254740993.01',
    },
    { type: 'demand', day: 1, sku: 'SECOND', sold: 0 },
    { type: 'purchase_budget_blocked', day: 1, sku: 'SECOND', required: '10.00', cash: '0.00' },
    { type: 'purchase_budget_blocked', day: 2, sku: 'SECOND', required: '10.00', cash: '0.00' },
  ],
});
const stateFor = (report) => ({
  ...navigate(initialState(), 5),
  panel: 'results',
  products: [{ sku: 'CURRENT-DRAFT' }],
  sim: {
    preset: 'Steady demand',
    seed: 42,
    controls: { days: 2, startingCash: '10.00' },
    result: {
      inputs: [
        {
          startingCash: '10.00',
          products: [
            {
              sku: 'EXECUTED-FIRST',
              initialStock: 0,
              unitPrice: '2.00',
              unitCost: '1.00',
              demand: { min: 0, max: 0 },
              reorderPoint: 0,
              targetStock: 10,
              leadTimeDays: 1,
            },
          ],
        },
      ],
      results: [report],
    },
  },
});

test('budget refusals count repeated attempts, retain the first recorded exact requirement and never modify the report', () => {
  const report = fixture();
  const before = structuredClone(report);
  const refusals = cashBudgetRefusals(report);
  assert.equal(refusals.length, 1);
  assert.equal(refusals[0].sku, 'SECOND');
  assert.equal(refusals[0].attempts, 3);
  assert.equal(refusals[0].first.day, 0);
  assert.equal(refusals[0].first.required, '9007199254740993.02');
  assert.equal(refusals[0].first.cash, '9007199254740993.01');
  assert.deepEqual(report, before);
});

test('comparison explains recorded cash, stockouts and executed priority with exact money intact', () => {
  const state = stateFor(fixture());
  const text = resultLines(state, 160).join('\n');
  assert.match(text, /Lowest cash USD 9007199254740992\.01 · day 0/);
  assert.match(text, /Purchased 0 · received 0 · stockout days 2/);
  assert.match(text, /SECOND: 3 blocked attempt\(s\)/);
  assert.match(text, /whole order USD 9007199254740993\.02; cash USD 9007199254740993\.01/);
  assert.match(text, /not distinct orders/);
  assert.match(text, /Priority 1 · EXECUTED-FIRST/);
  assert.doesNotMatch(text, /CURRENT-DRAFT/);
  for (const [columns, rows] of [
    [40, 14],
    [55, 20],
    [140, 44],
  ]) {
    const screens = [];
    for (let panelOffset = 0; panelOffset <= panelLimit(state, columns, rows); panelOffset++) {
      const screen = render({ ...state, panelOffset }, columns, rows, { color: false });
      assert.ok(screen.split('\n').every((line) => width(line) <= columns - 1));
      assert.ok(screen.split('\n').length <= rows - 1);
      screens.push(screen);
    }
    assert.match(screens.join('\n'), /CASH-BUDGET REFUSALS/);
    assert.match(screens.join('\n'), /stockout/);
    assert.match(screens.join('\n'), /days 2/);
  }
});

test('missing report evidence stays unavailable and differs from an observed zero', () => {
  const report = fixture();
  delete report.timeline;
  delete report.cashFlow;
  delete report.inventory[0].stockoutDays;
  assert.equal(cashBudgetRefusals(report), null);
  let text = resultLines(stateFor(report), 120).join('\n');
  assert.match(text, /Lowest cash Unavailable/);
  assert.match(text, /Timeline unavailable; refusal counts are unknown/);
  assert.match(text, /stockout days n\/a/);
  assert.doesNotMatch(text, /No cash-budget refusals/);
  report.timeline = [];
  assert.deepEqual(cashBudgetRefusals(report), []);
  text = resultLines(stateFor(report), 120).join('\n');
  assert.match(text, /No cash-budget refusals recorded/);
});

const exceptionReport = () => ({
  ...fixture(),
  timeline: [
    {
      type: 'purchase_constraint_blocked',
      day: 0,
      sku: 'PACK',
      reason: 'maximum_inventory_position',
      inventoryPosition: 2,
      requiredPosition: 10,
      maximumInventoryPosition: 9,
      requestedQuantity: 1,
      quantity: 8,
      extraUnits: 7,
      minimumOrderQuantity: 5,
      orderMultiple: 4,
      required: '0.800008',
      cash: '0.00',
    },
    {
      type: 'purchase_reserve_blocked',
      day: 0,
      sku: 'PACK',
      requestedQuantity: 10,
      quantity: 10,
      extraUnits: 0,
      minimumOrderQuantity: 1,
      orderMultiple: 1,
      required: '9007199254740993.02',
      cash: '9007199254740993.03',
      cashReserve: '0.02',
      spendableCash: '9007199254740993.01',
    },
    {
      type: 'purchase_review_deferred',
      day: 1,
      sku: 'PACK',
      inventoryPosition: 0,
      reviewIntervalDays: 3,
      nextReviewDay: 3,
      cash: '0.00',
    },
    { type: 'purchase_review_deferred', day: 2, sku: 'PACK', nextReviewDay: 3 },
    { type: 'purchase_reserve_blocked', day: 2, sku: 'PACK', required: '1.00', cash: '0.00' },
    { type: 'purchase_budget_blocked', day: 2, sku: 'PACK', required: '2.00', cash: '0.00' },
    { type: 'purchase_order_sent', day: 2, sku: 'PACK', quantity: 1 },
  ],
});

test('purchasing explanations separate reasons for one SKU without deduplicating days or changing recorded evidence', () => {
  const report = exceptionReport();
  const before = structuredClone(report);
  const groups = purchasingExceptions(report);
  assert.deepEqual(
    groups.map((entry) => [entry.first.type, entry.attempts, entry.first.day]),
    [
      ['purchase_constraint_blocked', 1, 0],
      ['purchase_reserve_blocked', 2, 0],
      ['purchase_review_deferred', 2, 1],
    ],
  );
  assert.equal(cashBudgetRefusals(report)[0].attempts, 1);
  const state = stateFor(report);
  state.sim.result.inputs[0].cashReserve = '0.02';
  state.sim.result.inputs[0].products[0].purchasing = {
    minimumOrderQuantity: 5,
    orderMultiple: 4,
    maximumInventoryPosition: 9,
    reviewIntervalDays: 3,
  };
  const text = resultLines(state, 180).join('\n');
  assert.match(text, /PACK: 1 capacity-blocked attempt\(s\)/);
  assert.match(text, /Inventory position with proposed order 10; capacity limit 9/);
  assert.match(text, /Target shortfall 1; rounded order 8; extra units 7/);
  assert.match(text, /Supplier minimum 5; case pack 4/);
  assert.match(text, /First block day 0: whole order USD 0\.800008; cash USD 0\.00/);
  assert.match(text, /PACK: 2 reserve-blocked attempt\(s\)/);
  assert.match(text, /Reserved cash USD 0\.02; spendable cash USD 9007199254740993\.01/);
  assert.match(text, /whole order USD 9007199254740993\.02; cash USD 9007199254740993\.03/);
  assert.match(text, /PACK: 2 deferred review attempt\(s\)/);
  assert.match(text, /First deferral day 1: inventory position 0/);
  assert.match(text, /Review interval 3 day\(s\); next review day 3/);
  assert.match(text, /Next review is outside this report's horizon/);
  assert.match(text, /does not guarantee a purchase/);
  assert.match(text, /Cash reserve USD 0\.02/);
  assert.match(text, /Maximum inventory position 9/);
  assert.match(text, /Purchase review interval 3 day\(s\)/);
  assert.deepEqual(report, before);
});

test('missing purchasing evidence and currency are never replaced by draft values or zero counts', () => {
  const report = exceptionReport();
  delete report.currency;
  delete report.timeline[0].quantity;
  report.timeline[0].reason = 'future_constraint';
  let text = resultLines(stateFor(report), 180).join('\n');
  assert.match(text, /Recorded constraint: future_constraint/);
  assert.match(text, /rounded order Unavailable/);
  assert.match(text, /Reserved cash Unavailable; spendable cash Unavailable/);
  assert.doesNotMatch(text, /USD|capacity limit/);
  delete report.timeline;
  assert.equal(purchasingExceptions(report), null);
  text = resultLines(stateFor(report), 180).join('\n');
  assert.match(text, /Timeline unavailable; constraint and review counts are unknown/);
  report.timeline = [];
  assert.deepEqual(purchasingExceptions(report), []);
  text = resultLines(stateFor(report), 180).join('\n');
  assert.match(text, /No capacity blocks, reserve blocks or review deferrals recorded/);
});

test('each plan keeps its own purchasing explanations and compact scrolling reaches the evidence', () => {
  const state = stateFor(exceptionReport());
  state.sim.result.results.push({ ...fixture(), name: 'Candidate', timeline: [] });
  const text = resultLines(state, 180).join('\n');
  const candidate = text.split('Candidate · PURCHASE CONSTRAINTS & REVIEWS')[1];
  assert.ok(candidate);
  assert.match(candidate, /No capacity blocks, reserve blocks or review deferrals recorded/);
  assert.doesNotMatch(candidate, /PACK:/);
  const screens = [];
  for (let panelOffset = 0; panelOffset <= panelLimit(state, 20, 8); panelOffset++) {
    const screen = render({ ...state, panelOffset }, 20, 8, { color: false });
    assert.ok(screen.split('\n').every((line) => width(line) <= 19));
    assert.ok(screen.split('\n').length <= 7);
    screens.push(screen);
  }
  assert.match(screens.join('\n'), /spendable/);
  assert.match(screens.join('\n'), /review/);
});
