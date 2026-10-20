import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {
  initialState,
  navigate,
  handleKey,
  simulationPlans,
  PRESETS,
} from '../../src/tui/model.js';
import { purchasingRulesMatch, comparisonEvidence } from '../../src/tui/report.js';
import { resultLines, render } from '../../src/tui/render.js';
import { TerminalWorkspace, runRecord, useRunAssumptions } from '../../src/tui/workspace.js';
import { importedScenario } from '../../src/tui/import.js';
import { inventoryPlanSchema, inventoryPlanComparisonSchema } from '../../src/simulation/schema.js';
import { projectDemand } from '../../src/simulation/demand.js';
import { projectSupply } from '../../src/simulation/supply.js';

const product = (sku = 'COFFEE') => ({
  sku,
  initialStock: 1,
  unitPrice: '2.00',
  unitCost: '1.00',
  demand: { min: 1, max: 1 },
  reorderPoint: 0,
  targetStock: 3,
  leadTimeDays: 1,
  purchasing: {
    minimumOrderQuantity: 1,
    orderMultiple: 1,
    reviewIntervalDays: 3,
    maximumInventoryPosition: 4,
  },
});
const settings = () => ({
  presetId: 'daily-review',
  seed: 42,
  controls: { ...initialState().planning, days: 3, startingCash: '10.00', cashReserve: '1.00' },
  products: [product()],
});
function report(input) {
  const parsed = inventoryPlanSchema.parse(input);
  return {
    name: input.name,
    days: input.days,
    currency: input.currency,
    summary: {
      startingCash: input.startingCash,
      endingCash: '10.00',
      demanded: 3,
      sold: 1,
      lostSales: 2,
    },
    assumptions: { products: parsed.products, cashReserve: input.cashReserve },
    demandForecast: projectDemand(parsed),
    supplyProjection: projectSupply(parsed),
    inventory: [],
    timeline: [],
  };
}
const plansFor = (s = settings()) => simulationPlans(s.presetId, s.seed, s.controls, s.products);

test('daily comparison changes only candidate calendars and preserves every other input without mutating baseline or draft', () => {
  const s = settings();
  s.products.push({
    ...product('MATCHA'),
    purchasing: { minimumOrderQuantity: 2, orderMultiple: 2, maximumInventoryPosition: 8 },
  });
  const before = structuredClone(s);
  const plans = plansFor(s);
  inventoryPlanComparisonSchema.parse(plans);
  assert.equal(plans[0].name, 'Baseline');
  assert.equal(plans[1].name, 'Daily reviews');
  assert.equal(plans[0].products[0].purchasing.reviewIntervalDays, 3);
  assert.ok(plans[1].products.every((p) => !Object.hasOwn(p.purchasing, 'reviewIntervalDays')));
  const expected = structuredClone(plans[0]);
  expected.name = 'Daily reviews';
  for (const p of expected.products) delete p.purchasing.reviewIntervalDays;
  assert.deepEqual(plans[1], expected);
  assert.deepEqual(s, before);
  plans[1].products[0].purchasing.maximumInventoryPosition = 99;
  assert.equal(plans[0].products[0].purchasing.maximumInventoryPosition, 4);
  assert.deepEqual(s, before);
  assert.throws(() => simulationPlans('daily-review', 42), /needs a selected SKU.*above 1 day/);
  const daily = structuredClone(before.products);
  delete daily[0].purchasing.reviewIntervalDays;
  assert.throws(
    () => simulationPlans('daily-review', 42, s.controls, daily),
    /needs a selected SKU/,
  );
});

test('purchasing rule checks compare SKU facts independently of priority and normalize documented defaults', () => {
  const baseline = report(plansFor()[0]);
  baseline.assumptions.products.push({ ...product('MATCHA') });
  const candidate = structuredClone(baseline);
  candidate.assumptions.products.reverse();
  assert.equal(purchasingRulesMatch([baseline, candidate]), true);
  for (const key of [
    'minimumOrderQuantity',
    'orderMultiple',
    'reviewIntervalDays',
    'maximumInventoryPosition',
  ]) {
    const changed = structuredClone(baseline);
    changed.assumptions.products[0].purchasing[key] += 1;
    assert.equal(purchasingRulesMatch([baseline, changed]), false, key);
  }
  for (const key of ['reorderPoint', 'targetStock']) {
    const changed = structuredClone(baseline);
    changed.assumptions.products[0][key] += 1;
    assert.equal(purchasingRulesMatch([baseline, changed]), false, key);
  }
  const legacy = report(plansFor()[0]);
  delete legacy.assumptions.products[0].purchasing;
  const explicit = structuredClone(legacy);
  explicit.assumptions.products[0].purchasing = {
    minimumOrderQuantity: 1,
    orderMultiple: 1,
    reviewIntervalDays: 1,
  };
  assert.equal(purchasingRulesMatch([legacy, explicit]), true);
});

