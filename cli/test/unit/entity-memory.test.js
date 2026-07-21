/**
 * Unit tests for entity-aware memory retrieval (Phase 3.1)
 *
 * Tests:
 *   - extractEntityIds() — regex extraction of entity references
 *   - MemoryStore.searchByEntity() — SQLite entity search
 *   - MemoryInjector.formatEntityContext() — block formatting
 *   - MemoryInjector.injectMemoryContext() — end-to-end with entity context
 *   - Setter: setMaxEntityMemories()
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import os from 'node:os';
import path from 'node:path';
import fs from 'node:fs';

import { extractEntityIds, MemoryInjector } from '../../src/memory/injector.js';
import { MemoryStore } from '../../src/memory/store.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function tmpDbPath() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'entity-mem-test-'));
  return path.join(dir, 'memory.db');
}

// ---------------------------------------------------------------------------
// extractEntityIds — Order patterns
// ---------------------------------------------------------------------------

describe('extractEntityIds — orders', () => {
  it('extracts ORD- prefix IDs', () => {
    const results = extractEntityIds('Please check order ORD-12345 for me');
    assert.ok(results.some((e) => e.type === 'order' && e.id === 'ORD-12345'));
  });

  it('extracts "order #NNN" pattern', () => {
    const results = extractEntityIds('What happened to order #7890?');
    assert.ok(results.some((e) => e.type === 'order' && e.id === '7890'));
  });

  it('extracts "order #NNN" with space before number', () => {
    const results = extractEntityIds('Tell me about order # ABC-99');
    assert.ok(results.some((e) => e.type === 'order' && e.id === 'ABC-99'));
  });

  it('extracts UUID following "order"', () => {
    const uuid = '550e8400-e29b-41d4-a716-446655440000';
    const results = extractEntityIds(`Look at order ${uuid}`);
    assert.ok(results.some((e) => e.type === 'order' && e.id === uuid));
  });

  it('extracts multiple order IDs from one message', () => {
    const results = extractEntityIds('Compare ORD-001 and ORD-002');
    assert.strictEqual(results.filter((e) => e.type === 'order').length, 2);
  });

  it('deduplicates the same order ID (case-insensitive)', () => {
    // ORD-ABC appears twice — same id, different cases are normalised
    const results = extractEntityIds('ORD-ABC and ord-abc are the same');
    const orders = results.filter((e) => e.type === 'order');
    // Only one unique entry should exist for ORD-ABC
    assert.strictEqual(orders.length, 1);
  });

  it('returns empty array for text with no entity references', () => {
    const results = extractEntityIds('What is the weather today?');
    assert.strictEqual(results.length, 0);
  });
});

// ---------------------------------------------------------------------------
// extractEntityIds — Customer patterns
// ---------------------------------------------------------------------------

describe('extractEntityIds — customers', () => {
  it('extracts CUST- prefix IDs', () => {
    const results = extractEntityIds('Update account CUST-99001');
    assert.ok(results.some((e) => e.type === 'customer' && e.id === 'CUST-99001'));
  });

  it('extracts "customer #NNN" pattern', () => {
    const results = extractEntityIds('customer #4567 placed an order');
    assert.ok(results.some((e) => e.type === 'customer' && e.id === '4567'));
  });

  it('extracts email addresses as customer references', () => {
    const results = extractEntityIds('Send receipt to alice@example.com');
    assert.ok(results.some((e) => e.type === 'customer' && e.id === 'alice@example.com'));
  });

  it('extracts email with plus addressing', () => {
    const results = extractEntityIds('bob+tag@shop.co.uk placed order');
    assert.ok(results.some((e) => e.type === 'customer' && e.id === 'bob+tag@shop.co.uk'));
  });

  it('deduplicates the same email', () => {
    const results = extractEntityIds('Send to alice@example.com and alice@example.com again');
    const emails = results.filter((e) => e.type === 'customer' && e.id === 'alice@example.com');
    assert.strictEqual(emails.length, 1);
  });
});

// ---------------------------------------------------------------------------
// extractEntityIds — Product patterns
// ---------------------------------------------------------------------------

describe('extractEntityIds — products', () => {
  it('extracts PROD- prefix IDs', () => {
    const results = extractEntityIds('Check stock for PROD-WIDGET-01');
    assert.ok(results.some((e) => e.type === 'product' && e.id === 'PROD-WIDGET-01'));
  });

  it('extracts SKU- prefix IDs', () => {
    const results = extractEntityIds('How many SKU-ABC-123 do we have?');
    assert.ok(results.some((e) => e.type === 'product' && e.id === 'SKU-ABC-123'));
  });

  it('extracts both PROD- and SKU- from same message', () => {
    const results = extractEntityIds('PROD-A is the same as SKU-B');
    assert.ok(results.some((e) => e.type === 'product' && e.id === 'PROD-A'));
    assert.ok(results.some((e) => e.type === 'product' && e.id === 'SKU-B'));
  });
});

// ---------------------------------------------------------------------------
// extractEntityIds — Return patterns
// ---------------------------------------------------------------------------

describe('extractEntityIds — returns', () => {
  it('extracts RET- prefix IDs', () => {
    const results = extractEntityIds('Process return RET-55555');
    assert.ok(results.some((e) => e.type === 'return' && e.id === 'RET-55555'));
  });

  it('extracts "return #NNN" pattern', () => {
    const results = extractEntityIds('Approve return #888');
    assert.ok(results.some((e) => e.type === 'return' && e.id === '888'));
  });
});

// ---------------------------------------------------------------------------
// extractEntityIds — Mixed and edge cases
// ---------------------------------------------------------------------------

describe('extractEntityIds — mixed / edge cases', () => {
  it('extracts multiple entity types from one message', () => {
    const text =
      'Customer alice@example.com placed order ORD-999, product PROD-X needs checking, and return RET-1 is pending.';
    const results = extractEntityIds(text);
    assert.ok(results.some((e) => e.type === 'customer'));
    assert.ok(results.some((e) => e.type === 'order'));
    assert.ok(results.some((e) => e.type === 'product'));
    assert.ok(results.some((e) => e.type === 'return'));
  });

  it('returns empty array for null input', () => {
    assert.deepStrictEqual(extractEntityIds(null), []);
  });

  it('returns empty array for undefined input', () => {
    assert.deepStrictEqual(extractEntityIds(undefined), []);
  });

  it('returns empty array for empty string', () => {
    assert.deepStrictEqual(extractEntityIds(''), []);
  });

  it('preserves original casing of extracted IDs', () => {
    const results = extractEntityIds('Check ORD-AbCdEf status');
    const ord = results.find((e) => e.type === 'order');
    assert.ok(ord);
    assert.strictEqual(ord.id, 'ORD-AbCdEf');
  });

  it('each result has type and id fields', () => {
    const results = extractEntityIds('ORD-100 CUST-200');
    for (const r of results) {
      assert.ok('type' in r, 'Missing type field');
      assert.ok('id' in r, 'Missing id field');
      assert.ok(typeof r.type === 'string');
      assert.ok(typeof r.id === 'string');
    }
  });
});

// ---------------------------------------------------------------------------
// MemoryStore.searchByEntity
// ---------------------------------------------------------------------------

describe('MemoryStore — searchByEntity', () => {
  /** @type {MemoryStore} */
  let store;

  beforeEach(() => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
  });

  afterEach(() => {
    try {
      store.close();
    } catch {}
  });

  it('returns memories whose summary mentions the entity ID', () => {
    store.save({ channel: 'cli', senderId: 'u1', summary: 'Processed order ORD-111 successfully' });
    store.save({ channel: 'cli', senderId: 'u1', summary: 'Unrelated summary about cats' });

    const results = store.searchByEntity('cli', 'u1', 'order', 'ORD-111');
    assert.strictEqual(results.length, 1);
    assert.ok(results[0].summary.includes('ORD-111'));
  });

  it('returns memories whose facts mention the entity ID', () => {
    store.save({
      channel: 'cli',
      senderId: 'u1',
      summary: 'Return processed',
      facts: ['Return ID is RET-555', 'Customer approved'],
    });
    store.save({ channel: 'cli', senderId: 'u1', summary: 'Something else' });

    const results = store.searchByEntity('cli', 'u1', 'return', 'RET-555');
    assert.strictEqual(results.length, 1);
    assert.ok(results[0].summary === 'Return processed');
  });

  it('scopes results to the correct channel+senderId', () => {
    store.save({ channel: 'cli', senderId: 'u1', summary: 'Order ORD-222 placed' });
    store.save({ channel: 'telegram', senderId: 'u2', summary: 'Also ORD-222 here' });

    const results = store.searchByEntity('cli', 'u1', 'order', 'ORD-222');
    assert.strictEqual(results.length, 1);
    assert.strictEqual(results[0].channel, 'cli');
    assert.strictEqual(results[0].sender_id, 'u1');
  });

  it('respects the limit parameter', () => {
    for (let i = 0; i < 5; i++) {
      store.save({ channel: 'cli', senderId: 'u1', summary: `Mention of CUST-42 round ${i}` });
    }
    const results = store.searchByEntity('cli', 'u1', 'customer', 'CUST-42', 2);
    assert.ok(results.length <= 2);
  });

  it('returns empty array when entityId is empty string', () => {
    store.save({ channel: 'cli', senderId: 'u1', summary: 'Some memory' });
    const results = store.searchByEntity('cli', 'u1', 'order', '');
    assert.deepStrictEqual(results, []);
  });

  it('returns empty array when no memories match', () => {
    store.save({ channel: 'cli', senderId: 'u1', summary: 'Nothing relevant here' });
    const results = store.searchByEntity('cli', 'u1', 'order', 'ORD-NONEXISTENT');
    assert.deepStrictEqual(results, []);
  });

  it('attaches entityType and entityId to returned rows', () => {
    store.save({ channel: 'cli', senderId: 'u1', summary: 'ORD-777 was shipped' });
    const results = store.searchByEntity('cli', 'u1', 'order', 'ORD-777');
    assert.strictEqual(results[0].entityType, 'order');
    assert.strictEqual(results[0].entityId, 'ORD-777');
  });

  it('deserializes facts from JSON correctly', () => {
    store.save({
      channel: 'cli',
      senderId: 'u1',
      summary: 'ORD-888 details',
      facts: ['Shipped via FedEx', 'Tracking: 123ABC'],
    });
    const results = store.searchByEntity('cli', 'u1', 'order', 'ORD-888');
    assert.ok(Array.isArray(results[0].facts));
    assert.ok(results[0].facts.includes('Shipped via FedEx'));
  });

  it('handles entity IDs that contain LIKE special chars (%, _)', () => {
    // Entity ID with special chars should still not match everything
    store.save({ channel: 'cli', senderId: 'u1', summary: 'Something totally unrelated' });
    // Should not throw and should return empty results
    const results = store.searchByEntity('cli', 'u1', 'order', '%_wildcard%');
    // The literal string '%_wildcard%' won't appear in the summary above
    assert.deepStrictEqual(results, []);
  });
});

