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
import { differingFields, runOracleIsolated, sealedAsReturned } from '../lib/oracle.mjs';
import { EXPECTED_DENIAL, normalizeConfig } from '../lib/plan.mjs';
import { createRng } from '../lib/rng.mjs';
import { runActors } from '../lib/scheduler.mjs';
import { executeSwarm, runSimulation } from '../runner.mjs';

const RUNNER = path.join(SIM_DIR, 'runner.mjs');
const TAMPER = fileURLToPath(new URL('../lib/tamper.mjs', import.meta.url));
const skip = bindingAvailable() ? false : 'the StateSet Node binding is not built (bindings/node)';
const TIMEOUT = 60_000;

describe('the scheduler', () => {
  it('records a journey that throws as a crashed actor and keeps running the others', async () => {
    const actorWith = (index, journey) => {
      const actor = { index, done: false, lastResult: undefined };
      actor.record = { index, outcome: null, thrown: [] };
      actor.gen = journey(actor);
      return actor;
    };
    const healthy = actorWith(0, function* (actor) {
      yield { kind: 'step' };
      yield { kind: 'step' };
      actor.record.outcome = 'completed';
    });
    const crashing = actorWith(1, function* () {
      yield { kind: 'step' };
      throw new Error('journey bug');
    });
    let steps = 0;
    await runActors({
      actors: [healthy, crashing],
      rng: createRng('scheduler-test', 'crash'),
      inflight: 1,
      execute: async () => {
        steps += 1;
        return {};
      },
    });
    assert.equal(steps, 3);
    assert.equal(healthy.record.outcome, 'completed');
    assert.equal(crashing.record.outcome, 'crashed');
    assert.equal(crashing.record.thrown.length, 1);
    assert.equal(crashing.record.thrown[0].kind, 'harness');
    assert.match(crashing.record.thrown[0].message, /journey bug/);
  });
});

