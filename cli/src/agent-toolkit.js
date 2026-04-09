import { getCommerce, getGlobalManager } from './database.js';
import { createStatesetMcpServer } from './mcp-server.js';
import { createMppHttpAgent, discoverMppHttpService } from './mpp/agent.js';
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
  if (format === 'anthropic-sdk') return 'anthropic';
  if (format === 'anthropic-messages') return 'anthropic';
  if (format === 'claude') return 'anthropic';
  return format || 'generic';
}

function isPlainObject(value) {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

function buildRemoteHttpUrl(baseUrl, routePath, query = null) {
  const url = new URL(String(routePath || '/'), new URL(baseUrl).toString());
  if (isPlainObject(query)) {
    for (const [key, value] of Object.entries(query)) {
      if (value === undefined || value === null) continue;
      if (Array.isArray(value)) {
        for (const item of value) {
          if (item === undefined || item === null) continue;
          url.searchParams.append(key, String(item));
        }
        continue;
      }
      url.searchParams.set(key, String(value));
    }
  }
  return url.toString();
}

function normalizeRemoteHttpHeaders(headers = {}) {
  if (!headers || typeof headers !== 'object') return {};
  return Object.fromEntries(Object.entries(headers).map(([key, value]) => [key, String(value)]));
}

function createRemoteHttpDescriptor(route, baseUrl, executeRoute) {
  return {
    name: route.operationId || `${route.method} ${route.path}`,
    description: route.summary || route.description || `${route.method} ${route.path}`,
    baseUrl,
    path: route.path,
    method: route.method,
    tags: Array.isArray(route.tags) ? [...route.tags] : [],
    payable: Boolean(route.paymentInfo),
    paymentInfo: route.paymentInfo || null,
    pluginId: route.pluginId || null,
    serviceInfo: route.serviceInfo || null,
    execute: async (request = {}, executionOptions = {}) =>
      executeRoute(route, request, executionOptions),
    executeWithPayment: async (request = {}, paymentOptions = {}, executionOptions = {}) =>
      executeRoute(route, request, {
        ...executionOptions,
        payment: {
          ...((executionOptions && executionOptions.payment) || {}),
          ...(paymentOptions || {}),
        },
      }),
  };
}

/**
 * Create the embedded toolkit facade over the live StateSet MCP tool runtime.
 *
 * All options are forwarded to `createStatesetMcpServer()`, so the toolkit and
 * MCP server stay aligned on permissions, policies, treasury, and delegation.
 *
 * @param {Object} [options]
 * @param {string} [options.dbPath='./store.db'] SQLite database path when no commerce instance is supplied.
 * @param {boolean} [options.allowApply=false] Enable write tools instead of preview-only execution.
 * @param {Object} [options.commerce] Reuse an existing embedded commerce instance.
 * @param {Object|null} [options.autonomousEngine=null] Enable runtime tools such as `delegate_to_agent`.
 * @param {string} [options.policyStorePath] Optional policy store root. When omitted, the MCP server derives `.stateset/` next to `dbPath`.
 * @param {Object} [options.treasury] Optional treasury configuration for priced tools.
 * @param {Object} [options.mpp] Optional Machine Payments Protocol defaults.
 * @returns {Object} Toolkit helpers for tool discovery, execution, and framework adapters.
 */
export function createEmbeddedAgentToolkit(options = {}) {
  const dbPath = options.dbPath || './store.db';
  const ownsCommerce = !options.commerce;
  const commerce = options.commerce || getCommerce(dbPath);
  const defaultPaymentOptions = {
    ...(options.mpp || {}),
    payer: options?.mpp?.payer || options?.treasury?.agentId || options?.agentId || null,
  };
  const server = createStatesetMcpServer({
    ...options,
    commerce,
    dbPath,
  });

  const getTools = ({ format = 'generic' } = {}) => {
    return server.getToolDefinitions({ format: normalizeToolFormat(format) });
  };

  const getRawTools = () => server.getRawToolDefinitions();

  const getToolCatalog = async ({ tool = null, format = 'generic', payableOnly = false } = {}) =>
    server.getToolCatalog({ tool, format, payableOnly });

  const getPayableToolCatalog = async ({ tool = null, format = 'generic' } = {}) =>
    server.getToolCatalog({ tool, format, payableOnly: true });

  const getPaymentDiscovery = async ({ tool = null, format = 'json', pricedOnly = false } = {}) =>
    server.getPaymentDiscovery({ tool, format, pricedOnly });

  const discoverPayableTools = async ({ tool = null, format = 'json' } = {}) =>
    server.getPaymentDiscovery({ tool, format, pricedOnly: true });

  const prepareToolPayment = async ({
    tool,
    params = {},
    requestId = null,
    sessionId = null,
    includeSchema = false,
  } = {}) =>
    server.preparePayment({
      tool: normalizeToolName(tool),
      params,
      requestId,
      sessionId,
      includeSchema,
    });

  const createHttpPaymentAgent = (httpOptions = {}) =>
    createMppHttpAgent({
      ...defaultPaymentOptions,
      ...(httpOptions || {}),
    });

  const discoverRemotePaymentService = async (baseUrl, options = {}) =>
    discoverMppHttpService(baseUrl, {
      ...defaultPaymentOptions,
      ...(options || {}),
    });

  const discoverRemotePayableRoutes = async (baseUrl, options = {}) => {
    const discovery = await discoverRemotePaymentService(baseUrl, options);
    return discovery.payableRoutes;
  };

  const executeRemoteHttpRoute = async (baseUrl, route, request = {}, executionOptions = {}) => {
    const method = String(request?.method || route?.method || 'GET').toUpperCase();
    const url = buildRemoteHttpUrl(baseUrl, route?.path || '/', request?.query || null);
    const headers = normalizeRemoteHttpHeaders(request?.headers);
    const requestOptions = {
      ...request,
      method,
      headers,
    };
    delete requestOptions.query;

    const httpAgent = createHttpPaymentAgent({
      ...((executionOptions && executionOptions.payment) || {}),
      ...((executionOptions && executionOptions.http) || {}),
    });

    return httpAgent.fetch(url, requestOptions);
  };

  const createRemoteHttpToolDescriptors = async (baseUrl, options = {}) => {
    const { executionOptions = {}, ...discoveryOptions } = options || {};
    const discovery = await discoverRemotePaymentService(baseUrl, discoveryOptions);

    return discovery.payableRoutes.map((route) =>
      createRemoteHttpDescriptor(
        route,
        discovery.baseUrl,
        (selectedRoute, request, routeExecutionOptions) =>
          executeRemoteHttpRoute(discovery.baseUrl, selectedRoute, request, {
            ...executionOptions,
            ...(routeExecutionOptions || {}),
            payment: {
              ...((executionOptions && executionOptions.payment) || {}),
              ...((routeExecutionOptions && routeExecutionOptions.payment) || {}),
            },
            http: {
              ...((executionOptions && executionOptions.http) || {}),
              ...((routeExecutionOptions && routeExecutionOptions.http) || {}),
            },
          }),
      ),
    );
  };

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

  const executeToolWithPayment = async (toolName, params = {}, executionOptions = {}) => {
    const mergedPayment = {
      ...defaultPaymentOptions,
      ...((executionOptions && executionOptions.payment) || {}),
    };
    return server.executeToolWithPayment(normalizeToolName(toolName), params, {
      ...executionOptions,
      payment: mergedPayment,
    });
  };

  const runTool = async (toolName, params = {}, executionOptions = {}) => {
    if (executionOptions && executionOptions.payment) {
      return executeToolWithPayment(toolName, params, executionOptions);
    }
    return executeTool(toolName, params, executionOptions);
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
        result: await runTool(
          name,
          parseToolArguments(toolCall?.arguments || toolCall?.params || {}),
          executionOptions,
        ),
      });
    }

    return results;
  };

  const executeOpenAIToolCall = async (toolCall, executionOptions = {}) => {
    const normalizedCall = normalizeOpenAIToolCall(toolCall);
    const result = await runTool(normalizedCall.name, normalizedCall.arguments, executionOptions);

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

  const executePaidTool = async (toolName, params = {}, executionOptions = {}) =>
    executeToolWithPayment(toolName, params, executionOptions);

  const executePaidOpenAIToolCall = async (toolCall, executionOptions = {}) =>
    executeOpenAIToolCall(toolCall, {
      ...executionOptions,
      payment: {
        ...defaultPaymentOptions,
        ...((executionOptions && executionOptions.payment) || {}),
      },
    });

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

  const createVercelAITools = ({
    tool: toolFactory,
    filter = null,
    executionOptions = {},
  } = {}) => {
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
          execute: async (params) => runTool(toolDef.name, params, executionOptions),
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
      .filter((tool) =>
        Array.isArray(filter) && filter.length > 0 ? filter.includes(tool.name) : true,
      )
      .map(
        (toolDef) =>
          new DynamicStructuredTool({
            name: toolDef.name,
            description: toolDef.description,
            schema: createToolInputSchema(toolDef.inputSchema),
            func: async (params) => {
              const result = await runTool(toolDef.name, params, executionOptions);
              return JSON.stringify(result);
            },
          }),
      );
  };

  const createToolDescriptors = ({ filter = null, executionOptions = {} } = {}) => {
    return getRawTools()
      .filter((tool) =>
        Array.isArray(filter) && filter.length > 0 ? filter.includes(tool.name) : true,
      )
      .map((toolDef) => ({
        name: toolDef.name,
        description: toolDef.description,
        schema: createToolInputSchema(toolDef.inputSchema),
        inputSchema: toolDef.inputSchema,
        permission: toolDef.permission,
        policyDomain: toolDef.policyDomain,
        runtime: toolDef.runtime,
        preparePayment: async ({
          params = {},
          requestId = null,
          sessionId = null,
          includeSchema = false,
        } = {}) =>
          prepareToolPayment({
            tool: toolDef.name,
            params,
            requestId,
            sessionId,
            includeSchema,
          }),
        execute: async (params) => runTool(toolDef.name, params, executionOptions),
        executeWithPayment: async (params, paymentOptions = {}) =>
          executeToolWithPayment(toolDef.name, params, {
            ...executionOptions,
            payment: {
              ...defaultPaymentOptions,
              ...((executionOptions && executionOptions.payment) || {}),
              ...(paymentOptions || {}),
            },
          }),
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
    getToolCatalog,
    getPayableToolCatalog,
    getPaymentDiscovery,
    discoverPayableTools,
    prepareToolPayment,
    createHttpPaymentAgent,
    discoverRemotePaymentService,
    discoverRemotePayableRoutes,
    executeRemoteHttpRoute,
    createRemoteHttpToolDescriptors,
    listTools: getTools,
    getTool,
    getRawTool,
    executeTool,
    executeToolWithPayment,
    executePaidTool,
    executeToolCalls,
    executePlan: (planOptions) => server.executePlan(planOptions),
    simulatePlan: (planOptions) => server.simulatePlan(planOptions),
    getRuntimeContract: (contractOptions) => server.getRuntimeContract(contractOptions),
    simulateMutation,
    replayMutation: (replayOptions) => server.replayMutation(replayOptions),
    getReplayLog: (replayOptions) => server.getReplayLog(replayOptions),
    executeOpenAIToolCall,
    executePaidOpenAIToolCall,
    createVercelAITools,
    createLangChainTools,
    createToolDescriptors,
    close,
  };
}

export const createEmbeddedAgentKit = createEmbeddedAgentToolkit;

export default createEmbeddedAgentToolkit;
