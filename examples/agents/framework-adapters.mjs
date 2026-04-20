import { isMain } from './x402-demo-helpers.mjs';
import { emitSummary, loadEmbeddedToolkitRuntime } from './embedded-toolkit-runtime.mjs';

export async function runFrameworkAdaptersDemo({ logger = console } = {}) {
  const {
    Commerce,
    createLangChainTools,
    createToolDescriptors,
    createVercelAITools,
    source,
  } = await loadEmbeddedToolkitRuntime();

  const commerce = new Commerce(':memory:');
  const vercelTools = createVercelAITools(commerce, {
    tool: (definition) => definition,
    filter: ['list_customers'],
  });

  class DynamicStructuredTool {
    constructor(config) {
      Object.assign(this, config);
    }
  }

  const langChainTools = createLangChainTools(commerce, {
    DynamicStructuredTool,
    filter: ['list_customers'],
  });

  const genericTools = createToolDescriptors(commerce, {
    filter: ['list_customers'],
  });

  const result = await vercelTools.list_customers.execute({});
  const summary = {
    runtimeSource: source,
    frameworks: ['vercel-ai', 'langchain', 'generic'],
    vercelToolKeys: Object.keys(vercelTools),
    langChainToolCount: langChainTools.length,
    genericDescriptorCount: genericTools.length,
    status: result.status,
  };

  emitSummary(
    summary,
    [
      `Runtime source: ${source}`,
      `Vercel AI tool keys: ${Object.keys(vercelTools).join(', ')}`,
      `LangChain tool count: ${langChainTools.length}`,
      `Generic descriptor count: ${genericTools.length}`,
      `Tool status: ${result.status}`,
    ],
    logger,
  );

  return summary;
}

if (isMain(import.meta)) {
  runFrameworkAdaptersDemo().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
