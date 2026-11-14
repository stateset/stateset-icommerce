#!/usr/bin/env node
/**
 * Seeded multi-agent load simulator for the governed (kernel) write path.
 *
 *   node sim/runner.mjs --agents 1000 --seed 7 --stock 50 [--budget 25.00]
 *                       [--procs 8] [--inflight 1] [--db path] [--json]
 *
 * One file-backed store; N actors, each a principal with its own durable
 * budget, walk the governed checkout journey through `executeKernelCommand`
 * in apply mode. A seeded scheduler interleaves them; `--procs P` forks P
 * processes against the same file. Afterwards the oracle checks stock,
 * budgets, receipts, idempotency and the receipt chain. Exit 0 when every
 * invariant holds, 1 when one fails, 2 on a usage or harness error.
 *
 * See sim/README.md. Dev-only: not part of any published package.
 */
import { fork } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync, writeSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

import { createActor, createExecutor, newStats } from './lib/actors.mjs';
import { loadBinding } from './lib/engine.mjs';
import { runOracleIsolated } from './lib/oracle.mjs';
import {
  actorPlan,
  buildPolicy,
  normalizeConfig,
  principalIndexOf,
  principalPlan,
} from './lib/plan.mjs';
import { buildSummary, renderText } from './lib/report.mjs';
import { createRng } from './lib/rng.mjs';
import { runActors } from './lib/scheduler.mjs';
import { prepareStore, readBudgetStatuses } from './lib/setup.mjs';

const RUNNER_FILE = fileURLToPath(import.meta.url);
/** `SIM_TRACE=1` writes phase markers to stderr synchronously (they survive a native crash). */
const trace = process.env.SIM_TRACE
  ? (message) => writeSync(2, `[swarm ${process.pid}] ${message}\n`)
  : () => {};

const OPTIONS = {
  agents: { type: 'string' },
  seed: { type: 'string' },
  stock: { type: 'string' },
  budget: { type: 'string' },
  price: { type: 'string' },
  procs: { type: 'string' },
  inflight: { type: 'string' },
  'max-purchases': { type: 'string' },
  'max-quantity': { type: 'string' },
  'deny-fraction': { type: 'string' },
  'duplicate-fraction': { type: 'string' },
  'conflict-fraction': { type: 'string' },
  'stock-policy': { type: 'string' },
  'actors-per-principal': { type: 'string' },
  retries: { type: 'string' },
  db: { type: 'string' },
  keep: { type: 'boolean' },
  json: { type: 'boolean' },
  help: { type: 'boolean', short: 'h' },
  child: { type: 'boolean' },
};

const USAGE = `usage: node sim/runner.mjs --agents N --seed S --stock K [options]

  --agents N                actors (default 50)
  --seed S                  seed; same seed + --procs 1 + --inflight 1 => same digest (default 1)
  --stock K                 initial units of the one SKU (default 10)
  --budget X                durable budget per principal, USD (default 25.00)
  --price X                 unit price, USD (default 10.00)
  --procs P                 processes sharing the DB file (default 1)
  --inflight C              steps outstanding per process (default 1; >1 is nondeterministic)
  --max-purchases M         purchases per actor, 1..M (default 2)
  --max-quantity Q          units per purchase, 1..Q (default 3)
  --deny-fraction F         principals missing checkout.commit (default 0.1)
  --duplicate-fraction F    checkouts resubmitted with the same key (default 0.2)
  --conflict-fraction F     later checkouts first reusing an earlier key (default 0.1)
  --stock-policy reject|backorder|mixed   checkout stock policy (default reject)
  --actors-per-principal A  actors sharing one principal and budget (default 1)
  --retries R               same-key retries after a thrown (unreceipted) error (default 2)
  --db PATH                 use this (fresh) DB file and keep it; default is a temp dir
  --keep                    keep the temp dir and print its path
  --json                    print only the JSON summary`;

function configFromArgs(values) {
  const camel = (key) => key.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
  const input = {};
  for (const [key, value] of Object.entries(values)) {
    if (['help', 'child'].includes(key)) continue;
    input[camel(key)] = value;
  }
  return input;
}

/** Run one process's share of the actors against an already-prepared store. */
async function runShard({ Commerce, dbPath, config, procIndex, indices, barrier }) {
  const commerce = await Commerce.open(dbPath);
  try {
    const stats = newStats();
    const actors = indices.map((index) =>
      createActor({
        plan: actorPlan(index, config),
        principal: principalPlan(principalIndexOf(index, config), config),
        config,
      }),
    );
    const execute = createExecutor({ commerce, policy: buildPolicy(), config, stats });
    if (barrier) await barrier();
    trace(`shard ${procIndex}: ${indices.length} actors`);
    const start = Date.now();
    await runActors({
      actors,
      rng: createRng(config.seed, `scheduler:${procIndex}`),
      inflight: config.inflight,
      execute,
    });
    trace(`shard ${procIndex}: done`);
    return {
      procIndex,
      records: actors.map((actor) => actor.record),
      stats,
      wallMs: Date.now() - start,
    };
  } finally {
    await commerce.close();
    trace(`shard ${procIndex}: closed`);
  }
}

