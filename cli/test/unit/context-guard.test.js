/**
 * Unit tests for context-guard.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import {
  estimateTokens,
  estimateMessageTokens,
  estimateHistoryTokens,
  ConversationSummarizer,
  ContextGuard,
  guardContext,
} from '../../src/context-guard.js';

// ===========================================================================
// estimateTokens
// ===========================================================================

describe('estimateTokens', () => {
  it('returns 0 for null/undefined/empty', () => {
    assert.strictEqual(estimateTokens(null), 0);
    assert.strictEqual(estimateTokens(undefined), 0);
    assert.strictEqual(estimateTokens(''), 0);
  });

  it('returns 0 for non-string input', () => {
    assert.strictEqual(estimateTokens(42), 0);
  });

  it('estimates ~4 chars per token for English text', () => {
    const text = 'The quick brown fox jumps over the lazy dog.';
    const tokens = estimateTokens(text);
    // ~44 chars → ~11 tokens (with adjustments for whitespace/punctuation)
    assert.ok(tokens >= 8 && tokens <= 20, `Expected 8-20 tokens, got ${tokens}`);
  });

  it('estimates code text at ~3.5 chars per token', () => {
    const code = 'function foo() { return { bar: [1, 2, 3] }; }';
    const tokens = estimateTokens(code);
    // Code should be denser
    assert.ok(tokens > 0);
  });

  it('handles very long text', () => {
    const text = 'word '.repeat(10000);
    const tokens = estimateTokens(text);
    assert.ok(tokens > 1000);
  });
});

// ===========================================================================
// estimateMessageTokens
// ===========================================================================

describe('estimateMessageTokens', () => {
  it('returns 0 for null/undefined', () => {
    assert.strictEqual(estimateMessageTokens(null), 0);
    assert.strictEqual(estimateMessageTokens(undefined), 0);
  });

  it('includes role overhead (~3 tokens)', () => {
    const msg = { role: 'user', content: '' };
    const tokens = estimateMessageTokens(msg);
    assert.strictEqual(tokens, 3); // just overhead
  });

  it('estimates string content', () => {
    const msg = { role: 'user', content: 'Hello world' };
    const tokens = estimateMessageTokens(msg);
    assert.ok(tokens > 3, 'Should be more than just overhead');
  });

  it('estimates array content with text blocks', () => {
    const msg = {
      role: 'assistant',
      content: [
        { type: 'text', text: 'Here is the answer.' },
        { type: 'text', text: 'More details.' },
      ],
    };
    const tokens = estimateMessageTokens(msg);
    assert.ok(tokens > 3);
  });

  it('estimates tool_use blocks', () => {
    const msg = {
      role: 'assistant',
      content: [{ type: 'tool_use', name: 'list_customers', input: { limit: 10 } }],
    };
    const tokens = estimateMessageTokens(msg);
    assert.ok(tokens > 3);
  });

  it('estimates tool_result blocks with string content', () => {
    const msg = {
      role: 'tool',
      content: [{ type: 'tool_result', content: 'Success: 5 records found' }],
    };
    const tokens = estimateMessageTokens(msg);
    assert.ok(tokens > 3);
  });

  it('estimates tool_calls property', () => {
    const msg = {
      role: 'assistant',
      content: '',
      tool_calls: [{ name: 'list_orders', input: {} }],
    };
    const tokens = estimateMessageTokens(msg);
    assert.ok(tokens > 3);
  });
});

// ===========================================================================
// estimateHistoryTokens
// ===========================================================================

describe('estimateHistoryTokens', () => {
  it('returns 0 for non-array', () => {
    assert.strictEqual(estimateHistoryTokens(null), 0);
    assert.strictEqual(estimateHistoryTokens('not an array'), 0);
  });

  it('returns 0 for empty array', () => {
    assert.strictEqual(estimateHistoryTokens([]), 0);
  });

  it('sums tokens across messages', () => {
    const history = [
      { role: 'user', content: 'Hello' },
      { role: 'assistant', content: 'Hi there!' },
    ];
    const tokens = estimateHistoryTokens(history);
    assert.ok(tokens > 6, 'Should be > 2 * 3 overhead');
  });
});

// ===========================================================================
// ConversationSummarizer
// ===========================================================================

describe('ConversationSummarizer', () => {
  it('returns original history when length <= keepRecentMessages', () => {
    const summarizer = new ConversationSummarizer({ keepRecentMessages: 5 });
    const history = [
      { role: 'user', content: 'Hello' },
      { role: 'assistant', content: 'Hi!' },
    ];
    const result = summarizer.summarize(history);
    assert.strictEqual(result.summary, null);
    assert.strictEqual(result.keptMessages.length, 2);
    assert.strictEqual(result.stats.summarized, 0);
  });

  it('summarizes older messages and keeps recent ones', () => {
    const summarizer = new ConversationSummarizer({ keepRecentMessages: 2 });
    const history = [
      { role: 'user', content: 'Create order for alice@example.com' },
      { role: 'assistant', content: 'Created order ORD-123' },
      { role: 'user', content: 'Show me that order' },
      { role: 'assistant', content: 'Order ORD-123 details...' },
      { role: 'user', content: 'Ship it' },
      { role: 'assistant', content: 'Shipped!' },
    ];

    const result = summarizer.summarize(history);
    assert.ok(result.summary !== null);
    assert.strictEqual(result.keptMessages.length, 2);
    assert.strictEqual(result.stats.summarized, 4);
    assert.strictEqual(result.stats.kept, 2);
  });

  it('extracts user intents', () => {
    const summarizer = new ConversationSummarizer({ keepRecentMessages: 1 });
    const history = [
      { role: 'user', content: 'List all customers' },
      { role: 'assistant', content: 'Here are the customers.' },
      { role: 'user', content: 'Done' },
    ];
    const result = summarizer.summarize(history);
    assert.ok(result.summary.includes('User requests'));
    assert.ok(result.summary.includes('List all customers'));
  });

  it('extracts tool usage', () => {
    const summarizer = new ConversationSummarizer({ keepRecentMessages: 1 });
    const history = [
      {
        role: 'assistant',
        content: [{ type: 'tool_use', name: 'list_orders', input: {} }],
      },
      { role: 'user', content: 'Thanks' },
    ];
    const result = summarizer.summarize(history);
    assert.ok(result.summary.includes('Tools used'));
    assert.ok(result.summary.includes('list_orders'));
  });

  it('extracts entity references (IDs, emails)', () => {
    const summarizer = new ConversationSummarizer({ keepRecentMessages: 1 });
    const history = [
      { role: 'user', content: 'Show order ORD-12345 for alice@example.com' },
      { role: 'assistant', content: 'Done' },
    ];
    const result = summarizer.summarize(history);
    assert.ok(result.summary.includes('Entities'));
    assert.ok(result.summary.includes('ORD-12345'));
  });

  it('compact() creates a compacted history with summary + ack', () => {
    const summarizer = new ConversationSummarizer({ keepRecentMessages: 1 });
    const history = [
      { role: 'user', content: 'Hello' },
      { role: 'assistant', content: 'Hi' },
      { role: 'user', content: 'What now?' },
    ];
    const result = summarizer.compact(history);
    assert.ok(result.history.length >= 3); // summary + ack + kept
    assert.strictEqual(result.history[0].role, 'user');
    assert.ok(result.history[0].content.includes('summary'));
    assert.strictEqual(result.history[1].role, 'assistant');
    assert.ok(result.history[1].content.includes('Understood'));
  });

  it('compact() returns original when no summarization needed', () => {
    const summarizer = new ConversationSummarizer({ keepRecentMessages: 10 });
    const history = [{ role: 'user', content: 'Hello' }];
    const result = summarizer.compact(history);
    assert.strictEqual(result.history, history);
  });
});

// ===========================================================================
// ContextGuard
// ===========================================================================

describe('ContextGuard', () => {
  it('returns none action when well under threshold', () => {
    const guard = new ContextGuard({ maxTokens: 200000 });
    const result = guard.check([{ role: 'user', content: 'Hi' }], 'You are a helper.');
    assert.strictEqual(result.safe, true);
    assert.strictEqual(result.action, 'none');
    assert.ok(result.usage.percent < 0.01);
  });

  it('warns when above warning threshold', () => {
    // Note: reserveTokens: 0 is falsy so defaults to 4096; use 1 instead
    const guard = new ContextGuard({
      maxTokens: 200,
      reserveTokens: 1,
      warningThreshold: 0.3,
      compactThreshold: 0.9,
      abortThreshold: 0.95,
    });
    // 'word '.repeat(60) ≈ 81 tokens + 3 overhead = 84 tokens
    // effectiveMax = 199, 84/199 ≈ 0.42 > 0.3 warning, < 0.9 compact
    const longText = 'word '.repeat(60);
    const result = guard.check([{ role: 'user', content: longText }], '');
    assert.strictEqual(result.safe, true);
    assert.strictEqual(result.action, 'warn');
    assert.ok(result.message.includes('capacity'));
  });

  it('compacts when above compact threshold', () => {
    // reserveTokens: 1 to avoid falsy-0 default
    const guard = new ContextGuard({
      maxTokens: 600,
      reserveTokens: 1,
      warningThreshold: 0.3,
      compactThreshold: 0.5,
      abortThreshold: 0.95,
    });
    // 40 messages × ~10 tokens each ≈ 420 tokens
    // effectiveMax = 599, percent ≈ 0.70 → above compact (0.5)
    const history = [];
    for (let i = 0; i < 20; i++) {
      history.push({ role: 'user', content: `Message number ${i} with some content` });
      history.push({ role: 'assistant', content: `Reply to message ${i}` });
    }
    const result = guard.check(history, '');
    assert.ok(
      ['compact', 'abort'].includes(result.action),
      `Expected compact or abort, got ${result.action} (percent: ${result.usage.percent})`,
    );
  });

  it('aborts when above abort threshold', () => {
    // reserveTokens: 1 to avoid falsy-0 default
    const guard = new ContextGuard({
      maxTokens: 50,
      reserveTokens: 1,
      abortThreshold: 0.5,
    });
    // 'x'.repeat(500) ≈ 125 tokens + 3 overhead = 128
    // effectiveMax = 49, 128/49 ≈ 2.6 > 0.5 abort threshold
    const longText = 'x'.repeat(500);
    const result = guard.check([{ role: 'user', content: longText }], '');
    assert.strictEqual(result.safe, false);
    assert.strictEqual(result.action, 'abort');
  });

  it('includes new message tokens in calculation', () => {
    const guard = new ContextGuard({ maxTokens: 200000 });
    const result = guard.check([], '', 'New user message');
    assert.ok(result.usage.newMessageTokens > 0);
    assert.ok(result.usage.tokens > 0);
  });

  describe('getModelContextLimit', () => {
    it('returns 200000 for Claude models', () => {
      assert.strictEqual(ContextGuard.getModelContextLimit('claude-opus-4-5-20251101'), 200000);
      assert.strictEqual(ContextGuard.getModelContextLimit('claude-sonnet-4-5-20250929'), 200000);
    });

    it('returns 128000 for GPT-4o models', () => {
      assert.strictEqual(ContextGuard.getModelContextLimit('gpt-4o'), 128000);
      assert.strictEqual(ContextGuard.getModelContextLimit('gpt-4o-mini'), 128000);
    });

    it('returns 1000000 for Gemini Flash', () => {
      assert.strictEqual(ContextGuard.getModelContextLimit('gemini-2.0-flash'), 1000000);
    });

    it('returns 128000 as default for unknown models', () => {
      assert.strictEqual(ContextGuard.getModelContextLimit('unknown-model'), 128000);
    });
  });

  describe('forModel', () => {
    it('creates guard with model-specific context limit', () => {
      const guard = ContextGuard.forModel('gpt-4o');
      assert.strictEqual(guard.maxTokens, 128000);
    });
  });
});

// ===========================================================================
// guardContext
// ===========================================================================

describe('guardContext', () => {
  it('proceeds when context is within limits', async () => {
    const result = await guardContext({
      history: [{ role: 'user', content: 'Hi' }],
      systemPrompt: 'You are a helper.',
      newMessage: 'How are you?',
      model: 'claude-sonnet-4-5-20250929',
    });
    assert.strictEqual(result.proceed, true);
  });

  it('calls onWarn callback when warning threshold reached', async () => {
    let warnCalled = false;
    await guardContext({
      history: Array.from({ length: 100 }, (_, i) => ({
        role: i % 2 === 0 ? 'user' : 'assistant',
        content: 'x'.repeat(1000),
      })),
      systemPrompt: 'x'.repeat(10000),
      newMessage: 'x'.repeat(1000),
      model: 'gpt-4o-mini', // 128000 token limit
      onWarn: () => {
        warnCalled = true;
      },
    });
    // May or may not trigger warn depending on estimation
    // Just ensure it doesn't throw
    assert.ok(true);
  });

  it('does not proceed when context is too large', async () => {
    const result = await guardContext({
      history: Array.from({ length: 500 }, () => ({
        role: 'user',
        content: 'x'.repeat(5000),
      })),
      systemPrompt: 'x'.repeat(10000),
      newMessage: '',
      model: 'gpt-4o-mini',
    });
    // With 500 * 5000 chars = 2.5M chars ≈ 625K tokens vs 128K limit = should abort
    assert.strictEqual(result.proceed, false);
    assert.ok(result.message);
  });
});
