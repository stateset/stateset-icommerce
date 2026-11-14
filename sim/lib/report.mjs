/**
 * Summary assembly: outcome counts, latency percentiles, contention, the
 * oracle's verdicts and an outcome digest -- a hash over sorted final-state
 * facts that excludes timings and engine-generated ids, so two runs of the
 * same seed can be compared with one string.
 */
import { createHash } from 'node:crypto';
import path from 'node:path';

import { formatDecimal, sumDecimals } from './decimal.mjs';
import { REPO_ROOT, bindingPath } from './engine.mjs';

export function percentiles(values) {
  if (values.length === 0) return { count: 0, p50: null, p95: null, p99: null, max: null };
  const sorted = Float64Array.from(values).sort();
  const rank = (q) =>
    sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(q * sorted.length) - 1))];
  const round = (value) => Math.round(value * 100) / 100;
  return {
    count: sorted.length,
    p50: round(rank(0.5)),
    p95: round(rank(0.95)),
    p99: round(rank(0.99)),
    max: round(sorted[sorted.length - 1]),
  };
}

const tally = (map, key, by = 1) => {
  map[key] = (map[key] ?? 0) + by;
  return map;
};

function sortKeys(object) {
  return Object.fromEntries(
    Object.entries(object).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)),
  );
}

export function outcomeDigest(records, oracle) {
  const facts = records.map(
    (record) =>
      `actor:${record.index}|${record.outcome}|units:${record.unitsOrdered}|spent:${formatDecimal(sumDecimals(record.spent))}|${record.codes.join(',')}`,
  );
  const { stock, budgets, chain, receipts } = oracle;
  facts.push(
    `stock|reserved:${stock.reserved}|ordered:${stock.ordered}|backordered:${stock.backordered}|available:${stock.available}`,
    `budgets|committed:${budgets.committedTotal}|exhausted:${budgets.exhausted}`,
    `chain|entries:${chain.entries}|valid:${chain.valid}`,
    ...Object.entries(receipts.mutationCounts).map(
      ([table, counts]) => `rows|${table}:${counts.rows}`,
    ),
  );
  facts.sort();
  return createHash('sha256').update(facts.join('\n')).digest('hex');
}

