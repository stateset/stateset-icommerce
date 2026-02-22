/**
 * Knowledge Loader Test Suite
 *
 * Tests for cli/src/knowledge/loader.js
 * Covers: chunk(), loadAll(), isLoaded, reset(), indexChunks(), error handling
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import fs from 'node:fs/promises';
import os from 'node:os';

import { KnowledgeLoader } from '../../src/knowledge/loader.js';

// ============================================================================
// Mock vector store factory
// ============================================================================

function makeMockVectorStore(overrides = {}) {
  const docs = [];
  return {
    upsert: async (doc) => {
      docs.push(doc);
    },
    docs,
    ...overrides,
  };
}

// ============================================================================
// chunk() — text splitting
// ============================================================================

describe('KnowledgeLoader — chunk()', () => {
  let loader;

  beforeEach(() => {
    loader = new KnowledgeLoader(makeMockVectorStore());
  });

  it('returns empty array for empty text', () => {
    const result = loader.chunk('');
    assert.deepEqual(result, []);
  });

  it('returns empty array for whitespace-only text', () => {
    const result = loader.chunk('   \n\n   ');
    assert.deepEqual(result, []);
  });

  it('returns a single chunk when text is within maxChunkSize', () => {
    const text = 'Hello world, this is a short paragraph.';
    const result = loader.chunk(text, 512);
    assert.equal(result.length, 1);
    assert.equal(result[0], text);
  });

  it('splits text on heading boundaries', () => {
    const text = [
      '# Heading One',
      'Content for section one.',
      '',
      '## Heading Two',
      'Content for section two.',
    ].join('\n');
    const result = loader.chunk(text, 512);
    // The split on /\n#{1,3}\s+/ produces sections; the heading text itself is consumed
    // by the split regex, so we get the content after each heading
    assert.ok(result.length >= 2, `Expected >= 2 chunks, got ${result.length}`);
  });

  it('splits oversized sections by paragraph', () => {
    // Create a section larger than maxChunkSize with multiple paragraphs
    const para1 = 'A'.repeat(30);
    const para2 = 'B'.repeat(30);
    const para3 = 'C'.repeat(30);
    const text = [para1, '', para2, '', para3].join('\n');
    const result = loader.chunk(text, 50);
    assert.ok(result.length >= 2, `Expected >= 2 chunks, got ${result.length}`);
  });

  it('respects custom maxChunkSize by splitting paragraphs', () => {
    // Build a section with multiple paragraphs separated by blank lines
    const paragraphs = [];
    for (let i = 0; i < 10; i++) {
      paragraphs.push(`Paragraph ${i}: ${'x'.repeat(40)}`);
    }
    const text = paragraphs.join('\n\n'); // ~500 chars total, multiple paragraphs
    const result = loader.chunk(text, 100);
    for (const chunk of result) {
      assert.ok(chunk.length > 0, 'chunk should not be empty');
    }
    assert.ok(result.length > 1, `should produce multiple chunks, got ${result.length}`);
  });

  it('trims whitespace from chunks', () => {
    const text = '  Some content with leading spaces  ';
    const result = loader.chunk(text, 512);
    assert.equal(result.length, 1);
    assert.equal(result[0], text.trim());
  });

  it('handles text with only headings and no content', () => {
    const text = '# Heading\n## Another\n### Third';
    const result = loader.chunk(text, 512);
    // After splitting on headings, the remaining sections should be heading text
    // The first section before any heading is empty, headings split away their markers
    // We just verify no crash and chunks (if any) are non-empty
    for (const chunk of result) {
      assert.ok(chunk.length > 0, 'all chunks should be non-empty');
    }
  });
});

// ============================================================================
// isLoaded getter
// ============================================================================

describe('KnowledgeLoader — isLoaded', () => {
  it('is false by default', () => {
    const loader = new KnowledgeLoader(makeMockVectorStore());
    assert.equal(loader.isLoaded, false);
  });
});

// ============================================================================
// reset()
// ============================================================================

describe('KnowledgeLoader — reset()', () => {
  it('sets isLoaded back to false', async () => {
    const store = makeMockVectorStore();
    const loader = new KnowledgeLoader(store);

    // Create a temp directory with a markdown file to trigger loadAll
    const tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'kl-test-'));
    try {
      await fs.writeFile(path.join(tmpDir, 'test.md'), '# Test\nHello');
      await loader.loadAll(tmpDir);
      assert.equal(loader.isLoaded, true);

      loader.reset();
      assert.equal(loader.isLoaded, false);
    } finally {
      await fs.rm(tmpDir, { recursive: true, force: true });
    }
  });
});

// ============================================================================
// loadAll() — directory scanning and indexing
// ============================================================================

describe('KnowledgeLoader — loadAll()', () => {
  it('indexes markdown files from a directory', async () => {
    const store = makeMockVectorStore();
    const loader = new KnowledgeLoader(store);

    const tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'kl-test-'));
    try {
      await fs.writeFile(path.join(tmpDir, 'orders.md'), '# Orders\nOrder content here.');
      await fs.writeFile(path.join(tmpDir, 'returns.md'), '# Returns\nReturn content here.');
      await fs.writeFile(path.join(tmpDir, 'readme.txt'), 'Not a markdown file.');

      await loader.loadAll(tmpDir);

      assert.equal(loader.isLoaded, true);
      // Should have indexed chunks from orders.md and returns.md but NOT readme.txt
      assert.ok(store.docs.length >= 2, `Expected >= 2 docs, got ${store.docs.length}`);
      const topics = store.docs.map((d) => d.metadata.topic);
      assert.ok(topics.includes('orders'), 'should index orders topic');
      assert.ok(topics.includes('returns'), 'should index returns topic');
    } finally {
      await fs.rm(tmpDir, { recursive: true, force: true });
    }
  });

  it('is idempotent — second call is a no-op', async () => {
    const store = makeMockVectorStore();
    const loader = new KnowledgeLoader(store);

    const tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'kl-test-'));
    try {
      await fs.writeFile(path.join(tmpDir, 'data.md'), 'Some knowledge');

      await loader.loadAll(tmpDir);
      const countAfterFirst = store.docs.length;

      await loader.loadAll(tmpDir);
      assert.equal(store.docs.length, countAfterFirst, 'second loadAll should not add more docs');
    } finally {
      await fs.rm(tmpDir, { recursive: true, force: true });
    }
  });

  it('handles missing directory gracefully', async () => {
    const store = makeMockVectorStore();
    const loader = new KnowledgeLoader(store);

    // Should not throw — just logs a warning
    await loader.loadAll('/nonexistent/path/that/does/not/exist');
    assert.equal(loader.isLoaded, false, 'should remain unloaded when dir is missing');
    assert.equal(store.docs.length, 0, 'should not index anything');
  });

  it('handles empty directory', async () => {
    const store = makeMockVectorStore();
    const loader = new KnowledgeLoader(store);

    const tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'kl-test-'));
    try {
      await loader.loadAll(tmpDir);
      assert.equal(loader.isLoaded, true);
      assert.equal(store.docs.length, 0, 'empty dir means no docs indexed');
    } finally {
      await fs.rm(tmpDir, { recursive: true, force: true });
    }
  });
});

// ============================================================================
// indexChunks() — vector store upsert
// ============================================================================

describe('KnowledgeLoader — indexChunks()', () => {
  it('upserts chunks with correct id and metadata', async () => {
    const store = makeMockVectorStore();
    const loader = new KnowledgeLoader(store);

    await loader.indexChunks('orders', ['chunk zero', 'chunk one']);

    assert.equal(store.docs.length, 2);
    assert.equal(store.docs[0].id, 'knowledge:orders:0');
    assert.equal(store.docs[0].content, 'chunk zero');
    assert.deepEqual(store.docs[0].metadata, {
      source: 'knowledge',
      topic: 'orders',
      chunkIndex: 0,
    });
    assert.equal(store.docs[1].id, 'knowledge:orders:1');
    assert.equal(store.docs[1].content, 'chunk one');
    assert.equal(store.docs[1].metadata.chunkIndex, 1);
  });

  it('skips silently when store is null', async () => {
    const loader = new KnowledgeLoader(null);
    // Should not throw
    await loader.indexChunks('test', ['chunk']);
  });

  it('skips silently when store has no upsert method', async () => {
    const loader = new KnowledgeLoader({ search: async () => [] });
    // Should not throw
    await loader.indexChunks('test', ['chunk']);
  });

  it('continues indexing when individual upsert fails', async () => {
    let callCount = 0;
    const store = makeMockVectorStore({
      upsert: async (doc) => {
        callCount++;
        if (callCount === 1) throw new Error('Simulated failure');
        // second call succeeds
      },
    });
    const loader = new KnowledgeLoader(store);

    // Should not throw — failures are logged but do not block
    await loader.indexChunks('test', ['fail-chunk', 'ok-chunk']);
    assert.equal(callCount, 2, 'should attempt both chunks');
  });
});