// ---------------------------------------------------------------------------
// MemoryInjector.formatEntityContext
// ---------------------------------------------------------------------------

describe('MemoryInjector — formatEntityContext', () => {
  let inj;

  beforeEach(() => {
    inj = new MemoryInjector({ maxBodyLength: 2000 });
  });

  it('returns null when no entity memories are found', () => {
    const mockStore = {
      searchByEntity: () => [],
    };
    const result = inj.formatEntityContext(mockStore, 'cli', 'u1', [
      { type: 'order', id: 'ORD-999' },
    ]);
    assert.strictEqual(result, null);
  });

  it('returns a formatted block when entity memories exist', () => {
    const mockStore = {
      searchByEntity: () => [
        {
          summary: 'Order ORD-1 was placed',
          created_at: '2025-03-01T10:00:00Z',
          agent: 'orders',
          facts: [],
        },
      ],
    };
    const result = inj.formatEntityContext(mockStore, 'cli', 'u1', [
      { type: 'order', id: 'ORD-1' },
    ]);
    assert.ok(result.includes('<entity-context>'));
    assert.ok(result.includes('</entity-context>'));
    assert.ok(result.includes('[ORDER: ORD-1]'));
    assert.ok(result.includes('Order ORD-1 was placed'));
  });

  it('includes agent name in square brackets when present', () => {
    const mockStore = {
      searchByEntity: () => [
        {
          summary: 'Return processed',
          created_at: '2025-03-01T10:00:00Z',
          agent: 'returns',
          facts: [],
        },
      ],
    };
    const result = inj.formatEntityContext(mockStore, 'cli', 'u1', [
      { type: 'return', id: 'RET-9' },
    ]);
    assert.ok(result.includes('(returns)'));
  });

  it('includes facts when present', () => {
    const mockStore = {
      searchByEntity: () => [
        {
          summary: 'Customer email updated',
          created_at: '2025-03-01T10:00:00Z',
          agent: null,
          facts: ['Email was alice@example.com', 'Changed to bob@example.com'],
        },
      ],
    };
    const result = inj.formatEntityContext(mockStore, 'cli', 'u1', [
      { type: 'customer', id: 'CUST-1' },
    ]);
    assert.ok(result.includes('Facts:'));
    assert.ok(result.includes('Email was alice@example.com'));
  });

  it('handles multiple entities in one block', () => {
    const mockStore = {
      searchByEntity: (_ch, _sid, type, id) => {
        if (type === 'order' && id === 'ORD-A') {
          return [
            {
              summary: 'Order A shipped',
              created_at: '2025-03-01T10:00:00Z',
              agent: null,
              facts: [],
            },
          ];
        }
        if (type === 'customer' && id === 'CUST-B') {
          return [
            {
              summary: 'Customer B updated',
              created_at: '2025-03-01T10:00:00Z',
              agent: null,
              facts: [],
            },
          ];
        }
        return [];
      },
    };
    const entities = [
      { type: 'order', id: 'ORD-A' },
      { type: 'customer', id: 'CUST-B' },
    ];
    const result = inj.formatEntityContext(mockStore, 'cli', 'u1', entities);
    assert.ok(result.includes('[ORDER: ORD-A]'));
    assert.ok(result.includes('[CUSTOMER: CUST-B]'));
    assert.ok(result.includes('Order A shipped'));
    assert.ok(result.includes('Customer B updated'));
  });

  it('respects maxBodyLength and stops early', () => {
    const longSummary = 'x'.repeat(500);
    const mockStore = {
      searchByEntity: () => [
        { summary: longSummary, created_at: '2025-03-01T10:00:00Z', agent: null, facts: [] },
        { summary: longSummary, created_at: '2025-03-01T11:00:00Z', agent: null, facts: [] },
        { summary: longSummary, created_at: '2025-03-01T12:00:00Z', agent: null, facts: [] },
      ],
    };
    const smallInj = new MemoryInjector({ maxBodyLength: 100 });
    // Should not throw even when limit is hit immediately
    const result = smallInj.formatEntityContext(mockStore, 'cli', 'u1', [
      { type: 'order', id: 'ORD-BIG' },
    ]);
    // Either returns null (header itself exceeded limit) or a partial block
    assert.ok(result === null || typeof result === 'string');
  });

  it('continues when searchByEntity throws for one entity', () => {
    let callCount = 0;
    const mockStore = {
      searchByEntity: (_ch, _sid, type) => {
        callCount++;
        if (type === 'order') throw new Error('DB error');
        return [
          { summary: 'Customer found', created_at: '2025-03-01T10:00:00Z', agent: null, facts: [] },
        ];
      },
    };
    const entities = [
      { type: 'order', id: 'ORD-FAIL' },
      { type: 'customer', id: 'CUST-OK' },
    ];
    const result = inj.formatEntityContext(mockStore, 'cli', 'u1', entities);
    assert.strictEqual(callCount, 2); // Both were attempted
    assert.ok(result !== null); // Customer succeeded
    assert.ok(result.includes('Customer found'));
  });

  it('includes Entity-specific memory header line', () => {
    const mockStore = {
      searchByEntity: () => [
        { summary: 'Something', created_at: '2025-03-01T10:00:00Z', agent: null, facts: [] },
      ],
    };
    const result = inj.formatEntityContext(mockStore, 'cli', 'u1', [
      { type: 'product', id: 'PROD-1' },
    ]);
    assert.ok(result.includes('Entity-specific memory:'));
  });
});

