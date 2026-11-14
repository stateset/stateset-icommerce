/**
 * Two sources declare each tool's permission: the tool definition itself
 * (what the MCP preview gate, kernel strict mode and the tool annotations
 * read) and TOOL_PERMISSIONS in src/permissions.js (what the CLI harness
 * PermissionGate reads). They must agree, or a tool is a write to one gate
 * and a read to the other.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { TOOL_PERMISSIONS } from '../../src/permissions.js';
import { AGENTIC_RUNTIME_TOOLS } from '../../src/mcp/agentic-runtime-tools.js';
import { ALL_DOMAIN_TOOLS } from '../../src/tools/domain-registry.js';
import { getStaticX402ToolDefinitions } from '../../src/x402-mcp-server.js';

const DEFINITIONS = [
  ...ALL_DOMAIN_TOOLS,
  ...AGENTIC_RUNTIME_TOOLS,
  ...getStaticX402ToolDefinitions(),
];

describe('TOOL_PERMISSIONS parity with tool definitions', () => {
  it('gives every tool definition the permission it declares', () => {
    const mismatched = DEFINITIONS.filter(
      (tool) => TOOL_PERMISSIONS[tool.name] !== tool.permission,
    ).map(
      (tool) =>
        `${tool.name}: definition=${tool.permission} TOOL_PERMISSIONS=${TOOL_PERMISSIONS[tool.name]}`,
    );
    assert.deepEqual(mismatched, []);
  });

  it('lists no tool that has no definition', () => {
    const defined = new Set(DEFINITIONS.map((tool) => tool.name));
    assert.deepEqual(
      Object.keys(TOOL_PERMISSIONS).filter((name) => !defined.has(name)),
      [],
    );
  });

  it('keeps the composite agentic tools writes in both sources', () => {
    for (const name of ['agentic_execute_plan', 'agentic_replay_mutation']) {
      assert.equal(TOOL_PERMISSIONS[name], 'write', name);
    }
  });
});
