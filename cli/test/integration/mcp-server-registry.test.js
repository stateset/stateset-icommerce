import test from 'node:test';
import assert from 'node:assert/strict';
import { TOOL_NAMES } from '../../src/mcp-server.js';
import { SCAFFOLD_MCP_TOOL_NAMES } from '../../src/scaffold-server.js';
import { X402_MCP_TOOL_NAMES } from '../../src/x402-mcp-server.js';
import {
  getAllStaticMcpToolDefinitions,
  getAllStaticMcpToolNames,
  getStaticMcpServerDefinitions,
} from '../../src/mcp-server-registry.js';

test('MCP server registry covers every exported server namespace', () => {
  const servers = getStaticMcpServerDefinitions();
  const serverNames = servers.map((server) => server.name).sort();

  assert.deepEqual(serverNames, ['stateset-commerce', 'stateset-scaffold', 'stateset-x402']);
  assert.equal(new Set(serverNames).size, serverNames.length);
});

test('registry tool names match per-server exports', () => {
  const allNames = new Set(getAllStaticMcpToolNames());
  const expectedNames = new Set([
    ...TOOL_NAMES,
    ...SCAFFOLD_MCP_TOOL_NAMES,
    ...X402_MCP_TOOL_NAMES.map((name) => `mcp__stateset-x402__${name}`),
  ]);

  assert.deepEqual([...allNames].sort(), [...expectedNames].sort());
});

test('registry tool definitions have stable metadata', () => {
  const tools = getAllStaticMcpToolDefinitions();

  assert.ok(tools.length > TOOL_NAMES.length);
  for (const tool of tools) {
    assert.ok(tool.name);
    assert.ok(tool.serverName);
    assert.ok(tool.source);
    assert.ok(tool.description);
    assert.ok(tool.qualifiedName.startsWith(`mcp__${tool.serverName}__`));
    assert.ok(['read', 'write', 'delete', 'admin', 'unknown'].includes(tool.permission));
  }
});

test('server registry counts match the flattened registry', () => {
  const servers = getStaticMcpServerDefinitions();
  const flattenedCounts = getAllStaticMcpToolDefinitions().reduce((counts, tool) => {
    counts.set(tool.serverName, (counts.get(tool.serverName) ?? 0) + 1);
    return counts;
  }, new Map());

  for (const server of servers) {
    assert.equal(flattenedCounts.get(server.name), server.tools.length);
  }
});