/** Fork one child per process, start them together, collect their records. */
async function runForked({ dbPath, config }) {
  const children = Array.from({ length: config.procs }, (_, procIndex) => {
    const indices = [];
    for (let index = procIndex; index < config.agents; index += config.procs) indices.push(index);
    const child = fork(RUNNER_FILE, ['--child'], {
      stdio: ['ignore', 'inherit', 'inherit', 'ipc'],
      serialization: 'advanced',
    });
    let markReady;
    let settle;
    const ready = new Promise((resolve) => {
      markReady = resolve;
    });
    const result = new Promise((resolve) => {
      settle = resolve;
    });
    let delivered = false;
    child.on('message', (message) => {
      if (message.type === 'ready') markReady();
      if (message.type === 'result') {
        delivered = true;
        settle({ ...message.result, exitCode: 0 });
      }
      if (message.type === 'error') {
        delivered = true;
        markReady();
        settle({
          procIndex,
          records: [],
          stats: newStats(),
          wallMs: 0,
          exitCode: 2,
          error: message.message,
        });
      }
    });
    child.on('exit', (code, signal) => {
      markReady();
      if (!delivered) {
        settle({
          procIndex,
          records: [],
          stats: newStats(),
          wallMs: 0,
          exitCode: code || 1,
          error: `child exited (${signal ?? code}) before reporting`,
        });
      }
    });
    child.send({ type: 'start', dbPath, config, procIndex, indices });
    return { child, ready, result };
  });
  await Promise.all(children.map((entry) => entry.ready));
  for (const { child } of children) if (child.connected) child.send({ type: 'go' });
  return Promise.all(children.map((entry) => entry.result));
}

/**
 * Prepare a fresh store at `dbPath` and run the swarm against it. Returns what
 * the oracle needs; judging is separate so tests can tamper in between.
 */
export async function executeSwarm(config, dbPath) {
  if (existsSync(dbPath))
    throw new Error(`${dbPath} already exists; the swarm needs a fresh store`);
  const { Commerce } = loadBinding();
  const timings = {};
  let mark = Date.now();
  trace(`setup ${dbPath}`);
  await prepareStore({ Commerce, dbPath, config });
  timings.setupMs = Date.now() - mark;

  trace('swarm');
  mark = Date.now();
  const procResults =
    config.procs === 1
      ? [
          await runShard({
            Commerce,
            dbPath,
            config,
            procIndex: 0,
            indices: [...Array(config.agents).keys()],
          }),
        ]
      : await runForked({ dbPath, config });
  timings.swarmMs = Date.now() - mark;

  trace('budgets');
  const records = procResults.flatMap((result) => result.records).sort((a, b) => a.index - b.index);
  const budgetStatuses = await readBudgetStatuses({ Commerce, dbPath, config });
  return { records, procResults, budgetStatuses, timings };
}

/** Run the oracle over a finished swarm and assemble the summary. */
export async function judgeSwarm(config, dbPath, run) {
  trace('oracle');
  const mark = Date.now();
  const oracle = await runOracleIsolated({
    dbPath,
    config,
    records: run.records,
    budgetStatuses: run.budgetStatuses,
  });
  const timings = { ...run.timings, oracleMs: Date.now() - mark };
  const summary = buildSummary({
    config,
    records: run.records,
    procResults: run.procResults,
    oracle,
    timings,
  });
  const crashed = run.procResults.filter((result) => result.exitCode);
  if (crashed.length > 0) {
    summary.invariants.passed = false;
    summary.processFailures = crashed.map((result) => ({
      proc: result.procIndex,
      exitCode: result.exitCode,
      error: result.error,
    }));
  }
  return summary;
}

export async function runSimulation(input = {}) {
  const config = normalizeConfig(input);
  const ownDir = config.db
    ? null
    : mkdtempSync(path.join(process.env.SIM_TMPDIR || os.tmpdir(), 'stateset-swarm-'));
  const dbPath = ownDir ? path.join(ownDir, 'store.db') : path.resolve(config.db);
  try {
    const run = await executeSwarm(config, dbPath);
    return await judgeSwarm(
      { ...config, db: config.db ?? (config.keep ? dbPath : null) },
      dbPath,
      run,
    );
  } finally {
    trace('cleanup');
    if (ownDir && !config.keep) rmSync(ownDir, { recursive: true, force: true });
  }
}

function runChild() {
  process.once('message', async (start) => {
    try {
      const { Commerce } = loadBinding();
      const result = await runShard({
        Commerce,
        dbPath: start.dbPath,
        config: start.config,
        procIndex: start.procIndex,
        indices: start.indices,
        barrier: () =>
          new Promise((resolve) => {
            process.on('message', (message) => {
              if (message.type === 'go') resolve();
            });
            process.send({ type: 'ready' });
          }),
      });
      process.send({ type: 'result', result }, () => process.exit(0));
    } catch (error) {
      process.send({ type: 'error', message: String(error?.stack ?? error) }, () =>
        process.exit(2),
      );
    }
  });
}

async function main() {
  let values;
  try {
    ({ values } = parseArgs({ options: OPTIONS, strict: true }));
  } catch (error) {
    console.error(`${error.message}\n\n${USAGE}`);
    return 2;
  }
  if (values.child) {
    runChild();
    return null;
  }
  if (values.help) {
    console.log(USAGE);
    return 0;
  }
  let summary;
  try {
    summary = await runSimulation(configFromArgs(values));
  } catch (error) {
    console.error(`swarm: ${error.message}`);
    return 2;
  }
  console.log(values.json ? JSON.stringify(summary, null, 2) : renderText(summary));
  return summary.invariants.passed ? 0 : 1;
}

if (process.argv[1] && path.resolve(process.argv[1]) === RUNNER_FILE) {
  const code = await main();
  if (code !== null) process.exitCode = code;
}
