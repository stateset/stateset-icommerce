import { getStaticMcpToolDefinitions } from './mcp-server.js';
import { getStaticScaffoldToolDefinitions } from './scaffold-server.js';
import { getStaticX402ToolDefinitions } from './x402-mcp-server.js';

export function getStaticMcpServerDefinitions() {
  return [
    {
      name: 'stateset-commerce',
      source: 'cli/src/mcp-server.js',
      tools: getStaticMcpToolDefinitions(),
    },
    {
      name: 'stateset-scaffold',
      source: 'cli/src/scaffold-server.js',
      tools: getStaticScaffoldToolDefinitions(),
    },
    {
      name: 'stateset-x402',
      source: 'cli/src/x402-mcp-server.js',
      tools: getStaticX402ToolDefinitions(),
    },
  ];
}

export function getAllStaticMcpToolDefinitions() {
  return getStaticMcpServerDefinitions().flatMap((server) =>
    server.tools.map((tool) => ({
      ...tool,
      serverName: server.name,
      source: server.source,
      qualifiedName: `mcp__${server.name}__${tool.name}`,
    })),
  );
}

export function getAllStaticMcpToolNames() {
  return getAllStaticMcpToolDefinitions().map((tool) => tool.qualifiedName);
}
