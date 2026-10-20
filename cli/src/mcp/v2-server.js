/**
 * Protocol-2026-07-28 MCP server construction.
 *
 * `@modelcontextprotocol/server` v2 serves the 2026-07-28 revision through a
 * per-request server factory: every exchange gets a freshly built `McpServer`,
 * which is what makes the endpoint stateless. This module is the bridge from
 * our existing tool surface (built once by `createStatesetMcpServer`) onto that
 * v2 server, so both eras and every transport serve identical tools.
 *
 * Two API differences from the v1 SDK are handled here:
 *   - Schemas. v1 registers tools from a Zod *raw shape*. v2 wants a Standard
 *     Schema it can render as JSON Schema, and refuses Zod 3 outright
 *     ("Upgrade to zod >=4.2.0, or wrap your JSON Schema with fromJsonSchema()").
 *     The CLI is on Zod 3 across ~98 modules, so rather than force that upgrade
 *     we expose JSON Schema for discovery while keeping the original Zod
 *     parser for execution. Literal JSON Schemas use `fromJsonSchema`.
 *   - Callbacks. v1 tool callbacks receive `(args, extra)`; v2 passes a request
 *     context in the second slot. It is forwarded unchanged — our wrappers only
 *     read optional fields off it (`sessionId`, auth metadata).
 */
import {
  McpServer,
  ProtocolError,
  ProtocolErrorCode,
  fromJsonSchema,
  isInputRequiredResult,
} from '@modelcontextprotocol/server';
import { z } from 'zod';
import { zodToJsonSchema } from 'zod-to-json-schema';
import { CLI_VERSION } from '../config.js';
import { attachContractToResponse, normalizeToolError } from './tool-result-contract.js';

/**
 * Tool schemas are static, but the server is rebuilt per request — converting
 * ~940 schemas on every exchange would dominate request latency. Cache the
 * whole adapter by schema identity: a tool name can refer to different schemas
 * in different server configurations. Weak keys also let dynamic schemas be
 * collected when their servers are no longer used.
 *
 * @type {WeakMap<object, object>}
 */
const schemaCache = new WeakMap();
const EMPTY_SCHEMA = Object.freeze({});

/** A Zod schema instance (as opposed to a raw shape) carries `_def`. */
const isZodType = (value) => Boolean(value && typeof value === 'object' && value._def);

/** An already-converted JSON Schema object, as a handful of tools declare. */
const isJsonSchema = (value) =>
  Boolean(value && typeof value === 'object' && value.type === 'object');

/**
 * Normalize the three `inputSchema` conventions our tool modules use — a Zod
 * raw shape (most), a Zod object instance, or a literal JSON Schema — into the
 * one thing v2 accepts.
 *
 * @param {object} inputSchema
 * @returns {object} A Standard Schema v2 can advertise in `tools/list`.
 */
function standardSchemaFor(inputSchema) {
  const cached = schemaCache.get(inputSchema);
  if (cached) return cached;

  const zodSchema = isJsonSchema(inputSchema)
    ? null
    : isZodType(inputSchema)
      ? inputSchema
      : z.object(inputSchema);
  // `$refStrategy: 'none'` inlines definitions: MCP clients expect a
  // self-contained schema per tool, not one with cross-references.
  const converted = {
    ...(zodSchema
      ? zodToJsonSchema(zodSchema, { $refStrategy: 'none', target: 'jsonSchema7' })
      : inputSchema),
  };
  delete converted.$schema;

  let standardSchema;
  if (zodSchema) {
    // JSON Schema cannot express Zod refinements, transforms, or parsing
    // defaults. Run the source parser before any tool gates or handler so the
    // transport preserves those checks and passes the parsed value onward.
    standardSchema = {
      '~standard': {
        version: 1,
        vendor: 'stateset-zod3',
        jsonSchema: {
          input: () => converted,
          output: () => converted,
        },
        validate: async (data) => {
          const result = await zodSchema.safeParseAsync(data);
          return result.success
            ? { value: result.data }
            : { issues: result.error.issues.map(({ message, path }) => ({ message, path })) };
        },
      },
    };
  } else {
    standardSchema = fromJsonSchema(converted);
  }
  schemaCache.set(inputSchema, standardSchema);
  return standardSchema;
}

/**
 * Convert one agent-sdk tool object into a v2 `registerTool` call.
 *
 * @param {import('@modelcontextprotocol/server').McpServer} server
 * @param {{name: string, description: string, inputSchema: object, handler: Function}} toolDef
 * @param {Promise<void>|undefined} servicesReady
 */
