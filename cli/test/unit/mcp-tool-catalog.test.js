// Unit tests for cli/src/mcp/tool-catalog.js
//
// Covers `createToolCatalogHelpers`:
//  - getToolDefinitions: generic / openai / anthropic / mcp formats + prefix
//  - getRawToolDefinitions: policyDomain precedence (def → registry → infer)
//  - buildToolCatalog: tool filter, payableOnly, openai + mcp naming
//  - buildPaymentDiscovery: json vs openapi documents, pricedOnly filter
//  - getToolDiscoveryEngine: lazily built once, registers every tool
//  - getAgenticRuntimeContract: pricing projection, legacy defaults, hash

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

import { createToolCatalogHelpers } from '../../src/mcp/tool-catalog.js';

const ALL_TOOL_DEFS = [
  {
    name: 'list_orders',
    description: 'List orders',
    inputSchema: { limit: z.number().optional() },
    permission: 'read',
  },
  {
    name: 'create_order',
    description: 'Create an order',
    inputSchema: { customerId: z.string() },
    permission: 'write',
    policyDomain: 'orders-explicit',
  },
];

const PRICING = { enabled: true, chainId: 'base', tokenSymbol: 'USDC', amount: '0.5' };

function makeHelpers({ pricingFor = () => null } = {}) {
  return createToolCatalogHelpers({
    allToolDefs: ALL_TOOL_DEFS,
    toolDomainByName: { list_orders: 'orders-registry' },
    serviceInfo: { id: 'svc', name: 'Svc', protocolVersion: '1' },
    resultSchemaVersion: 'result-v-test',
    getAgenticToolPricing: async (name) => pricingFor(name),
    getToolRuntimeMeta: (name) => ({
      name,
      permission: name === 'list_orders' ? 'read' : 'write',
      policyDomain: 'runtime-domain',
      sideEffect: name === 'list_orders' ? 'read' : 'write',
      compensations: [],
      idempotent: false,
    }),
    inferPolicyDomain: () => 'inferred',
  });
}

describe('getToolDefinitions', () => {
  it('returns generic descriptors with runtime metadata and optional prefix', () => {
    const { getToolDefinitions } = makeHelpers();
    const defs = getToolDefinitions({ mcpPrefix: 'pre_' });
    assert.equal(defs.length, 2);
    assert.equal(defs[0].name, 'pre_list_orders');
    assert.equal(defs[0].toolName, 'list_orders');
    assert.equal(defs[0].permission, 'read');
    assert.equal(defs[0].policyDomain, 'runtime-domain');
    assert.equal(defs[0].inputSchema.type, 'object');
    assert.equal(defs[0].runtime.name, 'list_orders');
  });

  it('emits openai, anthropic, and mcp shapes', () => {
    const { getToolDefinitions } = makeHelpers();
    const [openai] = getToolDefinitions({ format: 'openai' });
    assert.equal(openai.type, 'function');
    assert.equal(openai.function.name, 'list_orders');
    assert.deepEqual(openai.stateset, { permission: 'read', policyDomain: 'runtime-domain' });

    const [anthropic] = getToolDefinitions({ format: 'anthropic' });
    assert.equal(anthropic.name, 'list_orders');
    assert.equal(anthropic.input_schema.type, 'object');

    const [mcp] = getToolDefinitions({ format: 'mcp' });
    assert.equal(mcp.name, 'mcp__stateset-commerce__list_orders');
    assert.equal(mcp.toolName, 'list_orders');
  });
});

describe('getRawToolDefinitions', () => {
  it('prefers the def policyDomain, then the registry map, then inference', () => {
    const { getRawToolDefinitions } = makeHelpers();
    const raw = getRawToolDefinitions();
    assert.equal(raw.find((t) => t.name === 'create_order').policyDomain, 'orders-explicit');
    assert.equal(raw.find((t) => t.name === 'list_orders').policyDomain, 'orders-registry');
    assert.equal(raw[0].permission, 'read');
    assert.deepEqual(Object.keys(raw[0].inputSchema), ['limit']);
  });
});

