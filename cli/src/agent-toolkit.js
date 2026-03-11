import { getCommerce, getGlobalManager } from './database.js';
import { createStatesetMcpServer } from './mcp-server.js';
import { createToolInputSchema } from './tool-schema.js';

function normalizeToolName(toolName) {
  if (!toolName || typeof toolName !== 'string') return '';
  return toolName.trim().replace(/^mcp__[a-z0-9_-]+__/, '');
}

function parseToolArguments(rawArguments) {
  if (rawArguments === null || rawArguments === undefined || rawArguments === '') {
    return {};
  }

  if (typeof rawArguments === 'string') {
    const parsed = JSON.parse(rawArguments);
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return parsed;
    }
    throw new Error('Tool arguments JSON must decode to an object.');
  }

  if (typeof rawArguments === 'object' && !Array.isArray(rawArguments)) {
    return rawArguments;
  }

  throw new Error('Tool arguments must be an object or a JSON string.');
}

function normalizeOpenAIToolCall(toolCall) {
  if (!toolCall || typeof toolCall !== 'object') {
    throw new Error('OpenAI tool call payload must be an object.');
  }

  const functionPayload =
    toolCall.function && typeof toolCall.function === 'object' ? toolCall.function : toolCall;
  const name = normalizeToolName(functionPayload.name || toolCall.name || '');

  if (!name) {
    throw new Error('OpenAI tool call is missing a function name.');
  }

  return {
    callId: toolCall.call_id || toolCall.id || null,
    name,
    arguments: parseToolArguments(functionPayload.arguments || toolCall.arguments),
  };
}

function normalizeToolFormat(format) {
  if (format === 'openai-responses') return 'openai';
  if (format === 'chat-completions') return 'openai';
  return format || 'generic';
}

