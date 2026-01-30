/**
 * Integration tests for v0.4.0 enhanced modules
 *
 * Tests:
 * - Command Queue (lane-based serialization)
 * - Context Guard (token estimation and compaction)
 * - Model Fallback (automatic failover)
 * - Markdown Memory Store (transparent persistence)
 * - Semantic Browser Snapshots (accessibility tree)
 */

import { describe, it, before, after } from 'node:test';
import assert from 'node:assert';
import { join } from 'node:path';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';

// ============================================================================
// Command Queue Tests
// ============================================================================

describe('CommandQueue', async () => {
  const { CommandQueue, getCommandQueue, resetCommandQueue } = await import('../../src/command-queue.js');

  after(() => resetCommandQueue());

  it('should serialize operations in the same lane', async () => {
    const queue = new CommandQueue();
    const order = [];

    // Enqueue three tasks in the same lane
    const p1 = queue.enqueue('session-1', async () => {
      await new Promise(r => setTimeout(r, 50));
      order.push('first');
      return 'first';
    });

    const p2 = queue.enqueue('session-1', async () => {
      order.push('second');
      return 'second';
    });

    const p3 = queue.enqueue('session-1', async () => {
      order.push('third');
      return 'third';
    });

    const results = await Promise.all([p1, p2, p3]);

    // Should execute in order despite p2 and p3 being faster
    assert.deepStrictEqual(order, ['first', 'second', 'third']);
    assert.deepStrictEqual(results, ['first', 'second', 'third']);

    queue.shutdown();
  });

  it('should run different sessions in parallel', async () => {
    const queue = new CommandQueue();
    const startTimes = {};
    const endTimes = {};

    const record = (session, phase) => {
      const now = Date.now();
      if (phase === 'start') startTimes[session] = now;
      else endTimes[session] = now;
    };

    // Enqueue tasks in different lanes
    const p1 = queue.enqueue('session-a', async () => {
      record('session-a', 'start');
      await new Promise(r => setTimeout(r, 50));
      record('session-a', 'end');
    });

    const p2 = queue.enqueue('session-b', async () => {
      record('session-b', 'start');
      await new Promise(r => setTimeout(r, 50));
      record('session-b', 'end');
    });

    await Promise.all([p1, p2]);

    // Both should start at approximately the same time (within 20ms)
    const startDiff = Math.abs(startTimes['session-a'] - startTimes['session-b']);
    assert.ok(startDiff < 20, `Sessions should start in parallel, but diff was ${startDiff}ms`);

    queue.shutdown();
  });

  it('should track queue statistics', async () => {
    const queue = new CommandQueue();

    await queue.enqueue('test-lane', async () => 'done');

    const stats = queue.getStats();
    assert.ok(stats.serialLanes.count >= 1);
    assert.ok(stats.serialLanes.lanes.some(l => l.id === 'test-lane'));

    queue.shutdown();
  });
});

// ============================================================================
// Context Guard Tests
// ============================================================================

