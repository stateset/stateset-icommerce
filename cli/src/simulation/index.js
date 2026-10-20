import { fork } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import {
  commerceScenarioSchema,
  commerceComparisonSchema,
  inventoryPlanSchema,
  inventoryPlanComparisonSchema,
  inventoryExperimentSchema,
} from './schema.js';
import { stableStringify } from '../mcp/replay-sanitizer.js';
import { SIMULATION_KERNEL_VERSION, SIMULATION_LIMITS as limits } from './limits.js';

export { getSimulationKernelContract } from './contract.js';
export { SIMULATION_LIMITS } from './limits.js';

export {
  commerceScenarioSchema,
  commerceComparisonSchema,
  inventoryPlanSchema,
  inventoryPlanComparisonSchema,
  inventoryExperimentSchema,
} from './schema.js';

let activeRuns = 0;

async function run(scenarios, mode = 'commerce') {
  const encoded = stableStringify({ mode, scenarios });
  if (Buffer.byteLength(encoded) > limits.maxInputBytes)
    throw new Error('Simulation input exceeds 128 KiB');
  const identity = {
    schemaVersion: SIMULATION_KERNEL_VERSION,
    simulation: true,
    mode,
    engine: 'stateset-icommerce-native',
    isolation: 'fresh-private-sqlite-per-scenario',
    liveStoreRead: false,
    liveStoreModified: false,
    externalServicesCalled: false,
    scenarioHash: createHash('sha256').update(encoded).digest('hex'),
  };
  if (activeRuns >= limits.maxActiveWorkers) {
    return {
      ...identity,
      success: false,
      code: 'SIMULATION_BUSY',
      errorCode: 'SIMULATION_BUSY',
      retryable: true,
      message: 'The simulation worker limit is reached. Retry after an active run completes.',
    };
  }
  activeRuns += 1;
  let directory;
  try {
    directory = await fs.mkdtemp(path.join(os.tmpdir(), 'stateset-simulation-'));
    const response = await new Promise((resolve) => {
      const child = fork(fileURLToPath(new URL('./worker.js', import.meta.url)), [], {
        cwd: fileURLToPath(new URL('../../..', import.meta.url)),
        env: { NODE_ENV: 'test', STATESET_SQLITE_SYNCHRONOUS: 'NORMAL' },
        execArgv: ['--max-old-space-size=128'],
        stdio: ['ignore', 'ignore', 'ignore', 'ipc'],
      });
      let result;
      let expired = false;
      let workerError = false;
      const timer = setTimeout(() => {
        expired = true;
        child.kill('SIGKILL');
      }, limits.timeoutMs);
      child.once('message', (message) => {
        result = message;
      });
      child.once('error', () => {
        workerError = true;
        if (child.pid) child.kill('SIGKILL');
        else {
          clearTimeout(timer);
          resolve({
            success: false,
            code: 'SIMULATION_ENGINE_ERROR',
            message: 'The isolated simulation worker could not start.',
          });
        }
      });
      child.once('close', (code) => {
        clearTimeout(timer);
        if (expired)
          resolve({
            success: false,
            code: 'SIMULATION_TIMEOUT',
            message: 'The bounded simulation exceeded its time limit.',
          });
        else if (workerError || (code !== 0 && (!result || result.success)))
          resolve({
            success: false,
            code: 'SIMULATION_ENGINE_ERROR',
            message: 'The isolated simulation worker exited before returning results.',
          });
        else
          resolve(
            result || {
              success: false,
              code: 'SIMULATION_ENGINE_ERROR',
              message: 'The isolated simulation returned no results.',
            },
          );
      });
      child.send({ scenarios, directory, mode }, (error) => {
        if (error) {
          workerError = true;
          child.kill('SIGKILL');
        }
      });
    });
    return {
      ...identity,
      ...response,
      ...(response.success === false
        ? { errorCode: response.code, retryable: response.retryable === true }
        : {}),
      limitations: [
        'Results are conditional what-if projections from supplied assumptions, not validated forecasts or purchasing recommendations.',
        'Payments and refunds are local ledger operations; no money is transferred.',
        'Native UUIDs and timestamps are not deterministic; report aliases and financial outcomes are normalized.',
        'Steps may span multiple native transactions; inspect outcomes and final state after refusals.',
        'Each native planning run uses one conditional demand and supply path. Demand can be sampled or an explicit daily schedule. Experiment percentiles summarize supplied seeds, not calibrated confidence intervals; different seeds do not guarantee distinct paths or statistical independence. Lead-time ranges and split receipts are modeled; permanent supplier losses, automatic trend/seasonality fitting, taxes, fees and holding costs are not.',
      ],
    };
  } finally {
    try {
      if (directory) await fs.rm(directory, { recursive: true, force: true });
    } finally {
      activeRuns -= 1;
    }
  }
}

export async function runCommerceSimulation(input) {
  return run([commerceScenarioSchema.parse(input)]);
}

export async function compareCommerceSimulations(inputs) {
  return run(commerceComparisonSchema.parse(inputs));
}

export async function runInventorySimulation(input) {
  return run([inventoryPlanSchema.parse(input)], 'inventory-planning');
}

export async function compareInventorySimulations(inputs) {
  return run(inventoryPlanComparisonSchema.parse(inputs), 'inventory-planning');
}

export async function runInventoryExperiment(input) {
  return run([inventoryExperimentSchema.parse(input)], 'inventory-experiment');
}
