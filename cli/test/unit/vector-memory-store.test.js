import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  VectorMemoryStore,
  resetVectorMemoryStore,
} from '../../src/memory/vector-store.js';
import { MemoryStore, resetMemoryStore } from '../../src/memory/store.js';

function tmpDbPath() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vector-mem-test-'));
  return path.join(dir, 'memory.db');
}

describe('VectorMemoryStore', () => {
  let baseStore = null;
  let vectorStore = null;

  afterEach(() => {
    try {
      vectorStore?.close();
    } catch {}
    try {
      baseStore?.close();
    } catch {}
    vectorStore = null;
    baseStore = null;
    resetVectorMemoryStore();
    resetMemoryStore();
  });

  it('saves memories and returns vector matches', () => {
    const dbPath = tmpDbPath();
    baseStore = new MemoryStore({ dbPath });
    vectorStore = new VectorMemoryStore({ dbPath, memoryStore: baseStore });

    vectorStore.save({
      channel: 'cli',
      senderId: 'local',
      summary: 'Customer asked about the electronics return policy',
      facts: ['return_window:30_days'],
    });
    vectorStore.save({
      channel: 'cli',
      senderId: 'local',
      summary: 'Warehouse stock count for winter jackets',
      facts: ['inventory'],
    });

    const results = vectorStore.vectorSearch('electronics return policy', {
      channel: 'cli',
      senderId: 'local',
      limit: 5,
      minSimilarity: 0.01,
    });

    assert.ok(results.length > 0);
    assert.match(results[0].summary, /return policy/i);
    assert.ok(results[0].similarity > 0);
  });

  it('hybridSearch combines text and vector results', () => {
    const dbPath = tmpDbPath();
    baseStore = new MemoryStore({ dbPath });
    vectorStore = new VectorMemoryStore({ dbPath, memoryStore: baseStore });

    vectorStore.save({
      channel: 'cli',
      senderId: 'local',
      summary: 'Customer requested refund for damaged headphones',
      facts: ['refund', 'damaged'],
    });

    const results = vectorStore.hybridSearch('refund damaged headphones', {
      channel: 'cli',
      senderId: 'local',
      limit: 3,
    });

    assert.ok(results.length > 0);
    assert.equal(results[0].summary, 'Customer requested refund for damaged headphones');
    assert.ok(results[0].score > 0);
  });

  it('backfill indexes existing memories without vectors', () => {
    const dbPath = tmpDbPath();
    baseStore = new MemoryStore({ dbPath });
    vectorStore = new VectorMemoryStore({ dbPath, memoryStore: baseStore });

    baseStore.save({
      channel: 'cli',
      senderId: 'local',
      summary: 'Order ORD-123 shipped with tracking information',
    });

    const result = vectorStore.backfill('cli', 'local');
    assert.equal(result.processed, 1);
    assert.equal(result.errors, 0);
    assert.equal(vectorStore.vectorCount(), 1);
  });
});