describe('the oracle compares receipts in full', () => {
  // An audit-log entry stores the receipt as hashed (audit_hash null); the
  // caller gets that receipt with audit_hash set to the entry's hash.
  const entry = {
    audit_hash: 'abc123',
    parsed: {
      receipt_id: 'r-1',
      status: 'succeeded',
      result: { order_id: 'o-1', total_charged: '10.00' },
      audit_hash: null,
    },
  };

  it('accepts the sealed receipt with its audit hash filled in', () => {
    const returned = { ...entry.parsed, audit_hash: 'abc123' };
    assert.deepEqual(sealedAsReturned(entry), returned);
    assert.deepEqual(differingFields(returned, sealedAsReturned(entry)), []);
  });

  it('names the altered fields when the ids are unchanged', () => {
    const expected = sealedAsReturned(entry);
    assert.deepEqual(differingFields({ ...expected, status: 'rejected' }, expected), ['status']);
    assert.deepEqual(
      differingFields(
        { ...expected, result: { order_id: 'o-1', total_charged: '0.00' }, audit_hash: null },
        expected,
      ),
      ['audit_hash', 'result'],
    );
    const { result: _dropped, ...withoutResult } = expected;
    assert.deepEqual(differingFields(withoutResult, expected), ['result']);
  });
});

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

  it(
    'exits 1 and reports the exception when an actor crashes before its first write',
    { timeout: TIMEOUT },
    async () => {
      // The crashed actor never writes, so the store is consistent and every
      // store invariant holds: only the crash itself can fail this run. Two
      // processes, so the forked shards must carry the crash back too.
      const failure = await promisify(execFile)(
        process.execPath,
        [RUNNER, '--agents', '8', '--seed', '3', '--stock', '4', '--procs', '2', '--json'],
        { env: { ...process.env, SIM_CRASH_ACTORS: '5' } },
      ).then(
        () => null,
        (error) => error,
      );
      assert.ok(failure, 'a run with a crashed actor must not exit 0');
      assert.equal(failure.code, 1, failure.stderr);
      const summary = JSON.parse(failure.stdout);
      assert.equal(summary.invariants.passed, false);
      assert.equal(summary.invariants.crashedActors, 1);
      assert.equal(summary.actors.byOutcome.crashed, 1);
      assert.equal(summary.invariants.oversold, false);
      assert.equal(summary.invariants.receiptMismatches, 0);
      assert.equal(summary.invariants.duplicateApplies, 0);
      assert.equal(summary.invariants.chainValid, true);
      assert.equal(summary.invariants.expectationFailures, 0);
      assert.equal(summary.failures.crashedActors.length, 1);
      const [crash] = summary.failures.crashedActors;
      assert.equal(crash.actor, 5);
      assert.ok(
        crash.thrown.some(
          (thrown) =>
            thrown.label === 'journey' && /injected journey crash for actor 5/.test(thrown.message),
        ),
        JSON.stringify(crash),
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

    it('a sealed outcome whose materialized row is deleted', { timeout: TIMEOUT }, async () => {
      const oracle = await tampered(
        'materialized-deleted',
        'DELETE FROM kernel_receipts WHERE rowid = (SELECT MAX(rowid) FROM kernel_receipts)',
      );
      assert.equal(oracle.invariants.chainValid, false);
      assert.equal(oracle.chain.materializedRows, oracle.chain.entries - 1);
      assert.deepEqual(
        oracle.chain.issues.map((issue) => issue.kind),
        ['missing_materialized_receipt'],
      );
      assert.equal(oracle.invariants.passed, false);
    });

    it(
      'a materialized receipt moved to another idempotency key',
      { timeout: TIMEOUT },
      async () => {
        const oracle = await tampered(
          'materialized-key',
          `UPDATE kernel_receipts SET idempotency_key = idempotency_key || ':forged'
            WHERE rowid = (SELECT MIN(rowid) FROM kernel_receipts)`,
        );
        assert.equal(oracle.invariants.chainValid, false);
        assert.deepEqual(oracle.chain.issues.map((issue) => issue.kind).sort(), [
          'materialized_without_sealed_entry',
          'missing_materialized_receipt',
        ]);
        assert.equal(oracle.invariants.passed, false);
      },
    );

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

  describe('the oracle fails a replay that is not the original receipt', () => {
    // One untouched store: these cases alter what a caller was told, not the
    // database, so they share a single swarm run.
    const dir = mkdtempSync(
      path.join(process.env.SIM_TMPDIR || os.tmpdir(), 'stateset-swarm-replay-'),
    );
    after(() => rmSync(dir, { recursive: true, force: true }));
    let fixture;
    const swarm = () =>
      (fixture ??= (async () => {
        const dbPath = path.join(dir, 'replays.db');
        const config = normalizeConfig({ agents: 12, seed: 5, stock: 6, db: dbPath });
        const result = await executeSwarm(config, dbPath);
        const input = {
          dbPath,
          config,
          records: result.records,
          budgetStatuses: result.budgetStatuses,
        };
        const clean = await runOracleIsolated(input);
        assert.equal(clean.invariants.passed, true, 'the untouched replays must pass');
        return input;
      })());

    /** Edit one replayed checkout receipt a caller saw, keeping its ids. */
    function withAlteredReplay(records, edit) {
      const altered = structuredClone(records);
      const replay = altered
        .flatMap((record) => record.observed)
        .find((entry) => entry.role === 'replay' && entry.receipt.status === 'succeeded');
      assert.ok(replay, 'seed 5 resubmits a succeeded checkout');
      const { receipt_id: receiptId, audit_hash: auditHash } = replay.receipt;
      edit(replay.receipt);
      assert.equal(replay.receipt.receipt_id, receiptId);
      assert.equal(replay.receipt.audit_hash, auditHash);
      return { altered, key: replay.key };
    }

    async function judgeAltered(field, edit) {
      const input = await swarm();
      const { altered, key } = withAlteredReplay(input.records, edit);
      const oracle = await runOracleIsolated({ ...input, records: altered });
      assert.ok(
        oracle.receipts.issues.some(
          (issue) =>
            issue.kind === 'returned_receipt_differs_from_sealed' &&
            issue.key === key &&
            issue.role === 'replay' &&
            issue.fields.includes(field),
        ),
        JSON.stringify(oracle.receipts.issues),
      );
      assert.ok(
        oracle.idempotency.issues.some(
          (issue) =>
            issue.kind === 'key_returned_different_receipts' &&
            issue.key === key &&
            issue.fields.includes(field),
        ),
        JSON.stringify(oracle.idempotency.issues),
      );
      assert.equal(oracle.invariants.passed, false);
    }

    it('a replay with the same ids but another status', { timeout: TIMEOUT }, () =>
      judgeAltered('status', (receipt) => {
        receipt.status = 'rejected';
      }),
    );

    it('a replay with the same ids but another result', { timeout: TIMEOUT }, () =>
      judgeAltered('result', (receipt) => {
        receipt.result = { ...receipt.result, total_charged: '0.00' };
      }),
    );
  });
});
