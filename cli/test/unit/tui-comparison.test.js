import assert from 'node:assert/strict';
import { test } from 'node:test';
import { comparisonEvidence } from '../../src/tui/report.js';
import { initialState, navigate } from '../../src/tui/model.js';
import { resultLines, render, panelLimit } from '../../src/tui/render.js';
import { inventoryPlanSchema } from '../../src/simulation/schema.js';
import { projectDemand } from '../../src/simulation/demand.js';
import { projectSupply } from '../../src/simulation/supply.js';

const plan = () =>
  inventoryPlanSchema.parse({
    name: 'Baseline',
    seed: 42,
    days: 2,
    startingCash: '10.00',
    products: ['A', 'B'].map((sku) => ({
      sku,
      initialStock: 0,
      unitPrice: '2.00',
      unitCost: '1.00',
      demand: { min: 0, max: 0 },
      reorderPoint: 0,
      targetStock: 10,
      leadTimeDays: 1,
    })),
  });
const report = (input) => ({
  name: input.name,
  currency: input.currency,
  days: input.days,
  summary: {
    startingCash: input.startingCash,
    endingCash: '0.00',
    grossMargin: '0.00',
    purchasingSpend: '10.00',
    sold: 0,
    demanded: 0,
    lostSales: 0,
  },
  assumptions: { products: input.products },
  demandForecast: projectDemand(input),
  supplyProjection: projectSupply(input),
  inventory: [],
  timeline: [],
});
const stateFor = (inputs) => ({
  ...navigate(initialState(), 5),
  panel: 'results',
  products: [{ sku: 'CURRENT-DRAFT' }],
  sim: {
    preset: 'Cash constraint',
    seed: 42,
    controls: { days: 30, startingCash: '999.99' },
    result: { inputs, results: inputs.map(report) },
  },
});

test('engine comparison checks distinguish exact opening cash changes from matching projected paths', () => {
  const baseline = plan();
  const candidate = { ...structuredClone(baseline), name: 'Cash constraint', startingCash: '0.50' };
  const context = comparisonEvidence([report(baseline), report(candidate)]);
  assert.equal(context.sameOpeningCash, false);
  assert.equal(context.sameHorizon, true);
  assert.equal(context.sameDemandPaths, true);
  assert.equal(context.sameSupplyPaths, true);
  baseline.startingCash = '9007199254740993.01';
  candidate.startingCash = '9007199254740993.02';
  assert.equal(comparisonEvidence([report(baseline), report(candidate)]).sameOpeningCash, false);
});

test('priority and lead-time changes use engine facts without treating product reordering as a different demand path', () => {
  const baseline = plan();
  const candidate = structuredClone(baseline);
  candidate.products.reverse();
  let context = comparisonEvidence([report(baseline), report(candidate)]);
  assert.equal(context.sameProductPriority, false);
  assert.equal(context.sameOpeningInventory, true);
  assert.equal(context.sameUnitEconomics, true);
  assert.equal(context.sameDemandPaths, true);
  assert.equal(context.sameSupplyPaths, true);
  candidate.products[0].leadTimeDays = 4;
  context = comparisonEvidence([report(baseline), report(candidate)]);
  assert.equal(context.sameSupplyPaths, false);
  assert.equal(context.sameDemandPaths, true);
});

test('saved inputs for both plans remain visible and exclude subsequently edited draft assumptions', () => {
  const baseline = plan();
  const candidate = structuredClone(baseline);
  candidate.name = 'Cash constraint';
  candidate.startingCash = '0.50';
  candidate.products[0].leadTimeDays = 4;
  const state = stateFor([baseline, candidate]);
  const text = resultLines(state, 160).join('\n');
  assert.match(text, /Opening cash different · horizon same/);
  assert.match(text, /Projected paths: demand same · supply different/);
  assert.match(text, /Baseline · 2 days · USD 10\.00 opening cash/);
  assert.match(text, /Cash constraint · 2 days · USD 0\.50 opening cash/);
  assert.match(text, /lead 1d/);
  assert.match(text, /lead 4d/);
  assert.doesNotMatch(text, /999\.99|CURRENT-DRAFT|30 days/);
  assert.match(text, /do not establish equal assumptions/);
  for (const [columns, rows] of [
    [40, 14],
    [55, 20],
  ]) {
    const screens = [];
    for (let panelOffset = 0; panelOffset <= panelLimit(state, columns, rows); panelOffset++)
      screens.push(render({ ...state, panelOffset }, columns, rows, { color: false }));
    const visible = screens.join('\n');
    assert.match(visible, /COMPARISON CHECKS/);
    assert.match(visible, /Cash constraint/);
    assert.match(visible, /0\.50/);
  }
});

test('missing comparison evidence or mixed currencies stays unavailable instead of implying equivalence', () => {
  const baseline = plan();
  const a = report(baseline),
    b = report(baseline);
  delete b.demandForecast;
  assert.equal(comparisonEvidence([a, b]), null);
  assert.equal(comparisonEvidence([a, { ...report(baseline), currency: 'EUR' }]), null);
  assert.equal(comparisonEvidence([a, null]), null);
  assert.equal(comparisonEvidence([a]), null);
  const state = stateFor([baseline, structuredClone(baseline)]);
  state.sim.result.results[1] = b;
  delete state.sim.result.inputs;
  const text = resultLines(state, 160).join('\n');
  assert.match(text, /Saved comparison evidence is unavailable/);
  assert.match(text, /Saved input plans are unavailable/);
  assert.doesNotMatch(text, /Opening cash same|999\.99/);
});