export function buildSummary({ config, records, procResults, oracle, timings }) {
  const observed = records.flatMap((record) => record.observed);
  const byOutcome = {};
  const purchaseResults = {};
  for (const record of records) {
    tally(byOutcome, record.outcome ?? 'unfinished');
    for (const purchase of record.purchases)
      tally(purchaseResults, purchase.result ?? 'not_reached');
  }

  const byTypeStatus = {};
  const rejectionsByCode = {};
  const denialsByCode = {};
  const denialsByReason = {};
  const roles = {};
  for (const entry of observed) {
    const { status, error_code: errorCode, policy } = entry.receipt;
    tally(roles, entry.role);
    byTypeStatus[entry.commandType] ??= {};
    tally(
      byTypeStatus[entry.commandType],
      entry.role === 'primary' ? status : `${status}(${entry.role})`,
    );
    if (status !== 'rejected' || entry.role !== 'primary') continue;
    tally(rejectionsByCode, errorCode ?? 'unknown');
    if (errorCode === 'kernel.policy_denied') {
      tally(denialsByCode, errorCode);
      for (const reason of policy?.reason_codes ?? []) tally(denialsByReason, reason);
    }
  }

  const latency = {};
  const thrown = {};
  let retries = 0;
  for (const result of procResults) {
    for (const [type, values] of Object.entries(result.stats.latency))
      (latency[type] ??= []).push(...values);
    for (const [kind, count] of Object.entries(result.stats.thrown)) tally(thrown, kind, count);
    retries += result.stats.retries;
  }
  const commandLatencies = Object.entries(latency)
    .filter(([type]) => type !== 'snapshot')
    .flatMap(([, values]) => values);
  const turnaround = records
    .filter((record) => record.startedAt && record.finishedAt)
    .map((record) => record.finishedAt - record.startedAt);
  // Throughput counts receipted commands; thrown attempts are in the latency tail.
  const commandCount = observed.length;

  const { invariants } = oracle;
  return {
    harness: 'stateset-agent-swarm/1',
    config: {
      agents: config.agents,
      seed: config.seed,
      stock: config.stock,
      budget: config.budget,
      price: config.price,
      procs: config.procs,
      inflight: config.inflight,
      maxPurchases: config.maxPurchases,
      maxQuantity: config.maxQuantity,
      stockPolicy: config.stockPolicy,
      denyFraction: config.denyFraction,
      duplicateFraction: config.duplicateFraction,
      conflictFraction: config.conflictFraction,
      actorsPerPrincipal: config.actorsPerPrincipal,
      retries: config.retries,
      db: config.db ?? '(temp)',
    },
    engine: {
      binding: path.relative(REPO_ROOT, bindingPath()) || bindingPath(),
      executeKernelCommand: 'async (napi async fn on the binding tokio runtime)',
      oracleReader: oracle.reader,
    },
    deterministic: config.procs === 1 && config.inflight === 1,
    actors: { total: records.length, byOutcome: sortKeys(byOutcome) },
    purchases: {
      total: records.reduce((n, r) => n + r.purchases.length, 0),
      byResult: sortKeys(purchaseResults),
    },
    commands: {
      receipts: observed.length,
      byRole: sortKeys(roles),
      byTypeAndStatus: sortKeys(
        Object.fromEntries(Object.entries(byTypeStatus).map(([k, v]) => [k, sortKeys(v)])),
      ),
    },
    denials: {
      expectedDeniedActors: oracle.expectations.deniedActors,
      deniedActorsObserved: oracle.expectations.deniedObserved,
      byErrorCode: sortKeys(denialsByCode),
      byReasonCode: sortKeys(denialsByReason),
    },
    rejections: { byErrorCode: sortKeys(rejectionsByCode) },
    invariants: {
      oversold: invariants.oversold,
      overspent: invariants.overspent,
      receiptMismatches: invariants.receiptMismatches,
      duplicateApplies: invariants.duplicateApplies,
      chainValid: invariants.chainValid,
      budgetLedgerMismatches: invariants.budgetLedgerMismatches,
      expectationFailures: invariants.expectationFailures,
      crashedActors: invariants.crashedActors,
      passed: invariants.passed,
    },
    stock: {
      initial: oracle.stock.initial,
      reserved: oracle.stock.reserved,
      ordered: oracle.stock.ordered,
      backordered: oracle.stock.backordered,
      onHand: oracle.stock.onHand,
      allocated: oracle.stock.allocated,
      available: oracle.stock.available,
    },
    budgets: {
      principals: oracle.budgets.budgets,
      limitEach: oracle.budgets.limitEach,
      committedTotal: oracle.budgets.committedTotal,
      exhausted: oracle.budgets.exhausted,
    },
    idempotency: {
      replays: oracle.idempotency.replays,
      conflictAttempts: oracle.expectations.conflictAttempts,
      conflictsRejected: oracle.expectations.conflictsRejected,
    },
    receiptChain: {
      verifier: oracle.chain.verifier,
      entries: oracle.chain.entries,
      materializedRows: oracle.chain.materializedRows,
      headHash: oracle.chain.headHash,
    },
    contention: {
      busy: thrown.busy ?? 0,
      timeouts: thrown.timeout ?? 0,
      otherThrown: thrown.other ?? 0,
      retries,
      perProcess: procResults.map((result) => ({
        proc: result.procIndex,
        actors: result.records.length,
        commands: Object.entries(result.stats.latency)
          .filter(([type]) => type !== 'snapshot' && type !== 'thrown')
          .reduce((n, [, values]) => n + values.length, 0),
        busy: result.stats.thrown.busy ?? 0,
        timeouts: result.stats.thrown.timeout ?? 0,
        otherThrown: result.stats.thrown.other ?? 0,
        retries: result.stats.retries,
        wallMs: result.wallMs,
        exitCode: result.exitCode ?? 0,
      })),
    },
    latencyMs: {
      allCommands: percentiles(commandLatencies),
      byType: sortKeys(
        Object.fromEntries(
          Object.entries(latency).map(([type, values]) => [type, percentiles(values)]),
        ),
      ),
    },
    actorTurnaroundMs: percentiles(turnaround),
    wallMs: timings.swarmMs,
    setupMs: timings.setupMs,
    oracleMs: timings.oracleMs,
    commandsPerSecond:
      timings.swarmMs > 0 ? Math.round((commandCount / timings.swarmMs) * 1000 * 10) / 10 : null,
    digest: outcomeDigest(records, oracle),
    failures: invariants.passed ? null : failureDetails(oracle),
  };
}

