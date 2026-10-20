import assert from 'node:assert/strict';
import { test } from 'node:test';
import { cashMovement, cashMovementDelta, cashDelta } from '../../src/tui/report.js';
import { initialState, navigate } from '../../src/tui/model.js';
import { render, resultLines, panelLimit } from '../../src/tui/render.js';

const report = (startingCash = '10.00', endingCash = '0.00', name = 'Baseline') => ({
  name,
  currency: 'USD',
  days: 2,
  summary: {
    startingCash,
    endingCash,
    purchasingSpend: '10.00',
    grossMargin: '0.00',
    endingInventoryValue: '10.00',
    inTransitInventoryValue: '0.00',
    sold: 0,
    demanded: 0,
    lostSales: 0,
  },
  inventory: [],
  timeline: [],
});
const comparison = () => ({
  ...navigate(initialState(), 5),
  panel: 'results',
  planning: { ...initialState().planning, startingCash: '999999.99' },
  sim: {
    preset: 'Cash constraint',
    seed: 42,
    controls: { startingCash: '888888.88' },
    result: { results: [report(), report('0.50', '0.50', 'Cash constraint')] },
  },
});

test('cash movement uses exact recorded balances for large amounts, fractional cents, negative changes and zero', () => {
  assert.equal(cashMovement(report('9007199254740993.01', '9007199254740993.02')), '0.01');
  assert.equal(cashMovement(report('0.0005', '0.0004')), '-0.0001');
  assert.equal(cashMovement(report('10.00', '0.00')), '-10.00');
  assert.equal(cashMovement(report('0.00', '0.00')), '0.00');
  for (const value of [undefined, null, 0, '1e3', 'broken']) {
    const invalid = report();
    invalid.summary.startingCash = value;
    assert.equal(cashMovement(invalid), null);
    invalid.summary.startingCash = '10.00';
    invalid.summary.endingCash = value;
    assert.equal(cashMovement(invalid), null);
  }
});

test('movement delta separates funding from cash changes and refuses mismatched or missing evidence', () => {
  const baseline = report(),
    candidate = report('0.50', '0.50');
  assert.equal(cashDelta(candidate, baseline, 'endingCash'), '+0.50');
  assert.equal(cashMovementDelta(candidate, baseline), '+10.00');
  assert.equal(cashMovementDelta(baseline, candidate), '-10.00');
  assert.equal(
    cashMovementDelta(
      report('9007199254740993.01', '9007199254740993.02'),
      report('10.00', '10.00'),
    ),
    '+0.01',
  );
  for (const invalid of [
    { ...candidate, currency: 'EUR' },
    { ...candidate, currency: '' },
    { ...candidate, currency: undefined },
    { ...candidate, days: 3 },
    { ...candidate, days: undefined },
    { ...candidate, summary: { endingCash: '0.50' } },
  ])
    assert.equal(cashMovementDelta(invalid, baseline), null);
  for (const currency of [undefined, null, '']) {
    const unknown = { ...baseline, currency };
    assert.equal(cashMovement(unknown), null);
    assert.equal(cashMovementDelta(unknown, unknown), null);
    assert.equal(cashDelta(unknown, unknown, 'endingCash'), 'n/a');
  }
});

test('comparison shows executed cash and inventory values without inferring profit or using the current draft', () => {
  const state = comparison(),
    before = structuredClone(state);
  let text = resultLines(state, 160).join('\n');
  assert.match(text, /Opening cash USD 10\.00/);
  assert.match(text, /Opening cash USD 0\.50/);
  assert.match(text, /Cash movement USD -10\.00/);
  assert.match(text, /Cash movement USD 0\.00/);
  assert.match(text, /Change vs baseline: ending cash \+0\.50/);
  assert.match(text, /cash movement \+10\.00/);
  assert.match(text, /gross margin \+0\.00/);
  assert.match(text, /Inventory at cost: on hand USD 10\.00 · in transit USD 0\.00/);
  assert.match(text, /not profit/);
  assert.doesNotMatch(text, /999999\.99|888888\.88/);
  assert.deepEqual(state, before);
  delete state.sim.result.results[0].summary.startingCash;
  delete state.sim.result.results[0].summary.endingInventoryValue;
  delete state.sim.result.results[0].summary.inTransitInventoryValue;
  text = resultLines(state, 160).join('\n');
  assert.match(text, /Opening cash Unavailable/);
  assert.match(text, /Cash movement Unavailable/);
  assert.match(text, /cash movement Unavailable/);
  assert.match(text, /on hand Unavailable · in transit Unavailable/);
  assert.doesNotMatch(text, /999999\.99|888888\.88/);
  const unknownCurrency = comparison();
  for (const item of unknownCurrency.sim.result.results) delete item.currency;
  text = resultLines(unknownCurrency, 160).join('\n');
  assert.match(text, /Opening cash Unavailable/);
  assert.match(text, /Ending cash Unavailable/);
  assert.match(text, /Cash movement Unavailable/);
  assert.match(text, /on hand Unavailable · in transit Unavailable/);
  assert.doesNotMatch(text, /USD/);
});

test('cash movement and cost evidence remain reachable by scrolling in compact, narrow and wide panes', () => {
  const state = comparison();
  for (const [columns, rows] of [
    [20, 8],
    [40, 14],
    [140, 44],
  ]) {
    const frames = [];
    for (let panelOffset = 0; panelOffset <= panelLimit(state, columns, rows); panelOffset++)
      frames.push(render({ ...state, panelOffset }, columns, rows, { color: false }));
    const text = frames.join('\n').replace(/\s+/g, ' ');
    assert.match(text, /Cash movement/);
    assert.match(text, /-10\.00/);
    assert.match(text, /\+10\.00/);
    assert.match(text, /not profit/);
    assert.match(text, /in transit/);
  }
});