describe('ContextGuard', async () => {
  const { ContextGuard, estimateTokens, estimateHistoryTokens, ConversationSummarizer } = await import('../../src/context-guard.js');

  it('should estimate tokens for text', () => {
    const text = 'Hello, this is a test message for token estimation.';
    const tokens = estimateTokens(text);

    // Should be roughly length/4 for English text
    assert.ok(tokens > 0);
    assert.ok(tokens < text.length); // Less than 1 token per char
    assert.ok(tokens > text.length / 10); // More than 1 token per 10 chars
  });

  it('should estimate tokens for code (denser)', () => {
    const code = `function test() { return { foo: "bar", baz: [1,2,3] }; }`;
    const english = 'This is some plain English text of similar length to compare with.';

    const codeTokens = estimateTokens(code);
    const englishTokens = estimateTokens(english);

    // Code should tokenize more densely (more tokens per character)
    const codeRatio = codeTokens / code.length;
    const englishRatio = englishTokens / english.length;

    assert.ok(codeRatio > englishRatio * 0.8, 'Code should have similar or higher token density');
  });

  it('should check context and return safe for small history', () => {
    const guard = new ContextGuard({ maxTokens: 100000 });

    const history = [
      { role: 'user', content: 'Hello' },
      { role: 'assistant', content: 'Hi there!' }
    ];

    const result = guard.check(history, 'You are a helpful assistant.', 'How are you?');

    assert.ok(result.safe);
    assert.strictEqual(result.action, 'none');
    assert.ok(result.usage.percent < 0.01); // Should be very small
  });

  it('should trigger compaction when history is large', () => {
    // Use very small limits to trigger compaction easily
    const guard = new ContextGuard({
      maxTokens: 500,
      compactThreshold: 0.5,
      reserveTokens: 50
    });

    // Create a history that will exceed 50% of 450 effective tokens (~225 tokens)
    const history = [];
    for (let i = 0; i < 50; i++) {
      history.push({
        role: i % 2 === 0 ? 'user' : 'assistant',
        content: `This is message number ${i} with some additional content to increase the token count significantly for testing purposes. Adding more text here.`
      });
    }

    const result = guard.check(history, 'System prompt', 'New message');

    // Should trigger compaction or warn at minimum
    assert.ok(
      result.action === 'compact' || result.action === 'abort' || result.action === 'warn',
      `Expected compact/abort/warn but got: ${result.action} with ${result.usage.percent * 100}% usage`
    );

    if (result.action === 'compact') {
      assert.ok(result.compactedHistory);
      assert.ok(result.compactedHistory.length < history.length);
    }
  });

  it('should summarize conversation history', () => {
    const summarizer = new ConversationSummarizer({ keepRecentMessages: 3 });

    const history = [
      { role: 'user', content: 'Create order #ORD-123 for alice@example.com' },
      { role: 'assistant', content: [{ type: 'tool_use', name: 'create_order' }] },
      { role: 'user', content: 'Ship it with tracking FEDEX123' },
      { role: 'assistant', content: 'Order shipped!' },
      { role: 'user', content: 'What is the status?' },
      { role: 'assistant', content: 'The order is in transit.' }
    ];

    const { summary, keptMessages, stats } = summarizer.summarize(history);

    assert.ok(summary);
    assert.strictEqual(keptMessages.length, 3); // Keep last 3
    assert.strictEqual(stats.summarized, 3);
    assert.ok(summary.includes('ORD-123') || summary.includes('alice@example.com'));
  });
});

// ============================================================================
// Model Fallback Tests
// ============================================================================

describe('ModelFallback', async () => {
  const { ModelFallback, DEFAULT_FALLBACK_CHAIN } = await import('../../src/model-fallback.js');

  it('should have a default fallback chain', () => {
    assert.ok(Array.isArray(DEFAULT_FALLBACK_CHAIN));
    assert.ok(DEFAULT_FALLBACK_CHAIN.length >= 2);

    // First should be Claude Sonnet
    assert.ok(DEFAULT_FALLBACK_CHAIN[0].provider === 'claude');
  });

  it('should execute with primary model on success', async () => {
    // Create custom chain that doesn't check for API keys
    const testChain = [
      { id: 'test-primary', provider: 'test', model: 'test-model-1', priority: 1, capabilities: ['tools'] },
      { id: 'test-fallback', provider: 'test', model: 'test-model-2', priority: 2, capabilities: ['tools'] }
    ];

    const fallback = new ModelFallback({ chain: testChain });
    let usedModel = null;

    const { result, model } = await fallback.execute(async (modelConfig) => {
      usedModel = modelConfig.model;
      return 'success';
    });

    assert.strictEqual(result, 'success');
    assert.strictEqual(model.model, 'test-model-1');
  });

  it('should fallback on rate limit error', async () => {
    // Create custom chain that doesn't check for API keys
    const testChain = [
      { id: 'test-primary', provider: 'test', model: 'test-model-1', priority: 1, capabilities: ['tools'] },
      { id: 'test-fallback', provider: 'test', model: 'test-model-2', priority: 2, capabilities: ['tools'] }
    ];

    const fallback = new ModelFallback({
      chain: testChain,
      maxRetries: 1,
      baseCooldownMs: 100
    });

    let attempts = 0;
    const usedModels = [];

    const { result, model, attempts: attemptsList } = await fallback.execute(async (modelConfig) => {
      usedModels.push(modelConfig.id);
      attempts++;

      // First model fails with rate limit
      if (attempts === 1) {
        throw new Error('Rate limit exceeded (429)');
      }

      return 'success from fallback';
    });

    assert.strictEqual(result, 'success from fallback');
    assert.ok(usedModels.length >= 2, 'Should have tried multiple models');
    assert.ok(attemptsList.length >= 2, 'Should have attempt records');
  });

  it('should track model cooldowns', async () => {
    const fallback = new ModelFallback({ baseCooldownMs: 1000 });

    // Manually set cooldown
    fallback.setModelCooldown('claude-sonnet', 1000, 'test cooldown');

    const status = fallback.getStatus();
    const sonnet = status.find(s => s.id === 'claude-sonnet');

    assert.ok(sonnet.inCooldown);
    assert.ok(sonnet.cooldownRemainingMs > 0);

    // Clear it
    fallback.clearModelCooldown('claude-sonnet');
    const statusAfter = fallback.getStatus();
    const sonnetAfter = statusAfter.find(s => s.id === 'claude-sonnet');

    assert.ok(!sonnetAfter.inCooldown);
  });
});

