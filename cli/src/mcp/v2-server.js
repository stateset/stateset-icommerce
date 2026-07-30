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
 *     we convert each shape to JSON Schema and hand it back via `fromJsonSchema`.
 *   - Callbacks. v1 tool callbacks receive `(args, extra)`; v2 passes a request
 *     context in the second slot. It is forwarded unchanged — our wrappers only
 *     read optional fields off it (`sessionId`, auth metadata).
 */
import { McpServer, fromJsonSchema } from '@modelcontextprotocol/server';
import { z } from 'zod';
import { zodToJsonSchema } from 'zod-to-json-schema';
import { CLI_VERSION } from '../config.js';

/**
 * Tool schemas are static, but the server is rebuilt per request — converting
 * ~940 schemas on every exchange would dominate request latency. Conversion is
 * therefore memoized by tool name for the life of the process.
 *
 * @type {Map<string, object>}
 */
const schemaCache = new Map();

/** A Zod schema instance (as opposed to a raw shape) carries `_def`. */
const isZodType = (value) => Boolean(value && typeof value === 'object' && value._def);

/** An already-converted JSON Schema object, as a handful of tools declare. */
const isJsonSchema = (value) =>
  Boolean(value && typeof value === 'object' && value.type === 'object' && value.properties);

/**
 * Normalize the three `inputSchema` conventions our tool modules use — a Zod
 * raw shape (most), a Zod object instance, or a literal JSON Schema — into the
 * one thing v2 accepts.
 *
 * @param {string} name
 * @param {object} inputSchema
 * @returns {object} A Standard Schema v2 can advertise in `tools/list`.
 */
function standardSchemaFor(name, inputSchema) {
  let converted = schemaCache.get(name);
  if (!converted) {
    if (isJsonSchema(inputSchema)) {
      converted = inputSchema;
    } else {
      const zodSchema = isZodType(inputSchema) ? inputSchema : z.object(inputSchema);
      // `$refStrategy: 'none'` inlines definitions: MCP clients expect a
      // self-contained schema per tool, not one with cross-references.
      converted = zodToJsonSchema(zodSchema, { $refStrategy: 'none', target: 'jsonSchema7' });
    }
    converted = { ...converted };
    delete converted.$schema;
    schemaCache.set(name, converted);
  }
  return fromJsonSchema(converted);
}

/**
 * Convert one agent-sdk tool object into a v2 `registerTool` call.
 *
 * @param {import('@modelcontextprotocol/server').McpServer} server
 * @param {{name: string, description: string, inputSchema: object, handler: Function}} toolDef
 */
function registerAdaptedTool(server, toolDef) {
  const { name, description, inputSchema, handler } = toolDef;

  // An empty object is a legitimate no-argument tool and becomes `z.object({})`.
  const schema = inputSchema && typeof inputSchema === 'object' ? inputSchema : {};

  server.registerTool(
    name,
    { description, inputSchema: standardSchemaFor(name, schema) },
    (args, ctx) => handler(args, ctx),
  );
}

/**
 * Build a protocol-2026-07-28 MCP server exposing the full commerce tool surface.
 *
 * @param {object} options
 * @param {object} options.commerce - An open `Commerce` handle (shared or per-request).
 * @param {string} options.dbPath
 * @param {boolean} options.allowApply - Whether write tools are enabled.
 * @param {Function} options.createServer - Injected `createStatesetMcpServer`.
 * @returns {import('@modelcontextprotocol/server').McpServer}
 */
export function createStatesetV2McpServer({ commerce, dbPath, allowApply, createServer }) {
  const stateset = createServer({ commerce, dbPath, allowApply });
  const tools = stateset.getAdaptedTools();

  const server = new McpServer({ name: 'stateset-commerce', version: CLI_VERSION });
  for (const toolDef of tools) {
    registerAdaptedTool(server, toolDef);
  }
  return server;
}
