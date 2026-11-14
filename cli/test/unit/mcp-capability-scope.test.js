// Unit tests for cli/src/mcp/capability-scope.js
//
// The toolkit's capability predicate moved here so the MCP server can apply
// it to the tool map every path resolves against. These pin its semantics
// (the toolkit's, plus the explicit-grant rule for delegate_to_agent and a
// guard against matching an absent kernel capability) and check that a
// scoped server exposes exactly the tools the toolkit's own filter admits.

import { after, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  EXPLICIT_GRANT_ONLY_TOOLS,
  OUTSIDE_CAPABILITY_SCOPE,
  applyToolCapabilityScope,
  capabilityAllowsTool,
  normalizeToolCapabilities,
} from '../../src/mcp/capability-scope.js';
import { createStatesetMcpServer } from '../../src/mcp-server.js';

const scope = (...capabilities) => new Set(capabilities);

describe('capabilityAllowsTool', () => {
  const read = { name: 'list_orders', permission: 'read', policyDomain: 'orders' };
  const write = { name: 'create_payment', permission: 'write', policyDomain: 'payments' };

  it('admits every tool when no scope applies', () => {
    assert.equal(capabilityAllowsTool(write, null), true);
  });

  it('admits by wildcard, name, permission, read:*, domain, and kernel capability', () => {
    assert.equal(capabilityAllowsTool(write, scope('*')), true);
    assert.equal(capabilityAllowsTool(write, scope('create_payment')), true);
    assert.equal(
      capabilityAllowsTool(
        { ...write, name: 'mcp__stateset-commerce__create_payment' },
        scope('create_payment'),
      ),
      true,
    );
    assert.equal(capabilityAllowsTool(write, scope('permission:write')), true);
    assert.equal(capabilityAllowsTool(read, scope('read:*')), true);
    assert.equal(capabilityAllowsTool(write, scope('payments.*')), true);
    assert.equal(capabilityAllowsTool(write, scope('payments.create')), true);
  });

  it('refuses tools no capability names', () => {
    assert.equal(capabilityAllowsTool(write, scope()), false);
    assert.equal(capabilityAllowsTool(write, scope('read:*')), false);
    assert.equal(capabilityAllowsTool(write, scope('permission:read', 'orders.*')), false);
    assert.equal(capabilityAllowsTool(read, scope('permission:write')), false);
  });

  it('treats a tool without a permission as read', () => {
    assert.equal(capabilityAllowsTool({ name: 'anything' }, scope('read:*')), true);
  });

  it('admits delegate_to_agent only by name or *, never through a wildcard grant', () => {
    assert.deepEqual(EXPLICIT_GRANT_ONLY_TOOLS, ['delegate_to_agent']);
    const delegate = { name: 'delegate_to_agent', permission: 'write', policyDomain: 'agentic' };
    for (const capabilities of [
      ['permission:write'],
      ['agentic.*'],
      ['read:*'],
      ['permission:read'],
      ['permission:write', 'agentic.*', 'read:*'],
    ]) {
      assert.equal(
        capabilityAllowsTool(delegate, scope(...capabilities)),
        false,
        JSON.stringify(capabilities),
      );
    }
    assert.equal(capabilityAllowsTool(delegate, scope('delegate_to_agent')), true);
    assert.equal(capabilityAllowsTool(delegate, scope('*')), true);
    assert.equal(
      capabilityAllowsTool(
        { ...delegate, name: 'mcp__stateset-commerce__delegate_to_agent' },
        scope('delegate_to_agent'),
      ),
      true,
    );
    // The other agentic runtime tools keep their wildcard grants.
    const plan = { name: 'agentic_execute_plan', permission: 'write', policyDomain: 'agentic' };
    assert.equal(capabilityAllowsTool(plan, scope('permission:write')), true);
    assert.equal(capabilityAllowsTool(plan, scope('agentic.*')), true);
  });

  it('never matches a tool through an absent kernel capability or an empty name', () => {
    // list_orders has no kernel command: has(undefined) must not admit it.
    const noKernel = { name: 'list_orders', permission: 'write', policyDomain: 'orders' };
    assert.equal(capabilityAllowsTool(noKernel, new Set([undefined])), false);
    assert.equal(capabilityAllowsTool(noKernel, new Set([null])), false);
    assert.equal(capabilityAllowsTool({ permission: 'write' }, new Set(['', undefined])), false);
    // Inherited object keys are not kernel capabilities either.
    assert.equal(
      capabilityAllowsTool(
        { name: 'constructor', permission: 'write' },
        new Set([Object.prototype.constructor]),
      ),
      false,
    );
  });
});

describe('normalizeToolCapabilities', () => {
  it('returns null when no scope is given', () => {
    assert.equal(normalizeToolCapabilities(null), null);
    assert.equal(normalizeToolCapabilities(undefined), null);
  });

  it('trims entries and drops empty ones, from an array or a Set', () => {
    assert.deepEqual(
      [...normalizeToolCapabilities([' read:* ', '', 'orders.*'])],
      ['read:*', 'orders.*'],
    );
    assert.deepEqual([...normalizeToolCapabilities(new Set(['*']))], ['*']);
    assert.equal(normalizeToolCapabilities([]).size, 0);
  });

  it('refuses a scope it cannot read instead of admitting everything', () => {
    assert.throws(() => normalizeToolCapabilities('read:*'), TypeError);
    assert.throws(
      () => normalizeToolCapabilities({ length: 1 }, { optionName: 'capabilities' }),
      /^TypeError: capabilities must be an array or a Set/,
    );
  });
});