function registerAdaptedTool(server, toolDef, servicesReady) {
  const { name, description, inputSchema, handler } = toolDef;

  // An empty object is a legitimate no-argument tool and becomes `z.object({})`.
  const schema = inputSchema && typeof inputSchema === 'object' ? inputSchema : EMPTY_SCHEMA;

  server.registerTool(
    name,
    { description, inputSchema: standardSchemaFor(schema) },
    async (args, ctx) => {
      // Each HTTP request creates a fresh runtime whose optional services are
      // loaded asynchronously. Discovery can run immediately; execution must
      // wait so a first call does not observe a partially initialized runtime.
      await servicesReady;
      return handler(args, ctx);
    },
  );
}

/**
 * Use the SDK's public low-level handler API to retain our result contract
 * for schema refusals as well as handler outcomes. The high-level SDK turns
 * validation failures into text-only errors before our tool wrapper runs.
 * Discovery and parsing still use the same cached Standard Schema; invalid
 * input never reaches a handler, policy gate or commerce mutation.
 */
function installToolCallContract(server, tools, servicesReady) {
  if (tools.length === 0) return;
  const byName = new Map(tools.map((tool) => [tool.name, tool]));
  server.server.setRequestHandler('tools/call', async (request, ctx) => {
    const tool = byName.get(request.params.name);
    if (!tool) {
      throw new ProtocolError(
        ProtocolErrorCode.InvalidParams,
        `Tool ${request.params.name} not found`,
      );
    }
    try {
      const inputSchema = tool.inputSchema;
      const schema = standardSchemaFor(
        inputSchema && typeof inputSchema === 'object' ? inputSchema : EMPTY_SCHEMA,
      );
      const parsed = await schema['~standard'].validate(request.params.arguments ?? {});
      if (parsed.issues) {
        const message = `Invalid parameters for tool ${tool.name}: ${JSON.stringify(parsed.issues)}`;
        return attachContractToResponse(
          { content: [{ type: 'text', text: JSON.stringify({ success: false, error: message }) }] },
          normalizeToolError({ code: 'INVALID_INPUT', message }),
        );
      }
      await servicesReady;
      const result = await tool.handler(parsed.value, ctx);
      return isInputRequiredResult(result)
        ? result
        : server.server.projectCallToolResult(result, undefined);
    } catch (error) {
      if (error instanceof ProtocolError && error.code === ProtocolErrorCode.UrlElicitationRequired)
        throw error;
      const outcome = normalizeToolError(error);
      return attachContractToResponse(
        {
          content: [
            {
              type: 'text',
              text: JSON.stringify({ success: false, error: outcome.error.message }),
            },
          ],
        },
        outcome,
      );
    }
  });
}

/**
 * Build a protocol-2026-07-28 MCP server exposing the full commerce tool surface.
 *
 * Use this as the factory for either v2 serving entry point — `createMcpHandler`
 * (HTTP, one instance per request) or `serveStdio` (one instance pinned per
 * connection).
 *
 * @param {object} options
 * @param {Function} options.createServer - Injected `createStatesetMcpServer`.
 * @param {...any} options.serverOptions - Everything else is forwarded verbatim
 *   to `createStatesetMcpServer` (`commerce`, `dbPath`, `allowApply`,
 *   `structuredToolResults`, …), so callers are not gated on this bridge
 *   knowing every option.
 * @returns {import('@modelcontextprotocol/server').McpServer}
 */
export function createStatesetV2McpServer({ createServer, ...serverOptions }) {
  const stateset = createServer(serverOptions);
  let cleanupPromise;
  const cleanup = () => {
    // Defer invocation so even a synchronous cleanup error is a rejection,
    // and repeated close/onclose calls share the same cleanup operation.
    cleanupPromise ??= Promise.resolve().then(() =>
      typeof stateset.close === 'function' ? stateset.close() : stateset.dispose?.(),
    );
    return cleanupPromise;
  };
  const reportCleanupError = (error) => {
    console.error('[mcp-server] Resource cleanup failed:', error.message);
  };

  try {
    const tools = stateset.getAdaptedTools();
    const server = new McpServer({ name: 'stateset-commerce', version: CLI_VERSION });
    // Both hooks are needed: onclose handles transport disconnects, while
    // close() can run on an instance that never connected (such as warm-up).
    const originalClose = server.close.bind(server);
    server.close = async (...args) => {
      try {
        return await originalClose(...args);
      } finally {
        await cleanup();
      }
    };
    const previousOnClose = server.server.onclose;
    server.server.onclose = () => {
      void cleanup().catch(reportCleanupError);
      previousOnClose?.();
    };

    for (const toolDef of tools) {
      registerAdaptedTool(server, toolDef, stateset.servicesReady);
    }
    installToolCallContract(server, tools, stateset.servicesReady);
    return server;
  } catch (error) {
    // Schema conversion and registration can fail after the underlying
    // server has started initialization. Release it and preserve the cause.
    void cleanup().catch(reportCleanupError);
    throw error;
  }
}
