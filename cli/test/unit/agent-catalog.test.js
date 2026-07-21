/**
 * Unit tests for the Machine-Readable Agent Catalog.
 *
 * Tests cli/src/catalog/agent-catalog.js:
 *   - publishProduct — validation, ID generation, JSON storage
 *   - queryProducts — filtering by capability, trust, price, chain, category, pagination
 *   - getProductSpec — lookup by catalog ID or product ID, JSON Schema fragment
 *   - updateProduct — column whitelist, version increment, JSON stringify
 *   - matchAgentToProducts — capability overlap, trust filtering, relevance sort
 *   - matchProductToAgents — trust + capability filtering on agent list
 *   - exportCatalog — JSON format, OpenAPI format, category/status filters
 *   - delistProduct — status change, idempotency, query exclusion
 *   - Edge cases — Unicode, large arrays, concurrent publishes, multi-update versioning
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';
import { createAgentCatalog } from '../../src/catalog/agent-catalog.js';

// ============================================================================
// Helpers
// ============================================================================

function makeStore() {
  const db = new Database(':memory:');
  db.pragma('journal_mode = WAL');
  return { db };
}

function makeCatalog() {
  const store = makeStore();
  const catalog = createAgentCatalog(store);
  return { store, catalog };
}

/** Publish a product with sensible defaults; override any field. */
function publishDefault(catalog, overrides = {}) {
  return catalog.publishProduct({
    productId:
      overrides.productId || `prod-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    name: overrides.name || 'Test Widget',
    description: overrides.description || 'A test widget for agents',
    capabilities: overrides.capabilities || ['buy', 'fulfill'],
    agentRequirements: overrides.agentRequirements || {},
    fulfillmentAgents: overrides.fulfillmentAgents || [],
    fulfillmentChains: overrides.fulfillmentChains || [],
    minTrustLevel: overrides.minTrustLevel || 'sandbox',
    maxPrice: overrides.maxPrice ?? 99.99,
    currency: overrides.currency || 'USD',
    machineSpec: overrides.machineSpec || {},
    tags: overrides.tags || ['test'],
    category: overrides.category || 'widgets',
    ...overrides,
  });
}

// ============================================================================
// 1. publishProduct
// ============================================================================

describe('publishProduct', () => {
  let catalog;
  beforeEach(() => {
    ({ catalog } = makeCatalog());
  });

  it('creates a catalog entry and returns catalogEntryId', () => {
    const result = publishDefault(catalog);
    assert.ok(result.catalogEntryId, 'should have catalogEntryId');
    assert.ok(result.productId, 'should have productId');
    assert.equal(result.status, 'active');
  });

  it('auto-generates a UUID for the catalog entry ID', () => {
    const r1 = publishDefault(catalog);
    const r2 = publishDefault(catalog);
    assert.notEqual(r1.catalogEntryId, r2.catalogEntryId);
    // UUID v4 format
    assert.match(
      r1.catalogEntryId,
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/,
    );
  });

  it('stores capabilities as JSON string', () => {
    const { catalogEntryId } = publishDefault(catalog, { capabilities: ['buy', 'ship'] });
    const spec = catalog.getProductSpec(catalogEntryId);
    assert.deepEqual(spec.entry.capabilities, ['buy', 'ship']);
  });

  it('throws when productId is missing', () => {
    assert.throws(
      () => catalog.publishProduct({ name: 'x', capabilities: ['a'] }),
      /productId is required/,
    );
  });

  it('throws when name is missing', () => {
    assert.throws(
      () => catalog.publishProduct({ productId: 'p1', capabilities: ['a'] }),
      /name is required/,
    );
  });

  it('throws when capabilities is empty', () => {
    assert.throws(
      () => catalog.publishProduct({ productId: 'p1', name: 'x', capabilities: [] }),
      /capabilities must be a non-empty array/,
    );
  });

  it('throws when capabilities is not an array', () => {
    assert.throws(
      () => catalog.publishProduct({ productId: 'p1', name: 'x', capabilities: 'buy' }),
      /capabilities must be a non-empty array/,
    );
  });

  it('stores agent requirements as JSON', () => {
    const reqs = { minMemory: '4GB', gpu: true };
    const { catalogEntryId } = publishDefault(catalog, { agentRequirements: reqs });
    const spec = catalog.getProductSpec(catalogEntryId);
    assert.deepEqual(spec.entry.agent_requirements, reqs);
  });

  it('stores fulfillment agents', () => {
    const agents = ['agent-001', 'agent-002'];
    const { catalogEntryId } = publishDefault(catalog, { fulfillmentAgents: agents });
    const spec = catalog.getProductSpec(catalogEntryId);
    assert.deepEqual(spec.entry.fulfillment_agents, agents);
  });

  it('defaults status to active', () => {
    const { catalogEntryId } = publishDefault(catalog);
    const spec = catalog.getProductSpec(catalogEntryId);
    assert.equal(spec.entry.status, 'active');
  });

  it('defaults version to 1', () => {
    const { catalogEntryId } = publishDefault(catalog);
    const spec = catalog.getProductSpec(catalogEntryId);
    assert.equal(spec.entry.version, 1);
  });

  it('stores all fields correctly', () => {
    const { catalogEntryId } = publishDefault(catalog, {
      productId: 'prod-full',
      name: 'Full Product',
      description: 'Full desc',
      capabilities: ['buy', 'fulfill', 'ship'],
      fulfillmentChains: ['set_chain', 'base'],
      minTrustLevel: 'verified',
      maxPrice: 250.5,
      currency: 'EUR',
      machineSpec: { endpoint: '/api/v1' },
      tags: ['premium', 'fast'],
      category: 'services',
    });
    const spec = catalog.getProductSpec(catalogEntryId);
    const e = spec.entry;
    assert.equal(e.product_id, 'prod-full');
    assert.equal(e.name, 'Full Product');
    assert.equal(e.description, 'Full desc');
    assert.deepEqual(e.capabilities, ['buy', 'fulfill', 'ship']);
    assert.deepEqual(e.fulfillment_chains, ['set_chain', 'base']);
    assert.equal(e.min_trust_level, 'verified');
    assert.equal(e.max_price, 250.5);
    assert.equal(e.currency, 'EUR');
    assert.deepEqual(e.machine_spec, { endpoint: '/api/v1' });
    assert.deepEqual(e.tags, ['premium', 'fast']);
    assert.equal(e.category, 'services');
  });

  it('handles optional fields being omitted', () => {
    const result = catalog.publishProduct({
      productId: 'prod-minimal',
      name: 'Minimal',
      capabilities: ['read'],
    });
    assert.ok(result.catalogEntryId);
    const spec = catalog.getProductSpec(result.catalogEntryId);
    assert.equal(spec.entry.description, null);
    assert.equal(spec.entry.max_price, null);
    assert.equal(spec.entry.category, null);
    assert.deepEqual(spec.entry.agent_requirements, {});
    assert.deepEqual(spec.entry.fulfillment_agents, []);
    assert.deepEqual(spec.entry.tags, []);
  });
});

// ============================================================================
// 2. queryProducts
// ============================================================================

describe('queryProducts', () => {
  let catalog;

  beforeEach(() => {
    ({ catalog } = makeCatalog());
    // Seed catalog
    publishDefault(catalog, {
      productId: 'p1',
      name: 'Widget A',
      capabilities: ['buy', 'ship'],
      category: 'widgets',
      maxPrice: 50,
      minTrustLevel: 'sandbox',
      fulfillmentChains: ['set_chain'],
    });
    publishDefault(catalog, {
      productId: 'p2',
      name: 'Widget B',
      capabilities: ['buy', 'fulfill'],
      category: 'widgets',
      maxPrice: 100,
      minTrustLevel: 'verified',
      fulfillmentChains: ['base'],
    });
    publishDefault(catalog, {
      productId: 'p3',
      name: 'Service C',
      capabilities: ['compute', 'fulfill'],
      category: 'services',
      maxPrice: 200,
      minTrustLevel: 'enterprise',
      fulfillmentChains: ['set_chain', 'base'],
    });
    publishDefault(catalog, {
      productId: 'p4',
      name: 'Premium D',
      capabilities: ['admin-op'],
      category: 'admin',
      maxPrice: 500,
      minTrustLevel: 'admin',
    });
  });

  it('returns all active products by default', () => {
    const { products, total } = catalog.queryProducts();
    assert.equal(total, 4);
    assert.equal(products.length, 4);
  });

  it('filters by capability', () => {
    const { products } = catalog.queryProducts({ capability: 'buy' });
    assert.equal(products.length, 2);
    const names = products.map((p) => p.name).sort();
    assert.deepEqual(names, ['Widget A', 'Widget B']);
  });

  it('filters by trust level — sandbox sees only sandbox products', () => {
    const { products } = catalog.queryProducts({ agentTrustLevel: 'sandbox' });
    assert.equal(products.length, 1);
    assert.equal(products[0].name, 'Widget A');
  });

  it('filters by trust level — verified sees sandbox + verified', () => {
    const { products } = catalog.queryProducts({ agentTrustLevel: 'verified' });
    assert.equal(products.length, 2);
  });

  it('filters by trust level — enterprise sees sandbox + verified + enterprise', () => {
    const { products } = catalog.queryProducts({ agentTrustLevel: 'enterprise' });
    assert.equal(products.length, 3);
  });

  it('filters by trust level — admin sees all', () => {
    const { products } = catalog.queryProducts({ agentTrustLevel: 'admin' });
    assert.equal(products.length, 4);
  });

  it('filters by max price', () => {
    const { products } = catalog.queryProducts({ maxPrice: 100 });
    assert.equal(products.length, 2);
  });

  it('filters by fulfillment chain', () => {
    const { products } = catalog.queryProducts({ fulfillmentChain: 'set_chain' });
    assert.equal(products.length, 2); // p1 and p3
  });

  it('filters by category', () => {
    const { products } = catalog.queryProducts({ category: 'services' });
    assert.equal(products.length, 1);
    assert.equal(products[0].name, 'Service C');
  });

  it('combines multiple filters', () => {
    const { products } = catalog.queryProducts({
      capability: 'fulfill',
      agentTrustLevel: 'enterprise',
    });
    assert.equal(products.length, 2); // Widget B (verified) + Service C (enterprise)
  });

  it('respects limit', () => {
    const { products, total } = catalog.queryProducts({ limit: 2 });
    assert.equal(products.length, 2);
    assert.equal(total, 4);
  });

  it('respects offset', () => {
    const all = catalog.queryProducts({ limit: 100 });
    const offset = catalog.queryProducts({ limit: 2, offset: 2 });
    assert.equal(offset.products.length, 2);
    assert.equal(offset.total, 4);
    // Offset products should differ from first page
    const firstIds = all.products.slice(0, 2).map((p) => p.id);
    const offsetIds = offset.products.map((p) => p.id);
    for (const id of offsetIds) {
      assert.ok(!firstIds.includes(id), 'offset products should differ from first page');
    }
  });

  it('returns total count with filters', () => {
    const { total } = catalog.queryProducts({ capability: 'buy' });
    assert.equal(total, 2);
  });

  it('returns empty for no matches', () => {
    const { products, total } = catalog.queryProducts({ capability: 'nonexistent' });
    assert.equal(products.length, 0);
    assert.equal(total, 0);
  });

  it('handles null max_price (no price limit) — included when maxPrice filter used', () => {
    publishDefault(catalog, {
      productId: 'p-noprice',
      name: 'No Price Limit',
      capabilities: ['buy'],
      maxPrice: undefined,
    });
    const { products } = catalog.queryProducts({ maxPrice: 50 });
    const names = products.map((p) => p.name);
    assert.ok(names.includes('No Price Limit'), 'null max_price should be included');
    assert.ok(names.includes('Widget A'));
  });

  it('filters by status', () => {
    // Delist one product first
    const all = catalog.queryProducts();
    catalog.delistProduct(all.products[0].id);

    const active = catalog.queryProducts({ status: 'active' });
    assert.equal(active.total, 3);
    const delisted = catalog.queryProducts({ status: 'delisted' });
    assert.equal(delisted.total, 1);
  });
});

// ============================================================================
// 3. getProductSpec
// ============================================================================

describe('getProductSpec', () => {
  let catalog;
  beforeEach(() => {
    ({ catalog } = makeCatalog());
  });

  it('returns entry by catalog ID', () => {
    const { catalogEntryId } = publishDefault(catalog);
    const result = catalog.getProductSpec(catalogEntryId);
    assert.ok(result);
    assert.equal(result.entry.id, catalogEntryId);
  });

  it('returns entry by product ID', () => {
    publishDefault(catalog, { productId: 'prod-lookup' });
    const result = catalog.getProductSpec('prod-lookup');
    assert.ok(result);
    assert.equal(result.entry.product_id, 'prod-lookup');
  });

  it('parses all JSON fields', () => {
    const { catalogEntryId } = publishDefault(catalog, {
      capabilities: ['buy'],
      agentRequirements: { minMemory: '2GB' },
      fulfillmentAgents: ['agent-x'],
      fulfillmentChains: ['base'],
      machineSpec: { endpoint: '/v1' },
      tags: ['fast'],
    });
    const { entry } = catalog.getProductSpec(catalogEntryId);
    assert.ok(Array.isArray(entry.capabilities));
    assert.equal(typeof entry.agent_requirements, 'object');
    assert.ok(Array.isArray(entry.fulfillment_agents));
    assert.ok(Array.isArray(entry.fulfillment_chains));
    assert.equal(typeof entry.machine_spec, 'object');
    assert.ok(Array.isArray(entry.tags));
  });

  it('returns null for unknown ID', () => {
    const result = catalog.getProductSpec('nonexistent-id');
    assert.equal(result, null);
  });

  it('includes machine spec in result', () => {
    const { catalogEntryId } = publishDefault(catalog, {
      machineSpec: { schema: { type: 'object' }, version: 2 },
    });
    const { entry } = catalog.getProductSpec(catalogEntryId);
    assert.deepEqual(entry.machine_spec, { schema: { type: 'object' }, version: 2 });
  });

  it('includes JSON Schema fragment from agent requirements', () => {
    const { catalogEntryId } = publishDefault(catalog, {
      agentRequirements: { minMemory: '4GB', gpu: { type: 'boolean', const: true } },
    });
    const { spec } = catalog.getProductSpec(catalogEntryId);
    assert.ok(spec.schema);
    assert.equal(spec.schema.type, 'object');
    assert.ok(spec.schema.properties.minMemory);
    assert.ok(spec.schema.properties.gpu);
    assert.ok(spec.schema.required.includes('minMemory'));
    assert.ok(spec.schema.required.includes('gpu'));
  });

  it('includes all fields in the entry', () => {
    const { catalogEntryId } = publishDefault(catalog, {
      productId: 'spec-all',
      name: 'All Fields',
      description: 'desc',
      capabilities: ['x'],
      minTrustLevel: 'verified',
      maxPrice: 42,
      currency: 'GBP',
      category: 'test-cat',
    });
    const { entry } = catalog.getProductSpec(catalogEntryId);
    assert.equal(entry.product_id, 'spec-all');
    assert.equal(entry.name, 'All Fields');
    assert.equal(entry.description, 'desc');
    assert.equal(entry.min_trust_level, 'verified');
    assert.equal(entry.max_price, 42);
    assert.equal(entry.currency, 'GBP');
    assert.equal(entry.category, 'test-cat');
    assert.ok(entry.created_at);
    assert.ok(entry.updated_at);
  });

  it('returns null for empty string', () => {
    const result = catalog.getProductSpec('');
    assert.equal(result, null);
  });
});

// ============================================================================
// 4. updateProduct
// ============================================================================

describe('updateProduct', () => {
  let catalog;
  beforeEach(() => {
    ({ catalog } = makeCatalog());
  });

  it('updates a field', () => {
    const { catalogEntryId } = publishDefault(catalog, { name: 'Before' });
    const result = catalog.updateProduct(catalogEntryId, { name: 'After' });
    assert.equal(result.entry.name, 'After');
  });

  it('increments version on update', () => {
    const { catalogEntryId } = publishDefault(catalog);
    const before = catalog.getProductSpec(catalogEntryId);
    assert.equal(before.entry.version, 1);
    catalog.updateProduct(catalogEntryId, { name: 'Updated' });
    const after = catalog.getProductSpec(catalogEntryId);
    assert.equal(after.entry.version, 2);
  });

  it('validates column whitelist — rejects unknown columns', () => {
    const { catalogEntryId } = publishDefault(catalog);
    assert.throws(
      () => catalog.updateProduct(catalogEntryId, { id: 'hacked' }),
      /Column 'id' is not allowed/,
    );
  });

  it('rejects product_id column', () => {
    const { catalogEntryId } = publishDefault(catalog);
    assert.throws(
      () => catalog.updateProduct(catalogEntryId, { product_id: 'hacked' }),
      /Column 'product_id' is not allowed/,
    );
  });

  it('rejects created_at column', () => {
    const { catalogEntryId } = publishDefault(catalog);
    assert.throws(
      () => catalog.updateProduct(catalogEntryId, { created_at: 'hacked' }),
      /Column 'created_at' is not allowed/,
    );
  });

  it('JSON-stringifies array values', () => {
    const { catalogEntryId } = publishDefault(catalog);
    catalog.updateProduct(catalogEntryId, { capabilities: ['new-cap'] });
    const spec = catalog.getProductSpec(catalogEntryId);
    assert.deepEqual(spec.entry.capabilities, ['new-cap']);
  });

  it('JSON-stringifies object values', () => {
    const { catalogEntryId } = publishDefault(catalog);
    catalog.updateProduct(catalogEntryId, { machine_spec: { v: 2 } });
    const spec = catalog.getProductSpec(catalogEntryId);
    assert.deepEqual(spec.entry.machine_spec, { v: 2 });
  });

  it('updates timestamp', () => {
    const { catalogEntryId } = publishDefault(catalog);
    const before = catalog.getProductSpec(catalogEntryId).entry.updated_at;
    // Small delay to ensure timestamp differs
    catalog.updateProduct(catalogEntryId, { name: 'Time Check' });
    const after = catalog.getProductSpec(catalogEntryId).entry.updated_at;
    assert.ok(after >= before, 'updated_at should be same or later');
  });
});

// ============================================================================
// 5. matchAgentToProducts
// ============================================================================

describe('matchAgentToProducts', () => {
  let catalog;

  beforeEach(() => {
    ({ catalog } = makeCatalog());
    publishDefault(catalog, {
      productId: 'm1',
      capabilities: ['buy', 'ship'],
      minTrustLevel: 'sandbox',
    });
    publishDefault(catalog, {
      productId: 'm2',
      capabilities: ['buy', 'fulfill', 'invoice'],
      minTrustLevel: 'verified',
    });
    publishDefault(catalog, {
      productId: 'm3',
      capabilities: ['compute'],
      minTrustLevel: 'enterprise',
    });
    publishDefault(catalog, {
      productId: 'm4',
      capabilities: ['admin-op'],
      minTrustLevel: 'admin',
    });
  });

  it('matches by capability overlap', () => {
    const { compatibleProducts } = catalog.matchAgentToProducts(['buy'], 'admin');
    assert.equal(compatibleProducts.length, 2);
  });

  it('respects trust level — sandbox only sees sandbox products', () => {
    const { compatibleProducts } = catalog.matchAgentToProducts(['buy'], 'sandbox');
    assert.equal(compatibleProducts.length, 1);
    assert.equal(compatibleProducts[0].product_id, 'm1');
  });

  it('sorts by relevance (match score descending)', () => {
    const { compatibleProducts } = catalog.matchAgentToProducts(
      ['buy', 'fulfill', 'invoice'],
      'admin',
    );
    assert.ok(compatibleProducts.length >= 2);
    // m2 has 3 matching caps, m1 has 1
    assert.ok(
      compatibleProducts[0].matchScore >= compatibleProducts[1].matchScore,
      'should be sorted by match score',
    );
  });

  it('returns empty when no capabilities match', () => {
    const { compatibleProducts } = catalog.matchAgentToProducts(['nonexistent'], 'admin');
    assert.equal(compatibleProducts.length, 0);
  });

  it('handles multiple capabilities', () => {
    const { compatibleProducts } = catalog.matchAgentToProducts(['buy', 'compute'], 'enterprise');
    // m1 (buy), m2 (buy), m3 (compute)
    assert.equal(compatibleProducts.length, 3);
  });

  it('trust hierarchy is correct — verified cannot access enterprise', () => {
    const { compatibleProducts } = catalog.matchAgentToProducts(['compute'], 'verified');
    assert.equal(compatibleProducts.length, 0);
  });

  it('returns empty for empty capabilities array', () => {
    const { compatibleProducts } = catalog.matchAgentToProducts([], 'admin');
    assert.equal(compatibleProducts.length, 0);
  });

  it('returns empty for undefined trust level', () => {
    const { compatibleProducts } = catalog.matchAgentToProducts(['buy'], undefined);
    // Should default to sandbox
    assert.equal(compatibleProducts.length, 1);
  });
});

// ============================================================================
// 6. matchProductToAgents
// ============================================================================

describe('matchProductToAgents', () => {
  let catalog;
  let productId;

  beforeEach(() => {
    ({ catalog } = makeCatalog());
    const result = publishDefault(catalog, {
      productId: 'match-product',
      capabilities: ['buy', 'fulfill'],
      minTrustLevel: 'verified',
    });
    productId = result.productId;
  });

  it('filters agents by trust level and capabilities', () => {
    const agents = [
      { id: 'a1', capabilities: ['buy'], trustLevel: 'verified' },
      { id: 'a2', capabilities: ['ship'], trustLevel: 'admin' },
      { id: 'a3', capabilities: ['buy', 'fulfill'], trustLevel: 'sandbox' }, // trust too low
    ];
    const { compatibleAgents } = catalog.matchProductToAgents(productId, agents);
    assert.equal(compatibleAgents.length, 1);
    assert.equal(compatibleAgents[0].id, 'a1');
  });

  it('returns empty for empty agents list', () => {
    const { compatibleAgents } = catalog.matchProductToAgents(productId, []);
    assert.equal(compatibleAgents.length, 0);
  });

  it('filters by trust level', () => {
    const agents = [
      { id: 'a1', capabilities: ['buy'], trustLevel: 'sandbox' },
      { id: 'a2', capabilities: ['buy'], trustLevel: 'verified' },
      { id: 'a3', capabilities: ['buy'], trustLevel: 'enterprise' },
    ];
    const { compatibleAgents } = catalog.matchProductToAgents(productId, agents);
    // sandbox is excluded; verified and enterprise pass
    assert.equal(compatibleAgents.length, 2);
    const ids = compatibleAgents.map((a) => a.id).sort();
    assert.deepEqual(ids, ['a2', 'a3']);
  });

  it('filters by capability', () => {
    const agents = [
      { id: 'a1', capabilities: ['compute'], trustLevel: 'admin' },
      { id: 'a2', capabilities: ['buy'], trustLevel: 'admin' },
    ];
    const { compatibleAgents } = catalog.matchProductToAgents(productId, agents);
    assert.equal(compatibleAgents.length, 1);
    assert.equal(compatibleAgents[0].id, 'a2');
  });

  it('returns empty for unknown product', () => {
    const { compatibleAgents } = catalog.matchProductToAgents('nonexistent', [
      { id: 'a1', capabilities: ['buy'], trustLevel: 'admin' },
    ]);
    assert.equal(compatibleAgents.length, 0);
  });
});

// ============================================================================
// 7. exportCatalog
// ============================================================================

describe('exportCatalog', () => {
  let catalog;

  beforeEach(() => {
    ({ catalog } = makeCatalog());
    publishDefault(catalog, { productId: 'exp1', category: 'widgets', capabilities: ['buy'] });
    publishDefault(catalog, { productId: 'exp2', category: 'services', capabilities: ['compute'] });
  });

  it('exports all entries by default', () => {
    const result = catalog.exportCatalog();
    assert.equal(result.entries.length, 2);
    assert.equal(result.format, 'json');
    assert.ok(result.exportedAt);
  });

  it('filters by category', () => {
    const result = catalog.exportCatalog({ category: 'widgets' });
    assert.equal(result.entries.length, 1);
    assert.equal(result.entries[0].product_id, 'exp1');
  });

  it('filters by status', () => {
    const all = catalog.queryProducts();
    catalog.delistProduct(all.products[0].id);
    const result = catalog.exportCatalog({ status: 'delisted' });
    assert.equal(result.entries.length, 1);
    assert.equal(result.entries[0].status, 'delisted');
  });

  it('returns json format by default', () => {
    const result = catalog.exportCatalog();
    assert.equal(result.format, 'json');
  });

  it('generates OpenAPI format with paths', () => {
    const result = catalog.exportCatalog({ format: 'openapi' });
    assert.equal(result.format, 'openapi');
    assert.ok(result.openapi);
    assert.equal(result.openapi.openapi, '3.0.3');
    assert.ok(result.openapi.paths);
    const pathKeys = Object.keys(result.openapi.paths);
    assert.equal(pathKeys.length, 2);
    assert.ok(pathKeys.includes('/products/exp1'));
    assert.ok(pathKeys.includes('/products/exp2'));
  });

  it('includes export timestamp', () => {
    const result = catalog.exportCatalog();
    assert.ok(result.exportedAt);
    // Should be a valid ISO string
    assert.ok(!Number.isNaN(Date.parse(result.exportedAt)));
  });
});

// ============================================================================
// 8. delistProduct
// ============================================================================

describe('delistProduct', () => {
  let catalog;

  beforeEach(() => {
    ({ catalog } = makeCatalog());
  });

  it('sets status to delisted', () => {
    const { catalogEntryId } = publishDefault(catalog);
    const result = catalog.delistProduct(catalogEntryId);
    assert.equal(result.entry.status, 'delisted');
  });

  it('returns the updated entry', () => {
    const { catalogEntryId, productId } = publishDefault(catalog);
    const result = catalog.delistProduct(catalogEntryId);
    assert.equal(result.entry.id, catalogEntryId);
    assert.equal(result.entry.product_id, productId);
  });

  it('delisted products excluded from default queries', () => {
    const { catalogEntryId } = publishDefault(catalog);
    publishDefault(catalog, { productId: 'still-active' });
    catalog.delistProduct(catalogEntryId);
    const { products, total } = catalog.queryProducts();
    assert.equal(total, 1);
    assert.equal(products[0].product_id, 'still-active');
  });

  it('is idempotent — can delist already-delisted product', () => {
    const { catalogEntryId } = publishDefault(catalog);
    catalog.delistProduct(catalogEntryId);
    const result = catalog.delistProduct(catalogEntryId);
    assert.equal(result.entry.status, 'delisted');
  });
});

// ============================================================================
// 9. Edge cases
// ============================================================================

describe('edge cases', () => {
  let catalog;

  beforeEach(() => {
    ({ catalog } = makeCatalog());
  });

  it('handles Unicode in names and descriptions', () => {
    const { catalogEntryId } = publishDefault(catalog, {
      name: 'Widget \u00fcber-\u00e9l\u00e8gant \ud83d\ude80',
      description: '\u65e5\u672c\u8a9e\u306e\u8aac\u660e \u4e2d\u6587\u63cf\u8ff0',
    });
    const spec = catalog.getProductSpec(catalogEntryId);
    assert.equal(spec.entry.name, 'Widget \u00fcber-\u00e9l\u00e8gant \ud83d\ude80');
    assert.equal(
      spec.entry.description,
      '\u65e5\u672c\u8a9e\u306e\u8aac\u660e \u4e2d\u6587\u63cf\u8ff0',
    );
  });

  it('handles large capability arrays', () => {
    const caps = Array.from({ length: 100 }, (_, i) => `cap-${i}`);
    const { catalogEntryId } = publishDefault(catalog, { capabilities: caps });
    const spec = catalog.getProductSpec(catalogEntryId);
    assert.equal(spec.entry.capabilities.length, 100);
    assert.equal(spec.entry.capabilities[99], 'cap-99');
  });

  it('concurrent publishes are safe (different IDs)', () => {
    const results = [];
    for (let i = 0; i < 50; i++) {
      results.push(publishDefault(catalog, { productId: `concurrent-${i}` }));
    }
    const ids = new Set(results.map((r) => r.catalogEntryId));
    assert.equal(ids.size, 50, 'all 50 entries should have unique IDs');
    const { total } = catalog.queryProducts({ limit: 100 });
    assert.equal(total, 50);
  });

  it('version increments correctly on multiple updates', () => {
    const { catalogEntryId } = publishDefault(catalog);
    catalog.updateProduct(catalogEntryId, { name: 'v2' });
    catalog.updateProduct(catalogEntryId, { name: 'v3' });
    catalog.updateProduct(catalogEntryId, { name: 'v4' });
    const spec = catalog.getProductSpec(catalogEntryId);
    assert.equal(spec.entry.version, 4);
    assert.equal(spec.entry.name, 'v4');
  });
});