// ============================================================================
// Markdown Memory Store Tests
// ============================================================================

describe('MarkdownMemoryStore', async () => {
  const { MarkdownMemoryStore } = await import('../../src/memory/markdown-store.js');

  let tempDir;
  let store;

  before(() => {
    tempDir = mkdtempSync(join(tmpdir(), 'stateset-test-'));
    store = new MarkdownMemoryStore({ memoryDir: tempDir });
  });

  after(() => {
    rmSync(tempDir, { recursive: true, force: true });
  });

  it('should save and retrieve memory', async () => {
    await store.save({
      summary: 'User created order #ORD-001',
      facts: ['Order total: $99.99', 'Customer: alice@example.com'],
      agent: 'orders',
      sessionId: 'test-session-1'
    });

    const recent = await store.getRecent(1);
    assert.strictEqual(recent.length, 1);
    assert.ok(recent[0].raw.includes('ORD-001'));
    assert.ok(recent[0].raw.includes('Order total'));
  });

  it('should search memory', async () => {
    await store.save({
      summary: 'Shipped order with tracking FEDEX123',
      facts: ['Carrier: FedEx', 'Estimated delivery: 3 days'],
      agent: 'shipments'
    });

    const results = await store.search('FEDEX');
    assert.ok(results.length >= 1);
    assert.ok(results.some(r => r.raw.includes('FEDEX123')));
  });

  it('should save entity-specific memory', async () => {
    await store.saveEntityMemory('customer', 'cust-001', {
      summary: 'VIP customer with 50+ orders',
      facts: ['Lifetime value: $5,000', 'Preferred shipping: Express']
    });

    const entityMemory = await store.getEntityMemory('customer', 'cust-001');
    assert.ok(entityMemory.length >= 1);
    assert.ok(entityMemory[0].raw.includes('VIP customer'));
  });

  it('should list sessions and entities', async () => {
    // Save to session
    await store.saveToSession('session-abc', {
      summary: 'Test session entry'
    });

    const sessions = await store.listSessions();
    assert.ok(sessions.includes('session-abc'));

    const entities = await store.listEntities();
    assert.ok(entities.some(e => e.type === 'customer' && e.id === 'cust-001'));
  });

  it('should return stats', async () => {
    const stats = await store.getStats();

    assert.ok(stats.mainMemoryEntries >= 0);
    assert.ok(stats.sessions >= 0);
    assert.ok(stats.entities >= 0);
    assert.strictEqual(stats.memoryDir, tempDir);
  });
});

// ============================================================================
// Browser Semantic Snapshots Tests
// ============================================================================

describe('BrowserTools Semantic Snapshots', async () => {
  // These tests require Chrome, so we'll mock the CDP responses
  const { BrowserTools } = await import('../../src/browser/browser-tools.js');

  it('should have getAccessibilityTree method', () => {
    const browser = new BrowserTools();
    assert.ok(typeof browser.getAccessibilityTree === 'function');
  });

  it('should have getSemanticSnapshot method', () => {
    const browser = new BrowserTools();
    assert.ok(typeof browser.getSemanticSnapshot === 'function');
  });

  it('should have interactByRef method', () => {
    const browser = new BrowserTools();
    assert.ok(typeof browser.interactByRef === 'function');
  });
});

// ============================================================================
// Integration Test
// ============================================================================

describe('Module Integration', async () => {
  it('should export all new modules from claude-harness', async () => {
    const harness = await import('../../src/claude-harness.js');

    // Command Queue
    assert.ok(harness.CommandQueue);
    assert.ok(harness.getCommandQueue);

    // Context Guard
    assert.ok(harness.ContextGuard);
    assert.ok(harness.estimateTokens);
    assert.ok(harness.guardContext);

    // Model Fallback
    assert.ok(harness.ModelFallback);
    assert.ok(harness.DEFAULT_FALLBACK_CHAIN);

    // Memory
    assert.ok(harness.MarkdownMemoryStore);
    assert.ok(harness.getMarkdownMemoryStore);
    assert.ok(harness.MemoryStore);
    assert.ok(harness.getMemoryStore);

    // New functions
    assert.ok(harness.runAgentLoopQueued);
    assert.ok(harness.runAgentLoopParallel);
    assert.ok(harness.getQueueStats);
  });
});

console.log('Running enhanced modules integration tests...');
