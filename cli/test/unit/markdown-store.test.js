/**
 * Unit tests for memory/markdown-store.js — MarkdownMemoryStore
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import mdStore, {
  MarkdownMemoryStore,
  getMarkdownMemoryStore,
  resetMarkdownMemoryStore,
} from '../../src/memory/markdown-store.js';

const { parseMemoryFile, formatEntry } = mdStore;

// ===========================================================================
// Helpers
// ===========================================================================

function createTmpDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'md-store-'));
}

let tmpDir;

// ===========================================================================
// parseMemoryFile
// ===========================================================================

describe('parseMemoryFile', () => {
  it('returns empty array for empty string', () => {
    assert.deepStrictEqual(parseMemoryFile(''), []);
  });

  it('returns empty array for null', () => {
    assert.deepStrictEqual(parseMemoryFile(null), []);
  });

  it('returns empty array for whitespace-only string', () => {
    assert.deepStrictEqual(parseMemoryFile('   \n  \n  '), []);
  });

  it('parses a single entry', () => {
    const content = `**2025-01-15 10:30:00**

**Summary:** User created an order
**Facts:**
- Order ID is ORD-123
- Customer is Alice
**Agent:** orders
**Session:** sess-001`;

    const entries = parseMemoryFile(content);
    assert.strictEqual(entries.length, 1);
    assert.strictEqual(entries[0].timestamp, '2025-01-15 10:30:00');
    assert.strictEqual(entries[0].summary, 'User created an order');
    assert.deepStrictEqual(entries[0].facts, ['Order ID is ORD-123', 'Customer is Alice']);
    assert.strictEqual(entries[0].agent, 'orders');
    assert.strictEqual(entries[0].sessionId, 'sess-001');
  });

  it('parses multiple entries separated by ---', () => {
    const content = `**2025-01-15 10:30:00**

**Summary:** First entry

---

**2025-01-15 11:00:00**

**Summary:** Second entry`;

    const entries = parseMemoryFile(content);
    assert.strictEqual(entries.length, 2);
    assert.strictEqual(entries[0].summary, 'First entry');
    assert.strictEqual(entries[1].summary, 'Second entry');
  });

  it('extracts timestamps correctly', () => {
    const content = `**2026-02-09 14:25:33**

**Summary:** Test`;

    const entries = parseMemoryFile(content);
    assert.strictEqual(entries[0].timestamp, '2026-02-09 14:25:33');
  });

  it('handles entries without facts', () => {
    const content = `**2025-01-15 10:30:00**

**Summary:** No facts here`;

    const entries = parseMemoryFile(content);
    assert.strictEqual(entries.length, 1);
    assert.strictEqual(entries[0].facts, undefined);
  });

  it('preserves raw content', () => {
    const content = `**2025-01-15 10:30:00**

**Summary:** Raw test`;

    const entries = parseMemoryFile(content);
    assert.ok(entries[0].raw.includes('Raw test'));
  });
});

// ===========================================================================
// formatEntry
// ===========================================================================

describe('formatEntry', () => {
  it('includes timestamp line', () => {
    const result = formatEntry({ summary: 'Test' });
    assert.ok(/\*\*\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\*\*/.test(result));
  });

  it('includes summary', () => {
    const result = formatEntry({ summary: 'Order created' });
    assert.ok(result.includes('**Summary:** Order created'));
  });

  it('includes facts list', () => {
    const result = formatEntry({ facts: ['Fact A', 'Fact B'] });
    assert.ok(result.includes('**Facts:**'));
    assert.ok(result.includes('- Fact A'));
    assert.ok(result.includes('- Fact B'));
  });

  it('includes agent', () => {
    const result = formatEntry({ agent: 'orders' });
    assert.ok(result.includes('**Agent:** orders'));
  });

  it('includes sessionId', () => {
    const result = formatEntry({ sessionId: 'sess-abc' });
    assert.ok(result.includes('**Session:** sess-abc'));
  });

  it('uses provided createdAt date', () => {
    const result = formatEntry({ createdAt: '2025-06-15T12:00:00Z', summary: 'Test' });
    assert.ok(result.includes('2025-06-15 12:00:00'));
  });

  it('omits summary line when summary is missing', () => {
    const result = formatEntry({ agent: 'test' });
    assert.ok(!result.includes('**Summary:**'));
  });

  it('omits facts section when facts is empty', () => {
    const result = formatEntry({ summary: 'Test', facts: [] });
    assert.ok(!result.includes('**Facts:**'));
  });
});

