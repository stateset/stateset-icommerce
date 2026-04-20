import { isMain } from './x402-demo-helpers.mjs';
import { emitSummary, loadEmbeddedToolkitRuntime } from './embedded-toolkit-runtime.mjs';

export async function runOpenAIEmbeddedToolkitDemo({ logger = console } = {}) {
  const { Commerce, createOpenAITools, executeOpenAIToolCall, source } =
    await loadEmbeddedToolkitRuntime();

  const commerce = new Commerce(':memory:');
  const tools = createOpenAITools(commerce, {
    filter: ['list_customers'],
  });

  const execution = await executeOpenAIToolCall(commerce, {
    call_id: 'demo_call_1',
    function: {
      name: 'list_customers',
      arguments: '{}',
    },
  });

  const summary = {
    runtimeSource: source,
    surface: 'openai',
    toolCount: tools.length,
    firstTool: tools[0]?.function?.name || null,
    status: execution.result.status,
    outputMessageType: execution.outputMessage?.type || null,
  };

  emitSummary(
    summary,
    [
      `Runtime source: ${source}`,
      `Exported tools: ${tools.length}`,
      `First tool: ${tools[0]?.function?.name || 'n/a'}`,
      `Tool status: ${execution.result.status}`,
      'Responses API payload:',
      JSON.stringify(execution.outputMessage, null, 2),
    ],
    logger,
  );

  return summary;
}

if (isMain(import.meta)) {
  runOpenAIEmbeddedToolkitDemo().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