function failureDetails(oracle) {
  return {
    stock: oracle.stock.violations,
    overspent: oracle.budgets.overspent,
    budgetLedgerMismatches: oracle.budgets.ledgerMismatches,
    receipts: oracle.receipts.issues,
    idempotency: oracle.idempotency.issues,
    chain: oracle.chain.valid
      ? null
      : {
          firstInvalidSequence: oracle.chain.firstInvalidSequence,
          issueCount: oracle.chain.issueCount,
          issues: oracle.chain.issues,
        },
    expectations: oracle.expectations.failures,
    crashedActors: oracle.crashes.actors,
  };
}

export function renderText(summary) {
  const inv = summary.invariants;
  const lat = summary.latencyMs.allCommands;
  const lines = [
    `swarm: ${summary.config.agents} agents, seed ${summary.config.seed}, stock ${summary.config.stock}, budget ${summary.config.budget}, procs ${summary.config.procs}, inflight ${summary.config.inflight}${summary.deterministic ? ' (deterministic)' : ''}`,
    `actors: ${JSON.stringify(summary.actors.byOutcome)}`,
    `rejections: ${JSON.stringify(summary.rejections.byErrorCode)}`,
    `denials: ${JSON.stringify(summary.denials.byReasonCode)} (expected actors ${summary.denials.expectedDeniedActors}, observed ${summary.denials.deniedActorsObserved})`,
    `stock: initial ${summary.stock.initial}, reserved ${summary.stock.reserved}, backordered ${summary.stock.backordered}, available ${summary.stock.available}`,
    `budgets: committed ${summary.budgets.committedTotal} across ${summary.budgets.principals}, exhausted ${summary.budgets.exhausted}`,
    `contention: busy ${summary.contention.busy}, timeouts ${summary.contention.timeouts}, other thrown ${summary.contention.otherThrown}, retries ${summary.contention.retries}`,
    `latency ms: p50 ${lat.p50} p95 ${lat.p95} p99 ${lat.p99} max ${lat.max} over ${lat.count} commands; wall ${summary.wallMs} ms (${summary.commandsPerSecond}/s)`,
    `invariants: oversold=${inv.oversold} overspent=${inv.overspent} receiptMismatches=${inv.receiptMismatches} duplicateApplies=${inv.duplicateApplies} chainValid=${inv.chainValid} budgetLedger=${inv.budgetLedgerMismatches} expectations=${inv.expectationFailures} crashedActors=${inv.crashedActors} -> ${inv.passed ? 'PASS' : 'FAIL'}`,
    `digest: ${summary.digest}`,
  ];
  if (summary.failures) lines.push(`failures: ${JSON.stringify(summary.failures, null, 2)}`);
  if (summary.processFailures)
    lines.push(`process failures: ${JSON.stringify(summary.processFailures, null, 2)}`);
  return lines.join('\n');
}