// ===========================================================================
// MarkdownMemoryStore — constructor
// ===========================================================================

describe('MarkdownMemoryStore — constructor', () => {
  beforeEach(() => {
    tmpDir = createTmpDir();
  });
  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('creates memory directory and subdirectories', () => {
    const store = new MarkdownMemoryStore({ memoryDir: tmpDir });
    assert.ok(fs.existsSync(path.join(tmpDir, 'sessions')));
    assert.ok(fs.existsSync(path.join(tmpDir, 'entities')));
    assert.ok(fs.existsSync(path.join(tmpDir, 'topics')));
  });

  it('uses default values for maxMainEntries and maxSessionEntries', () => {
    const store = new MarkdownMemoryStore({ memoryDir: tmpDir });
    assert.strictEqual(store.maxMainEntries, 100);
    assert.strictEqual(store.maxSessionEntries, 50);
  });

  it('accepts custom maxMainEntries', () => {
    const store = new MarkdownMemoryStore({ memoryDir: tmpDir, maxMainEntries: 10 });
    assert.strictEqual(store.maxMainEntries, 10);
  });
});

// ===========================================================================
// MarkdownMemoryStore — save & getRecent
// ===========================================================================

describe('MarkdownMemoryStore — save', () => {
  let store;

  beforeEach(() => {
    tmpDir = createTmpDir();
    store = new MarkdownMemoryStore({ memoryDir: tmpDir });
  });
  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('creates file on first save', async () => {
    await store.save({ summary: 'First entry' });
    assert.ok(fs.existsSync(store.mainMemoryPath));
  });

  it('appends entries on subsequent saves', async () => {
    await store.save({ summary: 'Entry 1' });
    await store.save({ summary: 'Entry 2' });
    const recent = await store.getRecent(10);
    assert.strictEqual(recent.length, 2);
  });

  it('trims when exceeding maxMainEntries', async () => {
    const small = new MarkdownMemoryStore({ memoryDir: tmpDir, maxMainEntries: 3 });
    await small.save({ summary: 'A' });
    await small.save({ summary: 'B' });
    await small.save({ summary: 'C' });
    await small.save({ summary: 'D' });
    const recent = await small.getRecent(10);
    // Should have at most 3 entries since oldest gets trimmed
    assert.ok(recent.length <= 3);
  });

  it('saves to session file when sessionId is provided', async () => {
    await store.save({ summary: 'Session entry', sessionId: 'sess-123' });
    const sessionPath = store.getSessionPath('sess-123');
    assert.ok(fs.existsSync(sessionPath));
  });
});

// ===========================================================================
// MarkdownMemoryStore — getRecent
// ===========================================================================

describe('MarkdownMemoryStore — getRecent', () => {
  let store;

  beforeEach(() => {
    tmpDir = createTmpDir();
    store = new MarkdownMemoryStore({ memoryDir: tmpDir });
  });
  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('returns empty array when file does not exist', async () => {
    const result = await store.getRecent(10);
    assert.deepStrictEqual(result, []);
  });

  it('respects limit parameter', async () => {
    await store.save({ summary: 'A' });
    await store.save({ summary: 'B' });
    await store.save({ summary: 'C' });
    const result = await store.getRecent(2);
    assert.strictEqual(result.length, 2);
  });

  it('returns entries in reverse order (most recent first)', async () => {
    await store.save({ summary: 'First' });
    await store.save({ summary: 'Second' });
    const result = await store.getRecent(10);
    assert.ok(result[0].raw.includes('Second'));
    assert.ok(result[1].raw.includes('First'));
  });
});

// ===========================================================================
// MarkdownMemoryStore — search
// ===========================================================================

