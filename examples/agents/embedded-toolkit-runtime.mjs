async function loadPublishedRuntime() {
  const [toolkitModule, openaiModule, genericModule, langchainModule, vercelModule, embeddedModule] =
    await Promise.all([
    import('@stateset/embedded/agent-toolkit'),
    import('@stateset/embedded/openai'),
    import('@stateset/embedded/generic'),
    import('@stateset/embedded/langchain'),
    import('@stateset/embedded/vercel-ai'),
    import('@stateset/embedded'),
    ]);

  const Commerce = embeddedModule.Commerce || embeddedModule.default?.Commerce;
  if (typeof Commerce !== 'function') {
    throw new Error('Unable to resolve Commerce from @stateset/embedded.');
  }

  return {
    source: 'package',
    Commerce,
    createEmbeddedAgentToolkit: toolkitModule.createEmbeddedAgentToolkit,
    createOpenAITools: openaiModule.createOpenAITools,
    executeOpenAIToolCall: openaiModule.executeOpenAIToolCall,
    createToolDescriptors: genericModule.createToolDescriptors,
    createCallableRegistry: genericModule.createCallableRegistry,
    createLangChainTools: langchainModule.createLangChainTools,
    createVercelAITools: vercelModule.createVercelAITools,
  };
}

async function loadWorkspaceRuntime() {
  const [toolkitModule, openaiModule, genericModule, langchainModule, vercelModule, embeddedModule] =
    await Promise.all([
    import('../../bindings/node/agent-toolkit.mjs'),
    import('../../bindings/node/openai.mjs'),
    import('../../bindings/node/generic.mjs'),
    import('../../bindings/node/langchain.mjs'),
    import('../../bindings/node/vercel-ai.mjs'),
    import('../../bindings/node/index.js'),
    ]);

  const Commerce = embeddedModule.Commerce || embeddedModule.default?.Commerce;
  if (typeof Commerce !== 'function') {
    throw new Error('Unable to resolve Commerce from the workspace binding.');
  }

  return {
    source: 'workspace',
    Commerce,
    createEmbeddedAgentToolkit: toolkitModule.createEmbeddedAgentToolkit,
    createOpenAITools: openaiModule.createOpenAITools,
    executeOpenAIToolCall: openaiModule.executeOpenAIToolCall,
    createToolDescriptors: genericModule.createToolDescriptors,
    createCallableRegistry: genericModule.createCallableRegistry,
    createLangChainTools: langchainModule.createLangChainTools,
    createVercelAITools: vercelModule.createVercelAITools,
  };
}

function isMissingPublishedPackage(error) {
  const message = error instanceof Error ? error.message : String(error);
  return (
    error &&
    error.code === 'ERR_MODULE_NOT_FOUND' &&
    (message.includes('@stateset/embedded/agent-toolkit') ||
      message.includes('@stateset/embedded/openai') ||
      message.includes('@stateset/embedded/generic') ||
      message.includes('@stateset/embedded/langchain') ||
      message.includes('@stateset/embedded/vercel-ai') ||
      message.includes('@stateset/cli/agent-toolkit') ||
      message.includes('@stateset/embedded'))
  );
}

function isQuiet() {
  return process.env.STATESET_TOOLKIT_QUIET === '1';
}

function outputMode() {
  return process.env.STATESET_TOOLKIT_OUTPUT || 'text';
}

export function emitSummary(summary, lines = [], logger = console) {
  if (outputMode() === 'json') {
    process.stdout.write(`${JSON.stringify(summary)}\n`);
    return;
  }

  if (isQuiet()) {
    return;
  }

  for (const line of lines) {
    logger.log(line);
  }
}

export async function loadEmbeddedToolkitRuntime() {
  try {
    return await loadPublishedRuntime();
  } catch (error) {
    if (!isMissingPublishedPackage(error)) {
      throw error;
    }
    return loadWorkspaceRuntime();
  }
}