test('missing, invalid and unknown purchasing evidence remains unavailable instead of implying equivalence', () => {
  const baseline = report(plansFor()[0]);
  assert.equal(purchasingRulesMatch([baseline]), null);
  for (const purchasing of [
    null,
    [],
    { futureRule: 1 },
    { reviewIntervalDays: 0 },
    { orderMultiple: '1' },
    { maximumInventoryPosition: null },
    { reviewIntervalDays: 61 },
  ]) {
    const candidate = structuredClone(baseline);
    candidate.assumptions.products[0].purchasing = purchasing;
    assert.equal(purchasingRulesMatch([baseline, candidate]), null);
  }
  const missing = structuredClone(baseline);
  delete missing.assumptions;
  assert.equal(purchasingRulesMatch([baseline, missing]), null);
  const duplicate = structuredClone(baseline);
  duplicate.assumptions.products.push(structuredClone(duplicate.assumptions.products[0]));
  assert.equal(purchasingRulesMatch([baseline, duplicate]), null);
});

test('results distinguish changed purchasing rules from matching projected paths and show executed review intervals', () => {
  const inputs = plansFor(),
    reports = inputs.map(report);
  const before = structuredClone(reports);
  assert.equal(comparisonEvidence(reports).sameDemandPaths, true);
  assert.equal(comparisonEvidence(reports).sameSupplyPaths, true);
  assert.equal(purchasingRulesMatch(reports), false);
  const state = {
    ...navigate(initialState(), 5),
    panel: 'results',
    sim: { preset: 'Daily reviews', seed: 42, result: { inputs, results: reports } },
  };
  const text = resultLines(state, 160).join('\n');
  assert.match(text, /Reorder, sizing & review rules different/);
  assert.match(text, /Projected paths: demand same · supply same/);
  assert.match(text, /Purchase review interval 3 day\(s\)/);
  assert.match(text, /Purchase review interval 1 day\(s\)/);
  assert.deepEqual(reports, before);
  delete reports[1].assumptions;
  assert.match(resultLines(state, 160).join('\n'), /Reorder, sizing & review rules Unavailable/);
});

test('new comparison survives workspace restart, imported assumptions and editable replay without changing stored reports', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-daily-review-'));
  try {
    const s = settings(),
      inputs = plansFor(s);
    const run = runRecord({
      ...s,
      preset: 'Daily reviews',
      result: { success: true, inputs, results: inputs.map(report) },
    });
    const before = structuredClone(run);
    const workspace = new TerminalWorkspace(directory);
    await workspace.saveScenario('Review cadence', s);
    await workspace.appendRun(run);
    const loaded = await new TerminalWorkspace(directory).load();
    assert.equal(loaded.scenarios[0].presetId, 'daily-review');
    const preview = importedScenario({ format: 'stateset-tui-run', version: 1, ...loaded.runs[0] });
    assert.equal(preview.settings.presetId, 'daily-review');
    assert.equal(preview.settings.products[0].purchasing.reviewIntervalDays, 3);
    const draft = useRunAssumptions(navigate(initialState(), 5), loaded.runs[0]);
    assert.equal(draft.selectedPreset, 'daily-review');
    assert.equal(draft.products[0].purchasing.reviewIntervalDays, 3);
    assert.deepEqual(
      simulationPlans(draft.selectedPreset, draft.seed, draft.planning, draft.products),
      inputs,
    );
    assert.equal(
      handleKey({ ...draft, sim: { ...run, running: false } }, 'R', {}).effect,
      'replay',
    );
    draft.products[0].purchasing.reviewIntervalDays = 7;
    assert.deepEqual(loaded.runs[0], before);
    assert.deepEqual(run, before);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('compact lab selection reaches the new comparison without changing existing preset indices', () => {
  assert.deepEqual(
    PRESETS.slice(0, 4).map((p) => p.id),
    ['baseline', 'spike', 'delay', 'cash'],
  );
  const state = handleKey(navigate(initialState(), 5), '', { name: 'end' }).state;
  assert.equal(state.selectedPreset, 'daily-review');
  assert.match(render(state, 20, 8, { color: false }), /Daily reviews/);
  assert.equal(handleKey(state, 's', {}).effect, 'simulate');
});
