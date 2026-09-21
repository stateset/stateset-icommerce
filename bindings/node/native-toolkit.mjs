// Dependency-free agent toolkit over a `Commerce` instance.
//
// Every public method reachable from a `Commerce` getter is described in
// `tool-descriptors.json` (generated from index.d.ts by
// scripts/generate-tool-descriptors.mjs). This module turns those descriptors
// into framework-shaped tool definitions and executes them against the
// binding directly, so `@stateset/embedded/openai`, `/generic`, `/langchain`
// and `/vercel-ai` work with nothing but this package installed.
//
// It mirrors the `--apply` posture of the CLI and MCP server: a write tool
// does not execute unless the toolkit was created with `allowApply: true`;
// it returns a preview object instead. When `@stateset/cli` is installed,
// `toolkit-helpers.mjs` prefers that toolkit, which adds policy, budgets,
// replay and kernel governance on top of the same engine.
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const catalog = require('./tool-descriptors.json');

/** Wire formats (OpenAI, Anthropic, most MCP clients) forbid `.` in names. */
const WIRE_SEPARATOR = '__';

export function toWireName(name) {
  return String(name).replace(/\./g, WIRE_SEPARATOR);
}

export function toCanonicalName(name) {
  if (!name || typeof name !== 'string') return '';
  const stripped = name.trim().replace(/^mcp__[a-z0-9_-]+__/, '');
  return stripped.includes('.') ? stripped : stripped.replace(/__/g, '.');
}

const PREVIEW_NOTE =
  'Write tools are preview-only by default. This call was not executed. ' +
  'Create the toolkit with `allowApply: true` (or pass `allowApply: true` to the adapter) to enable mutations.';

// ---------------------------------------------------------------------------
// Schema inlining: the shipped file keeps every interface once under
// `definitions`; consumers get self-contained schemas.
// ---------------------------------------------------------------------------

const inlinedDefinitions = new Map();

function inlineSchema(schema, stack = []) {
  if (Array.isArray(schema)) return schema.map((item) => inlineSchema(item, stack));
  if (!schema || typeof schema !== 'object') return schema;

  if (typeof schema.$ref === 'string') {
    const name = schema.$ref.replace('#/definitions/', '');
    if (inlinedDefinitions.has(name)) return inlinedDefinitions.get(name);
    const definition = catalog.definitions?.[name];
    if (!definition) return { type: 'object', description: name };
    if (stack.includes(name)) {
      return { type: 'object', description: `${name} (recursive; see the enclosing schema)` };
    }
    const resolved = inlineSchema(definition, [...stack, name]);
    if (!stack.length) inlinedDefinitions.set(name, resolved);
    return resolved;
  }

  const out = {};
  for (const [key, value] of Object.entries(schema)) {
    out[key] = key === 'enum' || key === 'required' ? value : inlineSchema(value, stack);
  }
  return out;
}

let inlinedTools = null;
function getInlinedTools() {
  if (!inlinedTools) {
    inlinedTools = catalog.tools.map((tool) => ({
      ...tool,
      positional: [...tool.positional],
      parameters: inlineSchema(tool.parameters),
    }));
  }
  return inlinedTools;
}

/** The static descriptor catalog: `{ meta, tools }` with schemas inlined. */
export function loadToolDescriptors() {
  return { meta: catalog.meta, tools: getInlinedTools() };
}

// ---------------------------------------------------------------------------
// Helpers shared with the CLI toolkit's call shapes
// ---------------------------------------------------------------------------

function parseToolArguments(rawArguments) {
  if (rawArguments === null || rawArguments === undefined || rawArguments === '') return {};
  if (typeof rawArguments === 'string') {
    const parsed = JSON.parse(rawArguments);
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) return parsed;
    throw new Error('Tool arguments JSON must decode to an object.');
  }
  if (typeof rawArguments === 'object' && !Array.isArray(rawArguments)) return rawArguments;
  throw new Error('Tool arguments must be an object or a JSON string.');
}

function normalizeOpenAIToolCall(toolCall) {
  if (!toolCall || typeof toolCall !== 'object') {
    throw new Error('OpenAI tool call payload must be an object.');
  }
  const functionPayload =
    toolCall.function && typeof toolCall.function === 'object' ? toolCall.function : toolCall;
  const name = toCanonicalName(functionPayload.name || toolCall.name || '');
  if (!name) throw new Error('OpenAI tool call is missing a function name.');
  return {
    callId: toolCall.call_id || toolCall.id || null,
    name,
    arguments: parseToolArguments(functionPayload.arguments ?? toolCall.arguments),
  };
}

