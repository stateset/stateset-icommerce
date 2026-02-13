/**
 * Unit tests for A2A service listing MCP tool handlers
 *
 * Tests a2a_register_service, a2a_list_services, and a2a_get_service
 * from cli/src/tools/a2a.js by calling handlers directly with mocked commerce.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import a2aTools from '../../src/tools/a2a.js';

// ===========================================================================
// Helpers
// ===========================================================================

/**
 * Find a tool by name from the exported a2aTools array.
 */
function findTool(name) {
  const tool = a2aTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool ${name} not found in a2aTools`);
  return tool;
}

/**
 * Create a mock context with an in-memory services store.
 * The store methods are synchronous (matching the real handler calls).
 */
function createMockContext(walletAddress = '0xAgent') {
  const services = new Map();
  const now = new Date().toISOString();

  const commerce = {
    a2a: () => ({
      createService: (record) => {
        const id = record.id || randomUUID();
        const service = {
          id,
          agent_address: record.agent_address,
          name: record.name,
          description: record.description,
          category: record.category,
          pricing_model: record.pricing_model,
          pricing_details: record.pricing_details || null,
          endpoint_url: record.endpoint_url || null,
          active: true,
          transaction_count: 0,
          success_rate: null,
          avg_response_time: null,
          created_at: now,
          updated_at: now,
        };
        services.set(id, service);
        return service;
      },
      getService: (id) => services.get(id) || null,
      listServices: (filter) => {
        let results = [...services.values()];
        if (filter?.category) {
          results = results.filter((s) => s.category === filter.category);
        }
        if (filter?.agent_address) {
          results = results.filter((s) => s.agent_address === filter.agent_address);
        }
        if (filter?.active !== undefined) {
          results = results.filter((s) => s.active === filter.active);
        }
        if (filter?.limit) {
          results = results.slice(0, filter.limit);
        }
        return results;
      },
    }),
  };

  return {
    commerce,
    services,
    allowApply: true,
    agentConfig: { walletAddress },
  };
}

/**
 * Seed a service directly into the store map.
 */
function seedService(services, overrides = {}) {
  const id = overrides.id || randomUUID();
  const base = {
    id,
    agent_address: '0xAgent',
    name: 'Test Service',
    description: 'A test service',
    category: 'api',
    pricing_model: 'fixed',
    pricing_details: JSON.stringify({ basePrice: 10, currency: 'USDC' }),
    endpoint_url: 'https://example.com/api',
    active: true,
    transaction_count: 42,
    success_rate: 0.98,
    avg_response_time: 150,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
  };
  const service = { ...base, ...overrides };
  services.set(service.id, service);
  return service;
}

// ===========================================================================
// a2a_register_service
// ===========================================================================

describe('a2a_register_service', () => {
  const tool = findTool('a2a_register_service');
  let ctx;

  beforeEach(() => {
    ctx = createMockContext();
  });

  it('requires allowApply=true', async () => {
    const result = await tool.handler({
      commerce: ctx.commerce,
      params: { name: 'Svc', description: 'desc', category: 'api', pricingModel: 'fixed' },
      allowApply: false,
      agentConfig: ctx.agentConfig,
    });

    assert.ok(result.error);
    assert.match(result.error, /--apply/);
    assert.strictEqual(ctx.services.size, 0);
  });

  it('requires agentConfig.walletAddress', async () => {
    const result = await tool.handler({
      commerce: ctx.commerce,
      params: { name: 'Svc', description: 'desc', category: 'api', pricingModel: 'fixed' },
      allowApply: true,
      agentConfig: {},
    });

    assert.ok(result.error);
    assert.match(result.error, /wallet not configured/i);
  });

  it('requires agentConfig to exist', async () => {
    const result = await tool.handler({
      commerce: ctx.commerce,
      params: { name: 'Svc', description: 'desc', category: 'api', pricingModel: 'fixed' },
      allowApply: true,
      agentConfig: null,
    });

    assert.ok(result.error);
  });

  it('creates a service with correct fields', async () => {
    const result = await tool.handler({
      commerce: ctx.commerce,
      params: {
        name: 'Data Cruncher',
        description: 'Processes large datasets',
        category: 'compute',
        pricingModel: 'per_unit',
        pricingDetails: { basePrice: 0.01, currency: 'USDC', unitName: 'row' },
        endpointUrl: 'https://compute.agent/api',
      },
      allowApply: true,
      agentConfig: ctx.agentConfig,
    });

    assert.strictEqual(result.success, true);
    assert.match(result.message, /Data Cruncher/);
    assert.ok(result.service);
    assert.ok(result.service.id);
    assert.strictEqual(result.service.name, 'Data Cruncher');
    assert.strictEqual(result.service.description, 'Processes large datasets');
    assert.strictEqual(result.service.category, 'compute');
    assert.strictEqual(result.service.pricingModel, 'per_unit');
    assert.strictEqual(result.service.agentAddress, '0xAgent');
    assert.strictEqual(result.service.endpointUrl, 'https://compute.agent/api');
  });

  it('stores service in the a2a store', async () => {
    await tool.handler({
      commerce: ctx.commerce,
      params: { name: 'Svc', description: 'desc', category: 'api', pricingModel: 'fixed' },
      allowApply: true,
      agentConfig: ctx.agentConfig,
    });

    assert.strictEqual(ctx.services.size, 1);
    const stored = [...ctx.services.values()][0];
    assert.strictEqual(stored.name, 'Svc');
    assert.strictEqual(stored.agent_address, '0xAgent');
  });

  it('handles null endpointUrl', async () => {
    const result = await tool.handler({
      commerce: ctx.commerce,
      params: { name: 'NoUrl', description: 'desc', category: 'data', pricingModel: 'quote' },
      allowApply: true,
      agentConfig: ctx.agentConfig,
    });

    assert.strictEqual(result.success, true);
    const stored = [...ctx.services.values()][0];
    assert.strictEqual(stored.endpoint_url, null);
  });

  it('serializes pricingDetails as JSON', async () => {
    const details = { basePrice: 5, currency: 'USDC' };
    await tool.handler({
      commerce: ctx.commerce,
      params: {
        name: 'S',
        description: 'd',
        category: 'api',
        pricingModel: 'fixed',
        pricingDetails: details,
      },
      allowApply: true,
      agentConfig: ctx.agentConfig,
    });

    const stored = [...ctx.services.values()][0];
    assert.strictEqual(stored.pricing_details, JSON.stringify(details));
  });

  it('returns wouldRegister preview when allowApply=false', async () => {
    const result = await tool.handler({
      commerce: ctx.commerce,
      params: {
        name: 'Preview',
        description: 'desc',
        category: 'content',
        pricingModel: 'freemium',
      },
      allowApply: false,
      agentConfig: ctx.agentConfig,
    });

    assert.ok(result.wouldRegister);
    assert.strictEqual(result.wouldRegister.name, 'Preview');
    assert.strictEqual(result.wouldRegister.category, 'content');
  });
});

// ===========================================================================
// a2a_list_services
// ===========================================================================

describe('a2a_list_services', () => {
  const tool = findTool('a2a_list_services');
  let ctx;

  beforeEach(() => {
    ctx = createMockContext();
  });

  it('returns empty list when no services exist', async () => {
    const result = await tool.handler({
      commerce: ctx.commerce,
      params: {},
    });

    assert.strictEqual(result.success, true);
    assert.strictEqual(result.count, 0);
    assert.deepStrictEqual(result.services, []);
  });

  it('returns all services', async () => {
    seedService(ctx.services, { id: 'svc-1', name: 'Service A' });
    seedService(ctx.services, { id: 'svc-2', name: 'Service B' });

    const result = await tool.handler({
      commerce: ctx.commerce,
      params: {},
    });

    assert.strictEqual(result.success, true);
    assert.strictEqual(result.count, 2);
    assert.strictEqual(result.services.length, 2);
  });

  it('filters by category', async () => {
    seedService(ctx.services, { id: 'svc-1', category: 'api' });
    seedService(ctx.services, { id: 'svc-2', category: 'data' });
    seedService(ctx.services, { id: 'svc-3', category: 'api' });

    const result = await tool.handler({
      commerce: ctx.commerce,
      params: { category: 'api' },
    });

    assert.strictEqual(result.count, 2);
    assert.ok(result.services.every((s) => s.category === 'api'));
  });

  it('filters by agentAddress', async () => {
    seedService(ctx.services, { id: 'svc-1', agent_address: '0xAgent' });
    seedService(ctx.services, { id: 'svc-2', agent_address: '0xOther' });

    const result = await tool.handler({
      commerce: ctx.commerce,
      params: { agentAddress: '0xAgent' },
    });

    assert.strictEqual(result.count, 1);
    assert.strictEqual(result.services[0].agentAddress, '0xAgent');
  });

  it('formats service objects correctly', async () => {
    seedService(ctx.services, {
      id: 'svc-fmt',
      name: 'Formatted',
      description: 'Desc',
      category: 'content',
      pricing_model: 'tiered',
      agent_address: '0xFmt',
      endpoint_url: 'https://fmt.io',
      transaction_count: 100,
      success_rate: 0.95,
    });

    const result = await tool.handler({
      commerce: ctx.commerce,
      params: {},
    });

    const s = result.services[0];
    assert.strictEqual(s.id, 'svc-fmt');
    assert.strictEqual(s.name, 'Formatted');
    assert.strictEqual(s.description, 'Desc');
    assert.strictEqual(s.category, 'content');
    assert.strictEqual(s.pricingModel, 'tiered');
    assert.strictEqual(s.agentAddress, '0xFmt');
    assert.strictEqual(s.endpointUrl, 'https://fmt.io');
    assert.strictEqual(s.transactionCount, 100);
    assert.strictEqual(s.successRate, 0.95);
    assert.ok(s.createdAt);
  });

  it('respects limit param', async () => {
    for (let i = 0; i < 5; i++) {
      seedService(ctx.services, { id: `svc-${i}` });
    }

    const result = await tool.handler({
      commerce: ctx.commerce,
      params: { limit: 3 },
    });

    assert.strictEqual(result.services.length, 3);
  });

  it('defaults limit to 20', async () => {
    // Seed 25 services
    for (let i = 0; i < 25; i++) {
      seedService(ctx.services, { id: `svc-${i}` });
    }

    const result = await tool.handler({
      commerce: ctx.commerce,
      params: {},
    });

    assert.strictEqual(result.services.length, 20);
  });
});

// ===========================================================================
// a2a_get_service
// ===========================================================================

describe('a2a_get_service', () => {
  const tool = findTool('a2a_get_service');
  let ctx;

  beforeEach(() => {
    ctx = createMockContext();
  });

  it('returns service by id', async () => {
    seedService(ctx.services, { id: 'svc-42', name: 'Found' });

    const result = await tool.handler({
      commerce: ctx.commerce,
      params: { serviceId: 'svc-42' },
    });

    assert.strictEqual(result.success, true);
    assert.ok(result.service);
    assert.strictEqual(result.service.id, 'svc-42');
    assert.strictEqual(result.service.name, 'Found');
  });

  it('returns error when service not found', async () => {
    const result = await tool.handler({
      commerce: ctx.commerce,
      params: { serviceId: 'nonexistent' },
    });

    assert.ok(result.error);
    assert.match(result.error, /not found/i);
    assert.strictEqual(result.success, undefined);
  });

  it('returns full service details', async () => {
    seedService(ctx.services, {
      id: 'svc-full',
      name: 'Full Service',
      description: 'All the details',
      category: 'analysis',
      pricing_model: 'per_unit',
      pricing_details: JSON.stringify({ basePrice: 1.5, unitName: 'query' }),
      agent_address: '0xFull',
      endpoint_url: 'https://full.io/api',
      active: true,
      transaction_count: 200,
      success_rate: 0.99,
      avg_response_time: 50,
    });

    const result = await tool.handler({
      commerce: ctx.commerce,
      params: { serviceId: 'svc-full' },
    });

    const s = result.service;
    assert.strictEqual(s.name, 'Full Service');
    assert.strictEqual(s.description, 'All the details');
    assert.strictEqual(s.category, 'analysis');
    assert.strictEqual(s.pricingModel, 'per_unit');
    assert.deepStrictEqual(s.pricingDetails, { basePrice: 1.5, unitName: 'query' });
    assert.strictEqual(s.agentAddress, '0xFull');
    assert.strictEqual(s.endpointUrl, 'https://full.io/api');
    assert.strictEqual(s.active, true);
    assert.strictEqual(s.transactionCount, 200);
    assert.strictEqual(s.successRate, 0.99);
    assert.strictEqual(s.avgResponseTime, 50);
    assert.ok(s.createdAt);
    assert.ok(s.updatedAt);
  });

  it('returns null pricingDetails when not stored', async () => {
    seedService(ctx.services, {
      id: 'svc-nopricing',
      pricing_details: null,
    });

    const result = await tool.handler({
      commerce: ctx.commerce,
      params: { serviceId: 'svc-nopricing' },
    });

    assert.strictEqual(result.service.pricingDetails, null);
  });

  it('does not require agentConfig or allowApply (read operation)', async () => {
    seedService(ctx.services, { id: 'svc-read' });

    // No agentConfig or allowApply needed for read
    const result = await tool.handler({
      commerce: ctx.commerce,
      params: { serviceId: 'svc-read' },
    });

    assert.strictEqual(result.success, true);
  });
});

// ===========================================================================
// Integration: register then list/get
// ===========================================================================

describe('service registration + retrieval flow', () => {
  let ctx;

  beforeEach(() => {
    ctx = createMockContext('0xMyAgent');
  });

  it('registered service can be retrieved via get', async () => {
    const registerTool = findTool('a2a_register_service');
    const getTool = findTool('a2a_get_service');

    const regResult = await registerTool.handler({
      commerce: ctx.commerce,
      params: {
        name: 'Image Analysis',
        description: 'AI-powered image analysis',
        category: 'analysis',
        pricingModel: 'per_unit',
        pricingDetails: { basePrice: 0.05, unitName: 'image' },
      },
      allowApply: true,
      agentConfig: ctx.agentConfig,
    });

    assert.strictEqual(regResult.success, true);
    const serviceId = regResult.service.id;

    const getResult = await getTool.handler({
      commerce: ctx.commerce,
      params: { serviceId },
    });

    assert.strictEqual(getResult.success, true);
    assert.strictEqual(getResult.service.name, 'Image Analysis');
    assert.strictEqual(getResult.service.agentAddress, '0xMyAgent');
  });

  it('registered service appears in list', async () => {
    const registerTool = findTool('a2a_register_service');
    const listTool = findTool('a2a_list_services');

    await registerTool.handler({
      commerce: ctx.commerce,
      params: { name: 'Svc A', description: 'A', category: 'api', pricingModel: 'fixed' },
      allowApply: true,
      agentConfig: ctx.agentConfig,
    });

    await registerTool.handler({
      commerce: ctx.commerce,
      params: { name: 'Svc B', description: 'B', category: 'data', pricingModel: 'quote' },
      allowApply: true,
      agentConfig: ctx.agentConfig,
    });

    const listResult = await listTool.handler({
      commerce: ctx.commerce,
      params: {},
    });

    assert.strictEqual(listResult.count, 2);
    const names = listResult.services.map((s) => s.name);
    assert.ok(names.includes('Svc A'));
    assert.ok(names.includes('Svc B'));
  });

  it('list filters registered services by category', async () => {
    const registerTool = findTool('a2a_register_service');
    const listTool = findTool('a2a_list_services');

    await registerTool.handler({
      commerce: ctx.commerce,
      params: { name: 'API Svc', description: 'd', category: 'api', pricingModel: 'fixed' },
      allowApply: true,
      agentConfig: ctx.agentConfig,
    });

    await registerTool.handler({
      commerce: ctx.commerce,
      params: { name: 'Data Svc', description: 'd', category: 'data', pricingModel: 'fixed' },
      allowApply: true,
      agentConfig: ctx.agentConfig,
    });

    const apiOnly = await listTool.handler({
      commerce: ctx.commerce,
      params: { category: 'api' },
    });

    assert.strictEqual(apiOnly.count, 1);
    assert.strictEqual(apiOnly.services[0].name, 'API Svc');
  });
});
