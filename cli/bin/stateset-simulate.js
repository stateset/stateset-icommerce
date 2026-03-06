#!/usr/bin/env node

/**
 * StateSet Simulation CLI
 *
 * Structured playground for A2A scenario simulation.
 */

import { parseArgs } from 'node:util';
import fs from 'node:fs/promises';
import { A2AStore } from '../src/a2a/store.js';
import { makeCommerceProxy } from '../src/a2a/agent-runtime.js';
import { SIMULATION_SCENARIOS, runSimulationScenario } from '../src/a2a/simulator.js';
import { runMain } from '../src/graceful-shutdown.js';
import { CLI_VERSION } from '../src/config.js';

const HELP = `
StateSet Simulation CLI

USAGE:
  stateset-simulate --scenario <name> [options]
  stateset simulate --scenario <name> [options]

DESCRIPTION:
  Run sandboxed A2A simulations with failure injection, simulated time, and
  state snapshots.

OPTIONS:
  --scenario <name>        Scenario to run (default: supplier-goes-offline)
  --agents <a,b>           Comma-separated agent names for scenario roles
  --db <path>              SQLite path for simulation state (default: :memory:)
  --start-time <iso>       Initial simulated clock time
  --advance-hours <n>      Hours to advance for time-based scenarios (default: 2)
  --deadline-minutes <n>   RFQ deadline window in minutes (default: 60)
  --snapshot-file <path>   Write the full simulation artifact to a JSON file
  --chain <id>             Settlement chain for demo scenarios (base, solana, set_chain, arbitrum)
  --live                   Use live settlement instead of simulated settlement
  --skip-snapshots         Return outcome without embedded snapshots
  --list-scenarios         Print supported scenario names and exit
  --json                   Emit JSON
  --help, -h              Show help
  --version, -v           Show version

SCENARIOS:
  ${SIMULATION_SCENARIOS.join('\n  ')}

EXAMPLES:
  stateset simulate --scenario supplier-goes-offline --agents inventory,procurement --json
  stateset-simulate --scenario basic-negotiation --chain base --json
`;

function parseNumber(value, fallback) {
  if (value === undefined) return fallback;
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    throw new Error(`Expected numeric value, received ${value}`);
  }
  return parsed;
}

function parseAgentNames(value) {
  if (typeof value !== 'string' || !value.trim()) return undefined;
  return value
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function printSummary(result, snapshotFile) {
  console.log(`\nSimulation complete: ${result.scenario}`);
  if (result.failureProfile) {
    console.log(`  Failure profile: ${result.failureProfile.name}`);
  }
  if (result.agents?.supplier || result.agents?.buyer) {
    const supplier = result.agents.supplier?.name;
    const buyer = result.agents.buyer?.name;
    console.log(`  Agents: ${[supplier, buyer].filter(Boolean).join(' -> ')}`);
  }
  console.log(`  Simulated time advanced: ${result.simulatedTime?.advancedMs || 0}ms`);

  if (result.outcome) {
    if (result.outcome.finalRfqStatus) {
      console.log(`  Final RFQ status: ${result.outcome.finalRfqStatus}`);
    }
    if (result.outcome.finalQuoteStatus) {
      console.log(`  Final quote status: ${result.outcome.finalQuoteStatus}`);
    }
    if (result.outcome.fallbackRecommended !== undefined) {
      console.log(`  Fallback recommended: ${result.outcome.fallbackRecommended}`);
    }
  }

  if (Array.isArray(result.steps) && result.steps.length > 0) {
    console.log('  Steps:');
    for (const step of result.steps) {
      console.log(`    - ${step.name}: ${step.status}`);
    }
  }

  if (Array.isArray(result.snapshots)) {
    console.log(`  Snapshots captured: ${result.snapshots.length}`);
  }
  if (snapshotFile) {
    console.log(`  Artifact written: ${snapshotFile}`);
  }
  console.log('');
}

async function main() {
  const { values } = parseArgs({
    options: {
      scenario: { type: 'string' },
      agents: { type: 'string' },
      db: { type: 'string', default: ':memory:' },
      'start-time': { type: 'string' },
      'advance-hours': { type: 'string', default: '2' },
      'deadline-minutes': { type: 'string', default: '60' },
      'snapshot-file': { type: 'string' },
      chain: { type: 'string' },
      live: { type: 'boolean', default: false },
      'skip-snapshots': { type: 'boolean', default: false },
      'list-scenarios': { type: 'boolean', default: false },
      json: { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false },
      version: { type: 'boolean', short: 'v', default: false },
    },
    allowPositionals: false,
  });

  if (values.help) {
    console.log(HELP);
    return;
  }

  if (values.version) {
    console.log(`stateset-simulate v${CLI_VERSION}`);
    return;
  }

  if (values['list-scenarios']) {
    const payload = { scenarios: SIMULATION_SCENARIOS };
    if (values.json) {
      console.log(JSON.stringify(payload, null, 2));
    } else {
      console.log(SIMULATION_SCENARIOS.join('\n'));
    }
    return;
  }

  const store = new A2AStore({ dbPath: values.db || ':memory:' });
  store.init();

  try {
    const commerce = makeCommerceProxy(store);
    const scenario = values.scenario || 'supplier-goes-offline';
    const agentNames = parseAgentNames(values.agents);
    const advanceHours = parseNumber(values['advance-hours'], 2);
    const deadlineMinutes = parseNumber(values['deadline-minutes'], 60);
    const captureSnapshots = values['skip-snapshots'] !== true;
    const settlement =
      values.live || values.chain
        ? {
            chainId: values.chain || 'base',
            simulate: !values.live,
            configDir: '.stateset',
          }
        : null;

    const result = await runSimulationScenario({
      scenario,
      store,
      commerce,
      startTime: values['start-time'],
      agentNames,
      advanceHours,
      deadlineMinutes,
      captureSnapshots,
      settlement,
      log: values.json ? () => {} : console.log,
    });

    if (values['snapshot-file']) {
      await fs.writeFile(values['snapshot-file'], JSON.stringify(result, null, 2));
    }

    if (values.json) {
      console.log(JSON.stringify(result, null, 2));
      return;
    }

    printSummary(result, values['snapshot-file'] || null);
  } finally {
    store.close();
  }
}

runMain('stateset-simulate', main);