function normalizeFormat(format) {
  switch (format) {
    case undefined:
    case null:
    case '':
    case 'generic':
    case 'raw':
      return 'generic';
    case 'openai':
    case 'openai-responses':
    case 'chat-completions':
      return 'openai';
    case 'anthropic':
    case 'anthropic-sdk':
    case 'anthropic-messages':
    case 'claude':
      return 'anthropic';
    case 'mcp':
      return 'mcp';
    default:
      throw new Error(
        `Unknown tool format '${format}'. Expected one of generic, openai, anthropic, mcp.`,
      );
  }
}

function matchesFilter(tool, filter) {
  if (typeof filter === 'function') return Boolean(filter(tool));
  if (!Array.isArray(filter) || filter.length === 0) return true;
  return filter.some((entry) => {
    const wanted = toCanonicalName(entry);
    if (wanted === tool.name) return true;
    if (wanted.endsWith('.*')) return tool.module === wanted.slice(0, -2);
    return false;
  });
}

function errorEnvelope(error, tool) {
  if (error && typeof error === 'object') {
    return {
      error: {
        code: typeof error.code === 'string' && error.code ? error.code : 'INTERNAL',
        message: error.message || String(error),
        details: error.details ?? null,
        tool,
      },
    };
  }
  return { error: { code: 'INTERNAL', message: String(error), details: null, tool } };
}

function invalidArgument(tool, message, details = null) {
  return { error: { code: 'INVALID_ARGUMENT', message, details, tool } };
}

// ---------------------------------------------------------------------------
// Toolkit
// ---------------------------------------------------------------------------

/**
 * Create a toolkit over `commerce` that needs nothing beyond this package.
 *
 * @param {import('./index').Commerce} commerce
 * @param {{ allowApply?: boolean, filter?: Array<string> | ((tool: object) => boolean) | null }} [options]
 */
