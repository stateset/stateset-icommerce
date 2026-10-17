import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const execute = promisify(execFile);
const cli = fileURLToPath(new URL('../../bin/stateset-simulate.js', import.meta.url));
const invoke = (args) =>
  execute(process.execPath, [cli, ...args], { env: { NODE_ENV: 'test' }, timeout: 45000 });

test('native CLI rejects explicitly supplied legacy options before reading any file', async () => {
  for (const flags of [
    ['--live'],
    ['--db', ':memory:'],
    ['--start-time', '2026-01-01'],
    ['--list-scenarios'],
    ['--commerce', 'other.json'],
    ['--inventory-experiment', 'other.json'],
  ]) {
    await assert.rejects(
      invoke(['--inventory-plan', '/no-such-simulation-input.json', ...flags]),
      (error) => {
        assert.equal(error.code, 1);
        assert.match(error.stderr, /one JSON input mode/);
        assert.doesNotMatch(error.stderr, /ENOENT/);
        return true;
      },
    );
  }
});

test('native CLI compares JSON episodes, reports unmet expectations and bounds input bytes', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'stateset-simulation-cli-'));
  try {
    const file = path.join(directory, 'input.json');
    const scenario = {
      name: 'first',
      inventory: [{ sku: 'SIM', quantity: 1, unitPrice: '1.00' }],
      steps: [{ type: 'inspect_state' }],
    };
    await fs.writeFile(file, JSON.stringify([scenario, { ...scenario, name: 'second' }]));
    const output = JSON.parse((await invoke(['--commerce', file, '--json'])).stdout);
    assert.equal(output.success, true);
    assert.equal(output.results.length, 2);
    assert.equal(output.liveStoreModified, false);
    await fs.writeFile(
      file,
      JSON.stringify({
        ...scenario,
        steps: [{ type: 'place_order', id: 'oversell', items: [{ sku: 'SIM', quantity: 2 }] }],
      }),
    );
    await assert.rejects(invoke(['--commerce', file]), (error) => {
      assert.equal(error.code, 1);
      assert.equal(JSON.parse(error.stdout).results[0].expectationsMet, false);
      return true;
    });
    let deep = null;
    for (let level = 0; level < 40; level += 1) deep = { nested: deep };
    await fs.writeFile(file, JSON.stringify({ ...scenario, extra: deep }));
    await assert.rejects(invoke(['--commerce', file, '--json']), (error) => {
      assert.equal(error.code, 1);
      assert.match(error.stderr, /Simulation input exceeds 32 nesting levels/);
      return true;
    });
    await fs.writeFile(file, ' '.repeat(131073));
    await assert.rejects(invoke(['--inventory-plan', file]), (error) => {
      assert.match(error.stderr, /128 KiB/);
      return true;
    });
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('native CLI replays a bounded forecast schedule and refuses ambiguous seeds or incomplete horizons', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'stateset-experiment-cli-'));
  try {
    const file = path.join(directory, 'experiment.json');
    const plan = {
      name: 'baseline',
      days: 1,
      startingCash: '0.00',
      products: [
        {
          sku: 'SIM',
          initialStock: 1,
          unitPrice: '2.50',
          unitCost: '1.25',
          demand: { dailyUnits: [1] },
          reorderPoint: 0,
          targetStock: 1,
          leadTimeDays: { dailyValues: [3, 1] },
          delivery: { firstReceiptBps: { dailyValues: [0, 10000] }, remainderDelayDays: 1 },
        },
      ],
    };
    const input = {
      name: 'cli-experiment',
      plans: [plan],
      seeds: [0, 4294967295],
      goals: {
        minFillRateBps: 10000,
        minEndingCash: '1000',
        productFillRates: [{ sku: 'SIM', minFillRateBps: 10000 }],
      },
    };
    await fs.writeFile(file, JSON.stringify(input));
    const result = JSON.parse((await invoke(['--inventory-experiment', file, '--json'])).stdout);
    assert.equal(result.success, true);
    assert.equal(result.experiment.nativeRuns, 2);
    assert.equal(result.experiment.policies[0].metrics.revenue.p50, '2.50');
    assert.deepEqual(
      result.experiment.policies[0].plan.products[0].leadTimeDays.dailyValues,
      [3, 1],
    );
    assert.equal(result.experiment.policies[0].evaluation.missedRuns, 2);
    assert.equal(
      result.experiment.policies[0].evaluation.byGoal.productFillRates.products[0].metRuns,
      2,
    );
    assert.equal(result.experiment.policies[0].pathDiversity.jointPaths, 1);
    assert.equal(result.experiment.policies[0].pathDiversity.duplicateJointPathRuns, 1);
    assert.equal(result.liveStoreModified, false);
    assert.equal(result.experiment.policies[0].runs[0].reconciliation.inventoryFlowBalanced, true);
    assert.equal(result.experiment.policies[0].runs[0].inventoryFlow.firstStockout, null);
    assert.equal(result.experiment.policies[0].runs[0].inventoryFlow.daily[1].products[0].sold, 1);
    await fs.writeFile(
      file,
      JSON.stringify({
        ...input,
        goals: { productFillRates: [{ sku: 'UNKNOWN', minFillRateBps: 10000 }] },
      }),
    );
    await assert.rejects(invoke(['--inventory-experiment', file, '--json']), (error) => {
      assert.equal(error.code, 1);
      assert.match(error.stderr, /every policy/);
      return true;
    });
    await fs.writeFile(
      file,
      JSON.stringify({
        ...input,
        plans: [
          {
            ...plan,
            products: [{ ...plan.products[0], leadTimeDays: { dailyValues: [1, 1, 1] } }],
          },
        ],
      }),
    );
    await assert.rejects(invoke(['--inventory-experiment', file, '--json']), (error) => {
      assert.equal(error.code, 1);
      assert.match(error.stderr, /days\+1/);
      return true;
    });
    await fs.writeFile(file, JSON.stringify({ ...input, plans: [{ ...plan, days: 2 }] }));
    await assert.rejects(invoke(['--inventory-experiment', file, '--json']), (error) => {
      assert.equal(error.code, 1);
      assert.match(error.stderr, /exactly one entry per planning day/);
      return true;
    });
    await fs.writeFile(file, JSON.stringify({ ...input, plans: [{ ...plan, seed: 123 }] }));
    await assert.rejects(invoke(['--inventory-experiment', file]), (error) => {
      assert.equal(error.code, 1);
      assert.match(error.stderr, /seed/);
      return true;
    });
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('native CLI exposes periodic purchasing metrics and rejects invalid review intervals', async () => {
  const file = fileURLToPath(
    new URL('../../examples/inventory-review-interval-experiment.json', import.meta.url),
  );
  const result = JSON.parse((await invoke(['--inventory-experiment', file, '--json'])).stdout);
  assert.equal(result.success, true);
  assert.equal(result.experiment.policies[1].metrics.deferredPurchaseReviews.p50, 3);
  assert.equal(result.experiment.policies[1].reviewDeferredRuns, 2);
  assert.equal(result.experiment.policies[1].runs[0].inventoryFlow.firstStockout.day, 3);
  assert.equal(result.liveStoreModified, false);
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'stateset-review-cli-'));
  try {
    const invalid = JSON.parse(await fs.readFile(file, 'utf8'));
    invalid.plans[1].products[0].purchasing.reviewIntervalDays = 0;
    const pathName = path.join(directory, 'invalid.json');
    await fs.writeFile(pathName, JSON.stringify(invalid));
    await assert.rejects(invoke(['--inventory-experiment', pathName, '--json']), (error) => {
      assert.equal(error.code, 1);
      assert.match(error.stderr, /reviewIntervalDays/);
      return true;
    });
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});