describe('applyToolCapabilityScope', () => {
  const toolDefs = [
    { name: 'list_orders', permission: 'read' },
    { name: 'delete_customer', permission: 'write' },
    { name: 'export_full_data', permission: 'read', policyDomain: 'maintenance' },
    { name: 'legacy_tool' },
  ];
  const deps = {
    toolDomainByName: { list_orders: 'orders', delete_customer: 'customers' },
    inferPolicyDomain: () => 'inferred',
  };

  it('returns the definitions untouched when no scope applies', () => {
    const result = applyToolCapabilityScope({ toolDefs, ...deps });
    assert.equal(result.toolDefs, toolDefs);
    assert.equal(result.describeExcludedTool('delete_customer'), null);
  });

  it('admits in-scope tools and describes the excluded ones', () => {
    const result = applyToolCapabilityScope({
      toolDefs,
      capabilities: ['read:*', 'orders.*'],
      ...deps,
    });
    // A definition without a permission reads as `unknown`, as in the raw
    // listing the toolkit filters, so `read:*` does not admit legacy_tool.
    assert.deepEqual(
      result.toolDefs.map((tool) => tool.name),
      ['list_orders', 'export_full_data'],
    );
    assert.deepEqual(result.describeExcludedTool('mcp__stateset-commerce__delete_customer'), {
      reason: OUTSIDE_CAPABILITY_SCOPE,
      message: "Tool 'delete_customer' is outside this toolkit's capability scope.",
      sideEffect: 'write',
    });
    assert.equal(result.describeExcludedTool('legacy_tool').sideEffect, 'write');
    assert.equal(result.describeExcludedTool('list_orders'), null);
    assert.equal(result.describeExcludedTool('no_such_tool'), null);
  });

  it('judges domains through the registry first, then inference', () => {
    const customers = applyToolCapabilityScope({
      toolDefs,
      capabilities: ['customers.*'],
      ...deps,
    });
    assert.deepEqual(
      customers.toolDefs.map((tool) => tool.name),
      ['delete_customer'],
    );
    const inferred = applyToolCapabilityScope({
      toolDefs,
      capabilities: ['inferred.*'],
      ...deps,
    });
    assert.deepEqual(
      inferred.toolDefs.map((tool) => tool.name),
      ['legacy_tool'],
    );
  });
});

describe('createStatesetMcpServer toolCapabilities', () => {
  const directory = mkdtempSync(join(tmpdir(), 'stateset-capability-scope-'));
  const servers = [];
  after(async () => {
    for (const server of servers) await server.close().catch(() => {});
    rmSync(directory, { recursive: true, force: true });
  });
  const createServer = (options = {}) => {
    const server = createStatesetMcpServer({
      commerce: {},
      dbPath: ':memory:',
      policyStorePath: directory,
      ...options,
    });
    servers.push(server);
    return server;
  };

  it('exposes exactly what the toolkit filter admits from the unscoped listing', () => {
    const unscoped = createServer().getRawToolDefinitions();
    for (const capabilities of [
      ['read:*'],
      ['permission:write'],
      ['orders.*', 'customers.*'],
      ['payments.create', 'list_orders'],
      ['*'],
      [],
    ]) {
      const expected = unscoped
        .filter((tool) => capabilityAllowsTool(tool, new Set(capabilities)))
        .map((tool) => tool.name);
      if (capabilities.includes('*')) assert.equal(expected.length, unscoped.length);
      else if (capabilities.length)
        assert.ok(expected.length > 0 && expected.length < unscoped.length);
      else assert.equal(expected.length, 0);
      const scoped = createServer({ toolCapabilities: capabilities });
      assert.deepEqual(
        scoped.getRawToolDefinitions().map((tool) => tool.name),
        expected,
        `capabilities ${JSON.stringify(capabilities)}`,
      );
      assert.deepEqual(
        scoped.getAdaptedTools().map((tool) => tool.name),
        expected,
        `adapted tools for ${JSON.stringify(capabilities)}`,
      );
    }
  });

  it('exposes the whole profile when no scope is given', () => {
    const all = createServer().getRawToolDefinitions().length;
    assert.equal(createServer({ toolCapabilities: null }).getRawToolDefinitions().length, all);
  });

  it('exposes delegate_to_agent only to a scope that names it or holds *', () => {
    const exposes = (capabilities) =>
      createServer({ toolCapabilities: capabilities })
        .getRawToolDefinitions()
        .some((tool) => tool.name === 'delegate_to_agent');
    assert.equal(exposes(['permission:write']), false);
    assert.equal(exposes(['agentic.*']), false);
    assert.equal(exposes(['read:*', 'permission:write', 'agentic.*']), false);
    assert.equal(exposes(['delegate_to_agent']), true);
    assert.equal(exposes(['*']), true);
  });
});
