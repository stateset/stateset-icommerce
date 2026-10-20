import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { initialState, navigate, handleKey, simulationPlans } from '../../src/tui/model.js';
import { planningControls, planningEditor, planningFields } from '../../src/tui/planning.js';
import { TerminalWorkspace, runRecord, useRunAssumptions } from '../../src/tui/workspace.js';
import { importedScenario } from '../../src/tui/import.js';
import { render, importReviewLines, scenarioUpdateLines } from '../../src/tui/render.js';

const product = {
  sku: 'COFFEE',
  initialStock: 0,
  unitPrice: '2.00',
  unitCost: '1.00',
  demand: { min: 0, max: 0 },
  reorderPoint: 0,
  targetStock: 10,
  leadTimeDays: 1,
  purchasing: { minimumOrderQuantity: 1, orderMultiple: 1 },
};
const settings = () => ({
  presetId: 'cash',
  seed: 77,
  controls: { ...initialState().planning, days: 2, startingCash: '10.00', cashReserve: '3.25' },
  products: [structuredClone(product)],
});
const report = () => ({ name: 'Baseline', summary: {}, inventory: [], timeline: [] });
function record(s) {
  return runRecord({
    ...s,
    preset: s.presetId === 'cash' ? 'Cash constraint' : 'Steady demand',
    result: {
      success: true,
      inputs: simulationPlans(s.presetId, s.seed, s.controls, s.products),
      results: [report()],
    },
  });
}

test('reserve is an exact bounded decimal, independent of opening cash, with a legacy zero default', () => {
  assert.equal(
    planningControls({ startingCash: '0', cashReserve: '999999.99' }).cashReserve,
    '999999.99',
  );
  assert.equal(planningControls({ cashReserve: '3.2' }).cashReserve, '3.20');
  for (const cashReserve of [0, 0.01, null, '', '-1', '1e2', '0.001', '1000000', '01'])
    assert.throws(() => planningControls({ cashReserve }), /Cash reserve must be a decimal string/);
  const legacy = { ...initialState().planning };
  delete legacy.cashReserve;
  assert.equal(planningControls(legacy).cashReserve, '0.00');
  assert.equal(planningEditor(legacy).draft.cashReserve, '0.00');
});

test('reserve editing works for selected SKUs and demo recipes, validates atomically and survives resize', () => {
  for (const products of [[], [product]]) {
    let state = handleKey({ ...navigate(initialState(), 5), products }, 'e', {}).state;
    const fields = planningFields(products);
    const reserveIndex = fields.findIndex(({ key }) => key === 'cashReserve');
    for (let i = 0; i < reserveIndex; i++) state = handleKey(state, '', { name: 'tab' }).state;
    state = handleKey(state, '', { name: 'u', ctrl: true }).state;
    state = handleKey(state, '3.25', {}).state;
    assert.equal(handleKey(state, 'q', {}).effect, undefined);
    assert.match(render(state, 20, 8, { color: false }), /Cash reserve/);
    assert.match(render(state, 20, 8, { color: false }), /3\.25▏/);
    assert.match(render(state, 140, 44, { color: false }), /withheld from purchases/);
    const saved = handleKey(state, '', { name: 'return' });
    assert.equal(saved.effect, undefined);
    assert.equal(saved.state.input, null);
    assert.equal(saved.state.planning.cashReserve, '3.25');
    state = handleKey(saved.state, 'e', {}).state;
    state.input.draft.cashReserve = '0.001';
    state = handleKey(state, '', { name: 's', ctrl: true }).state;
    assert.match(state.input.error, /Cash reserve/);
    assert.equal(state.planning.cashReserve, '3.25');
    assert.equal(handleKey(state, '', { name: 'escape' }).state.planning.cashReserve, '3.25');
  }
});

