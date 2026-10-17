import path from 'node:path';
import { executeCommerceScenario } from './engine.js';
import { executeInventoryPlan } from './planning.js';
import { compareInventoryReports } from './comparison.js';
import { executeInventoryExperiment } from './experiment-engine.js';

process.once('message', async ({ scenarios, directory, mode }) => {
  try {
    if (mode === 'inventory-experiment') {
      const experiment = await executeInventoryExperiment(scenarios[0], directory);
      process.send({ success: true, experiment }, () => process.exit(0));
      return;
    }
    const results = [];
    for (const [index, scenario] of scenarios.entries()) {
      results.push(
        await (mode === 'inventory-planning' ? executeInventoryPlan : executeCommerceScenario)(
          scenario,
          path.join(directory, `scenario-${index}.db`),
        ),
      );
    }
    const comparison =
      mode === 'inventory-planning' && results.length > 1
        ? compareInventoryReports(results)
        : undefined;
    process.send({ success: true, results, ...(comparison && { comparison }) }, () =>
      process.exit(0),
    );
  } catch (error) {
    process.send(
      {
        success: false,
        code: 'SIMULATION_ENGINE_ERROR',
        message: 'The isolated native simulation could not complete.',
        reason: String(error.message || 'Native operation failed')
          .split(directory)
          .join('<sandbox>')
          .replace(/[0-9a-f]{8}-[0-9a-f-]{27,}/gi, '<entity>')
          .slice(0, 500),
      },
      () => process.exit(1),
    );
  }
});