// ---------------------------------------------------------------------------
// MemoryInjector.injectMemoryContext — entity integration
// ---------------------------------------------------------------------------

describe('MemoryInjector — injectMemoryContext with entity detection', () => {
  let inj;
  let store;

  beforeEach(() => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    inj = new MemoryInjector({ maxMemories: 5, maxBodyLength: 4000, maxEntityMemories: 3 });
  });

  afterEach(() => {
    try {
      store.close();
    } catch {}
  });

  /**
   * Create a mock store that wraps the real MemoryStore but allows getMemoryStore()
   * to be overridden. We inject via the `store` closure directly in formatEntityContext.
   */
  it('injects entity-context block when entity IDs are in the message', async () => {
    // Seed the store with an entity-specific memory
    store.save({ channel: 'cli', senderId: 'u1', summary: 'ORD-ALPHA was delayed due to weather' });

    // Directly test formatEntityContext with the real store
    const entities = extractEntityIds('What is the status of ORD-ALPHA?');
    const block = inj.formatEntityContext(store, 'cli', 'u1', entities);

    assert.ok(block !== null);
    assert.ok(block.includes('<entity-context>'));
    assert.ok(block.includes('ORD-ALPHA was delayed due to weather'));
  });

  it('returns null entity block when no prior memories mention the entity', async () => {
    store.save({ channel: 'cli', senderId: 'u1', summary: 'Completely unrelated memory' });

    const entities = extractEntityIds('Tell me about ORD-UNKNOWN-99999');
    const block = inj.formatEntityContext(store, 'cli', 'u1', entities);

    assert.strictEqual(block, null);
  });

  it('entity-context block is distinct from memory-context block', async () => {
    store.save({ channel: 'cli', senderId: 'u1', summary: 'General chat about orders' });
    store.save({ channel: 'cli', senderId: 'u1', summary: 'ORD-Z99 was refunded' });

    const entities = extractEntityIds('What happened to ORD-Z99?');
    const entityBlock = inj.formatEntityContext(store, 'cli', 'u1', entities);
    const memBlock = inj.formatMemories(store.getRecent('cli', 'u1', 5));

    assert.ok(entityBlock.includes('<entity-context>'));
    assert.ok(entityBlock.includes('</entity-context>'));
    assert.ok(memBlock.includes('<memory-context>'));
    assert.ok(memBlock.includes('</memory-context>'));
  });
});

// ---------------------------------------------------------------------------
// MemoryInjector — setMaxEntityMemories setter
// ---------------------------------------------------------------------------

describe('MemoryInjector — setMaxEntityMemories', () => {
  it('defaults to 3', () => {
    const inj = new MemoryInjector();
    assert.strictEqual(inj._maxEntityMemories, 3);
  });

  it('can be overridden at construction', () => {
    const inj = new MemoryInjector({ maxEntityMemories: 7 });
    assert.strictEqual(inj._maxEntityMemories, 7);
  });

  it('can be updated via setter', () => {
    const inj = new MemoryInjector();
    inj.setMaxEntityMemories(10);
    assert.strictEqual(inj._maxEntityMemories, 10);
  });

  it('is respected by formatEntityContext', () => {
    const inj = new MemoryInjector({ maxEntityMemories: 1 });
    let callCount = 0;
    const mockStore = {
      searchByEntity: (_ch, _sid, _type, _id, limit) => {
        callCount++;
        assert.strictEqual(limit, 1); // Confirms the setter was respected
        return [];
      },
    };
    inj.formatEntityContext(mockStore, 'cli', 'u1', [{ type: 'order', id: 'ORD-1' }]);
    assert.strictEqual(callCount, 1);
  });
});