describe('buildToolCatalog', () => {
  it('lists every tool with pricing + payment info in generic format', async () => {
    const { buildToolCatalog } = makeHelpers({
      pricingFor: (name) => (name === 'create_order' ? PRICING : null),
    });
    const catalog = await buildToolCatalog();
    assert.equal(catalog.format, 'generic');
    assert.equal(catalog.count, 2);
    const create = catalog.tools.find((t) => t.toolName === 'create_order');
    assert.equal(create.payable, true);
    assert.ok(create.paymentInfo);
    assert.equal('pricing' in create, false);
    const list = catalog.tools.find((t) => t.toolName === 'list_orders');
    assert.equal(list.payable, false);
    assert.equal(list.paymentInfo, null);
  });

  it('filters by tool (normalized) and payableOnly, and supports mcp/openai naming', async () => {
    const { buildToolCatalog } = makeHelpers({
      pricingFor: (name) => (name === 'create_order' ? PRICING : null),
    });
    const only = await buildToolCatalog({ tool: 'mcp__stateset-commerce__list_orders' });
    assert.equal(only.count, 1);
    assert.equal(only.tools[0].toolName, 'list_orders');

    const payable = await buildToolCatalog({ payableOnly: true });
    assert.deepEqual(
      payable.tools.map((t) => t.toolName),
      ['create_order'],
    );

    const mcp = await buildToolCatalog({ format: 'mcp', mcpPrefix: 'ignored_' });
    assert.equal(mcp.tools[0].name, 'mcp__stateset-commerce__list_orders');

    const openai = await buildToolCatalog({ format: 'openai' });
    assert.equal(openai.tools[0].type, 'function');
    assert.equal(openai.tools[1].stateset.payable, true);
  });
});

describe('buildPaymentDiscovery', () => {
  it('builds the json discovery document and honours pricedOnly', async () => {
    const { buildPaymentDiscovery } = makeHelpers({
      pricingFor: (name) => (name === 'create_order' ? PRICING : null),
    });
    const doc = await buildPaymentDiscovery();
    assert.equal(doc.protocol, 'mpp');
    assert.equal(doc.tools.length, 2);
    assert.deepEqual(Object.keys(doc.tools[0]), [
      'name',
      'description',
      'runtime',
      'pricing',
      'paymentInfo',
    ]);

    const priced = await buildPaymentDiscovery({ pricedOnly: true });
    assert.deepEqual(
      priced.tools.map((t) => t.name),
      ['create_order'],
    );
  });

  it('builds an openapi document when requested', async () => {
    const { buildPaymentDiscovery } = makeHelpers({ pricingFor: () => PRICING });
    const doc = await buildPaymentDiscovery({ format: 'openapi', tool: 'create_order' });
    assert.ok(doc.openapi || doc.paths, 'expected an OpenAPI-shaped document');
  });
});

describe('getToolDiscoveryEngine', () => {
  it('builds the engine once and registers every catalog tool', async () => {
    const { getToolDiscoveryEngine } = makeHelpers();
    const engine = await getToolDiscoveryEngine();
    const again = await getToolDiscoveryEngine();
    assert.equal(engine, again);
    assert.equal(typeof engine.registerTool, 'function');
  });
});

describe('getAgenticRuntimeContract', () => {
  it('projects runtime meta + pricing for every tool and hashes the tool list', async () => {
    const { getAgenticRuntimeContract } = makeHelpers({
      pricingFor: (name) =>
        name === 'create_order' ? { ...PRICING, amountSmallest: '500' } : null,
    });
    const contract = await getAgenticRuntimeContract();
    assert.equal(contract.engine, 'stateset-icommerce');
    assert.equal(contract.purpose, 'agentic_runtime_contract');
    assert.equal(contract.agenticToolResultSchema.version, 'result-v-test');
    assert.equal(contract.totalTools, 2);
    // Sorted by name.
    assert.deepEqual(
      contract.tools.map((t) => t.name),
      ['create_order', 'list_orders'],
    );
    assert.deepEqual(contract.tools[0].pricing, {
      enabled: true,
      chainId: 'base',
      tokenSymbol: 'USDC',
      amount: '0.5',
      amountSmallest: '500',
    });
    assert.equal(contract.tools[1].pricing, null);
    assert.match(contract.contractHash, /^[0-9a-f]{64}$/);
    // `includeLegacy` is always an array, so the legacy block is always present.
    assert.deepEqual(contract.legacy, { deprecatedPrefixes: [] });
    assert.equal(contract.mpp.transport.http.paymentRequiredStatus, 402);
  });

  it('filters to one tool and adds legacy defaults when asked', async () => {
    const { getAgenticRuntimeContract } = makeHelpers();
    const contract = await getAgenticRuntimeContract({
      tool: 'list_orders',
      includeLegacyDefaults: true,
    });
    assert.equal(contract.totalTools, 1);
    assert.deepEqual(contract.legacyDefaults, ['create', 'read', 'update', 'delete', 'list']);
    assert.deepEqual(contract.legacy, {
      deprecatedPrefixes: ['create', 'read', 'update', 'delete', 'list'],
    });
  });
});