export function createNativeToolkit(commerce, { allowApply = false, filter = null } = {}) {
  if (!commerce || typeof commerce !== 'object') {
    throw new Error('createNativeToolkit requires a Commerce instance.');
  }

  const tools = getInlinedTools().filter((tool) => matchesFilter(tool, filter));
  const byName = new Map(tools.map((tool) => [tool.name, tool]));

  const toRaw = (tool) => ({
    name: tool.name,
    wireName: toWireName(tool.name),
    description: tool.description,
    inputSchema: tool.parameters,
    permission: tool.permission,
    readOnly: tool.readOnly,
    module: tool.module,
    method: tool.method,
    positional: [...tool.positional],
    returnType: tool.returnType,
    signature: tool.signature,
    runtime: 'native',
  });

  const format = (tool, normalizedFormat) => {
    switch (normalizedFormat) {
      case 'openai':
        return {
          type: 'function',
          function: {
            name: toWireName(tool.name),
            description: tool.description,
            parameters: tool.parameters,
          },
        };
      case 'anthropic':
        return {
          name: toWireName(tool.name),
          description: tool.description,
          input_schema: tool.parameters,
        };
      case 'mcp':
        return {
          name: toWireName(tool.name),
          title: tool.name,
          description: tool.description,
          inputSchema: tool.parameters,
          annotations: {
            title: tool.name,
            readOnlyHint: tool.readOnly,
            openWorldHint: false,
          },
        };
      default:
        return toRaw(tool);
    }
  };

  const getRawTools = () => tools.map(toRaw);

  const getTools = ({ format: requested = 'generic' } = {}) => {
    const normalizedFormat = normalizeFormat(requested);
    return tools.map((tool) => format(tool, normalizedFormat));
  };

  const getRawTool = (toolName) => {
    const tool = byName.get(toCanonicalName(toolName));
    return tool ? toRaw(tool) : undefined;
  };

  const getTool = (toolName, { format: requested = 'generic' } = {}) => {
    const tool = byName.get(toCanonicalName(toolName));
    return tool ? format(tool, normalizeFormat(requested)) : undefined;
  };

  // `executionOptions` is accepted for call-shape parity with the CLI toolkit
  // (payment, http, trace ids); the native executor has nothing to do with it.
  const executeTool = async (toolName, params = {}, _executionOptions = {}) => {
    const name = toCanonicalName(toolName);
    const tool = byName.get(name);
    if (!tool) {
      return {
        error: {
          code: 'TOOL_NOT_FOUND',
          message: `Unknown tool '${toolName}'.`,
          details: null,
          tool: name || String(toolName),
        },
      };
    }

    let args;
    try {
      args = parseToolArguments(params);
    } catch (error) {
      return invalidArgument(name, error.message);
    }

    const unknownKeys = Object.keys(args).filter((key) => !tool.positional.includes(key));
    if (unknownKeys.length > 0) {
      return invalidArgument(
        name,
        `Unknown parameter(s) ${unknownKeys.join(', ')}; expected ${tool.positional.join(', ') || 'no parameters'}.`,
        { unknown: unknownKeys, expected: tool.positional },
      );
    }
    const missing = (tool.parameters.required || []).filter(
      (key) => args[key] === undefined || args[key] === null,
    );
    if (missing.length > 0) {
      return invalidArgument(name, `Missing required parameter(s) ${missing.join(', ')}.`, {
        missing,
      });
    }

    if (!tool.readOnly && !allowApply) {
      return { preview: true, tool: name, params: args, note: PREVIEW_NOTE };
    }

    const target = commerce[tool.module];
    if (!target || typeof target[tool.method] !== 'function') {
      return {
        error: {
          code: 'UNSUPPORTED',
          message: `This Commerce instance does not expose ${tool.module}.${tool.method}.`,
          details: null,
          tool: name,
        },
      };
    }

    const positionalArgs = tool.positional.map((key) => args[key]);
    while (positionalArgs.length > 0 && positionalArgs[positionalArgs.length - 1] === undefined) {
      positionalArgs.pop();
    }

    try {
      const result = await target[tool.method](...positionalArgs);
      return result === undefined ? { ok: true } : result;
    } catch (error) {
      return errorEnvelope(error, name);
    }
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

  const executeToolCalls = async (toolCalls = [], executionOptions = {}) => {
    const results = [];
    for (const toolCall of Array.isArray(toolCalls) ? toolCalls : []) {
      if (toolCall && typeof toolCall === 'object' && 'function' in toolCall) {
        results.push(await executeOpenAIToolCall(toolCall, executionOptions));
        continue;
      }
      const name = toCanonicalName(toolCall?.name || toolCall?.tool || '');
      const args = parseToolArguments(toolCall?.arguments ?? toolCall?.params ?? {});
      results.push({
        callId: toolCall?.callId || toolCall?.id || null,
        name,
        arguments: args,
        result: await executeTool(name, args, executionOptions),
      });
    }
    return results;
  };

  const selectTools = (subFilter) => tools.filter((tool) => matchesFilter(tool, subFilter));

  const createToolDescriptors = ({ filter: subFilter = null, executionOptions = {} } = {}) =>
    selectTools(subFilter).map((tool) => ({
      name: tool.name,
      wireName: toWireName(tool.name),
      description: tool.description,
      schema: tool.parameters,
      inputSchema: tool.parameters,
      permission: tool.permission,
      readOnly: tool.readOnly,
      runtime: 'native',
      execute: async (params = {}) => executeTool(tool.name, params, executionOptions),
    }));

  const createVercelAITools = ({
    tool: toolFactory,
    filter: subFilter = null,
    executionOptions = {},
    jsonSchema = null,
  } = {}) => {
    if (typeof toolFactory !== 'function') {
      throw new Error('createVercelAITools requires the Vercel AI tool() factory.');
    }
    return Object.fromEntries(
      selectTools(subFilter).map((tool) => {
        const schema = typeof jsonSchema === 'function' ? jsonSchema(tool.parameters) : tool.parameters;
        return [
          toWireName(tool.name),
          toolFactory({
            description: tool.description,
            // AI SDK v4 reads `parameters`, v5 reads `inputSchema`.
            parameters: schema,
            inputSchema: schema,
            execute: async (params) => executeTool(tool.name, params, executionOptions),
          }),
        ];
      }),
    );
  };

  const createLangChainTools = ({
    DynamicStructuredTool,
    filter: subFilter = null,
    executionOptions = {},
  } = {}) => {
    if (typeof DynamicStructuredTool !== 'function') {
      throw new Error('createLangChainTools requires the LangChain DynamicStructuredTool constructor.');
    }
    return selectTools(subFilter).map(
      (tool) =>
        new DynamicStructuredTool({
          name: toWireName(tool.name),
          description: tool.description,
          schema: tool.parameters,
          func: async (params) => JSON.stringify(await executeTool(tool.name, params, executionOptions)),
        }),
    );
  };

  return {
    engine: 'stateset-icommerce',
    backend: 'native',
    runtime: 'native',
    allowApply,
    commerce,
    getTools,
    listTools: getTools,
    getRawTools,
    getTool,
    getRawTool,
    executeTool,
    executeToolCalls,
    executeOpenAIToolCall,
    createToolDescriptors,
    createVercelAITools,
    createLangChainTools,
    /** The toolkit never owns the Commerce instance; closing is the caller's job. */
    close: () => false,
  };
}

export default createNativeToolkit;
