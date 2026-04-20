import { isMain } from './x402-demo-helpers.mjs';
import { emitSummary, loadEmbeddedToolkitRuntime } from './embedded-toolkit-runtime.mjs';

export async function runCustomFrameworkAdapterDemo({ logger = console } = {}) {
  const { Commerce, createCallableRegistry, createToolDescriptors, source } =
    await loadEmbeddedToolkitRuntime();

  const commerce = new Commerce(':memory:');
  const descriptors = createToolDescriptors(commerce, {
    filter: ['list_customers', 'list_orders', 'get_sales_summary'],
  });

  // This is the lowest-common-denominator contract most custom agent runtimes need:
  // { name, description, schema, execute }.
  const registry = createCallableRegistry(commerce, {
    filter: ['list_customers', 'list_orders', 'get_sales_summary'],
  });

  const result = await registry.list_customers({ limit: 5 });
  const summary = {
    runtimeSource: source,
    surface: 'generic',
    descriptorCount: descriptors.length,
    registryKeys: Object.keys(registry),
    status: result.status,
  };

  emitSummary(
    summary,
    [
      `Runtime source: ${source}`,
      'Framework-neutral descriptors:',
      ...descriptors.map((descriptor) => `- ${descriptor.name}: ${descriptor.description}`),
      '',
      'Executed through generic descriptor surface:',
      JSON.stringify(result, null, 2),
    ],
    logger,
  );

  return summary;
}

if (isMain(import.meta)) {
  runCustomFrameworkAdapterDemo().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
