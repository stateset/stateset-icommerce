/**
 * The swarm harness at small N: it passes on a healthy engine, it is
 * deterministic for one process, it observes the denials it plans, it runs
 * multi-process, and its oracle actually fails when the store is tampered
 * with (so a green run means something).
 *
 *   node --test sim/test/
 */
import assert from 'node:assert/strict';
import { execFile, execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { after, describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

import { SIM_DIR, bindingAvailable } from '../lib/engine.mjs';
import { runOracleIsolated } from '../lib/oracle.mjs';
import { EXPECTED_DENIAL, normalizeConfig } from '../lib/plan.mjs';
import { executeSwarm, runSimulation } from '../runner.mjs';

const RUNNER = path.join(SIM_DIR, 'runner.mjs');
const TAMPER = fileURLToPath(new URL('../lib/tamper.mjs', import.meta.url));
const skip = bindingAvailable() ? false : 'the StateSet Node binding is not built (bindings/node)';
const TIMEOUT = 60_000;

describe('agent swarm', { skip }, () => {
  const runs = new Map();
  const run = (config) => {
    const key = JSON.stringify(config);
    if (!runs.has(key)) runs.set(key, runSimulation(config));
    return runs.get(key);
  };

  it(
    'passes every invariant at --agents 50 --seed 7 --stock 10',
    { timeout: TIMEOUT },
    async () => {
      const summary = await run({ agents: 50, seed: 7, stock: 10 });
      assert.equal(summary.invariants.passed, true, JSON.stringify(summary.failures, null, 2));
      assert.equal(summary.invariants.oversold, false);
      assert.equal(summary.invariants.overspent, 0);
      assert.equal(summary.invariants.receiptMismatches, 0);
      assert.equal(summary.invariants.duplicateApplies, 0);
      assert.equal(summary.invariants.chainValid, true);
      // Demand far exceeds 10 units, so the run is only meaningful if stock ran out.
      assert.equal(summary.stock.reserved, '10');
      assert.equal(summary.stock.available, '0');
      assert.ok(summary.rejections.byErrorCode['commerce.inventory.insufficient_available'] > 0);
      assert.ok(summary.idempotency.replays > 0, 'some checkouts were resubmitted');
      assert.equal(summary.receiptChain.entries, summary.commands.byRole.primary);
    },
  );

  it(
    'is deterministic for one process: same seed, same digest; new seed, new digest',
    { timeout: TIMEOUT },
    async () => {
      const first = await run({ agents: 50, seed: 7, stock: 10 });
      const again = await runSimulation({ agents: 50, seed: 7, stock: 10 });
      const other = await runSimulation({ agents: 50, seed: 8, stock: 10 });
      assert.equal(first.deterministic, true);
      assert.equal(again.digest, first.digest);
      assert.deepEqual(again.actors, first.actors);
      assert.deepEqual(again.rejections, first.rejections);
      assert.notEqual(other.digest, first.digest);
    },
  );

  it(
    'denies exactly the capability-less actors, with the policy code',
    { timeout: TIMEOUT },
    async () => {
      const summary = await run({ agents: 50, seed: 7, stock: 10 });
      const expected = summary.denials.expectedDeniedActors;
      assert.ok(expected > 0, 'seed 7 plans some capability-less principals');
      assert.equal(summary.denials.deniedActorsObserved, expected);
      assert.deepEqual(summary.denials.byErrorCode, { [EXPECTED_DENIAL.errorCode]: expected });
      assert.deepEqual(summary.denials.byReasonCode, { [EXPECTED_DENIAL.reasonCode]: expected });
      assert.equal(summary.actors.byOutcome.denied, expected);
      assert.equal(summary.invariants.expectationFailures, 0);
    },
  );

  it('passes the invariants with three processes on one file', { timeout: TIMEOUT }, async () => {
    const summary = await runSimulation({ agents: 30, seed: 11, stock: 8, procs: 3 });
    assert.equal(
      summary.invariants.passed,
      true,
      JSON.stringify(summary.failures ?? summary.processFailures, null, 2),
    );
    assert.equal(summary.deterministic, false);
    assert.equal(summary.contention.perProcess.length, 3);
    assert.deepEqual(
      summary.contention.perProcess.map((proc) => proc.actors),
      [10, 10, 10],
    );
    assert.equal(summary.actors.total, 30);
    assert.ok(Number(summary.stock.reserved) <= 8);
  });

  it(
    'exits 0 with a JSON summary from the CLI, and 2 on bad usage',
    { timeout: TIMEOUT },
    async () => {
      const { stdout } = await promisify(execFile)(process.execPath, [
        RUNNER,
        '--agents',
        '8',
        '--seed',
        '3',
        '--stock',
        '4',
        '--json',
      ]);
      const summary = JSON.parse(stdout);
      assert.equal(summary.invariants.passed, true);
      assert.equal(summary.config.agents, 8);
      await assert.rejects(
        promisify(execFile)(process.execPath, [RUNNER, '--agents', '0']),
        (error) => error.code === 2,
      );
    },
  );

  describe('the oracle fails a tampered store', () => {
    const dir = mkdtempSync(
      path.join(process.env.SIM_TMPDIR || os.tmpdir(), 'stateset-swarm-test-'),
    );
    after(() => rmSync(dir, { recursive: true, force: true }));

    async function tampered(name, sql) {
      const dbPath = path.join(dir, `${name}.db`);
      const config = normalizeConfig({ agents: 12, seed: 5, stock: 6, db: dbPath });
      const result = await executeSwarm(config, dbPath);
      const clean = await runOracleIsolated({
        dbPath,
        config,
        records: result.records,
        budgetStatuses: result.budgetStatuses,
      });
      assert.equal(clean.invariants.passed, true, `${name}: untampered store must pass`);
      execFileSync(process.execPath, [TAMPER, dbPath, sql]);
      return runOracleIsolated({
        dbPath,
        config,
        records: result.records,
        budgetStatuses: result.budgetStatuses,
      });
    }

    it('a rewritten sealed receipt breaks the chain', { timeout: TIMEOUT }, async () => {
      const oracle = await tampered(
        'chain',
        `UPDATE kernel_receipt_audit_log SET receipt = json_set(receipt, '$.error_message', 'forged')
          WHERE sequence = (SELECT MIN(sequence) + 3 FROM kernel_receipt_audit_log)`,
      );
      assert.equal(oracle.invariants.chainValid, false);
      assert.equal(oracle.invariants.passed, false);
    });

    it(
      'a budget whose counter disagrees with its receipts is a ledger mismatch',
      { timeout: TIMEOUT },
      async () => {
        const oracle = await tampered(
          'budget',
          `UPDATE kernel_economic_budgets SET committed_amount = '0.00'
          WHERE budget_id = (SELECT budget_id FROM kernel_economic_budgets WHERE committed_amount <> '0' LIMIT 1)`,
        );
        assert.ok(oracle.invariants.budgetLedgerMismatches > 0);
        assert.equal(oracle.invariants.passed, false);
      },
    );

    it(
      'an order with no receipt behind it is a receipt mismatch and a stock violation',
      { timeout: TIMEOUT },
      async () => {
        const oracle = await tampered(
          'order',
          `INSERT INTO order_items (id, order_id, product_id, sku, name, quantity, unit_price, total)
           SELECT 'forged-line', id, 'forged-product', 'SIM-WIDGET-1', 'Forged', 1, '10.00', '10.00'
             FROM orders LIMIT 1;
         UPDATE orders SET cart_id = NULL WHERE id = (SELECT id FROM orders LIMIT 1)`,
        );
        assert.ok(oracle.invariants.receiptMismatches > 0);
        assert.equal(oracle.invariants.oversold, true);
        assert.equal(oracle.invariants.passed, false);
      },
    );
  });
});
