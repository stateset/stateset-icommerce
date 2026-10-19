import assert from 'node:assert/strict';
import { test } from 'node:test';
import { cashBudgetRefusals } from '../../src/tui/report.js';
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