export function createEmbeddedAgentToolkit(options = {}) {
  const dbPath = options.dbPath || './store.db';
  const ownsCommerce = !options.commerce;
  const commerce = options.commerce || getCommerce(dbPath);
  const server = createStatesetMcpServer({
    ...options,
    commerce,
    dbPath,
  });

  const getTools = ({ format = 'generic' } = {}) => {
    return server.getToolDefinitions({ format: normalizeToolFormat(format) });
  };

  const getRawTools = () => server.getRawToolDefinitions();

  const getTool = (toolName, { format = 'generic' } = {}) => {
    const normalizedFormat = normalizeToolFormat(format);
    const normalizedToolName = normalizeToolName(toolName);
    return getTools({ format: normalizedFormat }).find((tool) => {
      if (normalizedFormat === 'openai') {
        return normalizeToolName(tool?.function?.name) === normalizedToolName;
      }
      return normalizeToolName(tool?.toolName || tool?.name) === normalizedToolName;
    });
  };

  const getRawTool = (toolName) => {
    const normalizedToolName = normalizeToolName(toolName);
    return getRawTools().find((tool) => normalizeToolName(tool?.name) === normalizedToolName);
  };

  const executeTool = async (toolName, params = {}, executionOptions = {}) => {
    return server.executeTool(normalizeToolName(toolName), params, executionOptions);
  };

  const executeToolCalls = async (toolCalls = [], executionOptions = {}) => {
    const normalizedCalls = Array.isArray(toolCalls) ? toolCalls : [];
    const results = [];

    for (const toolCall of normalizedCalls) {
      if (toolCall && typeof toolCall === 'object' && 'function' in toolCall) {
        results.push(await executeOpenAIToolCall(toolCall, executionOptions));
        continue;
      }

      const name = normalizeToolName(toolCall?.name || toolCall?.tool || '');
      results.push({
        callId: toolCall?.callId || toolCall?.id || null,
        name,
        arguments: parseToolArguments(toolCall?.arguments || toolCall?.params || {}),
        result: await executeTool(name, parseToolArguments(toolCall?.arguments || toolCall?.params || {}), executionOptions),
      });
    }

    return results;
  };

  const executeOpenAIToolCall = async (toolCall, executionOptions = {}) => {
    const normalizedCall = normalizeOpenAIToolCall(toolCall);
    const result = await executeTool(normalizedCall.name, normalizedCall.arguments, executionOptions);

    return {
      ...normalizedCall,
      result,
      outputMessage: normalizedCall.callId
        ? {
            type: 'function_call_output',
            call_id: normalizedCall.callId,
            output: JSON.stringify(result),
          }
        : null,
    };
  };

  const simulateMutation = async ({
    tool,
    params = {},
    policyDomain = null,
    requestId = null,
    sessionId = null,
    includeHooks = false,
  }) => {
    return server.simulateMutation({
      tool: normalizeToolName(tool),
      params,
      policyDomain,
      requestId,
      sessionId,
      includeHooks,
    });
  };

  const createVercelAITools = ({ tool: toolFactory, filter = null, executionOptions = {} } = {}) => {
    if (typeof toolFactory !== 'function') {
      throw new Error('createVercelAITools requires the Vercel AI tool() factory.');
    }

    const selected = getRawTools().filter((tool) =>
      Array.isArray(filter) && filter.length > 0 ? filter.includes(tool.name) : true,
    );

    return Object.fromEntries(
      selected.map((toolDef) => [
        toolDef.name,
        toolFactory({
          description: toolDef.description,
          parameters: createToolInputSchema(toolDef.inputSchema),
          execute: async (params) => executeTool(toolDef.name, params, executionOptions),
        }),
      ]),
    );
  };

  const createLangChainTools = ({
    DynamicStructuredTool,
    filter = null,
    executionOptions = {},
  } = {}) => {
    if (typeof DynamicStructuredTool !== 'function') {
      throw new Error(
        'createLangChainTools requires the LangChain DynamicStructuredTool constructor.',
      );
    }

    return getRawTools()
      .filter((tool) => (Array.isArray(filter) && filter.length > 0 ? filter.includes(tool.name) : true))
      .map(
        (toolDef) =>
          new DynamicStructuredTool({
            name: toolDef.name,
            description: toolDef.description,
            schema: createToolInputSchema(toolDef.inputSchema),
            func: async (params) => {
              const result = await executeTool(toolDef.name, params, executionOptions);
              return JSON.stringify(result);
            },
          }),
      );
  };

  const createToolDescriptors = ({ filter = null, executionOptions = {} } = {}) => {
    return getRawTools()
      .filter((tool) => (Array.isArray(filter) && filter.length > 0 ? filter.includes(tool.name) : true))
      .map((toolDef) => ({
        name: toolDef.name,
        description: toolDef.description,
        schema: createToolInputSchema(toolDef.inputSchema),
        inputSchema: toolDef.inputSchema,
        permission: toolDef.permission,
        policyDomain: toolDef.policyDomain,
        runtime: toolDef.runtime,
        execute: async (params) => executeTool(toolDef.name, params, executionOptions),
      }));
  };

  const close = () => {
    if (!ownsCommerce) return false;
    return getGlobalManager().close(dbPath);
  };

  return {
    engine: 'stateset-icommerce',
    dbPath,
    commerce,
    server,
    getTools,
    getRawTools,
    listTools: getTools,
    getTool,
    getRawTool,
    executeTool,
    executeToolCalls,
    executePlan: (planOptions) => server.executePlan(planOptions),
    simulatePlan: (planOptions) => server.simulatePlan(planOptions),
    getRuntimeContract: (contractOptions) => server.getRuntimeContract(contractOptions),
    simulateMutation,
    replayMutation: (replayOptions) => server.replayMutation(replayOptions),
    getReplayLog: (replayOptions) => server.getReplayLog(replayOptions),
    executeOpenAIToolCall,
    createVercelAITools,
    createLangChainTools,
    createToolDescriptors,
    close,
  };
}

export const createEmbeddedAgentKit = createEmbeddedAgentToolkit;

export default createEmbeddedAgentToolkit;
