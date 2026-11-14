// Unit tests for cli/src/mcp/capability-scope.js
//
// The toolkit's capability predicate moved here so the MCP server can apply
// it to the tool map every path resolves against. These pin its semantics
// (unchanged from the toolkit) and check that a scoped server exposes exactly
// the tools the toolkit's own filter admits.

import { after, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
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
});
