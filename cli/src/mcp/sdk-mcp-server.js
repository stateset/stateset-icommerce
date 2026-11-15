// The agent-sdk's in-process MCP server, with each tool's MCP `annotations`
// on its tools/list entry whatever agent-sdk release is installed.
//
// createSdkMcpServer() forwards a tool's `annotations` only from agent-sdk
// 0.2.27 on. That release gave tool() its `extras.annotations` argument and
// switched createSdkMcpServer() from
//
//   server.tool(name, description, inputSchema, handler)
//
// to
//
//   server.registerTool(name, { description, inputSchema, annotations }, handler)
//
// Earlier releases, including the 0.1.x line package.json allows, drop them,
// so tools/list on the in-process server carried no read-only or destructive
// hints.
//
// On such a release we create the server without tools and make that same
// registerTool call ourselves on the McpServer the SDK returns as `instance`
// (public MCP SDK API). For a zod raw-shape inputSchema, which every tool this
// CLI exposes uses, McpServer stores exactly what server.tool() would have,
// plus the annotations: same schema, same handler. On a release that forwards
// annotations the tools go to createSdkMcpServer() untouched, so the SDK's own
// per-tool handling (0.3.x adjusts zod 4 optional fields and leaves out tools
// whose schema cannot be converted) still applies.

import {
  createSdkMcpServer as agentSdkCreateSdkMcpServer,
  tool as agentSdkTool,
} from '@anthropic-ai/claude-agent-sdk';

/**
 * @typedef {{
 *   name: string,
 *   description: string,
 *   inputSchema: any,
 *   handler: (args: any, extra: any) => Promise<any>,
 *   annotations?: Record<string, unknown>,
 * }} SdkTool
 */

/**
 * Whether this agent-sdk's createSdkMcpServer() forwards tool annotations.
 *
 * tool() and createSdkMcpServer() gained annotation support in the same
 * release (0.2.27), so tool() keeping `extras.annotations` on the definition it
 * returns is a synchronous, public signal for both. A tool() that throws on
 * the probe is treated as not forwarding: the fallback registration is correct
 * on every release, the SDK path only on the ones that forward.
 *
 * @param {Function} [sdkTool] the agent-sdk `tool` export
 * @returns {boolean}
 */
export function sdkForwardsToolAnnotations(sdkTool = agentSdkTool) {
  try {
    const probe = sdkTool('annotations_probe', '', {}, async () => ({ content: [] }), {
      annotations: { readOnlyHint: true },
    });
    return probe?.annotations?.readOnlyHint === true;
  } catch {
    return false;
  }
}

/**
 * createSdkMcpServer(options), except that each tool's `annotations` reach
 * tools/list on every agent-sdk release.
 *
 * @param {{name: string, version?: string, tools?: SdkTool[]} & Record<string, unknown>} options
 * @param {{createSdkMcpServer?: Function, sdkTool?: Function}} [sdk] agent-sdk exports (injectable for tests)
 * @returns {any} what createSdkMcpServer returns: `{ type: 'sdk', name, instance }`
 */
export function createAnnotatedSdkMcpServer(
  options,
  { createSdkMcpServer = agentSdkCreateSdkMcpServer, sdkTool = agentSdkTool } = {},
) {
  const tools = options.tools ?? [];
  if (!tools.some((entry) => entry.annotations) || sdkForwardsToolAnnotations(sdkTool)) {
    return createSdkMcpServer(options);
  }

  // An empty (truthy) list keeps the constructor's `tools` capability as it
  // would be with the tools passed in.
  const server = createSdkMcpServer({ ...options, tools: [] });
  for (const { name, description, inputSchema, handler, annotations } of tools) {
    server.instance.registerTool(
      name,
      { description, inputSchema, ...(annotations && { annotations }) },
      handler,
    );
  }
  return server;
}
