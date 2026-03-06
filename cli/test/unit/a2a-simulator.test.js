import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const cliSrc = path.join(__dirname, '..', '..', 'src');

const { A2AStore } = await import(path.join(cliSrc, 'a2a', 'store.js'));
const { makeCommerceProxy } = await import(path.join(cliSrc, 'a2a', 'agent-runtime.js'));
const {
  SIMULATION_SCENARIOS,
  runSimulationScenario,
  withSimulatedClock,
} = await import(path.join(cliSrc, 'a2a', 'simulator.js'));

let dbPath;
let store;
let commerce;

beforeEach(() => {
  dbPath = path.join(
    __dirname,
    `.test-a2a-sim-${Date.now()}-${Math.random().toString(36).slice(2)}.db`,
  );
  store = new A2AStore({ dbPath });
  store.init();
  commerce = makeCommerceProxy(store);
});

afterEach(() => {
  try {
    store.close();
  } catch {
    // ignored
  }
  try {
    fs.unlinkSync(dbPath);
  } catch {
    // ignored
  }
});

describe('A2A simulator', () => {
  it('exposes supplier-goes-offline as a built-in scenario', () => {
    assert.ok(SIMULATION_SCENARIOS.includes('supplier-goes-offline'));
  });

  it('advances simulated time in a scoped clock', async () => {
    const realBefore = Date.now();
    const result = await withSimulatedClock('2026-03-06T00:00:00.000Z', async (clock) => {
      const before = new Date().toISOString();
      clock.advanceHours(3);
      const after = new Date().toISOString();
      return { before, after, advancedMs: clock.advancedMs() };
    });

    assert.equal(result.before, '2026-03-06T00:00:00.000Z');
    assert.equal(result.after, '2026-03-06T03:00:00.000Z');
    assert.equal(result.advancedMs, 3 * 60 * 60 * 1000);
    assert.ok(Date.now() >= realBefore, 'global Date should be restored after the scoped clock');
  });

  it('runs the supplier-goes-offline scenario with snapshots and expiry outcome', async () => {
    const result = await runSimulationScenario({
      scenario: 'supplier-goes-offline',
      store,
      commerce,
      agentNames: ['inventory', 'procurement'],
      advanceHours: 2,
      deadlineMinutes: 30,
      captureSnapshots: true,
    });

    assert.equal(result.success, true);
    assert.equal(result.failureProfile.name, 'supplier-goes-offline');
    assert.equal(result.agents.supplier.name, 'inventory');
    assert.equal(result.agents.buyer.name, 'procurement');
    assert.equal(result.outcome.finalRfqStatus, 'expired');
    assert.equal(result.outcome.supplierActive, false);
    assert.equal(result.outcome.fallbackRecommended, true);
    assert.ok(Array.isArray(result.steps));
    assert.ok(result.steps.some((step) => step.name === 'inject-supplier-offline'));
    assert.ok(Array.isArray(result.snapshots));
    assert.ok(result.snapshots.length >= 4);
  });

  it('can wrap an existing demo scenario in simulation mode', async () => {
    const result = await runSimulationScenario({
      scenario: 'basic-negotiation',
      store,
      commerce,
      captureSnapshots: true,
    });

    assert.equal(result.success, true);
    assert.equal(result.scenario, 'basic-negotiation');
    assert.ok(result.outcome.quoteId);
    assert.equal(result.steps.length, 1);
    assert.equal(result.snapshots.length, 2);
  });
});
