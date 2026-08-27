// Smoke tests for the AGENTIC_RUNTIME_TOOLS array extracted from mcp-server.js.
// We don't exercise the handlers (they need runtime dependency injection) —
// we lock down the shape, the tool surface, and structural invariants so a
// future refactor that drops or mistypes a tool fails this suite immediately.

import { strict as assert } from 'node:assert';
import { describe, it } from 'node:test';

import { AGENTIC_RUNTIME_TOOLS } from '../../src/mcp/agentic-runtime-tools.js';

describe('agentic-runtime-tools · array structure', () => {
  it('is a non-empty array', () => {
    assert.ok(Array.isArray(AGENTIC_RUNTIME_TOOLS));
    assert.ok(AGENTIC_RUNTIME_TOOLS.length > 0);
  });

  it('every entry has the required shape', () => {
    for (const tool of AGENTIC_RUNTIME_TOOLS) {
      assert.equal(typeof tool.name, 'string', `tool name should be string`);
      assert.ok(tool.name.length > 0, `tool name must be non-empty: ${JSON.stringify(tool)}`);
      assert.equal(typeof tool.description, 'string', `${tool.name}: description should be string`);
      assert.ok(tool.description.length > 0, `${tool.name}: description must be non-empty`);
      assert.equal(
        typeof tool.inputSchema,
        'object',
        `${tool.name}: inputSchema must be an object`,
      );
      assert.ok(tool.inputSchema !== null, `${tool.name}: inputSchema must not be null`);
      assert.ok(
        ['read', 'write', 'preview', 'admin', 'delete'].includes(tool.permission),
        `${tool.name}: permission '${tool.permission}' is not a recognised level`,
      );
      assert.equal(typeof tool.policyDomain, 'string', `${tool.name}: policyDomain must be string`);
      assert.equal(typeof tool.handler, 'function', `${tool.name}: handler must be a function`);
    }
  });

  it('tool names are unique', () => {
    const names = AGENTIC_RUNTIME_TOOLS.map((t) => t.name);
    const unique = new Set(names);
    assert.equal(unique.size, names.length, 'duplicate tool names found');
  });

  it('tool names use snake_case', () => {
    for (const tool of AGENTIC_RUNTIME_TOOLS) {
      assert.match(tool.name, /^[a-z][a-z0-9_]*$/, `tool name '${tool.name}' should be snake_case`);
    }
  });

  it('all entries belong to the agentic policy domain', () => {
    for (const tool of AGENTIC_RUNTIME_TOOLS) {
      assert.equal(tool.policyDomain, 'agentic', `${tool.name}: should be in 'agentic' domain`);
    }
  });
});

describe('agentic-runtime-tools · expected tool surface', () => {
  // Locking down the marquee tool names so a refactor doesn't silently rename them.
  const expectedNames = [
    'agentic_runtime_contract',
    'agentic_tool_catalog',
    'agentic_payment_discovery',
    'agentic_prepare_payment',
    'agentic_plan',
    'agentic_simulate_mutation',
    'agentic_replay_mutation',
    'agentic_replay',
    'agentic_subscribe_events',
    'agentic_unsubscribe_events',
    'agentic_list_event_subscriptions',
    'agentic_get_event_history',
    'agentic_execute_plan',
    'discover_tools',
    'delegate_to_agent',
  ];

  it('exposes all expected tool names', () => {
    const names = new Set(AGENTIC_RUNTIME_TOOLS.map((t) => t.name));
    for (const name of expectedNames) {
      assert.ok(names.has(name), `expected tool '${name}' is missing`);
    }
  });

  it('classifies the governed plan orchestrator and delegation as writes', () => {
    const writers = AGENTIC_RUNTIME_TOOLS.filter((t) => t.permission === 'write').map(
      (t) => t.name,
    );
    assert.deepEqual(writers, ['agentic_execute_plan', 'delegate_to_agent']);
  });

  it('all non-mutating tools are read-permission', () => {
    const reads = AGENTIC_RUNTIME_TOOLS.filter((t) => t.permission === 'read');
    assert.ok(reads.length >= expectedNames.length - 2);
  });
});

describe('agentic-runtime-tools · handler shape', () => {
  it('every handler accepts a single object argument', () => {
    for (const tool of AGENTIC_RUNTIME_TOOLS) {
      assert.equal(
        tool.handler.length,
        1,
        `${tool.name}: handler should take exactly one destructured arg, got ${tool.handler.length}`,
      );
    }
  });

  it('every handler is async (returns a Promise)', () => {
    for (const tool of AGENTIC_RUNTIME_TOOLS) {
      // Async functions have the AsyncFunction constructor.
      assert.equal(
        tool.handler.constructor.name,
        'AsyncFunction',
        `${tool.name}: handler must be async`,
      );
    }
  });
});
