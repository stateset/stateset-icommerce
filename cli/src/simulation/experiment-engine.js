import path from 'node:path';
import { inventoryExperimentSchema } from './schema.js';
import { executeInventoryPlan } from './planning.js';
import { summarizeInventoryExperiment } from './experiment.js';

export async function executeInventoryExperiment(input, directory) {
  const experiment = inventoryExperimentSchema.parse(input);
  const reports = [];
  // Execute sequentially inside one bounded worker. Every plan/seed pair has
  // its own database; no inventory or payments leak between sample paths.
  for (const seed of experiment.seeds) {
    for (const plan of experiment.plans) {
      reports.push(
        await executeInventoryPlan(
          { ...plan, seed },
          path.join(directory, `experiment-${reports.length}.db`),
        ),
      );
    }
  }
  return summarizeInventoryExperiment(experiment, reports);
}