describe('MarkdownMemoryStore — search', () => {
  let store;

  beforeEach(() => {
    tmpDir = createTmpDir();
    store = new MarkdownMemoryStore({ memoryDir: tmpDir });
  });
  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('finds matching entries', async () => {
    await store.save({ summary: 'Order created for Alice' });
    await store.save({ summary: 'Inventory adjusted for widgets' });
    const results = await store.search('alice');
    assert.strictEqual(results.length, 1);
    assert.ok(results[0].raw.includes('Alice'));
  });

  it('is case-insensitive', async () => {
    await store.save({ summary: 'Order for ALICE' });
    const results = await store.search('alice');
    assert.strictEqual(results.length, 1);
  });

  it('respects limit', async () => {
    await store.save({ summary: 'Match one' });
    await store.save({ summary: 'Match two' });
    await store.save({ summary: 'Match three' });
    const results = await store.search('match', 2);
    assert.strictEqual(results.length, 2);
  });

  it('returns empty array when no match', async () => {
    await store.save({ summary: 'Hello world' });
    const results = await store.search('nonexistent');
    assert.strictEqual(results.length, 0);
  });
});

// ===========================================================================
// MarkdownMemoryStore — session memory
// ===========================================================================

describe('MarkdownMemoryStore — session memory', () => {
  let store;

  beforeEach(() => {
    tmpDir = createTmpDir();
    store = new MarkdownMemoryStore({ memoryDir: tmpDir });
  });
  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('saves and reads session memory', async () => {
    await store.saveToSession('sess-1', { summary: 'Session note' });
    const entries = await store.getSessionMemory('sess-1');
    assert.ok(entries.length > 0);
    assert.ok(entries[0].raw.includes('Session note'));
  });

  it('returns empty array for nonexistent session', async () => {
    const entries = await store.getSessionMemory('nonexistent');
    assert.deepStrictEqual(entries, []);
  });

  it('writes to sessions/ subdirectory', async () => {
    await store.saveToSession('sess-abc', { summary: 'Test' });
    const expected = path.join(tmpDir, 'sessions', 'sess-abc.md');
    assert.ok(fs.existsSync(expected));
  });
});

// ===========================================================================
// MarkdownMemoryStore — entity memory
// ===========================================================================

describe('MarkdownMemoryStore — entity memory', () => {
  let store;

  beforeEach(() => {
    tmpDir = createTmpDir();
    store = new MarkdownMemoryStore({ memoryDir: tmpDir });
  });
  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('saves and reads entity memory', async () => {
    await store.saveEntityMemory('customer', 'CUST-001', { summary: 'VIP customer' });
    const entries = await store.getEntityMemory('customer', 'CUST-001');
    assert.ok(entries.length > 0);
  });

  it('sanitizes entity ID in path', () => {
    const p = store.getEntityPath('order', 'ORD/../../etc/passwd');
    assert.ok(!p.includes('..'));
    assert.ok(p.includes('entities'));
  });

  it('returns empty array for nonexistent entity', async () => {
    const entries = await store.getEntityMemory('customer', 'nonexistent');
    assert.deepStrictEqual(entries, []);
  });

  it('writes to entities/ subdirectory with correct filename', async () => {
    await store.saveEntityMemory('order', 'ORD-123', { summary: 'Test' });
    const expected = path.join(tmpDir, 'entities', 'order_ORD-123.md');
    assert.ok(fs.existsSync(expected));
  });
});

// ===========================================================================
// MarkdownMemoryStore — topic memory
// ===========================================================================

describe('MarkdownMemoryStore — topic memory', () => {
  let store;

  beforeEach(() => {
    tmpDir = createTmpDir();
    store = new MarkdownMemoryStore({ memoryDir: tmpDir });
  });
  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('saves and reads topic memory', async () => {
    await store.saveTopicMemory('inventory', { summary: 'Stock knowledge' });
    const entries = await store.getTopicMemory('inventory');
    assert.ok(entries.length > 0);
  });

  it('writes to topics/ subdirectory', async () => {
    await store.saveTopicMemory('shipping', { summary: 'Test' });
    const expected = path.join(tmpDir, 'topics', 'shipping.md');
    assert.ok(fs.existsSync(expected));
  });

  it('normalizes topic name to lowercase', () => {
    const p = store.getTopicPath('Shipping Rates');
    assert.ok(p.includes('shipping_rates'));
  });

  it('returns empty array for nonexistent topic', async () => {
    const entries = await store.getTopicMemory('nonexistent');
    assert.deepStrictEqual(entries, []);
  });
});