test('all comparisons preserve the shared reserve while cash shocks can put opening cash below it', () => {
  const s = settings();
  for (const preset of ['baseline', 'spike', 'delay', 'cash']) {
    const plans = simulationPlans(preset, s.seed, s.controls, s.products);
    assert.ok(plans.every((plan) => plan.cashReserve === '3.25'));
    if (preset === 'cash') {
      assert.equal(plans[1].startingCash, '0.50');
      assert.equal(plans[0].startingCash, '10.00');
    }
  }
  assert.ok(!Object.hasOwn(simulationPlans('baseline', 42)[0], 'cashReserve'));
});

test('save, reviewed update, import and run drafts retain reserves without changing the executed report', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-reserve-'));
  try {
    const workspace = new TerminalWorkspace(directory);
    const original = settings();
    original.controls.cashReserve = '1.25';
    await workspace.saveScenario('Reserved purchasing', original);
    const expected = structuredClone(workspace.data.scenarios[0]);
    const proposed = settings();
    const review = scenarioUpdateLines({ expected, settings: proposed }, 160).join('\n');
    assert.match(review, /Cash reserve USD 1\.25/);
    assert.match(review, /Cash reserve USD 3\.25/);
    await workspace.updateScenario(expected, proposed);
    const run = record(proposed);
    const before = structuredClone(run);
    await workspace.appendRun(run);
    const loaded = await new TerminalWorkspace(directory).load();
    assert.equal(loaded.scenarios[0].controls.cashReserve, '3.25');
    const draft = useRunAssumptions(navigate(initialState(), 5), loaded.runs[0]);
    assert.equal(draft.planning.cashReserve, '3.25');
    assert.deepEqual(
      simulationPlans(draft.selectedPreset, draft.seed, draft.planning, draft.products),
      run.result.inputs,
    );
    const preview = importedScenario({ format: 'stateset-tui-run', version: 1, ...run });
    assert.equal(preview.settings.controls.cashReserve, '3.25');
    assert.match(importReviewLines(preview, 160).join('\n'), /Cash reserve USD 3\.25/);
    draft.planning.cashReserve = '9.00';
    assert.deepEqual(run, before);
    assert.deepEqual(loaded.runs[0], before);
    assert.throws(
      () =>
        importedScenario({
          format: 'stateset-tui-run',
          version: 1,
          ...run,
          controls: { ...run.controls, cashReserve: '1.00' },
        }),
      /differ from executed inputs/,
    );
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('legacy zero-reserve runs retain their original serialized inputs and digests on load, import and draft use', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-reserve-legacy-'));
  try {
    const controls = {
      days: 7,
      startingCash: '200.00',
      initialStock: 12,
      reorderPoint: 8,
      targetStock: 24,
      leadTimeDays: 1,
    };
    const inputs = [
      {
        name: 'Baseline',
        seed: 42,
        currency: 'USD',
        days: 7,
        startingCash: '200.00',
        products: [
          {
            sku: 'COFFEE-250G',
            initialStock: 12,
            unitPrice: '12.50',
            unitCost: '5.25',
            demand: { min: 2, max: 5 },
            reorderPoint: 8,
            targetStock: 24,
            leadTimeDays: 1,
          },
        ],
      },
    ];
    const legacy = runRecord({
      presetId: 'baseline',
      preset: 'Steady demand',
      seed: 42,
      controls,
      products: [],
      result: { success: true, inputs, results: [report()] },
    });
    delete legacy.controls.cashReserve;
    const before = structuredClone(legacy);
    const workspace = new TerminalWorkspace(directory);
    await workspace.appendRun(legacy);
    const loaded = await new TerminalWorkspace(directory).load();
    assert.deepEqual(loaded.runs[0], before);
    const preview = importedScenario({ format: 'stateset-tui-run', version: 1, ...legacy });
    assert.equal(preview.settings.controls.cashReserve, '0.00');
    const state = { ...navigate(initialState(), 5), sim: { ...legacy, running: false } };
    const draft = useRunAssumptions(state, legacy);
    assert.equal(draft.planning.cashReserve, '0.00');
    assert.deepEqual(simulationPlans('baseline', 42, draft.planning), inputs);
    assert.doesNotMatch(render(draft, 140, 44, { color: false }), /Assumptions changed/);
    assert.deepEqual(legacy, before);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});
