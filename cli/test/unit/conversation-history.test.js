/**
 * Unit tests for conversation-history.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import {
  extractHistoryText,
  formatConversationHistory,
  buildPromptReport,
  buildPromptWithHistory,
  extractCompactionSummary,
  estimateTokensFromText,
} from '../../src/conversation-history.js';

// ===========================================================================
// extractHistoryText
// ===========================================================================

describe('extractHistoryText', () => {
  it('returns empty string for null', () => {
    assert.strictEqual(extractHistoryText(null), '');
  });

  it('returns empty string for undefined', () => {
    assert.strictEqual(extractHistoryText(undefined), '');
  });

  it('returns empty string for empty string', () => {
    assert.strictEqual(extractHistoryText(''), '');
  });

  it('returns the string as-is for string input', () => {
    assert.strictEqual(extractHistoryText('hello world'), 'hello world');
  });

  it('extracts text from array of text blocks', () => {
    const content = [
      { type: 'text', text: 'Hello' },
      { type: 'text', text: 'World' },
    ];
    assert.strictEqual(extractHistoryText(content), 'Hello World');
  });

  it('extracts text from tool_result blocks', () => {
    const content = [{ type: 'tool_result', content: 'Tool output here' }];
    assert.strictEqual(extractHistoryText(content), 'Tool output here');
  });

  it('handles mixed array of strings and blocks', () => {
    const content = ['plain text', { type: 'text', text: 'block text' }];
    assert.strictEqual(extractHistoryText(content), 'plain text block text');
  });

  it('skips null/undefined entries in arrays', () => {
    const content = [null, { type: 'text', text: 'valid' }, undefined];
    assert.strictEqual(extractHistoryText(content), 'valid');
  });

  it('handles blocks with only a text property (no type)', () => {
    const content = [{ text: 'fallback text' }];
    assert.strictEqual(extractHistoryText(content), 'fallback text');
  });

  it('skips blocks with no extractable text', () => {
    const content = [
      { type: 'image', url: 'http://example.com/img.png' },
      { type: 'text', text: 'real text' },
    ];
    assert.strictEqual(extractHistoryText(content), 'real text');
  });

  it('handles an empty array', () => {
    assert.strictEqual(extractHistoryText([]), '');
  });

  it('extracts text from an object with .text property', () => {
    assert.strictEqual(extractHistoryText({ text: 'object text' }), 'object text');
  });

  it('recurses into object with .content property', () => {
    const content = { content: 'nested content' };
    assert.strictEqual(extractHistoryText(content), 'nested content');
  });

  it('recurses into object with nested .content array', () => {
    const content = { content: [{ type: 'text', text: 'deep' }] };
    assert.strictEqual(extractHistoryText(content), 'deep');
  });

  it('returns empty string for an object with no text or content', () => {
    assert.strictEqual(extractHistoryText({ foo: 'bar' }), '');
  });

  it('returns empty string for a number', () => {
    assert.strictEqual(extractHistoryText(42), '');
  });
});

// ===========================================================================
// formatConversationHistory
// ===========================================================================

describe('formatConversationHistory', () => {
  it('returns empty string for null', () => {
    assert.strictEqual(formatConversationHistory(null), '');
  });

  it('returns empty string for undefined', () => {
    assert.strictEqual(formatConversationHistory(undefined), '');
  });

  it('returns empty string for empty array', () => {
    assert.strictEqual(formatConversationHistory([]), '');
  });

  it('returns empty string for non-array', () => {
    assert.strictEqual(formatConversationHistory('not an array'), '');
  });

  it('formats a single message with role', () => {
    const history = [{ role: 'user', content: 'Hello' }];
    assert.strictEqual(formatConversationHistory(history), 'USER: Hello');
  });

  it('formats multiple messages', () => {
    const history = [
      { role: 'user', content: 'Hello' },
      { role: 'assistant', content: 'Hi there' },
    ];
    assert.strictEqual(formatConversationHistory(history), 'USER: Hello\nASSISTANT: Hi there');
  });

  it('uses type as role fallback', () => {
    const history = [{ type: 'system', content: 'You are helpful' }];
    assert.strictEqual(formatConversationHistory(history), 'SYSTEM: You are helpful');
  });

  it('uses "MESSAGE" as default role when no role or type', () => {
    const history = [{ content: 'No role here' }];
    assert.strictEqual(formatConversationHistory(history), 'MESSAGE: No role here');
  });

  it('uppercases the role', () => {
    const history = [{ role: 'assistant', content: 'response' }];
    assert.strictEqual(formatConversationHistory(history), 'ASSISTANT: response');
  });

  it('skips null entries in history array', () => {
    const history = [null, { role: 'user', content: 'Hello' }];
    assert.strictEqual(formatConversationHistory(history), 'USER: Hello');
  });

  it('skips messages with empty content after extraction', () => {
    const history = [
      { role: 'user', content: '   ' },
      { role: 'assistant', content: 'Valid' },
    ];
    assert.strictEqual(formatConversationHistory(history), 'ASSISTANT: Valid');
  });

  it('extracts content from msg.message.content', () => {
    const history = [{ role: 'bot', message: { content: 'inner content' } }];
    assert.strictEqual(formatConversationHistory(history), 'BOT: inner content');
  });

  it('extracts content from msg.text', () => {
    const history = [{ role: 'user', text: 'text field' }];
    assert.strictEqual(formatConversationHistory(history), 'USER: text field');
  });

  it('extracts content from msg.message when it is a string', () => {
    const history = [{ role: 'user', message: 'string message' }];
    assert.strictEqual(formatConversationHistory(history), 'USER: string message');
  });

  it('trims whitespace from extracted content', () => {
    const history = [{ role: 'user', content: '  padded content  ' }];
    assert.strictEqual(formatConversationHistory(history), 'USER: padded content');
  });
});

// ===========================================================================
// buildPromptWithHistory
// ===========================================================================

describe('buildPromptWithHistory', () => {
  it('returns request as-is when history is empty', () => {
    assert.strictEqual(buildPromptWithHistory('my request', []), 'my request');
  });

  it('returns request as-is when history is null', () => {
    assert.strictEqual(buildPromptWithHistory('my request', null), 'my request');
  });

  it('returns request as-is when history produces no text', () => {
    const history = [{ role: 'user', content: '   ' }];
    assert.strictEqual(buildPromptWithHistory('my request', history), 'my request');
  });

  it('prepends history context to request', () => {
    const history = [
      { role: 'user', content: 'What is the price?' },
      { role: 'assistant', content: 'The price is $29.99' },
    ];
    const result = buildPromptWithHistory('Show me alternatives', history);
    assert.ok(result.startsWith('Conversation history:\n'));
    assert.ok(result.includes('USER: What is the price?'));
    assert.ok(result.includes('ASSISTANT: The price is $29.99'));
    assert.ok(result.includes('Current request:\nShow me alternatives'));
  });

  it('formats with correct section headers', () => {
    const history = [{ role: 'user', content: 'hi' }];
    const result = buildPromptWithHistory('hello', history);
    const lines = result.split('\n');
    assert.strictEqual(lines[0], 'Conversation history:');
    assert.strictEqual(lines[1], 'USER: hi');
    assert.strictEqual(lines[2], '');
    assert.strictEqual(lines[3], 'Current request:');
    assert.strictEqual(lines[4], 'hello');
  });

  it('applies redaction to history when redactHistory is true', () => {
    const history = [{ role: 'user', content: 'My email is alice@example.com' }];
    const result = buildPromptWithHistory('tell me', history, { redactHistory: true });
    assert.ok(result.includes('[email]'), 'Should redact the email in history');
    assert.ok(!result.includes('alice@example.com'), 'Email should not appear');
  });

  it('applies redaction to request when redactRequest is true', () => {
    const history = [{ role: 'user', content: 'hi' }];
    const result = buildPromptWithHistory('My email is bob@test.com', history, {
      redactRequest: true,
    });
    assert.ok(result.includes('[email]'), 'Should redact the email in request');
    assert.ok(!result.includes('bob@test.com'), 'Email should not appear in request');
  });

  it('does not redact when options are not set', () => {
    const history = [{ role: 'user', content: 'alice@example.com' }];
    const result = buildPromptWithHistory('bob@test.com', history);
    assert.ok(result.includes('alice@example.com'));
    assert.ok(result.includes('bob@test.com'));
  });

  it('passes redactOptions through to redactSensitive', () => {
    const history = [{ role: 'user', content: 'alice@example.com' }];
    // enabled: false should prevent redaction even when redactHistory is true
    const result = buildPromptWithHistory('request', history, {
      redactHistory: true,
      redactOptions: { enabled: false },
    });
    assert.ok(result.includes('alice@example.com'));
  });
});

// ===========================================================================
// buildPromptReport
// ===========================================================================

describe('buildPromptReport', () => {
  it('captures injected history, system prompt, and compaction data', () => {
    const report = buildPromptReport({
      request: 'Show me alternatives',
      history: [
        { role: 'user', content: 'What is the price?' },
        { role: 'assistant', content: 'It is $29.99' },
      ],
      systemPrompt: 'You are a commerce assistant.',
      includeHistory: true,
      historySource: 'conversation_history',
      compactionSummary: 'Prior order and pricing discussion.',
      contextGuardResult: {
        action: 'compact',
        usage: {
          tokens: 400,
          afterCompaction: { tokens: 250, tokensSaved: 150 },
        },
      },
    });

    assert.strictEqual(report.historySource, 'conversation_history');
    assert.strictEqual(report.historyInjected, true);
    assert.strictEqual(report.historyMessagesAvailable, 2);
    assert.strictEqual(report.historyMessagesInjected, 2);
    assert.ok(report.historyTokensInjected > 0);
    assert.ok(report.systemPromptTokens > 0);
    assert.ok(report.userPromptTokens > report.requestTokens);
    assert.strictEqual(report.compactionApplied, true);
    assert.strictEqual(report.estimatedContextTokensSaved, 150);
  });

  it('tracks available but non-injected history for resumed sessions', () => {
    const report = buildPromptReport({
      request: 'Continue',
      history: [{ role: 'user', content: 'Earlier context' }],
      systemPrompt: 'You are a commerce assistant.',
      includeHistory: false,
      resumeSession: true,
      historySource: 'session_summary',
    });

    assert.strictEqual(report.resumeSession, true);
    assert.strictEqual(report.historyInjected, false);
    assert.strictEqual(report.historyMessagesAvailable, 1);
    assert.strictEqual(report.historyMessagesInjected, 0);
    assert.strictEqual(report.historyTokensInjected, 0);
    assert.ok(report.totalInputTokens >= report.requestTokens + report.systemPromptTokens);
  });
});

// ===========================================================================
// extractCompactionSummary
// ===========================================================================

describe('extractCompactionSummary', () => {
  it('returns null for null input', () => {
    assert.strictEqual(extractCompactionSummary(null), null);
  });

  it('returns null for undefined', () => {
    assert.strictEqual(extractCompactionSummary(undefined), null);
  });

  it('returns null for empty array', () => {
    assert.strictEqual(extractCompactionSummary([]), null);
  });

  it('returns null when no user message exists', () => {
    const history = [{ role: 'assistant', content: 'Summary of conversation' }];
    assert.strictEqual(extractCompactionSummary(history), null);
  });

  it('extracts text from first user message', () => {
    const history = [
      { role: 'user', content: 'Summary of previous conversation' },
      { role: 'assistant', content: 'OK, I understand' },
    ];
    assert.strictEqual(extractCompactionSummary(history), 'Summary of previous conversation');
  });

  it('finds first user message even if not at index 0', () => {
    const history = [
      { role: 'system', content: 'system prompt' },
      { role: 'user', content: 'The user summary' },
    ];
    assert.strictEqual(extractCompactionSummary(history), 'The user summary');
  });

  it('returns null when user message has empty content', () => {
    const history = [{ role: 'user', content: '' }];
    assert.strictEqual(extractCompactionSummary(history), null);
  });

  it('handles user message with .message.content', () => {
    const history = [{ role: 'user', message: { content: 'From message.content' } }];
    assert.strictEqual(extractCompactionSummary(history), 'From message.content');
  });

  it('returns null for non-array input (string)', () => {
    assert.strictEqual(extractCompactionSummary('not an array'), null);
  });

  it('extracts from array content blocks', () => {
    const history = [{ role: 'user', content: [{ type: 'text', text: 'Block summary' }] }];
    assert.strictEqual(extractCompactionSummary(history), 'Block summary');
  });
});

// ===========================================================================
// estimateTokensFromText
// ===========================================================================

describe('estimateTokensFromText', () => {
  it('returns 0 for null', () => {
    assert.strictEqual(estimateTokensFromText(null), 0);
  });

  it('returns 0 for undefined', () => {
    assert.strictEqual(estimateTokensFromText(undefined), 0);
  });

  it('returns 0 for empty string', () => {
    assert.strictEqual(estimateTokensFromText(''), 0);
  });

  it('returns 1 for a single character', () => {
    assert.strictEqual(estimateTokensFromText('a'), 1);
  });

  it('returns 1 for text with 1-4 chars', () => {
    assert.strictEqual(estimateTokensFromText('ab'), 1);
    assert.strictEqual(estimateTokensFromText('abc'), 1);
    assert.strictEqual(estimateTokensFromText('abcd'), 1);
  });

  it('estimates ~4 chars per token for longer text', () => {
    // 20 chars -> ceil(20/4) = 5 tokens
    assert.strictEqual(estimateTokensFromText('01234567890123456789'), 5);
  });

  it('returns ceil(len/4) for various lengths', () => {
    assert.strictEqual(estimateTokensFromText('12345'), 2); // ceil(5/4) = 2
    assert.strictEqual(estimateTokensFromText('12345678'), 2); // ceil(8/4) = 2
    assert.strictEqual(estimateTokensFromText('123456789'), 3); // ceil(9/4) = 3
  });

  it('handles a longer paragraph', () => {
    const text = 'The quick brown fox jumps over the lazy dog.'; // 44 chars
    assert.strictEqual(estimateTokensFromText(text), Math.ceil(44 / 4));
  });

  it('converts non-string input to string', () => {
    // number 12345 -> "12345" -> 5 chars -> ceil(5/4) = 2
    assert.strictEqual(estimateTokensFromText(12345), 2);
  });

  it('ensures minimum of 1 for non-empty text', () => {
    assert.strictEqual(estimateTokensFromText('x'), 1);
  });

  it('handles very long text', () => {
    const text = 'a'.repeat(10000);
    assert.strictEqual(estimateTokensFromText(text), 2500);
  });
});
