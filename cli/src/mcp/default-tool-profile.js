/**
 * What `stateset-mcp`, `stateset-mcp-http` and `stateset-mcp-events` expose
 * when no `--profile` is given: exactly the core tier (src/tools/tool-tiers.js).
 *
 * Dependency-free on purpose: the entrypoints read it while parsing arguments,
 * before they lazily load the tool registry.
 */
export const DEFAULT_MCP_TOOL_PROFILE = 'core';