// ===========================================================================
// MarkdownMemoryStore — listing
// ===========================================================================

describe('MarkdownMemoryStore — listSessions / listEntities / listTopics', () => {
  let store;

  beforeEach(() => {
    tmpDir = createTmpDir();
    store = new MarkdownMemoryStore({ memoryDir: tmpDir });
  });
  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('listSessions returns session IDs', async () => {
    await store.saveToSession('sess-a', { summary: 'A' });
    await store.saveToSession('sess-b', { summary: 'B' });
    const sessions = await store.listSessions();
    assert.ok(sessions.includes('sess-a'));
    assert.ok(sessions.includes('sess-b'));
  });

  it('listEntities returns type and id', async () => {
    await store.saveEntityMemory('customer', 'C1', { summary: 'X' });
    await store.saveEntityMemory('order', 'O1', { summary: 'Y' });
    const entities = await store.listEntities();
    assert.strictEqual(entities.length, 2);
    const types = entities.map((e) => e.type);
    assert.ok(types.includes('customer'));
    assert.ok(types.includes('order'));
  });

  it('listTopics returns topic names', async () => {
    await store.saveTopicMemory('returns', { summary: 'X' });
    const topics = await store.listTopics();
    assert.ok(topics.includes('returns'));
  });
});

// ===========================================================================
// MarkdownMemoryStore — getStats
// ===========================================================================

describe('MarkdownMemoryStore — getStats', () => {
  let store;

  beforeEach(() => {
    tmpDir = createTmpDir();
    store = new MarkdownMemoryStore({ memoryDir: tmpDir });
  });
  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('returns correct counts', async () => {
    await store.save({ summary: 'A' });
    await store.save({ summary: 'B' });
    await store.saveToSession('s1', { summary: 'S' });
    await store.saveEntityMemory('customer', 'C1', { summary: 'E' });
    await store.saveTopicMemory('topic1', { summary: 'T' });

    const stats = await store.getStats();
    assert.strictEqual(stats.mainMemoryEntries, 2);
    assert.strictEqual(stats.sessions, 1);
    assert.strictEqual(stats.entities, 1);
    assert.strictEqual(stats.topics, 1);
    assert.strictEqual(stats.memoryDir, tmpDir);
  });
});

// ===========================================================================
// MarkdownMemoryStore — clear
// ===========================================================================

describe('MarkdownMemoryStore — clear', () => {
  let store;

  beforeEach(() => {
    tmpDir = createTmpDir();
    store = new MarkdownMemoryStore({ memoryDir: tmpDir });
  });
  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('removes all memory files', async () => {
    await store.save({ summary: 'Main' });
    await store.saveToSession('s1', { summary: 'Session' });
    await store.saveEntityMemory('customer', 'C1', { summary: 'Entity' });
    await store.saveTopicMemory('t1', { summary: 'Topic' });

    await store.clear();

    assert.ok(!fs.existsSync(store.mainMemoryPath));
    const stats = await store.getStats();
    assert.strictEqual(stats.mainMemoryEntries, 0);
    assert.strictEqual(stats.sessions, 0);
    assert.strictEqual(stats.entities, 0);
    assert.strictEqual(stats.topics, 0);
  });
});

// ===========================================================================
// Singleton — getMarkdownMemoryStore / resetMarkdownMemoryStore
// ===========================================================================

describe('getMarkdownMemoryStore / resetMarkdownMemoryStore', () => {
  afterEach(() => {
    resetMarkdownMemoryStore();
  });

  it('returns same instance on repeated calls', () => {
    const a = getMarkdownMemoryStore();
    const b = getMarkdownMemoryStore();
    assert.strictEqual(a, b);
  });

  it('returns new instance after reset', () => {
    const a = getMarkdownMemoryStore();
    resetMarkdownMemoryStore();
    const b = getMarkdownMemoryStore();
    assert.notStrictEqual(a, b);
  });

  it('is an instance of MarkdownMemoryStore', () => {
    const store = getMarkdownMemoryStore();
    assert.ok(store instanceof MarkdownMemoryStore);
  });
});
