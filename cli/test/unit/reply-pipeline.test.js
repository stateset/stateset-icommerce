/**
 * Tests for cli/src/channels/reply-pipeline.js
 *
 * Covers: ReplyPipeline, StreamSession, createReplyPipeline.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

import { ReplyPipeline, createReplyPipeline } from '../../src/channels/reply-pipeline.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createTestPipeline(overrides = {}) {
  const sent = [];
  const pipeline = new ReplyPipeline({
    onBlockReply: async (payload) => {
      sent.push(payload);
    },
    dedup: true,
    dedupWindowMs: 1000,
    bufferMs: 0,
    rateLimitMs: 0,
    ...overrides,
  });
  return { pipeline, sent };
}

// ---------------------------------------------------------------------------
// Basic send
// ---------------------------------------------------------------------------

describe('ReplyPipeline', () => {
  let pipeline;
  let sent;

  afterEach(async () => {
    if (pipeline) await pipeline.shutdown();
  });

  it('sends a message directly when no buffering', async () => {
    ({ pipeline, sent } = createTestPipeline());
    const result = await pipeline.send({ targetId: 'ch1', text: 'hello' });
    assert.ok(result.sent);
    assert.equal(sent.length, 1);
    assert.equal(sent[0].text, 'hello');
  });

  it('deduplicates identical messages', async () => {
    ({ pipeline, sent } = createTestPipeline());
    await pipeline.send({ targetId: 'ch1', text: 'hello' });
    const result = await pipeline.send({ targetId: 'ch1', text: 'hello' });
    assert.ok(!result.sent);
    assert.equal(result.reason, 'duplicate');
    assert.equal(sent.length, 1);
  });

  it('allows dedup bypass with different key', async () => {
    ({ pipeline, sent } = createTestPipeline());
    await pipeline.send({ targetId: 'ch1', text: 'hello', key: 'k1' });
    const result = await pipeline.send({ targetId: 'ch1', text: 'hello', key: 'k2' });
    assert.ok(result.sent);
    assert.equal(sent.length, 2);
  });

  it('disables dedup when dedup=false', async () => {
    ({ pipeline, sent } = createTestPipeline({ dedup: false }));
    await pipeline.send({ targetId: 'ch1', text: 'hello' });
    const result = await pipeline.send({ targetId: 'ch1', text: 'hello' });
    assert.ok(result.sent);
    assert.equal(sent.length, 2);
  });

  it('respects abort signal', async () => {
    ({ pipeline, sent } = createTestPipeline());
    const ac = new AbortController();
    ac.abort();
    const result = await pipeline.send({ targetId: 'ch1', text: 'hi' }, { signal: ac.signal });
    assert.ok(!result.sent);
    assert.equal(result.reason, 'aborted');
  });

  it('sendAll sends multiple payloads', async () => {
    ({ pipeline, sent } = createTestPipeline({ dedup: false }));
    const results = await pipeline.sendAll([
      { targetId: 'ch1', text: 'a' },
      { targetId: 'ch1', text: 'b' },
    ]);
    assert.equal(results.length, 2);
    assert.ok(results[0].sent);
    assert.ok(results[1].sent);
    assert.equal(sent.length, 2);
  });

  it('handles send errors gracefully', async () => {
    pipeline = new ReplyPipeline({
      onBlockReply: async () => {
        throw new Error('send failed');
      },
      dedup: false,
    });
    const result = await pipeline.send({ targetId: 'ch1', text: 'fail' });
    assert.ok(!result.sent);
    assert.equal(result.reason, 'send failed');
  });
});

// ---------------------------------------------------------------------------
// Buffering
// ---------------------------------------------------------------------------

describe('ReplyPipeline buffering', () => {
  let pipeline;
  let sent;

  afterEach(async () => {
    if (pipeline) await pipeline.shutdown();
  });

  it('buffers messages and flushes', async () => {
    ({ pipeline, sent } = createTestPipeline({ bufferMs: 50, dedup: false }));
    const result = await pipeline.send({ targetId: 'ch1', text: 'buffered' });
    assert.ok(result.sent);
    assert.equal(result.reason, 'buffered');
    // Message is in buffer, not yet sent
    assert.equal(sent.length, 0);

    // Wait for flush
    await new Promise((r) => setTimeout(r, 100));
    assert.equal(sent.length, 1);
  });

  it('coalesces buffered messages when enabled', async () => {
    ({ pipeline, sent } = createTestPipeline({
      bufferMs: 50,
      dedup: false,
      coalescing: { enabled: true, separator: ' | ' },
    }));

    await pipeline.send({ targetId: 'ch1', text: 'a' });
    await pipeline.send({ targetId: 'ch1', text: 'b' });

    await new Promise((r) => setTimeout(r, 100));
    assert.equal(sent.length, 1);
    assert.equal(sent[0].text, 'a | b');
  });

  it('flush() sends all buffered messages', async () => {
    ({ pipeline, sent } = createTestPipeline({ bufferMs: 60000, dedup: false }));
    await pipeline.send({ targetId: 'ch1', text: 'delayed' });
    assert.equal(sent.length, 0);

    await pipeline.flush();
    assert.equal(sent.length, 1);
  });
});

// ---------------------------------------------------------------------------
// Rate limiting
// ---------------------------------------------------------------------------

describe('ReplyPipeline rate limiting', () => {
  let pipeline;
  let sent;

  afterEach(async () => {
    if (pipeline) await pipeline.shutdown();
  });

  it('enforces minimum time between sends', async () => {
    ({ pipeline, sent } = createTestPipeline({ rateLimitMs: 50, dedup: false }));

    const start = Date.now();
    await pipeline.send({ targetId: 'ch1', text: 'a' });
    await pipeline.send({ targetId: 'ch1', text: 'b' });
    const elapsed = Date.now() - start;

    assert.equal(sent.length, 2);
    assert.ok(elapsed >= 40, `Expected >= 40ms, got ${elapsed}ms`);
  });
});

// ---------------------------------------------------------------------------
// Streaming
// ---------------------------------------------------------------------------

describe('ReplyPipeline streaming', () => {
  let pipeline;
  let sent;

  afterEach(async () => {
    if (pipeline) await pipeline.shutdown();
  });

  it('startStream / write / end flushes accumulated text', async () => {
    ({ pipeline, sent } = createTestPipeline({
      coalescing: { enabled: true, flushIntervalMs: 60000 },
    }));

    const stream = pipeline.startStream('ch1');
    await stream.write('Hello ');
    await stream.write('World');
    await stream.end();

    assert.equal(sent.length, 1);
    assert.equal(sent[0].text, 'Hello World');
  });

  it('stream abort does not send', async () => {
    ({ pipeline, sent } = createTestPipeline({
      coalescing: { enabled: true, flushIntervalMs: 60000 },
    }));

    const stream = pipeline.startStream('ch1');
    await stream.write('data');
    stream.abort();

    assert.equal(sent.length, 0);
  });

  it('stream write after end throws', async () => {
    ({ pipeline, sent } = createTestPipeline({
      coalescing: { enabled: true, flushIntervalMs: 60000 },
    }));

    const stream = pipeline.startStream('ch1');
    await stream.end();
    await assert.rejects(() => stream.write('late'), /already ended/);
  });
});

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

describe('ReplyPipeline stats', () => {
  let pipeline;

  afterEach(async () => {
    if (pipeline) await pipeline.shutdown();
  });

  it('tracks sent and deduped counters', async () => {
    ({ pipeline } = createTestPipeline());
    await pipeline.send({ targetId: 'ch1', text: 'a' });
    await pipeline.send({ targetId: 'ch1', text: 'a' }); // dedup

    const stats = pipeline.getStats();
    assert.equal(stats.totalSent, 1);
    assert.equal(stats.totalDeduped, 1);
    assert.equal(stats.totalErrors, 0);
  });
});

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

describe('createReplyPipeline', () => {
  it('returns a ReplyPipeline instance', async () => {
    const p = createReplyPipeline({ onBlockReply: async () => {} });
    assert.ok(p instanceof ReplyPipeline);
    await p.shutdown();
  });
});
